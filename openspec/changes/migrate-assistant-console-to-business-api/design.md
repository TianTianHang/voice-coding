## Context

`redesign-frontend-business-api` 已经在后端提供了面向前端的业务命令、状态快照和事件，包括 `get_app_status`、`prepare_app`、语音会话命令、Agent turn 状态、语音输出命令与偏好设置。当前主助手控制台虽然已经引入 `useBusinessApi`，但仍同时直接消费旧的 debug/compat hook：`useBackendVAD` 负责主语音状态，`useAsrStatus` 负责模型准备状态，`useAgentEvents` 负责 Agent 连接状态，自动播报状态由本地兼容映射拼接。

这会让主界面继续理解 ASR、VAD、TTS、ACP 的内部模块边界。迁移目标是让主控制台只把业务 API 视为主流程 source of truth，同时把旧 hook 限定在调试窗口、兼容组件或尚未业务化的 Agent 内容流里。

## Goals / Non-Goals

**Goals:**

- 扩展 `useBusinessApi`，覆盖主控制台所需的业务状态、语音会话动作、转写草稿动作、Agent turn 动作、朗读动作和语音偏好设置。
- 将 `AssistantConsole` 的主流程状态派生改为基于 `AppStatus`、`VoiceSessionStatus`、`AgentStatus`、`AgentTurnStatus`、`VoiceUtteranceEvent`、`SpeechOutputStatus` 和 `RuntimeErrorEvent`。
- 保留 `useAgentEvents` 作为 Agent 内容流适配层，用于 thinking、tool、result、diff、confirm、plan 等细粒度渲染。
- 更新本地 AGENTS 文档，明确旧 debug/compat hook 不得再作为主助手控制台的主流程入口。
- 增加前端测试，覆盖业务状态派生、业务命令封装和主控制台关键状态。

**Non-Goals:**

- 不重构或移除后端 debug 命令。
- 不改变 `agent-event` 的细粒度内容流协议。
- 不重做主控制台视觉设计。
- 不扩大 Rust 后端业务 API 范围，除非实现时发现现有契约无法支撑已定义的前端迁移。

## Decisions

### 1. `useBusinessApi` 成为主界面的业务 facade

`useBusinessApi` 负责启动时订阅业务事件、读取 `get_app_status` 快照，并暴露主界面动作。新增封装应贴近后端业务命令命名，例如 `prepare`、`startVoiceSession`、`stopVoiceSession`、`pauseVoiceSession`、`resumeVoiceSession`、`discardTranscript`、`editAndSubmitTranscript`、`sendAgentMessage`、`cancelAgentTurn`、`speakAgentResult`、`setSpeechPreferences`。

备选方案是让 `AssistantConsole` 继续直接 `invoke` 缺失命令。拒绝该方案，因为它会把业务编排散落到组件中，后续迁移又会回到旧的模块耦合。

### 2. 主流程 UI 状态从业务状态派生

`deriveVoiceExperienceState` 一类纯 helper 应改为接收业务状态，而不是接收 `VADState`、旧 ACP connection state 或 debug ASR 状态。语音会话状态映射到展示状态：`starting/listening/recording/paused` 体现监听或等待，`transcribing` 体现处理中，`failed` 和 runtime error 体现错误；Agent turn running 体现处理中；speech playing/synthesizing/ready 体现播报或语音输出状态。

备选方案是先把业务状态转换回旧 `VADState` 再复用旧 helper。拒绝该方案，因为这会隐藏迁移成果，并继续让主控制台以旧状态机为核心建模。

### 3. Agent 内容流继续由 `useAgentEvents` 承担

业务 API 当前负责 Agent 连接与 turn 状态，不负责替代 thinking/tool/result/diff/confirm 等内容流。`AssistantConsole` 可以继续使用 `useAgentEvents` 渲染事件时间线和确认按钮，但不得把它的连接状态作为主连接 source of truth；连接、错误和当前 turn 状态以 `business.status.agent` 与业务 turn 事件为准。

备选方案是本变更顺手把 `agent-event` 也业务化。拒绝该方案，因为它会扩大协议迁移范围，并把主控制台迁移和 Agent stream 设计耦合到同一个变更中。

### 4. 自动朗读从业务 speech 状态和偏好派生

主控制台不再调用 debug TTS 状态 hook 来判断自动播报是否开启、是否正在播放或是否失败。界面展示应使用 `SpeechOutputStatus.autoSpeakAgentResults`、`state`、`source` 和 `error`，动作使用 `set_speech_preferences`、`speak_text`、`speak_agent_result` 和 `stop_speech` 的 hook 封装。

备选方案是保留旧 auto-TTS 兼容状态作为主展示模型。拒绝该方案，因为后端已经把 TTS 编排和录音暂停恢复封装到业务 speech 命令中。

## Risks / Trade-offs

- [业务事件乱序或首帧缺失] → 启动时先订阅事件再调用 `get_app_status`，组件只依赖完整快照恢复 UI。
- [Agent 内容流仍非业务 API] → 在设计和 AGENTS 文档中明确这是有意保留的边界，后续单独提案迁移。
- [旧 hook 删除过猛影响调试窗口] → 本变更只迁移主控制台，不删除 debug/compat hook，不改变 DebugToolsWindow。
- [类型迁移引起 UI 状态回归] → 将状态派生 helper 保持为纯函数，并用 vitest 覆盖错误、监听、处理中、响应和空闲场景。

## Migration Plan

1. 补齐 `useBusinessApi` 的类型、事件订阅和业务命令封装。
2. 将 `AssistantConsole` 的主状态派生、准备按钮、语音控制、转写草稿、Agent 发送和朗读控制迁移到 `useBusinessApi`。
3. 保留 `useAgentEvents` 仅用于内容流渲染与确认响应。
4. 更新 `src/hooks/AGENTS.md` 与 `src/components/AGENTS.md` 的模块说明。
5. 补充或更新 `src/hooks`、`src/components` 下的测试，运行前端构建和相关测试。

## Open Questions

- 无。当前变更按已有业务 API 契约迁移；若实现发现缺少后端字段或命令，再以最小后端补丁和规格更新处理。
