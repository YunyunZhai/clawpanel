import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { buildLocales } from '../src/locales/index.js'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const read = file => fs.readFileSync(path.join(root, file), 'utf8')

test('日志目录跟随 openclaw_dir，便携模式不再读用户主目录', () => {
  const logs = read('src-tauri/src/commands/logs.rs')
  assert.match(logs, /fn log_dir\(\) -> PathBuf \{\s*crate::commands::openclaw_dir\(\)\.join\("logs"\)/)
  // 曾经写死 dirs::home_dir()/.openclaw/logs，便携版（U 盘 data/openclaw）永远读空
  assert.doesNotMatch(logs, /dirs::home_dir\(\)[\s\S]{0,80}\.join\("\.openclaw"\)/)
})

test('read_log_tail / search_log 都走统一路径解析并拒绝路径穿越', () => {
  const logs = read('src-tauri/src/commands/logs.rs')
  assert.match(logs, /fn resolve_log_path\(log_name: &str\) -> Result<PathBuf, String>/)
  assert.equal((logs.match(/resolve_log_path\(&log_name\)\?/g) || []).length, 2)
  assert.match(logs, /log_name\.contains\("\.\."\)/)
  assert.match(logs, /log_name\.contains\('\\\\'\)/)
})

test('已知别名保留，dashboard 启动诊断的 key 调用不受影响', () => {
  const logs = read('src-tauri/src/commands/logs.rs')
  for (const [key, file] of [['gateway', 'gateway.log'], ['gateway-err', 'gateway.err.log'], ['guardian', 'guardian.log'], ['config-audit', 'config-audit.jsonl']]) {
    assert.match(logs, new RegExp(`"${key}" => Some\\("${file.replace('.', '\\.')}"\\)`))
  }
  // 死别名 guardian-backup 全项目无写入方，随硬编码 tab 一起移除
  assert.doesNotMatch(logs, /guardian-backup/)
})

test('list_log_files 已注册到 invoke_handler', () => {
  assert.match(read('src-tauri/src/lib.rs'), /logs::list_log_files/)
  assert.match(read('src/lib/tauri-api.js'), /listLogFiles: \(\) => cachedInvoke\('list_log_files', \{\}, 10000\)/)
})

test('日志页文件列表来自后端扫描结果，不再硬编码文件名', () => {
  const page = read('src/pages/logs.js')
  assert.match(page, /await api\.listLogFiles\(\)/)
  assert.match(page, /api\.readLogTail\(currentFile, 200\)/)
  assert.match(page, /api\.searchLog\(currentFile, query\)/)
  // 已删除的硬编码 tab key 不能复活
  assert.doesNotMatch(page, /tabGateway|tabBackup|tabAudit|tabGuardian/)
})

test('日志页文件列表与内容都能显示空状态', () => {
  const page = read('src/pages/logs.js')
  assert.match(page, /t\('logs\.noFiles'\)/)
  assert.match(page, /t\('logs\.empty'\)/)
  assert.match(page, /t\('logs\.noResults'\)/)
})

test('日志页刷新会清掉读缓存，否则 TTL 内点刷新仍拿旧内容', () => {
  const page = read('src/pages/logs.js')
  assert.match(page, /invalidate\('read_log_tail', 'list_log_files'\)/)
  assert.match(page, /await loadFiles\(\)/)
})

test('日志页 render() 不 await 后端，避免撞上 router 的 15s 渲染超时', () => {
  // router.js:90 对 render() 包了 withTimeout(…, 15000)，超时整页报加载失败
  const page = read('src/pages/logs.js')
  assert.match(read('src/router.js'), /await withTimeout\(renderFn\(\), 15000/)
  // render() 尾部必须先挂起加载再返回页面；刷新按钮内部仍然 await
  assert.match(page, /loadFiles\(\)\.then\(loadLog\)\s*\n\s*return page/)
  assert.match(page, /btn-refresh'\)\.onclick = async \(\) => \{[\s\S]{0,200}await loadFiles\(\)/)
})

test('日志 tab 数量不定时不会横向溢出', () => {
  assert.match(read('src/style/components.css'), /\.tab-bar \{[^}]*flex-wrap: wrap;/)
  assert.match(read('src/style/pages.css'), /\.log-file-tab \{/)
})

test('日志页使用的 i18n key 在 11 种语言里都存在', () => {
  const keys = ['title', 'desc', 'noFiles', 'searchPlaceholder', 'refresh', 'autoScroll', 'loading', 'empty', 'loadFailed', 'noResults', 'searchFailed']
  for (const [lang, dict] of Object.entries(buildLocales())) {
    for (const key of keys) {
      assert.ok(dict.logs?.[key], `${lang} 缺少 logs.${key}`)
    }
  }
})
