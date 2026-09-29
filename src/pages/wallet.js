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
      <button class="btn btn-primary btn-sm" id="wallet-recharge">${t('wallet.recharge')}</button>
      <div class="form-hint" style="margin-top:8px">${t('wallet.rechargeHint')}</div>
      <div class="form-hint" id="wallet-error" style="display:none;color:var(--error)"></div>
    </div>

    <div class="config-section">
      <div class="config-section-title">${t('wallet.oneapiConfig')}</div>
      <div class="form-hint" style="overflow-wrap:anywhere">${esc(config.url)}</div>
      <button class="btn btn-secondary btn-sm" id="wallet-edit" style="margin-top:8px">${t('wallet.editConfig')}</button>
    </div>
  `

  body.querySelector('#wallet-recharge').onclick = () => openRechargeModal(config)
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

function openRechargeModal(config) {
  const overlay = document.createElement('div')
  overlay.className = 'modal-overlay'

  // 默认金额选项（后端未配置时使用）
  const DEFAULT_AMOUNTS = [10, 20, 50, 100, 200, 500]
  // 默认支付方式
  const DEFAULT_METHODS = [
    { name: '支付宝', type: 'alipay', color: 'var(--primary)' },
    { name: '微信', type: 'wxpay', color: 'var(--success)' },
  ]

  let topupInfo = null
  let fetchingInfo = true

  overlay.innerHTML = `
    <div class="modal" style="max-width:400px">
      <div class="modal-header">
        <span class="modal-title">${t('wallet.recharge')}</span>
        <button class="btn btn-sm modal-close-btn" style="border:none;background:none;cursor:pointer;font-size:18px">&times;</button>
      </div>
      <div class="modal-content-body" id="recharge-form-body">
        <div style="text-align:center;padding:20px 0;color:var(--text-tertiary)">加载支付信息...</div>
      </div>
    </div>
  `
  document.body.appendChild(overlay)

  const close = () => { stopPolling(); overlay.remove() }
  overlay.addEventListener('click', e => { if (e.target === overlay) close() })
  overlay.querySelector('.modal-close-btn').onclick = close

  const body = overlay.querySelector('#recharge-form-body')

  // 并行加载配置和余额基线
  Promise.all([
    api.oneapiGetTopupInfo().catch(() => null),
    api.oneapiGetBalance().catch(() => null),
  ]).then(([_info, _balance]) => {
    topupInfo = _info
    fetchingInfo = false

    const amounts = (topupInfo?.amount_options && topupInfo.amount_options.length)
      ? topupInfo.amount_options
      : DEFAULT_AMOUNTS

    const payMethods = (topupInfo?.pay_methods && topupInfo.pay_methods.length)
      ? topupInfo.pay_methods
      : DEFAULT_METHODS

    const onlineEnabled = topupInfo?.enable_online_topup === true

    if (!onlineEnabled) {
      body.innerHTML = `
        <div style="text-align:center;padding:20px 0;color:var(--text-tertiary)">
          <p style="margin-bottom:12px">管理员尚未配置支付渠道</p>
          <p style="font-size:var(--font-size-xs)">请前往 OneAPI 管理后台设置易支付 (Epay) 参数</p>
        </div>
      `
      return
    }

    let selectedAmount = amounts[0]
    let selectedMethod = payMethods[0]?.type || 'alipay'

    const step1Html = `
      <div style="margin-bottom:16px">
        <label style="display:block;font-size:var(--font-size-xs);color:var(--text-tertiary);margin-bottom:6px">充值金额</label>
        <div style="display:flex;flex-wrap:wrap;gap:8px" id="amount-options">
          ${amounts.map((a, i) => `
            <button class="btn btn-sm ${i === 0 ? 'btn-primary' : 'btn-secondary'}" data-amount="${a}">${a} USD</button>
          `).join('')}
        </div>
      </div>
      <div style="margin-bottom:16px">
        <label style="display:block;font-size:var(--font-size-xs);color:var(--text-tertiary);margin-bottom:6px">支付方式</label>
        <div style="display:flex;flex-wrap:wrap;gap:8px" id="pay-methods">
          ${payMethods.map((m, i) => `
            <button class="btn btn-sm ${i === 0 ? 'btn-primary' : 'btn-secondary'}" data-method="${esc(m.type)}"
                    style="${m.color ? `--btn-bg:${m.color};--btn-border:${m.color}` : ''}">${esc(m.name)}</button>
          `).join('')}
        </div>
      </div>
      <div id="pay-error" style="color:var(--error);font-size:var(--font-size-xs);margin-bottom:12px;display:none"></div>
      <button class="btn btn-primary" id="btn-pay" style="width:100%">生成支付二维码</button>
      <div id="qr-container" style="text-align:center;margin-top:16px;display:none"></div>
    `

    body.innerHTML = step1Html

    // 金额选择
    body.querySelector('#amount-options').addEventListener('click', e => {
      const btn = e.target.closest('[data-amount]')
      if (!btn) return
      selectedAmount = Number(btn.dataset.amount)
      body.querySelectorAll('#amount-options button').forEach(b => {
        b.className = `btn btn-sm ${Number(b.dataset.amount) === selectedAmount ? 'btn-primary' : 'btn-secondary'}`
      })
    })

    // 支付方式选择
    body.querySelector('#pay-methods').addEventListener('click', e => {
      const btn = e.target.closest('[data-method]')
      if (!btn) return
      selectedMethod = btn.dataset.method
      body.querySelectorAll('#pay-methods button').forEach(b => {
        b.className = `btn btn-sm ${b.dataset.method === selectedMethod ? 'btn-primary' : 'btn-secondary'}`
      })
    })

    // 生成支付二维码
    const errorEl = body.querySelector('#pay-error')
    const qrContainer = body.querySelector('#qr-container')
    body.querySelector('#btn-pay').onclick = async () => {
      errorEl.style.display = 'none'
      qrContainer.style.display = 'none'
      qrContainer.innerHTML = ''

      try {
        const result = await api.oneapiRequestEpay(selectedAmount, selectedMethod)
        const payUrl = result?.pay_url
        if (!payUrl) throw new Error('未获取到支付链接')

        const qrSrc = `https://api.qrserver.com/v1/create-qr-code/?size=280x280&margin=8&data=${encodeURIComponent(payUrl)}`
        qrContainer.innerHTML = `
          <div style="margin-top:12px;padding:12px;background:var(--bg-secondary);border-radius:8px">
            <img src="${qrSrc}" alt="支付二维码" width="280" height="280"
                 style="background:#fff;border-radius:8px;padding:8px;display:block;margin:0 auto">
            <p style="margin-top:10px;font-size:var(--font-size-xs);color:var(--text-tertiary)">
              请使用手机扫码完成支付，支付成功后余额将自动更新
            </p>
          </div>
        `
        qrContainer.style.display = ''

        // 开始轮询余额
        pollBalanceUntilPaid(overlay, config)
      } catch (e) {
        errorEl.textContent = String(e?.message || e)
        errorEl.style.display = ''
      }
    }
  })

  // 轮询余额
  function pollBalanceUntilPaid(overlayEl, cfg) {
    let baseline = null
    stopPolling()
    const poll = async () => {
      if (!_page || !_page.isConnected) { stopPolling(); return }
      try {
        const data = await api.oneapiGetBalance()
        const current = Number(data.balance)
        if (baseline == null) { baseline = current; return }
        if (current > baseline) {
          close()
          toast(t('wallet.rechargeSuccess'), 'success')
          const walletBody = _page?.querySelector('#wallet-body')
          if (walletBody) loadBalance(walletBody, cfg)
          return
        }
      } catch {}
    }
    // 首次调用建立基线
    poll()
    _pollTimer = setInterval(poll, POLL_INTERVAL_MS)
  }
}