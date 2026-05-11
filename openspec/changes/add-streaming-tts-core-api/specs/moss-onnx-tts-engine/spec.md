## ADDED Requirements

### Requirement: MOSS 外部流式合成 SHALL 映射 codec decode step chunk
MOSS ONNX TTS 引擎 SHALL 在实现 core 流式 TTS session 时，将内部 codec decode step 产生的 PCM chunk 按顺序映射为对外可消费的 TTS 流式音频事件。

#### Scenario: codec decode step chunk 映射为流式音频事件
- **WHEN** MOSS 引擎在流式 session 中使用 codec decode step 分批解码 PCM
- **THEN** 每个可用 PCM chunk SHALL 被包装为 `AudioChunk` 事件
- **AND** 事件 SHALL 保留递增 sequence
- **AND** 事件音频 SHALL 满足 48kHz 立体声 PCM 格式

#### Scenario: V1 未实现外部流式时保持明确错误
- **WHEN** MOSS 引擎尚未实现外部流式 TTS session
- **THEN** 调用流式能力 SHALL 返回明确的 unsupported 错误
- **AND** 现有批量 `synthesize` 行为 MUST 保持不变
