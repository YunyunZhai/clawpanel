/**
 * 便携版（Portable Edition）界面裁剪开关
 *
 * 便携版只内置 OpenClaw 一个运行时，并隐藏「晴辰助手」入口和「关于」页的
 * 「社群交流」分区。这里 **只裁界面**，引擎 / 助手 / 社群的实现代码全部保留，
 * 把开关改回 false 就能恢复成完整面板，不需要回滚提交。
 *
 * 便携行为本身由 portable.json + src-tauri/src/commands/portable.rs 决定，
 * 与本文件无关。
 */

/** 只保留 OpenClaw：隐藏 Hermes / OpenCode / DeepSeek Harness / 心甜Claw 的入口 */
export const HIDE_NON_OPENCLAW_ENGINES = true

/** 隐藏晴辰助手：侧栏菜单、路由、悬浮球、setup 卡片、模型渠道同步 */
export const HIDE_ASSISTANT = true

/** 隐藏「关于」页的「社群交流」分区（二维码 + 加群按钮） */
export const HIDE_ABOUT_COMMUNITY = true

/** 隐藏社群导流弹窗（engagement.js，Gateway 启动成功等时机弹出） */
export const HIDE_ENGAGEMENT = true

/** 便携版唯一内置的运行时 */
export const SOLE_ENGINE_ID = 'openclaw'

/** 运行时入口是否可见（引擎本身始终注册，仍可被 switchEngine 显式激活） */
export function isEngineVisible(id) {
  return !HIDE_NON_OPENCLAW_ENGINES || id === SOLE_ENGINE_ID
}

/** 是否还需要「选择运行时」这一步：只剩一个可见运行时时无需选择 */
export function isEngineChoiceNeeded() {
  return !HIDE_NON_OPENCLAW_ENGINES
}

/** 路由是否可见（用于侧栏菜单项与引擎路由表） */
export function isRouteVisible(route) {
  if (HIDE_ASSISTANT && route === '/assistant') return false
  if (!isEngineChoiceNeeded() && route === '/engine-select') return false
  return true
}
