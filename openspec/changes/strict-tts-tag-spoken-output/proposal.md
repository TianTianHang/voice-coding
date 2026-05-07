## Why

当前自动 TTS 直接播报 agent 最终 `result` 内容，容易把 Markdown、代码片段、diff、日志、文件路径或长说明一起合成出来，导致语音输出不纯净且可能过度播报。

需要让 agent 显式声明“适合朗读的话”，并让后端严格只播报这段文本；同时 UI 不暴露控制标签，保持结果展示干净。

## What Changes

- 自动 TTS 改为严格读取 agent 最终 `result` 中唯一一对 `<tts>...</tts>` 标签内的文本。
- 没有 `<tts>`、标签不完整、内容为空、嵌套或出现多对 `<tts>` 时，自动 TTS SHALL 跳过播报。
- UI 展示 agent result 时 SHALL 隐藏所有 `<tts>...</tts>` 块，避免协议标签出现在用户可见输出中。
- Agent 提示词/会话约束 SHALL 要求：只有需要短口语播报时才输出恰好一对 `<tts>` 标签，标签内只能放自然口语文本。
- 自动 TTS 的去重和最近结果状态 SHALL 基于提取出的 TTS 文本，而不是完整 result 展示文本。
- **BREAKING**：自动 TTS 不再默认播报完整 `resultEvent.content`；未显式包含有效 `<tts>` 块的结果不会发声。

## Capabilities

### New Capabilities
- `backend-auto-tts`: 定义后端自动播报的严格 `<tts>` 标签提取、跳过、去重和状态行为。

### Modified Capabilities
- `assistant-console-ui`: agent result 展示需要隐藏 `<tts>...</tts>` 协议块，并继续正常展示其余结果内容。

## Impact

- `src-tauri/src/acp/client.rs`：可能需要在 agent result 归一化或 tracker 层区分 UI 展示文本与原始完整文本。
- `src-tauri/src/acp/session.rs`：自动 TTS 触发点继续在 turn stop 后执行，但输入文本应来自严格提取的 TTS 文本。
- `src-tauri/src/tts.rs`：增加 `<tts>` 提取/校验、严格跳过状态、基于 TTS 文本的去重，以及相关单元测试。
- `src/components/AgentEventStream.tsx` / `src/hooks/useAgentEvents.ts`：展示 result 时隐藏 `<tts>` 块，且保持流式增量合并行为。
- Agent profile 或系统提示词配置：加入 `<tts>` 单块输出契约。
- 验证期望：补充 Rust 单元测试覆盖有效标签、缺失标签、多标签、不完整标签、空标签和去重；补充前端测试覆盖 UI 隐藏标签块。
