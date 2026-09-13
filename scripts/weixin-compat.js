import fs from 'node:fs'

// 与桌面端共用已核实的发布包边界，不跟随可变 latest 将旧内核升级到不兼容插件。
const policy = JSON.parse(fs.readFileSync(new URL('../src/lib/weixin-compat-policy.json', import.meta.url), 'utf8'))
function versionParts(value) {
  const match = String(value || '').trim().replace(/^v/, '').replace(/-zh\..*$/, '').match(/^(\d+)\.(\d+)\.(\d+)(?:-\d+)?$/)
  return match ? match.slice(1).map(Number) : null
}
function compare(a, b) {
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] - b[i]
  return 0
}
function hostFits(host, line) {
  return compare(host, versionParts(line.hostMin)) >= 0 && (!line.hostBefore || compare(host, versionParts(line.hostBefore)) < 0)
}

export function weixinCompatibility(hostVersion, pluginVersion) {
  const host = versionParts(hostVersion), plugin = versionParts(pluginVersion)
  const line = plugin && policy.lines.find(l => compare(plugin, versionParts(l.pluginMin)) >= 0 && compare(plugin, versionParts(l.pluginMax)) <= 0)
  if (!host || !line) return { compatible: null, compatError: 'OpenClaw 或微信插件版本尚未核实，请先检查版本；不会自动更新。' }
  const range = `>=${line.hostMin}${line.hostBefore ? ` <${line.hostBefore}` : ''}`
  const compatible = hostFits(host, line)
  return { compatible, compatError: compatible ? '' : `微信插件 ${pluginVersion} 要求 OpenClaw ${range}，当前为 ${hostVersion}。` }
}

export function resolveWeixinInstallVersion(hostVersion, requestedVersion) {
  const host = versionParts(hostVersion)
  if (!host) throw new Error('未识别 OpenClaw 稳定版本，请先安装或检查内核版本，再安装微信插件。')
  const line = [...policy.lines].reverse().find(l => hostFits(host, l))
  const version = requestedVersion || line?.target
  const check = weixinCompatibility(hostVersion, version)
  if (check.compatible !== true) throw new Error(check.compatError)
  return version
}

export function buildWeixinCompatibilityStatus(hostVersion, installedVersion) {
  let recommendedVersion = null, installError = ''
  try { recommendedVersion = resolveWeixinInstallVersion(hostVersion) } catch (error) { installError = error.message }
  const installed = weixinCompatibility(hostVersion, installedVersion)
  // 新于已核实版本时不提供自动降级，需人工复核新版本契约。
  const known = !installedVersion || installed.compatible !== null
  return {
    ...installed, hostVersion, recommendedVersion,
    installAllowed: !!recommendedVersion && known,
    installError: installError || (!known ? installed.compatError : ''),
    updateAvailable: !!installedVersion && known && !!recommendedVersion && compare(versionParts(installedVersion), versionParts(recommendedVersion)) < 0,
  }
}
