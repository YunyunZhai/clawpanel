/**
 * 日志查看页面
 *
 * 文件列表来自后端 `list_log_files`（扫描 `openclaw_dir()/logs`），
 * 不再硬编码文件名：便携模式下日志在 U 盘 `data/openclaw/logs`，
 * 硬编码的主目录路径读不到任何东西。
 */
import { api, invalidate } from '../lib/tauri-api.js'
import { toast } from '../components/toast.js'
import { humanizeError } from '../lib/humanize-error.js'
import { t } from '../lib/i18n.js'

let _searchTimer = null

function formatSize(bytes) {
  const value = Number(bytes)
  if (!Number.isFinite(value) || value <= 0) return '0 B'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}

/** 后端返回 Unix 秒；格式化成用户时区的短时间，取不到就留空 */
function formatModified(epochSecs) {
  const secs = Number(epochSecs)
  if (!Number.isFinite(secs) || secs <= 0) return ''
  const d = new Date(secs * 1000)
  if (Number.isNaN(d.getTime())) return ''
  const pad = n => String(n).padStart(2, '0')
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

export async function render() {
  const page = document.createElement('div')
  page.className = 'page'

  page.innerHTML = `
    <div class="page-header">
      <h1 class="page-title">${t('logs.title')}</h1>
      <p class="page-desc">${t('logs.desc')}</p>
    </div>
    <div class="tab-bar" id="log-files"></div>
    <div class="log-toolbar">
      <input type="text" class="form-input" id="log-search" placeholder="${t('logs.searchPlaceholder')}" style="max-width:300px">
      <button class="btn btn-secondary btn-sm" id="btn-refresh">${t('logs.refresh')}</button>
      <label style="display:flex;align-items:center;gap:6px;font-size:var(--font-size-sm);color:var(--text-secondary)">
        <input type="checkbox" id="log-autoscroll" checked> ${t('logs.autoScroll')}
      </label>
    </div>
    <div class="log-viewer" id="log-content" style="height:calc(100vh - 280px)"><div class="stat-card loading-placeholder" style="height:16px;margin:8px 0"></div><div class="stat-card loading-placeholder" style="height:16px;margin:8px 0"></div><div class="stat-card loading-placeholder" style="height:16px;margin:8px 0"></div><div class="stat-card loading-placeholder" style="height:16px;margin:8px 0"></div></div>
  `

  /** @type {{name: string, size: number, modified: number}[]} */
  let files = []
  let currentFile = ''

  // --- 文件列表 ---
  function renderFileList() {
    const bar = page.querySelector('#log-files')
    if (!files.length) {
      bar.innerHTML = `<div class="log-empty-hint">${escapeHtml(t('logs.noFiles'))}</div>`
      return
    }
    bar.innerHTML = files
      .map(f => {
        const size = formatSize(f.size)
        const modified = formatModified(f.modified)
        const meta = [size, modified].filter(Boolean).join(' · ')
        return `<div class="tab log-file-tab${f.name === currentFile ? ' active' : ''}" data-file="${escapeHtml(f.name)}" title="${escapeHtml(meta)}">${escapeHtml(f.name)}<span class="log-file-tab-meta">${escapeHtml(meta)}</span></div>`
      })
      .join('')

    bar.querySelectorAll('.log-file-tab').forEach(tab => {
      tab.onclick = () => {
        const name = tab.dataset.file
        if (name === currentFile) return
        currentFile = name
        page.querySelector('#log-search').value = ''
        renderFileList()
        loadLog()
      }
    })
  }

  async function loadFiles() {
    try {
      files = (await api.listLogFiles()) || []
    } catch (e) {
      console.error('[logs] failed to list log files:', e)
      files = []
    }
    // 当前文件可能已被轮转/删除，回落到第一个
    if (!files.some(f => f.name === currentFile)) {
      currentFile = files.length ? files[0].name : ''
    }
    renderFileList()
  }

  // --- 内容 ---
  async function loadLog() {
    const el = page.querySelector('#log-content')
    const refreshBtn = page.querySelector('#btn-refresh')
    if (!currentFile) {
      el.innerHTML = `<div style="color:var(--text-tertiary)">${escapeHtml(t('logs.noFiles'))}</div>`
      return
    }
    el.innerHTML = '<div class="log-loading"><div class="service-spinner"></div><span style="color:var(--text-tertiary);margin-left:8px">' + escapeHtml(t('logs.loading')) + '</span></div>'
    if (refreshBtn) { refreshBtn.classList.add('btn-loading'); refreshBtn.disabled = true }
    try {
      const content = await api.readLogTail(currentFile, 200)
      if (!content || !content.trim()) {
        el.innerHTML = `<div style="color:var(--text-tertiary)">${escapeHtml(t('logs.empty'))}</div>`
        return
      }
      const lines = content.trim().split('\n')
      el.innerHTML = lines.map(l => `<div class="log-line">${escapeHtml(l)}</div>`).join('')
      if (page.querySelector('#log-autoscroll')?.checked) {
        el.scrollTop = el.scrollHeight
      }
    } catch (e) {
      el.innerHTML = `<div style="color:var(--error);padding:12px">${escapeHtml(t('logs.loadFailed'))}: ${escapeHtml(String(e))}</div>`
      toast(humanizeError(e, t('logs.loadFailed')), 'error')
    } finally {
      if (refreshBtn) { refreshBtn.classList.remove('btn-loading'); refreshBtn.disabled = false }
    }
  }

  async function searchLog(query) {
    const el = page.querySelector('#log-content')
    try {
      const results = await api.searchLog(currentFile, query)
      if (!results || !results.length) {
        el.innerHTML = `<div style="color:var(--text-tertiary)">${escapeHtml(t('logs.noResults'))}</div>`
        return
      }
      el.innerHTML = results.map(l => `<div class="log-line">${highlightMatch(escapeHtml(l), query)}</div>`).join('')
    } catch (e) {
      el.innerHTML = `<div style="color:var(--error);padding:12px">${escapeHtml(t('logs.searchFailed'))}: ${escapeHtml(String(e))}</div>`
      toast(humanizeError(e, t('logs.searchFailed')), 'error')
    }
  }

  // 搜索
  page.querySelector('#log-search').addEventListener('input', (e) => {
    clearTimeout(_searchTimer)
    const query = e.target.value.trim()
    _searchTimer = setTimeout(() => {
      if (!currentFile) return
      if (query) searchLog(query)
      else loadLog()
    }, 300)
  })

  // 刷新：清掉读缓存，否则 5s/10s TTL 内点刷新拿到的还是旧内容
  page.querySelector('#btn-refresh').onclick = async () => {
    clearTimeout(_searchTimer)
    page.querySelector('#log-search').value = ''
    invalidate('read_log_tail', 'list_log_files')
    await loadFiles()
    await loadLog()
  }

  // 立即返回页面，数据异步填充。
  // 不能在这里 await：router.js 对 render() 有 15s 超时，超时会整页报
  // 「页面加载失败」；而且等待期间容器已被清空，用户连骨架屏都看不到。
  loadFiles().then(loadLog)
  return page
}

export function cleanup() {
  clearTimeout(_searchTimer)
  _searchTimer = null
}

function escapeHtml(str) {
  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

function highlightMatch(html, query) {
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return html.replace(new RegExp(escaped, 'gi'), m => `<mark>${m}</mark>`)
}
