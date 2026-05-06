# Spec: Streaming TTS Playback

## ADDED Requirements

### Requirement: 系统 SHALL 提供 TTS 合成抽象

系统 SHALL 提供一个 TTS 引擎抽象层，用于统一文本到语音的合成接口。

#### Scenario: 合成接口可被不同实现复用

- **WHEN** 新的 TTS 实现接入后端
- **THEN** 它 MUST 实现统一的 TTS 合成抽象
- **AND** 它 SHALL 接收文本输入与合成配置
- **AND** 它 SHALL 能返回可播放的音频结果

#### Scenario: 合成过程支持流式内部产出

- **WHEN** TTS 引擎在合成长文本
- **THEN** 它 SHALL 允许内部以流式方式产出中间音频块或 token 进度
- **AND** 这些内部进度 SHALL 用于缓存与状态更新
- **AND** 最终播放前 MUST 形成完整音频结果

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
