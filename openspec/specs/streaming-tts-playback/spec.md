# Spec: Streaming TTS Playback

## Requirements

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

### Requirement: 系统 SHALL 在合成完成后再播放

系统 SHALL 先完成语音合成，再将完整音频交给播放层输出到扬声器。

#### Scenario: 先合成后播放

- **WHEN** 用户触发 TTS 播放
- **THEN** 系统 SHALL 先生成完整音频数据
- **AND** 系统 SHALL 在合成成功后开始播放
- **AND** 系统 MUST NOT 在合成未完成时直接开始播放

#### Scenario: 合成失败不进入播放

- **WHEN** TTS 合成失败
- **THEN** 系统 SHALL 返回错误
- **AND** 系统 MUST NOT 进入播放状态
- **AND** 系统 SHALL 保持录音恢复策略可执行

### Requirement: 系统 SHALL 使用 48kHz 立体声播放格式

系统 SHALL 将 TTS 最终播放音频规范化为 48kHz、立体声格式。

#### Scenario: 输出格式符合播放要求

- **WHEN** TTS 合成结果准备进入播放层
- **THEN** 系统 SHALL 提供 48kHz 采样率音频
- **AND** 系统 SHALL 提供 2 声道立体声音频
- **AND** 系统 SHALL 使用可被输出设备消费的 PCM 形式

#### Scenario: 需要适配设备能力

- **WHEN** 目标播放设备不支持原始输出格式
- **THEN** 系统 SHALL 在播放边界执行必要的格式适配
- **AND** 系统 MUST 保持对上层暴露的语义为“最终播放仍是 48kHz 立体声”

### Requirement: 系统 SHALL 在播放期间暂停录音

系统 SHALL 在播放 TTS 音频期间暂停麦克风录音与 VAD 识别，防止回灌和误识别。

#### Scenario: 播放开始时暂停录音

- **WHEN** TTS 播放开始
- **THEN** 系统 SHALL 停止当前录音会话或将其置于暂停状态
- **AND** 系统 SHALL 让 VAD 进入空闲语义
- **AND** 系统 MUST NOT 继续处理麦克风输入

#### Scenario: 播放结束后恢复录音

- **WHEN** TTS 播放完成或被正常结束
- **THEN** 系统 SHALL 恢复录音会话
- **AND** 系统 SHALL 恢复 VAD 监听
- **AND** 系统 SHALL 恢复此前可用的语音交互流程

#### Scenario: 播放中断时也要恢复

- **WHEN** TTS 播放被取消或失败中断
- **THEN** 系统 SHALL 释放播放资源
- **AND** 系统 SHALL 恢复录音和 VAD 状态

### Requirement: 系统 SHALL 管理 TTS 播放会话状态

系统 SHALL 为 TTS 播放提供明确的运行时状态，以便前端和后端同步。

#### Scenario: 状态流转

- **WHEN** TTS 任务尚未开始
- **THEN** 状态 SHALL 为 idle
- **WHEN** 系统正在合成语音
- **THEN** 状态 SHALL 为 synthesizing
- **WHEN** 合成完成且等待播放
- **THEN** 状态 SHALL 为 ready
- **WHEN** 系统正在输出音频
- **THEN** 状态 SHALL 为 playing

#### Scenario: 状态查询

- **WHEN** 调用 TTS 状态查询命令
- **THEN** 系统 SHALL 返回当前状态及必要的运行信息
- **AND** 返回值 MUST 反映播放会话真实状态

### Requirement: 系统 SHALL 提供可测试的错误处理

系统 SHALL 为 TTS 合成和播放定义明确的错误信息，便于定位失败原因。

#### Scenario: 合成错误

- **WHEN** 模型加载失败、推理失败或文本不合法
- **THEN** 系统 SHALL 返回可读错误
- **AND** 错误信息 SHOULD 包含失败阶段

#### Scenario: 播放错误

- **WHEN** 音频输出设备不可用或播放流创建失败
- **THEN** 系统 SHALL 返回播放错误
- **AND** 系统 SHALL 恢复录音相关状态

### Requirement: 内部流式解码保持完整音频播放契约
系统 SHALL 允许 TTS 引擎在内部使用流式音频解码优化合成，但对播放层暴露的契约仍为合成完成后提供完整音频。

#### Scenario: 内部 chunk 用于缓存
- **WHEN** TTS 引擎在合成过程中产生内部音频 chunk
- **THEN** 系统 SHALL 将 chunk 缓存在 TTS 合成结果中
- **AND** 系统 MUST NOT 因 chunk 可用而提前进入播放状态

#### Scenario: 完整音频形成后进入 Ready
- **WHEN** 所有内部 chunk 已拼接为完整 PCM
- **THEN** 系统 SHALL 验证完整音频满足播放格式
- **AND** 系统 SHALL 允许 TTS runtime 从 `synthesizing` 转为 `ready`

#### Scenario: 内部流式失败不破坏恢复策略
- **WHEN** TTS 引擎内部流式解码失败
- **THEN** 系统 SHALL 返回合成错误或使用已定义 fallback
- **AND** 系统 MUST NOT 进入播放状态
- **AND** 系统 SHALL 保持录音恢复策略可执行
