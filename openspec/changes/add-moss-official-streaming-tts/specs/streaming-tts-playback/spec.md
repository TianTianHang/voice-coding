## MODIFIED Requirements

### Requirement: 系统 SHALL 提供真正的 TTS 流式合成 session 抽象
系统 SHALL 在 TTS core 层提供可由具体引擎实现的流式合成 session 抽象，使调用方可以推入文本并异步消费合成事件；MOSS ONNX TTS 在实现该抽象时 SHALL 通过官方 frame callback 与 codec streaming decode 模式产出真实音频 chunk。

#### Scenario: 创建流式合成 session
- **WHEN** 调用方请求支持流式合成的 TTS 引擎启动流式合成
- **THEN** 引擎 SHALL 返回一个流式 session
- **AND** session SHALL 绑定本次合成配置
- **AND** 对于 MOSS ONNX TTS，session SHALL 初始化或校验 codec streaming decode 所需 metadata 和 ONNX I/O

#### Scenario: 推入完整文本或增量文本
- **WHEN** 调用方向流式 session 推入文本 chunk
- **THEN** session SHALL 接收新增文本内容
- **AND** chunk SHALL 能表达本次输入是否为最终输入
- **AND** chunk SHALL 能表达是否请求立即 flush 当前缓冲文本
- **AND** MOSS session SHALL 在文本达到自然边界、配置阈值、flush 或 final 条件时提交可朗读片段进入推理

#### Scenario: 拉取可即时消费事件
- **WHEN** session 已产生合成进度、文本边界、音频 chunk 或结束结果
- **THEN** 调用方 SHALL 能通过 `next_event` 拉取下一个事件
- **AND** 无可用事件时 `next_event` SHALL 返回空结果而不是阻塞到最终合成完成
- **AND** 空结果 SHALL 表示当前暂无可消费事件，不表示 session 已结束
- **AND** 调用方 SHOULD 通过输入推进、定时轮询或等待式封装避免忙等

#### Scenario: MOSS 音频 chunk 可在最终结果前到达
- **WHEN** MOSS 流式 session 正在处理已提交文本片段
- **THEN** session SHALL 能在最终 `TtsResult` 完成前产生 `AudioChunk` 事件
- **AND** 每个 `AudioChunk` SHALL 可被调用方立即播放或缓存
- **AND** 事件 sequence MUST 在同一 session 内单调递增

#### Scenario: 结束流式合成
- **WHEN** 调用方调用 `finish`
- **THEN** session SHALL 完成剩余文本处理
- **AND** session SHALL 返回最终完整 `TtsResult`
- **AND** 最终音频 SHALL 仍满足 48kHz 立体声播放契约
- **AND** 如果事件流也产生 `End` 事件，`End` 携带的 `TtsResult` SHALL 与 `finish` 返回的最终结果一致

#### Scenario: 取消流式合成
- **WHEN** 调用方调用 `cancel`
- **THEN** session SHALL 终止当前流式合成
- **AND** session SHALL 释放或丢弃未消费的内部缓冲和事件
- **AND** MOSS session SHALL 在 ONNX 调用边界、frame loop 边界或 codec batch 边界停止继续处理后续数据
- **AND** 取消完成后 MUST NOT 再发送新的 `AudioChunk` 或 `End` 事件
