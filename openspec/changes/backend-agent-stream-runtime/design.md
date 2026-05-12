## Context

当前后端 `src-tauri/src/acp/client.rs` 会把 ACP SDK notification 映射为 `AgentEvent`，并通过 `agent-event` 推给前端。前端 `useAgentEvents` 负责把这些事件合并成 UI 可渲染状态：同一 message 追加文本、同一 toolCall 更新工具块、plan 替换、session state 分流、confirmation 状态乐观更新。

上一轮主控制台迁移已经把 Agent 连接和 turn 状态切到业务 API，但 Agent 内容流仍是例外。用户明确希望 ACP 推送回来的事件在后端处理完整，前端只保留渲染逻辑。本设计把 Agent 内容流从“前端 reducer 解释 ACP 语义”改为“后端 AgentStreamRuntime 维护 authoritative timeline”。

## Goals / Non-Goals

**Goals:**

- 后端维护 Agent timeline 的 authoritative snapshot，负责归属、排序、归并、确认状态和 stale event 处理。
- 对前端提供 UI-ready snapshot/patch 事件，使 `useAgentStream` 只做订阅、机械更新和动作封装。
- 保留现有 thinking、tool、result、diff、confirm、plan、session state、fallback/error 的展示能力。
- 将 `AssistantConsole` 从 `useAgentEvents` 迁移到 `useAgentStream`，让主控制台不再归并 legacy ACP raw event。
- 保留 `useAgentEvents` 和 `agent-event` 作为兼容/debug 入口，降低一次迁移风险。

**Non-Goals:**

- 不重做 ACP SDK 接入方式，不替换官方 Rust SDK。
- 不重做主控制台视觉设计。
- 不实现跨会话永久历史存储；本变更只维护当前运行期的 session/turn timeline。
- 不新增文件系统、终端交互等 ACP client capabilities。
- 不删除旧 `agent-event`，除非后续单独清理。

## Decisions

### 1. 新增 `AgentStreamRuntime` 作为后端内容流状态机

后端新增一个 runtime，接收内部归一化的 `AgentEvent` 或更窄的 adapter event，并维护：

- 当前 `sessionId`
- 当前或最近 `turnId`
- 单调递增 `sequence`
- `items: AgentTimelineItem[]`
- `plan?: AgentPlanSnapshot`
- `sessionState`
- `pendingConfirmations`

备选方案是继续让前端 reducer 处理，只补 turnId。拒绝该方案，因为它仍让前端理解 append/update/replace/sessionState 等 ACP 语义，无法达成“前端只渲染”的目标。

### 2. 后端发布 snapshot/patch，而不是裸 ACP event

后端提供 `get_agent_timeline` 命令用于首帧恢复，并发布 `agent-timeline-changed` 事件。事件 payload 使用 patch 形式：

- `reset`: 携带完整 `AgentTimelineSnapshot`
- `upsertItem`: 插入或更新一个 timeline item
- `removeItem`: 删除或隐藏一个 item（第一版可选）
- `updatePlan`: 替换当前计划
- `updateSessionState`: 替换或合并当前 session state
- `resolveConfirmation`: 更新确认状态
- `streamError`: 追加或更新错误项

备选方案是每次都发完整 snapshot。拒绝该方案，因为流式文本和 tool update 频繁，完整快照会增加序列化和前端 diff 成本。但保留 `reset` 作为初始化和异常恢复机制。

### 3. Timeline item 使用 UI 语义，而不是 ACP operation 语义

前端接收的 item 不再暴露 `operation=append/create/update/replace/sessionState/fallback` 作为主要行为依据，而是暴露稳定的 UI item 类型，例如：

- `message`：assistant result/status 文本，可带 `messageId`、`text`、`contentBlocks`
- `thinking`：思考文本
- `tool`：工具调用状态、内容、locations、raw input/output
- `diff`：独立 diff 或工具内 diff 展示
- `confirmation`：确认请求和处理状态
- `error`：可读错误
- `fallback`：未知 update 的可读降级

备选方案是把现有 `AgentEvent` 原样改名为 `AgentTimelineItem`。拒绝该方案，因为原结构仍混合协议操作和 UI 展示语义，前端 reducer 会继续承担业务判断。

### 4. Turn/session 归属由业务 Agent turn 驱动

`send_agent_message`、`edit_and_submit_transcript` 和 voice auto-send 创建业务 `AgentTurnStatus` 后，Agent stream runtime SHALL 关联后续 ACP 输出到当前 turn。若当前没有业务 turn，但 ACP 产生 session/status 事件，runtime 可以归属为 session-level item。

后端 SHALL 在 turn 取消、失败或完成后处理 late event：对已关闭 turn 的 late event 默认丢弃或标记为 ignored，不再污染当前 turn 的主 timeline。实现时需要保守处理无法识别归属的 legacy event，避免误删 session-level 状态。

备选方案是让前端按最新 turn 归属。拒绝该方案，因为 late event 和取消后的竞态只能由后端 authoritative state 可靠处理。

### 5. 确认状态以后端为准

前端点击确认/拒绝后调用 stream/business 封装命令，例如 `respond_agent_stream_confirmation` 或复用现有命令但经 `useAgentStream` 包装。前端不再乐观修改 confirmation item；后端 SDK permission response 完成后更新 confirmation 状态并推送 patch。

备选方案是保留前端乐观更新。拒绝该方案，因为 permission response 可能失败，乐观状态会与后端 pending permission map 不一致。

### 6. 兼容旧事件，分阶段迁移

第一阶段保留现有 `agent-event` 和 `useAgentEvents`，同时新增 `agent-timeline-changed` 和 `useAgentStream`。`AssistantConsole` 迁移到新 hook 后，旧 hook 只用于 debug/compat 测试。后续再单独清理旧事件和前端 reducer。

## Risks / Trade-offs

- [后端状态机复杂度上升] → 将 timeline reducer 写成纯 Rust helper，覆盖 message append、tool update、plan replace、confirmation resolve、stale event 丢弃等单元测试。
- [patch 丢失导致前端状态不同步] → 提供 `get_agent_timeline` 和 `reset` patch，前端 hook 在初始化和异常时可恢复完整快照。
- [turn 归属不准确] → 优先使用业务 turn 创建点绑定 active turn；无法归属的 session update 作为 session-level item，不强行挂到当前 turn。
- [流式文本频繁 patch 影响性能] → 后端可先逐 chunk 推送；如性能不足再增加节流或合并窗口，不改变契约。
- [旧 hook 与新 hook 双轨造成混淆] → 更新 AGENTS 文档和 imports 守护，明确主控制台只能使用 `useAgentStream`。

## Migration Plan

1. 在后端定义 Agent timeline DTO、patch DTO 和 reducer helper。
2. 将 ACP adapter 输出接入 `AgentStreamRuntime`，保留旧 `agent-event` emit。
3. 在业务 Agent turn 生命周期中通知 stream runtime 当前 active turn 的开始、完成、失败和取消。
4. 增加 `get_agent_timeline` 和确认响应封装命令，发布 `agent-timeline-changed`。
5. 新增前端 `useAgentStream`，用 snapshot/patch 维护 UI-ready state。
6. 将 `AssistantConsole` 和必要的 `AgentEventStream` props 迁移到 timeline model。
7. 更新本地 AGENTS 文档，将 `useAgentEvents` 标记为 debug/compat。
8. 补充 Rust、hook 和组件测试，运行前端构建和相关 Rust 测试。

## Open Questions

- 第一版是否需要保留跨 turn 历史，还是只展示当前 turn timeline？建议第一版保留当前 session 内最近若干 turn，但主界面默认聚焦当前 turn。
- patch 类型是否需要 `removeItem`？若第一版没有隐藏/删除需求，可以只做 reset/upsert/update/resolve。
- 旧 `agent-event` 是否继续由 ACP adapter 直接 emit，还是由 stream runtime 从同一归一化事件旁路 emit？建议先旁路保留，减少行为变化。
