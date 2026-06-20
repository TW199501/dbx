#!/usr/bin/env node
/**
 * 跨平台啟動 dbx-web 後端開發(cargo watch)。
 *
 * 取代原本只在 bash 能用的:
 *   RUST_LOG=${RUST_LOG:-info} DBX_PASSWORD=${DBX_PASSWORD:-test} cargo watch -x 'run -p dbx-web'
 * Windows 的 cmd 不支援這種行內環境變數,會報「'RUST_LOG' 不是內部或外部命令」。
 *
 * 這裡用 Node 設定環境變數(保留「已設則用、未設則用預設」的回退),再啟動 cargo watch。
 */

import { spawn } from 'node:child_process'

const env = {
  ...process.env,
  RUST_LOG: process.env.RUST_LOG ?? 'info',
  DBX_PASSWORD: process.env.DBX_PASSWORD ?? 'test',
}

// shell: true 讓 Windows 能解析 cargo(.exe),並正確處理 -x 的帶空格引數
const child = spawn('cargo watch -x "run -p dbx-web"', {
  stdio: 'inherit',
  env,
  shell: true,
})

child.on('exit', (code) => process.exit(code ?? 0))
child.on('error', (err) => {
  console.error(
    `✗ 啟動 cargo watch 失敗: ${err.message}\n  請確認已安裝 cargo-watch: cargo install cargo-watch`,
  )
  process.exit(1)
})
