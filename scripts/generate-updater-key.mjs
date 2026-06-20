#!/usr/bin/env node
/**
 * 產生 DBX Tauri updater 簽名密鑰(本地開發 / 測試用)。
 *
 * 用法:
 *   node scripts/generate-updater-key.mjs [選項]
 *   pnpm gen:key -- [選項]
 *
 * 選項:
 *   --out <path>       私鑰輸出路徑(預設:~/.tauri/dbx-updater.key,在 repo 外)
 *   --password <pw>    私鑰密碼;省略則改為「互動式」由 tauri 提示輸入
 *   --no-password      不設密碼(空密碼,非互動);本地測試用
 *   --force            覆寫既有私鑰
 *   -h, --help         顯示說明
 *
 * ⚠️ 安全須知:
 *   1. 私鑰預設寫到 repo 外的 ~/.tauri/,不會被 git 追蹤;腳本會拒絕寫進倉庫目錄。
 *   2. 產生的 pubkey 與官方(tauri.conf.json 內建那把)不同。用此密鑰簽出的更新包,
 *      正式發布的 App 驗章會失敗 —— 僅供本地打包 / 測試。
 *   3. 若要本地測試自動更新,才需把 tauri.conf.json 的 pubkey 換成此密鑰的 .pub 內容,
 *      但【絕對不要 commit】該變更,否則會切斷正式發布的更新鏈。
 *   4. 正式發布請使用 CI secret(TAURI_SIGNING_PRIVATE_KEY_BASE64)裡的官方私鑰。
 */

import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync } from 'node:fs'
import { homedir, platform } from 'node:os'
import { dirname, join, resolve, sep } from 'node:path'

const args = process.argv.slice(2)
const has = (flag) => args.includes(flag)
const val = (name) => {
  const i = args.indexOf(name)
  return i !== -1 ? args[i + 1] : undefined
}

if (has('-h') || has('--help')) {
  console.log(
    [
      'Generate a DBX Tauri updater signing key (local dev / test only).',
      '',
      'Usage: node scripts/generate-updater-key.mjs [options]',
      '',
      '  --out <path>      私鑰輸出路徑(預設 ~/.tauri/dbx-updater.key)',
      '  --password <pw>   私鑰密碼;省略則互動式輸入',
      '  --no-password     不設密碼(空密碼,非互動;本地測試用)',
      '  --force           覆寫既有私鑰',
      '  -h, --help        顯示說明',
    ].join('\n'),
  )
  process.exit(0)
}

const isWin = platform() === 'win32'
const outPath = resolve(val('--out') ?? join(homedir(), '.tauri', 'dbx-updater.key'))
const noPassword = has('--no-password')
const password = val('--password')
const force = has('--force')

// --- 安全檢查:拒絕把私鑰寫進 git 倉庫,避免誤 commit 外洩 ---
const repoRoot = spawnSync('git', ['rev-parse', '--show-toplevel'], {
  encoding: 'utf8',
}).stdout?.trim()
if (repoRoot) {
  const repo = resolve(repoRoot)
  if (outPath === repo || outPath.startsWith(repo + sep)) {
    console.error(
      `✗ 拒絕:私鑰路徑位於 git 倉庫內,有誤 commit 外洩風險:\n    ${outPath}\n` +
        `  請用 --out 指定倉庫外的位置(預設 ~/.tauri/)。`,
    )
    process.exit(1)
  }
}

// --- 確保輸出目錄存在 ---
mkdirSync(dirname(outPath), { recursive: true })

// --- 組裝並執行 tauri signer generate ---
const genArgs = ['tauri', 'signer', 'generate', '-w', outPath]
if (force) genArgs.push('-f')
// 密碼處理:
//   --no-password → 只加 --ci(tauri 以空密碼非互動產生;避免傳空字串給 -p 被 shell 吞掉)
//   --password    → 非互動,帶密碼
//   兩者皆無      → 互動式,由 tauri 提示輸入密碼
if (noPassword) genArgs.push('--ci')
else if (password !== undefined) genArgs.push('--ci', '-p', password)

console.log(`→ 產生簽名密鑰到:${outPath}`)
const res = spawnSync('pnpm', genArgs, { stdio: 'inherit', shell: isWin })
if (res.status !== 0) {
  console.error('✗ 密鑰產生失敗(請確認已安裝依賴:pnpm install)')
  process.exit(res.status ?? 1)
}

// --- 讀回 pubkey,印出後續打包步驟 ---
const pubPath = `${outPath}.pub`
const pubkey = existsSync(pubPath)
  ? readFileSync(pubPath, 'utf8').trim()
  : '(找不到 .pub,請檢查輸出目錄)'

// 打包簽名讀 TAURI_SIGNING_PRIVATE_KEY(可填私鑰「檔案路徑」或「內容」)
const setKey = isWin
  ? `  $env:TAURI_SIGNING_PRIVATE_KEY = "${outPath}"`
  : `  export TAURI_SIGNING_PRIVATE_KEY="${outPath}"`
const hasPw = !noPassword
const setPw = hasPw
  ? isWin
    ? '  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<你的密碼>"   # 空密碼可省略\n'
    : '  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<你的密碼>"   # 空密碼可省略\n'
  : ''

console.log(
  `
✓ 已產生密鑰
  私鑰:${outPath}
  公鑰:${pubPath}

公鑰(base64,僅本地測試自動更新時才貼到 tauri.conf.json 的 plugins.updater.pubkey):
${pubkey}

──────── 下一步:用此密鑰打包 ────────
${setKey}
${setPw}  pnpm tauri build

備註:
  • TAURI_SIGNING_PRIVATE_KEY 可填私鑰「檔案路徑」或「內容」(打包簽名讀此變數)。
  • 公鑰只有在本地測試自動更新時才需貼進 tauri.conf.json,且【勿 commit】。
  • 正式發布請改用 CI secret 裡的官方私鑰,不要用這把本地密鑰。
────────────────────────────────────
`,
)
