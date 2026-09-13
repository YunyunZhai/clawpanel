# DeepSeek Harness 最新版兼容检查（2026-09-14）

## 上游与版本策略

- 官方最新 Release 为 [dsh-v0.1.5-rc.2](https://github.com/deepseek-ai/deepseek-harness/releases/tag/dsh-v0.1.5-rc.2)，发布于 2026-09-10，明确标记为 **Pre-release**。
- 检查时 npm `next` 为 `0.1.5-rc.2`，`latest` 仍是 `0.1.5-rc.1`。面板固定已测试的 `0.1.5-rc.2`，不追随漂移标签，不将 RC 表述为稳定版。
- 原受管目标为 `0.1.1-rc.2`。本次升级目标及协议适配，不自动替用户更新运行时；用户停止受管服务后手动点击更新。相同版本、更高 RC、同核心正式版不提示降级。
- 本节是 ClawPanel `0.21.6` 工作树上的发布前验收快照；改动整理入 `0.21.7`，实际 CI/发布状态以对应 Actions / Release 为准。此前 OpenClaw/Hermes 改动保留。

## 已复现问题与修复

1. **启动误报失败**：新版需用上游启动 URL 交换 HttpOnly Cookie；旧探测直接调用 API 返回 401。Web/Rust 后端识别受管进程就绪行，在内存中交换并缓存认证，启动日志对令牌脱敏。服务“进程受管”和“RPC 就绪”分别判断，认证异常时仍可停止进程。
2. **RPC 变更**：新端点采用 `settings/describe` 等斜线路径与 `{args: ...}` 命名参数；Provider 列表为 `llm/listProviders`，模型目录为 `session/modelCatalog`。适配 Credential、Provider 回包，并保留旧版点分端点/封套。
3. **内嵌白屏**：新版 HTML 的 `<base href="/">` 使相对 assets 访问面板根目录。代理改写 base，预加载与脚本统一匿名 CORS，保持 Web iframe 的不透明源沙箱。上游认证 Cookie 只在后端注入，不向浏览器透传 Set-Cookie，不转发面板 Cookie/Authorization。
4. **远程目录选择失效**：新版按“回环地址+本地显示器”自动选系统目录窗口，远程面板用户看不到。受管新版以 `--profile web --patch` 应用官方 browse 组件；叠加文件只写受管目录，不修改用户 Profile。参考上游 [目录选择组件](https://github.com/deepseek-ai/deepseek-harness/tree/dsh-v0.1.5-rc.2/packages/client/ui-directory-picker-native) 与 [浏览器测试叠加层](https://github.com/deepseek-ai/deepseek-harness/blob/dsh-v0.1.5-rc.2/apps/web/tests/pin-browse-picker.overlay.yml)。
5. **更新与诊断**：安装链实际固定 pnpm 11.7.0，而非优先使用未知全局版本；运行期间拒绝更新/卸载。配置页新增目标版本、更新入口并保留操作及探测错误。

## 实测与证据

环境：Windows x64、Node 24.15.0；隔离 HOME/USERPROFILE/DSH_HOME、受管目录、端口 19870/19872，本地测试模型接口 19874。没有修改真实用户配置、使用真实模型密钥或调用付费推理。

| 检查 | 结果 |
| --- | --- |
| 官方 npm 包真实安装与原生依赖构建 | 0.1.5-rc.2 安装成功，版本回读一致 |
| 受管启动与状态 | running/managed 均为 true，error 为空 |
| 模型渠道同步 | 自定义 OpenAI-compatible Provider、两个模型、凭据状态、默认模型均回读成功 |
| 上下文与输出上限 | 131072/8192、65536/4096 保留；真实请求模型 A 携带 max_completion_tokens=8192 |
| 内嵌浏览器 | 页面加载、网页选目录、添加工作区、模型菜单与模型切换通过；修复后新页面无控制台错误/警告 |
| 流式对话 | 官方 DSH 运行时 + 本地模拟模型 HTTP 接口返回 DSH_COMPAT_OK；页面展示回答、用量及会话 |
| 重装/重启 | 运行中更新被拒绝；停止后同版重装成功；重新启动后 Provider、上下文、默认模型及会话记录保留 |
| 凭据边界 | 代理响应无上游 Cookie、启动 token；上游认证不出现在普通状态中 |
| Node 全量回归 | 629 项，628 通过、1 跳过、0 失败；跳过项是 Windows 不适用的 POSIX 权限测试 |
| Rust 回归 | 371 通过；格式、Clippy 检查通过 |
| 前端生产构建 | 通过，保留原有大分包提示 |

可复核本地证据：`output/dsh-compat-2026-09-14/` 内的 `readback-result.json`、`restart-result.json`、`ui-chat-success.txt`、`ui-restart-success.txt`、Node/Rust/构建日志；截图为 `output/playwright/dsh-015-rc2-chat.png`。这些测试产物不进入发布包。

## 验证边界

- 本次完整运行时与浏览器链路在 Windows Web 模式实测；Tauri 代码通过 Rust 测试/编译，但原生 WebView 的 Cookie/iframe 交互、Linux/macOS 实机尚待验收。
- 旧版 RPC 通过回归测试保留，本次没有重新执行旧版完整二进制矩阵或跨版本原有配置迁移；重装验证是新版同版重装，不等同于旧版升级迁移验收。
- 真实推理供应商、Anthropic/Responses 云端请求、全部 DSH 工具/插件不在本次验收范围。模拟模型对话用于验证传输、模型选择和界面链路，不代表云端服务验收。
- 面板后端重启会丢弃内存认证。如果遗留 DSH 进程仍在运行，页面会提示认证异常；需停止后由面板重新启动，不从日志恢复密钥，不关闭上游认证。
- 本快照记录时尚未执行 GitHub CI 或发布；发布须通过跨平台 CI。Linux Web 与原生桌面运行验证仍是独立门槛，不以构建通过替代。
