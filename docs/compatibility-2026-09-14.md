# 2026-09-14 上游兼容与渠道安装复查

状态：v0.21.7 发布前的本地验收记录，已完成下列修复与验证，不代表全部渠道端到端验收完成。CI 和发布状态以该版本的 Actions / Release 为准。

## 上游基准

| 项目 | 核实版本 | 本次结论 |
| --- | --- | --- |
| OpenClaw | [2026.9.4](https://github.com/openclaw/openclaw/releases/tag/v2026.9.4)，源码 `3a9d69db306cd7f081e06254cb89c4bcc14a7107` | 有实际适配点：Node 要求、npm 安装来源确认、新插件注册表。Gateway 协议仍为 4，现有 3..4 协商范围适用。 |
| Hermes Agent | [0.21.2 / v2026.9.11](https://github.com/NousResearch/hermes-agent/releases/tag/v2026.9.11)，tag 解析到 `939e45c91d751fadd94dcd1b873ac3cb44846213` | 包含 state.db、并发锁和 Profile 隔离修复。已核对模型/Provider/Profile、Web 服务和安装器协议，未发现本次需要修改的对应字段；尚未完成完整运行时升级验收。 |

版本依据 GitHub Release API、标签源码和 npm 元数据，而不是搜索引擎缓存。
Hermes 最新安装器来自固定 commit，并核对原始字节 SHA-256：

- Windows `install.ps1`：`226c70a90ad47e8a4d34cb11aca4ecbeb649e2f9b67fbd009ea49791de2d56f5`
- Linux `install.sh`：`5854b15670b51a8daae8f59ddfa917062de9f74be261eb73b4b8d719710f8968`

两个安装器的 manifest 均为协议 1，当前 ClawPanel 使用的安装阶段仍存在。
这是安装器协议检查，不是执行了完整 Linux/Windows 安装。

## 已修复的根因

1. 渠道注册表中包名已带 `@latest`，前后端又追加内核版本，产生非法 npm 规格。
2. 旧版 `split('-')[0]` 丢失官方 `-2` 修订号；现仅删除 `-zh.*` 后缀。
3. Web 安装状态只检查旧目录，漏掉 `extensions`、QQ manifest 别名、新版 `dist/extensions` 和 npm 原生注册表。
4. 同步安装进程阻塞 Node 服务，旧版超时过短且 Web 不接收桌面日志事件；现改为异步子进程、认证后的 NDJSON 日志与心跳，最长等待十分钟。
5. 原先仅凭退出码报告成功；现核对插件记录和实际 manifest，再启用插件。新注册表通过上游 `plugins list --json` 获取，不直接读写其 SQLite。
6. Web 微信状态/动作原为占位实现；现接入真实安装、更新、登录。桌面微信不再安装前删除旧目录及账号配置。
7. 安装与更新明确区分：普通重复安装幂等，普通插件升级继续使用原生 update；微信更新改为精确兼容版本安装，避免历史 latest 更新记录跨越兼容版本线。

OpenClaw 9.4 的 npm 首次安装使用其 `--force` 来源确认选项；普通已存在插件不进入此覆盖分支。微信用户主动点击安装/更新时使用精确包版本和 `--force`，由原生 CLI 执行替换事务；面板不预删目录。未添加跳过安装安全策略、接受扩大权限等选项。
Node 版本要求按上游 `engines.node` 适配：`>=24.16.0 <25 || >=26.1.0`，旧内核继续使用旧要求。

## 验证记录

在工作区 `output/compat-2026-09-14/` 下准备独立 Node 24.16.0、官方 OpenClaw 2026.9.4、配置和 npm 插件目录；未改用户全局安装或真实配置。

| 验证 | 结果 |
| --- | --- |
| 官方 CLI 配置校验、Gateway 协议 4 握手 | 通过 |
| health、status、agents.list、models.list、sessions.list、channels.status | 全部通过 |
| 内置 Telegram 检测与重复安装 | 正确识别，未重新下载 |
| LINE 2026.9.4 | 真实安装，修复原生注册表检测后回读正常 |
| IRC 2026.9.4 | Web 流式接口真实安装、文件回读、启用成功；期间 8 次健康请求正常 |
| 微信插件 2.4.8 | Web 流式接口真实安装、文件回读、启用成功；期间 10 次健康请求正常 |
| 微信 Web 页面 | Chromium 真实检测 2.4.8，点击安装按钮触发原生更新检查，上游返回已是最新，操作完成 |
| Node 回归 | 622 项：621 通过、1 跳过、0 失败；跳过项为 Windows 上不适用的 POSIX 文件权限测试 |
| Rust 回归 | 368 通过 |
| 前端生产构建 | 通过；存在原有大分包提示 |
| cargo fmt / clippy --locked --all-targets -- -D warnings | 通过 |
| Linux 部署脚本 bash -n、git diff --check | 通过 |

本地证据包括 `runtime-smoke-result.json`、`channel-api-smoke-result.json`、安装日志、Node/Rust 日志和 `output/playwright/channel-weixin-update-20260914.png`。

## 边界和后续门槛

- 本次没有真实账号扫码、消息发送、云模型计费请求或渠道全平台端到端验收。
- Linux/macOS 实际安装、Hermes 完整升级、全部渠道插件、旧 OpenClaw 的真实二进制矩阵和 GitHub CI 尚未执行；本地单测保留了旧版逻辑回归。
- OpenClaw 在隔离环境中有外部目录获取的 DNS/网络策略提示，主要本地 RPC 通过；不把外部网络可达性计为已验证。
- 默认稳定基线仍是 OpenClaw 2026.8.2 和 Hermes 0.20.5 / v2026.8.19。切换默认版本前应完成 Linux 和 Hermes 完整升级/回滚验收，再走 CI 和发版流程。

## 对照微信及区域渠道文档的追加复查

### 已完成修复

- 微信插件实际 npm latest 为 **2.4.8**。官方文档举例仍为 2.4.6；以发布包 `openclaw.install.minHostVersion`、peerDependencies 及 OpenClaw 9.4 的 channel-catalog 交叉核实，不直接照抄旧 README 中的 2026.3.22 门槛。
- Web 原先缺失兼容性检查，桌面统一按旧门槛判断。现在两端共用 `src/lib/weixin-compat-policy.json`：

  | 宿主 OpenClaw | 精确安装目标 | 证据/边界 |
  | --- | --- | --- |
  | >=2026.1.0、<2026.3.22 | 1.0.3 | 官方文档 legacy 线及 npm legacy 标签 |
  | >=2026.3.22、<2026.5.12 | 2.4.4 | 发布包的 minHostVersion/peerDependencies |
  | >=2026.5.12 | 2.4.8 | 当前包元数据与 9.4 核心目录共同确认 |

- 未识别的宿主/未核实的新插件版本显示“兼容性待核实”，不自动覆盖；已安装插件和目标版本分别显示。并不声称已在所有旧内核二进制上运行过以上版本。
- 微信升级不再沿用历史 `latest` 安装记录；用精确兼容版本交给 CLI 替换，结束后核对 package.json 中的实际版本，再报告成功。失败不预删账号和插件目录。
- `wechat` / `weixin` 与 `openclaw-weixin` 在操作、配置和路由入口映射一致。
- 页面说明微信仅支持私信和媒体，增加显式“重启 Gateway”入口。扫码只表示凭据保存完成，不再宣称消息已连接。不改全局 `session.dmScope`，只提示多账号隔离的推荐值。
- 手动复制的安装命令也填入精确兼容版本；未完成检测前不提供可复制的 latest 安装命令。已知不兼容时禁用登录/重启动作，保留兼容版本重装入口。
- 飞书保存配置曾强制启用 `openclaw-lark` 并禁用官方 `feishu`；现停止这种隐式迁移。新安装默认官方包，存在 Lark 安装时复用原实现；两个实现同时启用时明确提示处理冲突，而不是擅自关闭其中一端。
- Zalo Personal 增加原有 Web/桌面登录接口对应的扫码按钮。

### 本轮验证（覆盖此前局部计数）

| 检查 | 结果 |
| --- | --- |
| Node 全量回归 | 649 项，648 通过，1 项 POSIX 权限用例在 Windows 跳过，0 失败 |
| Rust 全量回归 | 372 通过，含与 Web 共用的 15 组微信版本边界数据 |
| build / cargo fmt / clippy --locked --all-targets -- -D warnings | 通过；保留已有前端大分包提示 |
| 微信 2.4.8 | 真实 Web 流式接口使用 `wechat` 别名执行精确版本替换、回读和启用；期间 10 次健康检查通过 |
| 飞书 2026.9.4 | 真实安装官方包、文件回读、启用；期间 9 次健康检查通过 |
| Zalo Personal 2026.9.4 | 真实安装、回读、启用；期间 9 次健康检查通过 |
| Zalo 2026.9.4 | 本隔离发行包没有检测为内置，走真实 npm 安装后回读、启用；期间 8 次健康检查通过 |
| Feishu / Zalo / Zalo Personal / LINE 配置 | 用纯合并函数生成假凭据样本，官方 2026.9.4 `config validate --json` 返回 valid=true、warnings=[]，不进行平台认证 |
| 安装新插件后的 Gateway | 重新启动隔离 Gateway，协议 4 握手及 health/status/agents.list/models.list/sessions.list/channels.status 全部通过 |
| Chromium 真实页面 | 检测 2.4.8，点击安装、等待日志完成并回读；手动命令使用 2.4.8 |
| Chromium 旧宿主状态（模拟 API） | 2026.5.11+插件2.4.8 显示不兼容和目标2.4.4，登录/重启禁用，手动命令为2.4.4；这不是旧内核实测 |

本轮证据：`output/channel-doc-review-2026-09-14/` 中的运行日志、runtime-results.json、config-validation.log、Node/Rust 构建日志，以及 `output/playwright/weixin-update-real-20260914.png`、`weixin-old-host-mocked-20260914.png`。
页面测试期间隔离 Gateway 曾处于停止状态，浏览器出现相应 WebSocket 重连错误；另行启动后的真实 RPC 测试通过。Gateway 日志仍有外部模型目录刷新失败，不计为外网请求验收通过。

### 仍未闭环的区域渠道差异

**这些不计入“全部渠道兼容完成”：**

- **QQ Bot 2.0.3**：中文文档仍列 `@openclaw/qqbot`，但 9.4 实际核心目录使用 `@tencent-connect/openclaw-qqbot@2.0.3`，并将插件 ID 改为 **`openclaw-qqbot`**（渠道 ID 仍是 `qqbot`）。当前面板的安装/修复、允许插件列表和账号保存仍有旧 ID 假设，需要独立适配。核心的 `qqbot.tencent-2.0-compatibility` 也新增 allowFrom 和默认账号约束，不能只替换包名。npm `@openclaw/qqbot` 最新仅2026.7.1，不存在2026.9.4，禁止机械按内核版本拼接安装。**本轮没有声称 QQ 2.0 安装/收发通过。**
- **腾讯元宝**：文档渠道键 `yuanbao`，实际插件 ID `openclaw-plugin-yuanbao`，私信策略在嵌套 `dm`。面板尚缺专属安装/配置向导；不能照搬普通 dmPolicy 表单。9.4 核心固定2.18.2，npm最新2.18.3，仍需对新包单独验证。
- **Zalo ClawBot**：`@zalo-platforms/openclaw-zaloclawbot@0.1.4`，插件和渠道键均为 `openclaw-zaloclawbot`，不是 Zalo 或 Zalo Personal 的别名；面板尚缺独立安装/扫码入口。
- 本轮没有真实微信/Zalo账号扫码、配对、多账号消息隔离或收发验收，也没有 Linux/macOS 运行时回归或 GitHub CI。本地配置、安装和 RPC 通过不代表这些门槛通过。

### 主要改动入口

- `src/lib/weixin-compat-policy.json`、`scripts/weixin-compat.js`、`src-tauri/src/commands/weixin_compat.rs`：共享版本策略及双后端判断。
- `scripts/channel-plugin-install.js`、`scripts/dev-api.js`、`src-tauri/src/commands/messaging.rs`：精确安装、回读、别名处理和飞书保存修复。
- `src/pages/channels.js`、`src/lib/feishu-plugin-selection.js`、`src/locales/modules/channels.js`：兼容状态、安装目标、重启/扫码、飞书实现选择。
- `tests/weixin-compat.test.js`、`tests/fixtures/weixin-compat.json`：版本边界、安装回读和选择逻辑回归。

### 一手参考

- [微信文档](https://docs.openclaw.ai/zh-CN/channels/wechat)
- [飞书文档](https://docs.openclaw.ai/zh-CN/channels/feishu)
- [QQ Bot 文档](https://docs.openclaw.ai/zh-CN/channels/qqbot)
- [腾讯元宝文档](https://docs.openclaw.ai/zh-CN/channels/yuanbao)
- [Zalo ClawBot 文档](https://docs.openclaw.ai/zh-CN/channels/zaloclawbot)
- [Zalo 文档](https://docs.openclaw.ai/zh-CN/channels/zalo)、[Zalo Personal 文档](https://docs.openclaw.ai/zh-CN/channels/zalouser)、[LINE 文档](https://docs.openclaw.ai/zh-CN/channels/line)
- npm 发布元数据与已下载发布包；官方 openclaw@2026.9.4 的 `dist/channel-catalog.json`。对应精简快照为 `regional-core-catalog.json` 与 `qqbot-latest-metadata.json`。
