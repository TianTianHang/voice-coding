## 设计说明

### Session 模型

`tts-core` 新增 `StreamingTts` 和 `StreamingTtsSession`，与 STT 的 `start_stream -> push -> next_event -> finish/cancel` 模型保持一致。引擎可以选择不实现 `StreamingTts`；未实现时现有批量合成行为不受影响。

### 文本输入

`StreamingTextChunk` 表达增量文本输入：

- `text` 是本次新增文本。
- `is_final` 表示调用方确认后续不会再追加文本。
- `flush` 表示希望 session 尽快合成当前缓冲文本，即使尚未遇到自然语言边界。

完整文本合成可以用一次 `push_text(text, is_final=true, flush=true)` 表达。

### 事件输出

`TtsSynthesisEvent` 扩展为可表达真正流式生命周期：

- `Started`：session 已开始接受/处理输入。
- `Progress`：阶段和 chunk 进度。
- `TextBoundary`：引擎确认某段文本进入合成或已完成。
- `AudioChunk`：可立即播放或缓存的 PCM chunk。
- `End`：最终完整 `TtsResult` 已形成。

### 与现有播放契约的关系

该接口只定义 core 级别的流式合成能力，不强制上层 runtime 立刻边合成边播放。现有 Tauri runtime 可以继续等待完整 `TtsResult` 后进入 `ready` 和 `playing` 状态。

### MOSS 后续实现方向

MOSS 外部流式实现可复用内部 `decode_step_buffered` 的每批 PCM 输出，将 chunk 按顺序映射为 `TtsAudioChunk`。V1 只要求 core 接口稳定，MOSS 可继续依赖默认 unsupported 行为。
