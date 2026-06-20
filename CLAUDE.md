# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What DBX Is

DBX is a cross-platform database management tool. The same Rust database core powers four surfaces: a Tauri desktop app, a web/Docker backend, a CLI, and an MCP server for AI agents.

## Monorepo Layout (two independent workspaces)

This repo contains **two parallel workspaces that do not overlap**:

**Rust — cargo workspace (root `Cargo.toml`, members = `src-tauri`, `crates/*`):**
- `crates/dbx-core/` — the shared database core. **All driver logic lives here**: Postgres, MySQL, SQLite, SQL Server (tiberius), Redis, MongoDB, DuckDB, plus SSH tunneling, connection encryption, and SQL parsing (`sqlparser`). Every other Rust surface depends on this crate.
- `crates/dbx-web/` — Axum HTTP/WS backend (`dbx-web` binary) for the web/Docker deployment; a thin layer over `dbx-core`.
- `src-tauri/` — Tauri 2 desktop shell. Native commands in `src-tauri/src/commands/*.rs` (one file per domain — connection, query, schema, redis, mongo, mq, transfer, export…) bridge the Vue frontend to `dbx-core`.

**Node/JS — pnpm workspace (`pnpm-workspace.yaml`, members = `packages/*`):**
- `packages/node-core/` (`@dbx-app/node-core`) — shared Node DB utilities (pg, mysql2, better-sqlite3, keytar) with subpath exports (`./sql-safety`, `./connections`, `./schema-context`, …).
- `packages/cli/` (`@dbx-app/cli`, bin `dbx`) — terminal CLI; depends on node-core.
- `packages/mcp-server/` (`@dbx-app/mcp-server`) — MCP server so Claude/Cursor agents can query databases; depends on node-core.

**In neither workspace:**
- `apps/desktop/` — Vue 3 + TypeScript frontend (Vite 8 + rolldown, Pinia, reka-ui/shadcn-vue, CodeMirror, ECharts, Tailwind). Managed by the **root** `package.json` (its deps live there), not `packages/*`. Builds to `dist/`, which Tauri bundles.
- `plugins/` (optional plugins, e.g. the Java/Maven JDBC plugin), `docs/` (docs site), `deploy/` (Docker/deployment).

## Database Driver Manifest — read before touching DB support

`crates/dbx-core/assets/database-drivers.manifest.json` is the **single source of truth** for every database type: driver mode, MCP/CLI routing, agent keys, support level, and product capabilities. When adding or changing a database type, edit the manifest **first**, then the feature code.

Support levels (conservative → advanced): `connect` (connection + SQL/command only) < `browse` (+ metadata browsing) < `understand` (+ search, object sources, diagrams) < `operate` (+ data/structure editing, import, transfer, DB creation, explain plans, user admin). Keep custom JDBC conservative unless a profile proves the capability.

After manifest changes, all three consumers must agree — run:
```bash
cargo test -p dbx-core --test database_capabilities
pnpm --filter @dbx-app/node-core exec tsx --test tests/driver-manifest.test.ts
pnpm --filter @dbx-app/mcp-server exec tsx --test tests/driver-manifest.test.ts
```

## Common Commands

Toolchain: Node `>=22.13.0`, pnpm `10.27.0`, Rust stable, Java 17 (only for JDBC plugin packaging).

```bash
pnpm install

# Develop
pnpm dev:tauri        # full desktop app (frontend HMR + Rust)
pnpm dev:web          # web-mode frontend only (vite, port 5173)
pnpm dev:backend      # web backend (cargo watch -x 'run -p dbx-web')

# Build / package
pnpm build            # frontend only (vite -> dist/)
pnpm tauri build      # desktop installers -> src-tauri/target/release/bundle/

# Checks before a PR
pnpm check            # format(oxfmt)+lint(oxlint)+typecheck(vue-tsc)+test(vitest) via scripts/run-check.mjs
cargo fmt --check
cargo check --workspace --locked

# Frontend granular: pnpm lint | pnpm fmt | pnpm typecheck | pnpm test (vitest)
# Packages: pnpm test:packages ; pnpm publish:dry-run
```

Run one frontend test: `pnpm vitest run <path>` (or `-t "<name>"`). One Node package test: `pnpm --filter @dbx-app/<pkg> exec tsx --test tests/<file>.test.ts`. One Rust test target: `cargo test -p dbx-core --test <name>`.

## DuckDB Builds Are Slow

`dbx-core` and `src-tauri` enable `duckdb-bundled` by default, compiling DuckDB from C++ source. When not touching DuckDB, skip it to speed up local builds:
```bash
cargo check --workspace --no-default-features
cargo test  --workspace --no-default-features
pnpm tauri dev -- --no-default-features
```
Release builds and CI always include DuckDB (omit the flag).

## Conventions & Gotchas

- **Conventional Commits** (e.g. `fix(app): clamp window size`).
- The root `Cargo.toml` **patches several crates.io drivers to git forks**: `tokio-postgres`/`postgres-types`/`postgres-protocol` → a GaussDB-capable fork; `mysql_async`/`mysql_common` → a fork supporting the deprecated `sha256_password` auth plugin. Expect git dependencies.
- The release profile is tuned for **binary size** (`lto`, `codegen-units=1`, `opt-level="s"`, `panic="abort"`), so release compiles are notably slow.
- `pnpm dev:backend` uses bash-style inline env vars (`RUST_LOG=… DBX_PASSWORD=… cargo watch …`); on Windows shells set them with `$env:` first.
