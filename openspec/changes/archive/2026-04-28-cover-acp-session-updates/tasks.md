## 1. 后端事件模型

- [x] 1.1 扩展 `AgentEvent` 和相关 payload 类型，加入 `messageId`、`toolCallId`、操作语义、工具内容、计划快照和会话状态所需字段。
- [x] 1.2 为文本、工具、计划、diff、terminal、非文本 content block 和 session state 定义稳定的序列化结构。
- [x] 1.3 保持现有 `kind/content/id/createdAt` 字段兼容，避免前端基础输出流在迁移期间中断。

## 2. ACP sessionUpdate 归一化

- [x] 2.1 显式处理 `user_message_chunk`、`agent_message_chunk` 和 `agent_thought_chunk`，按 `messageId` 透传流式身份。
- [x] 2.2 显式处理 `tool_call` 和 `tool_call_update`，按 `toolCallId` 保留工具 kind、status、title、content、locations、raw input 和 raw output。
- [x] 2.3 从 `ToolCallContent` 中识别 `Content`、`Diff` 和 `Terminal`，保留 diff path/old/new 文本和 terminal id。
- [x] 2.4 显式处理 `plan`，生成包含所有 entries 的当前计划快照。
- [x] 2.5 显式处理 `available_commands_update`、`current_mode_update`、`config_option_update` 和 `session_info_update`，生成 session-level 状态更新。
- [x] 2.6 保留未知或未来新增 `SessionUpdate` 的可读 fallback，确保不会静默丢失。

## 3. 前端 reducer 和状态

- [x] 3.1 将 `useAgentEvents` 从 append-only 列表扩展为集中 reducer，维护输出事件列表和 ACP session state。
- [x] 3.2 实现文本 chunk 追加合并：优先按相同 `messageId + kind` 合并；当缺少 `messageId` 且为 `thinking/result` 的 `append` 增量时，合并到最近同类流式块。
- [x] 3.3 确保每个 result/thinking chunk 到达时立即触发对应块的可见文本更新，不等待 stop reason 或完整消息结束。
- [x] 3.4 实现相同 `toolCallId` 的工具块更新/替换，并支持 update 先于 tool call 的降级块。
- [x] 3.5 实现 plan 快照替换，不追加重复计划历史。
- [x] 3.6 将 available commands、current mode、config options 和 session info 保存为最新会话状态，不混入普通输出日志。

## 4. 前端展示

- [x] 4.1 更新 `AgentEventStream`，展示 result、thinking、tool、diff、confirm、error、status 和未知 fallback。
- [x] 4.2 为工具块展示 title、kind、status、locations、content 摘要和 failed 状态样式。
- [x] 4.3 展示 diff 的路径和变更内容摘要或完整文本，并与普通文本视觉区分。
- [x] 4.4 展示 terminal 引用和非文本 content block 的可读占位，避免空白块。
- [x] 4.5 在助手面板中展示当前 plan 和 session-level 状态摘要，包括 mode、session title、commands 和 config options。

## 5. 测试

- [x] 5.1 为 Rust ACP mapper 增加 unit tests，覆盖每类已知 `SessionUpdate` 的归一化。
- [x] 5.2 为 Rust ACP mapper 增加 tool content 测试，覆盖 diff、terminal 和非文本 content block。
- [x] 5.3 为前端 reducer 增加 Vitest，覆盖 message 合并、逐 chunk 增量更新、tool update 替换、plan 替换和 session state 更新。
- [x] 5.4 为前端展示组件增加必要测试或快照断言，覆盖工具失败、diff、terminal 和未知 fallback。

## 6. 验证

- [x] 6.1 运行 `nix develop -c cargo test acp`，确认 ACP 后端测试通过。
- [x] 6.2 运行 `nix develop -c cargo clippy`，确认 Rust lint 通过或记录明确阻塞。
- [x] 6.3 运行 `pnpm test`，确认前端测试通过。
- [x] 6.4 运行 `pnpm build`，确认 TypeScript 和 Vite 构建通过。
- [x] 6.5 记录最终验证结果和任何未解决风险。
