## Why

当前语音链路已经完成了“ASR 转写 → Agent 处理 → 前端展示”，但没有把 Agent 的最终回复自动送入 TTS。结果是语音助手的输出仍然依赖手动测试按钮，而不是作为语音交互链路的一部分自动播放，用户会感觉“说完了，但没有回声”。

## What Changes

- 增加一个由后端托管的自动播报能力，在 Agent 发出最终 `result` 事件后自动合成并播放回复。
- 自动播报仅使用 `resultEvent.content` 作为播报文本，不对 `thinking`、`tool`、`status` 等事件发声。
- 保留现有非流式 TTS 合成/播放能力，自动播报复用现有 TTS runtime，而不是引入流式合成。
- 暴露少量前端可介入的命令，用于启停自动播报、停止当前播报、查询播报状态，必要时重新播报最近一次结果。
- 保留现有手动 TTS 测试入口作为开发辅助，但不作为主流程。

## Capabilities

### New Capabilities
- `backend-auto-tts`: Agent 最终回复的自动合成与播放，以及前端介入控制。

### Modified Capabilities
- `assistant-console-ui`: 增加自动播报状态与控制入口的展示。
- `acp-client-runtime`: 需要在 Agent 结果事件和 TTS 托管逻辑之间建立新的协调关系。

## Impact

- `src-tauri/src/acp/session.rs`：需要在最终结果事件路径上接入自动播报触发点，并为前端控制保留事件/状态接口。
- `src-tauri/src/tts.rs`：需要增加自动播报托管状态、控制命令和自动播放编排。
- `src-tauri/src/lib.rs`：需要注册新的 Tauri 命令和状态。
- `src/components/AssistantConsole.tsx`：需要展示自动播报开关、停止/重播等控制入口与状态。
- `src/hooks/useAgentEvents.ts`：如需前端介入更细粒度状态，可能要消费新增的后端事件或状态。
- 测试：需要补充 Rust 和前端测试，验证只对 `result` 发声、停止后恢复、重复结果不重复播报。
