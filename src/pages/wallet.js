/**
 * 钱包页面 — 对接云端 OneAPI 网关
 * 展示余额/已用额度，生成充值二维码，并提供 OneAPI 连接配置
 */
import { t } from '../lib/i18n.js'
import { icon } from '../lib/icons.js'
import { toast } from '../components/toast.js'
import { api } from '../lib/tauri-api.js'

const POLL_INTERVAL_MS = 15000

let _page = null
let _pollTimer = null

export async function render() {
  const page = document.createElement('div')
  page.className = 'page'
  _page = page

  page.innerHTML = `
    <div class="page-header">
      <h1 class="page-title">${t('wallet.title')}</h1>
      <p class="page-desc">${t('wallet.desc')}</p>
    </div>
    <div id="wallet-body">
      <div class="config-section loading-placeholder" style="height:160px"></div>
    </div>
  `

  const body = page.querySelector('#wallet-body')
  let config = { url: '', username: '', password: '' }
  try {
    config = (await api.oneapiGetConfig()) || config
  } catch (e) {
    toast(String(e?.message || e), 'error')
  }

  if (config.url && config.username && config.password) {
    renderConfigured(body, config)
    await loadBalance(body, config)
  } else {
    renderConfigForm(body, config)
  }
  return page
}

export function cleanup() {
  stopPolling()
  _page = null
}

function stopPolling() {
  if (_pollTimer) { clearInterval(_pollTimer); _pollTimer = null }
}

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, c => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
  ))
}

// 与 OneAPI 官方面板一致：开启货币展示时按 $ 展示两位小数，否则展示原始额度整数
function formatQuota(value, displayInCurrency) {
  const n = Number(value)
  if (displayInCurrency === false) return Number.isFinite(n) ? String(Math.round(n)) : '0'
  return `$${(Number.isFinite(n) ? n : 0).toFixed(2)}`
}

function renderConfigForm(body, config) {
  body.innerHTML = `
    <div class="config-section">
      <div class="config-section-title">${t('wallet.oneapiConfig')}</div>
      <div style="max-width:460px">
        <div style="margin-bottom:12px">
          <label style="display:block;font-size:var(--font-size-xs);color:var(--text-tertiary);margin-bottom:4px">${t('wallet.oneapiUrl')}</label>
          <input class="form-input" id="wallet-url" type="url" spellcheck="false" style="width:100%"
                 placeholder="https://api.example.com" value="${esc(config.url || '')}">
          <div class="form-hint">${t('wallet.oneapiUrlHint')}</div>
        </div>
        <div style="margin-bottom:12px">
          <label style="display:block;font-size:var(--font-size-xs);color:var(--text-tertiary);margin-bottom:4px">${t('wallet.oneapiUsername')}</label>
          <input class="form-input" id="wallet-username" type="text" spellcheck="false" autocomplete="off" style="width:100%"
                 value="${esc(config.username || '')}">
          <div class="form-hint">${t('wallet.oneapiUsernameHint')}</div>
        </div>
        <div style="margin-bottom:16px">
          <label style="display:block;font-size:var(--font-size-xs);color:var(--text-tertiary);margin-bottom:4px">${t('wallet.oneapiPassword')}</label>
          <input class="form-input" id="wallet-password" type="password" spellcheck="false" autocomplete="off" style="width:100%"
                 value="${esc(config.password || '')}">
          <div class="form-hint">${t('wallet.oneapiPasswordHint')}</div>
        </div>
        <div style="display:flex;gap:8px">
          <button class="btn btn-primary btn-sm" id="wallet-save">${t('wallet.saveConfig')}</button>
          <button class="btn btn-secondary btn-sm" id="wallet-test">${t('wallet.testConnection')}</button>
        </div>
      </div>
    </div>
  `

  const readForm = () => ({
    url: body.querySelector('#wallet-url').value.trim().replace(/\/+$/, ''),
    username: body.querySelector('#wallet-username').value.trim(),
    password: body.querySelector('#wallet-password').value,
  })
  const valid = next => Boolean(next.url && next.username && next.password)

  body.querySelector('#wallet-save').onclick = async () => {
    const next = readForm()
    if (!valid(next)) { toast(t('wallet.configRequired'), 'warning'); return }
    try {
      await api.oneapiSaveConfig(next)
    } catch (e) {
      toast(String(e?.message || e), 'error')
      return
    }
    renderConfigured(body, next)
    loadBalance(body, next)
  }

  body.querySelector('#wallet-test').onclick = async () => {
    const next = readForm()
    if (!valid(next)) { toast(t('wallet.configRequired'), 'warning'); return }
    try {
      await api.oneapiSaveConfig(next)
    } catch (e) {
      toast(String(e?.message || e), 'error')
      return
    }
    const result = await api.oneapiTestConnection().catch(e => ({ ok: false, error: String(e?.message || e) }))
    if (result?.ok) toast(`${t('wallet.connectionOK')} (${result.latency_ms}ms)`, 'success')
    else toast(result?.error || t('wallet.connectionFailed'), 'error')
  }
}

function renderConfigured(body, config) {
  body.innerHTML = `
    <div class="stat-cards">
      <div class="stat-card">
        <div class="stat-card-label">${t('wallet.balance')}</div>
        <div class="stat-card-value" id="wallet-balance">--</div>
        <div class="stat-card-meta" id="wallet-account"></div>
      </div>
      <div class="stat-card">
        <div class="stat-card-label">${t('wallet.usedQuota')}</div>
        <div class="stat-card-value" id="wallet-used">--</div>
      </div>
    </div>

    <div class="config-section">
      <div class="config-section-title">
        ${t('wallet.recharge')}
        <span style="margin-left:auto">
          <button class="btn btn-secondary btn-sm" id="wallet-refresh">${icon('refresh-cw', 14)} ${t('wallet.refreshBalance')}</button>
        </span>
      </div>
      <button class="btn btn-primary btn-sm" id="wallet-generate">${t('wallet.generateQR')}</button>
      <div class="form-hint" style="margin-top:8px">${t('wallet.rechargeHint')}</div>
      <div class="form-hint" id="wallet-error" style="display:none;color:var(--error)"></div>
    </div>

    <div class="config-section">
      <div class="config-section-title">${t('wallet.oneapiConfig')}</div>
      <div class="form-hint" style="overflow-wrap:anywhere">${esc(config.url)}</div>
      <button class="btn btn-secondary btn-sm" id="wallet-edit" style="margin-top:8px">${t('wallet.editConfig')}</button>
    </div>
  `

  body.querySelector('#wallet-generate').onclick = () => openTopupQr(config)
  body.querySelector('#wallet-refresh').onclick = () => loadBalance(body, config)
  body.querySelector('#wallet-edit').onclick = () => renderConfigForm(body, config)
}

async function loadBalance(body, config) {
  const errorEl = body.querySelector('#wallet-error')
  if (errorEl) errorEl.style.display = 'none'
  try {
    const data = await api.oneapiGetBalance()
    const display = data.display_in_currency !== false
    const balanceEl = body.querySelector('#wallet-balance')
    const usedEl = body.querySelector('#wallet-used')
    if (balanceEl) balanceEl.textContent = formatQuota(data.balance, display)
    if (usedEl) usedEl.textContent = formatQuota(data.used_quota, display)
    const accountEl = body.querySelector('#wallet-account')
    if (accountEl) {
      accountEl.textContent = [data.username, data.email, data.group].filter(Boolean).join(' · ')
    }
  } catch (e) {
    const message = String(e?.message || e)
    if (errorEl) {
      errorEl.textContent = message
      errorEl.style.display = ''
    } else {
      toast(message, 'error')
    }
  }
}

function openTopupQr(config) {
  const topupUrl = `${config.url}/topup`
  const qrSrc = `https://api.qrserver.com/v1/create-qr-code/?size=240x240&margin=8&data=${encodeURIComponent(topupUrl)}`

  const overlay = document.createElement('div')
  overlay.className = 'modal-overlay'
  overlay.innerHTML = `
    <div class="modal" style="max-width:360px;text-align:center">
      <div class="modal-title">${t('wallet.scanQR')}</div>
      <div class="modal-content-body">
        <img src="${qrSrc}" alt="QR" width="240" height="240"
             style="background:#fff;border-radius:8px;padding:8px">
        <div class="form-hint" style="margin-top:10px">${t('wallet.rechargeHint')}</div>
      </div>
      <div class="modal-actions" style="justify-content:center">
        <button class="btn btn-secondary btn-sm" data-action="close">${t('common.close')}</button>
        <a class="btn btn-primary btn-sm" href="${esc(topupUrl)}" target="_blank" rel="noopener">${t('wallet.openTopup')}</a>
      </div>
    </div>
  `
  document.body.appendChild(overlay)

  const close = () => { stopPolling(); overlay.remove() }
  overlay.addEventListener('click', e => { if (e.target === overlay) close() })
  overlay.querySelector('[data-action="close"]').onclick = close

  // 支付在手机端完成，这里轮询余额直到到账
  const body = _page?.querySelector('#wallet-body')
  let baseline = null
  const poll = async () => {
    if (!_page || !_page.isConnected) { stopPolling(); return }
    try {
      const data = await api.oneapiGetBalance()
      const current = Number(data.balance)
      if (baseline == null) baseline = current
      else if (current > baseline) {
        close()
        toast(t('wallet.rechargeSuccess'), 'success')
        if (body) loadBalance(body, config)
        return
      }
    } catch {}
  }
  _pollTimer = setInterval(poll, POLL_INTERVAL_MS)
}
