# OpenClaw 交互提问与聊天截断修复

## 问题与根因

反馈对比：ClawPanel 显示“输出超时，已自动结束”，同一会话的 OpenClaw Control UI 显示待答选择题。

1. 聊天页只处理 `chat`、`agent` 等事件，未处理独立的 `question.requested` / `question.resolved`，也没有答案提交入口。
2. 流式文本 90 秒没有更新即结束、首回复 180 秒超时，未区分模型无响应与等待人工输入；工具活动也未重置流式空闲计时。
3. 收到 `final` 时，已有流式气泡仍使用 `_currentAiText`，没有用 `final` 全文更新，最后一帧 delta 缺少的内容会继续缺失。

## 修复

- `src/lib/chat-questions.js`：内存中的请求状态、终态去重、并发提交互斥、连接代际校验、过期、列表与实时事件竞态保护。
- `src/lib/ws-client.js`：能力探测后的 `question.list`；写入精确使用 `{ id, answers: { answers: { questionId: [value] } } }` 或 `{ id, cancel: true }`，失败不当作成功。
- `src/components/chat-question-card.js` / `src/style/chat-questions.css`：单选、多选、自由输入、逐题填写、上一题、提交、取消、失败重试及断线/终态提示；不预选、不自动回答。通过文本节点展示外部问题和选项。
- `src/pages/chat.js`：问题按 sessionKey 隔离；待答时暂停 90/180 秒计时、普通消息队列和托管续发；结束待答后继续关联 run。超时前补查待答列表，避免漏事件时假结束。final 使用权威全文渲染及保存。
- 中英文文案及所有语言的既有 fallback 字典已生成。

## 协议依据

- [OpenClaw WebChat 官方文档](https://docs.openclaw.ai/platforms/mac/webchat) 描述 `question.list` / `question.resolve`、事件以及重连恢复。
- 直接核对已安装的官方 `openclaw@2026.9.4` 发布包中的 question schema、Gateway handlers 与 QuestionManager：每次 1–3 题；选项按 label 提交；取消针对整次请求，不伪造跳过单题的空答案。
- `question.list` 返回待答列表；已处理记录仅短暂保留，所以重连后消失的请求标记为“已结束或已由其他客户端处理”，不臆断其具体答案。

## 验证

### 自动化

- `node --test tests/chat-questions.test.js`：22 项，覆盖输入形状、单多选、三题完整性、取消、重复提交、失败重试、旧内核能力降级、过期、连接与列表竞态、跨会话隔离。
- 使用真实聊天页事件/计时器函数配合虚拟时间：待答超过 240 秒不超时；回答后同一 run 继续；工具活动延长空闲计时；真正无响应仍结束；final 全文保存。
- 全量 `node --test tests/*.test.js`：671 项，670 通过、1 项 Windows 下跳过的 POSIX 测试、0 失败；`npm run build` 与 `git diff --check` 通过。

### Windows 浏览器 + 官方 Gateway

隔离环境：OpenClaw 2026.9.4、Node 24.16.0，Gateway 仅监听 `127.0.0.1:19894`；测试面板仅监听 `127.0.0.1:19890`，独立 HOME/配置。使用真实聊天页组件和 WebSocket/HTTP 链路，不使用现有用户账户、模型密钥或服务配置。

- 浏览器通过真实 `question.request` 触发三题，界面依次填写单选、多选与文本；`question.get` 回读状态为 `answered`，三题答案完全一致。
- 页面刷新恢复待答卡；WebSocket 重连恢复可提交状态，输入草稿保留。
- 实际等待 212 秒仍为 pending、卡片仍可操作、无“输出超时”；取消后 Gateway 待答列表为空。
- 注入一次客户端网络错误，界面保留草稿和错误提示；重试提交后真实 Gateway 回读答案一致。
- Gateway 真实 1 秒过期请求在界面显示过期并移除提交控件。
- 430×880 亮色、1280×900 暗色截图检查，未出现横向溢出。

本地证据：`output/question-compat-2026-09-15/`；截图：`output/playwright/question-mobile.png`、`question-desktop-dark.png`、`question-answered.png`。

## 边界

- 浏览器中提问是向官方 Gateway 直接提交的测试请求；流式正文和超时回归包含事件/虚拟时间注入，不等同于商业模型端到端验收。
- 旧内核已测能力降级分支，未在本轮启动所有历史 OpenClaw 版本。原生 Tauri WebView、Linux/macOS 实机未在本轮执行。
- `isSecret` / `secretStore` 提问保留明确的原生面板提示，不在普通表单实现密钥存储授权；普通提问和答案不写入本地聊天历史。
- 上述记录为本地修复阶段的验证结果；修复纳入 v0.21.8，发布状态以对应 GitHub CI 与 Release 工作流为准。
