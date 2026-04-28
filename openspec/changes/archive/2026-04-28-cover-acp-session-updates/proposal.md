## Why

当前 ACP runtime 只把少数 `sessionUpdate` 类型粗略映射成前端事件，文本流之外的工具更新、计划、命令、配置和会话元信息会被追加为普通状态块，导致输出流重复、结构丢失，也让后续 UI 难以可靠覆盖不同 ACP agent 的行为。

现在已经接入官方 Rust SDK，并启用了 `unstable_message_id` 处理流式消息，正适合把所有已知 `sessionUpdate` 类型纳入稳定的内部事件与会话状态模型。

## What Changes

- 完整覆盖当前 SDK 暴露的 `SessionUpdate` 类型：`user_message_chunk`、`agent_message_chunk`、`agent_thought_chunk`、`tool_call`、`tool_call_update`、`plan`、`available_commands_update`、`current_mode_update`、`config_option_update`、`session_info_update`，并为未来新增类型保留兜底。
- 将流式文本 chunk 按协议 `messageId` 增量合并，不再把同一条 agent 输出拆成多个可见块。
- 前端 SHALL 在每个流式 chunk 到达时立即更新对应输出块，实现实时增量渲染，而不是等待 turn 结束或完整消息完成后一次性刷新。
- 将工具调用按 `toolCallId` 建模，`tool_call_update` 更新已有工具块，而不是追加重复工具块。
- 从工具内容中识别 diff、terminal、标准 content block，并映射为可区分的前端事件或嵌套内容。
- 将 plan 作为可替换的当前计划快照展示，而不是每次更新都追加历史。
- 将 available commands、current mode、config options、session info 等 session-level 更新分流为会话状态，而不是混入普通输出流。
- 保持非文本 content block、未知更新类型和不支持的 unstable 类型可读可见，避免静默丢失信息。
- 补充 Rust 与前端测试，覆盖每类 `sessionUpdate` 的归一化和 UI reducer 行为。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `acp-client-runtime`: 扩展 SDK notification 归一化要求，覆盖所有当前已知 `SessionUpdate` 类型、协议标识和未来兜底。
- `assistant-console-ui`: 扩展输出流和会话状态展示要求，支持流式合并、工具更新替换、计划快照、diff 展示和 session-level 状态分流。

## Impact

- 后端 ACP 映射层：`src-tauri/src/acp/client.rs`、`src-tauri/src/acp/events.rs` 需要扩展内部 payload 与归一化逻辑。
- 前端事件 hook：`src/hooks/useAgentEvents.ts` 需要从 append-only 列表演进为支持 message/tool/plan/session state 的 reducer。
- 前端展示组件：`src/components/AgentEventStream.tsx` 和 `src/components/AssistantConsole.tsx` 需要渲染更丰富的事件结构，并避免重复块。
- 测试：新增或扩展 Rust unit tests、Vitest reducer tests，并在实现后运行 `nix develop -c cargo test acp`、`pnpm test`、`pnpm build`。
