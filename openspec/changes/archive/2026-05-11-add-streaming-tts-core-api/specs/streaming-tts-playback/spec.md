## ADDED Requirements

### Requirement: 系统 SHALL 提供真正的 TTS 流式合成 session 抽象
系统 SHALL 在 TTS core 层提供可由具体引擎实现的流式合成 session 抽象，使调用方可以推入文本并异步消费合成事件。

#### Scenario: 创建流式合成 session
- **WHEN** 调用方请求支持流式合成的 TTS 引擎启动流式合成
- **THEN** 引擎 SHALL 返回一个流式 session
- **AND** session SHALL 绑定本次合成配置

#### Scenario: 推入完整文本或增量文本
- **WHEN** 调用方向流式 session 推入文本 chunk
- **THEN** session SHALL 接收新增文本内容
- **AND** chunk SHALL 能表达本次输入是否为最终输入
- **AND** chunk SHALL 能表达是否请求立即 flush 当前缓冲文本

#### Scenario: 拉取可即时消费事件
- **WHEN** session 已产生合成进度、文本边界、音频 chunk 或结束结果
- **THEN** 调用方 SHALL 能通过 `next_event` 拉取下一个事件
- **AND** 无可用事件时 `next_event` SHALL 返回空结果而不是阻塞到最终合成完成
- **AND** 空结果 SHALL 表示当前暂无可消费事件，不表示 session 已结束
- **AND** 调用方 SHOULD 通过输入推进、定时轮询或等待式封装避免忙等

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

### Requirement: 对外流式事件不强制改变默认播放策略
系统 SHALL 允许上层 runtime 继续选择完整合成后播放，即使 core 层已经提供对外流式事件抽象。

#### Scenario: 默认 runtime 仍可等待完整结果
- **WHEN** 上层 TTS runtime 未接入边合成边播放
- **THEN** 系统 SHALL 继续使用完整 `TtsResult` 进入 ready 状态
- **AND** 系统 MUST NOT 因 core 层存在 `AudioChunk` 事件而强制提前播放

## MODIFIED Requirements

### Requirement: 系统 SHALL 提供 TTS 合成抽象
系统 SHALL 提供一个 TTS 引擎抽象层，用于统一文本到语音的批量合成接口，并允许支持流式能力的引擎额外实现真正的流式 session 接口。

#### Scenario: 合成接口可被不同实现复用
- **WHEN** 新的 TTS 实现接入后端
- **THEN** 它 MUST 实现统一的 TTS 合成抽象
- **AND** 它 SHALL 接收文本输入与合成配置
- **AND** 它 SHALL 能返回可播放的音频结果

#### Scenario: 合成过程支持内部或外部流式产出
- **WHEN** TTS 引擎在合成长文本
- **THEN** 它 MAY 在内部以流式方式产出中间音频块或 token 进度
- **AND** 支持对外流式的引擎 SHALL 能通过 stream session 暴露可即时消费事件
- **AND** 不支持对外流式的引擎 SHALL 继续返回完整音频结果
