## Context

当前 ACP client runtime 已经通过官方 Rust SDK 接收 typed `SessionNotification`，并把 notification 映射为内部 `AgentEvent` 后推送到前端。现有模型适合早期展示：文本、工具、状态和确认都能出现在输出流里。但 ACP 的 `SessionUpdate` 本身比当前 UI 模型丰富：

- 文本类 chunk 具有流式追加语义，`unstable_message_id` 可以标识同一条消息。
- 工具调用具有 `toolCallId` 和 `tool_call_update`，更新语义是修改同一个工具调用。
- plan 是完整快照，更新语义是替换当前计划。
- commands、mode、config、session info 是会话状态，不是普通输出消息。
- diff、terminal 和非文本 content block 可能嵌在工具内容中。

如果继续把所有更新都追加为普通块，输出流会重复，计划和工具状态会失真，前端也无法稳定支持不同 ACP agent。

## Goals / Non-Goals

**Goals:**

- 覆盖当前 SDK 暴露的所有 `SessionUpdate` 变体，并保留未来未知变体兜底。
- 建立稳定的内部事件/状态模型，表达 append、replace、tool update 和 session state update 等不同语义。
- 保持前端输出流实时增量，同时避免重复工具块和重复计划块。
- 从工具内容中识别 diff、terminal 和标准 content block，提供可读展示。
- 用 Rust unit tests 和 Vitest reducer tests 固化每类更新的行为。

**Non-Goals:**

- 不实现 ACP terminal client capability 本身；terminal 内容仅作为引用或可读占位展示。
- 不实现 commands/config options 的交互执行或配置修改，只接收并展示当前会话状态。
- 不新增远程 ACP transport 或多 agent 并发会话。
- 不为 image/audio/resource content 提供完整富媒体预览；第一版保证不丢失并可读展示。

## Decisions

1. **后端继续作为 ACP 协议归一化边界。**  
   Rust 侧已经持有 SDK typed payload、session 生命周期和权限回调，最适合把协议细节转换为前端稳定模型。前端不直接依赖 ACP schema，减少 SDK feature 变化对 UI 的扩散。备选方案是把完整 JSON 透传给前端，但会让 UI reducer 背负协议解析责任。

2. **内部 payload 区分输出事件和会话状态。**  
   `agent_message_chunk`、`agent_thought_chunk`、`tool_call`、`tool_call_update`、diff 和 confirm 进入输出流；`available_commands_update`、`current_mode_update`、`config_option_update`、`session_info_update` 更新独立 session state。这样输出流保持为执行记录，状态栏/侧区展示当前上下文。备选方案是全部放入输出流，简单但会制造噪声。

3. **使用协议标识驱动合并和替换。**  
   文本 chunk 使用 `messageId` 追加；工具调用使用 `toolCallId` 替换/合并；plan 使用固定 plan identity 替换；session-level 状态使用固定字段替换。避免基于相邻事件或标题猜测合并关系。若 `messageId` 缺失，文本 chunk 保持独立追加，避免错误合并。

4. **文本流采用到达即渲染的增量更新。**  
   前端 reducer 每收到一个文本 chunk 就更新对应输出块，使用户看到同一块内容持续增长。实现不应缓存到 `StopReason`、prompt turn 完成或完整消息结束后再统一写入 UI。备选方案是等待完整响应再渲染，逻辑更简单但会失去 ACP/SSE 流式反馈的产品价值。

5. **保留 `AgentEvent` 的兼容形态，同时扩展结构化字段。**  
   第一版可以在现有 `AgentEvent` 上增加 `messageId`、`toolCallId`、`operation`、`metadata` 等可选字段，并在前端 reducer 中解释。这样迁移范围小。备选方案是一次性拆成多个 Tauri event channel，例如 `agent-output-event` 和 `agent-session-state`，语义更干净但改动更大。

6. **tool update 在前端 reducer 中更新已有工具块。**  
   后端负责生成结构化工具事件，前端维护当前展示列表。这样前端能把 pending → in_progress → completed 作为同一块视觉状态变化。若 update 到达时没有对应 tool call，则创建一个降级工具块并展示已有字段。

7. **diff 从工具内容中显式映射。**  
   当 `ToolCallContent::Diff` 出现在 tool call 或 update 中，系统生成或嵌入 `diff` 语义，包含路径、old/new 文本或可展示摘要。前端第一版可以用 pre-wrap 文本展示 diff 摘要，后续再升级为专门 diff viewer。

8. **未知和非文本内容采用可读兜底。**  
   `ContentBlock::Text` 正常提取文本；image/audio/resource/resource_link 第一版展示类型、mime/uri/path 等摘要，并保留 JSON fallback。未知 `SessionUpdate` 继续映射为 status/update，避免 SDK 非穷尽类型导致信息丢失。

## Risks / Trade-offs

- [内部事件结构变复杂] → 将 reducer 逻辑集中在 `useAgentEvents`，并用纯函数测试覆盖 append、replace 和状态更新。
- [不同 agent 对 `messageId` 支持不一致] → 支持 `messageId` 时合并，缺失时保持独立事件，优先保证不误合并。
- [tool update 可能先于 tool call 到达] → reducer 在缺少原始 tool call 时创建降级工具块，并在后续更新中继续合并。
- [session state 从输出流移走后用户可能看不到变化历史] → 当前状态区展示最新值，必要的未知/错误仍进入输出流。
- [diff 内容可能很大] → 第一版保留完整内容但允许 UI 折叠，测试只验证数据不丢失和类型正确。

## Migration Plan

1. 扩展后端 `AgentEvent` / session state payload，保持已有字段兼容。
2. 更新 `event_from_notification` 映射，覆盖所有当前 `SessionUpdate` 类型。
3. 将前端 `useAgentEvents` reducer 从简单列表扩展为输出流 + session state。
4. 更新 `AgentEventStream` 和 `AssistantConsole` 展示工具、计划、diff 和会话状态。
5. 增加 Rust 和前端测试，验证每类更新的映射与 reducer 行为。
6. 实现期间保持现有错误、确认和连接状态事件不回退。

回滚策略：保留原有 `agent-event` 通道和基本 `kind/content` 字段。如果结构化展示有问题，可以临时退回到 JSON fallback 输出，同时保留后端 typed 映射测试。

## Open Questions

- plan 应展示在输出流中的固定块，还是单独的当前计划区域？本提案倾向固定计划块，具体 UI 可在实现时根据现有面板空间决定。
- diff 第一版是否需要完整 side-by-side 视图？本提案仅要求可读和不丢失，专门 diff viewer 可作为后续增强。
