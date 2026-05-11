## ADDED Requirements

### Requirement: 管理业务语音会话状态
系统 SHALL 在现有 VAD 状态机之上维护业务语音会话状态，包含 session 身份、启动/停止、暂停/恢复、转写中和失败状态。

#### Scenario: 开始语音会话
- **WHEN** 前端调用 `start_voice_session`
- **THEN** 系统 SHALL 分配新的 `sessionId`
- **AND** 系统 SHALL 启动底层 VAD 监听
- **AND** 系统 SHALL 发布 `voice-session-changed` 事件，状态为 `starting` 或 `listening`

#### Scenario: TTS 播放暂停语音会话
- **WHEN** 语音输出开始播放并且当前存在活动语音会话
- **THEN** 系统 SHALL 暂停底层 VAD 录音
- **AND** 系统 SHALL 保持当前 `sessionId`
- **AND** 系统 SHALL 发布 `voice-session-changed` 事件，状态为 `paused` 且 `pauseReason` 为 `ttsPlayback`

#### Scenario: TTS 播放结束恢复语音会话
- **WHEN** 语音输出播放结束或被停止
- **AND** 语音会话因 `ttsPlayback` 暂停
- **THEN** 系统 SHALL 恢复同一个底层 VAD 录音会话
- **AND** 系统 SHALL NOT 通过重新创建新 session 来恢复录音

### Requirement: 支持语音输入模式
系统 SHALL 支持配置每段转写完成后的处理模式：仅转写、自动发送 Agent、确认后发送。

#### Scenario: 仅转写模式
- **WHEN** 语音输入模式为 `dictationOnly`
- **AND** ASR 返回非空转写文本
- **THEN** 系统 SHALL 创建 utterance 记录并发布转写事件
- **AND** 系统 SHALL NOT 自动发送文本到 Agent

#### Scenario: 自动发送模式
- **WHEN** 语音输入模式为 `autoSendToAgent`
- **AND** ASR 返回非空转写文本
- **AND** 存在活动 Agent session
- **THEN** 系统 SHALL 创建 utterance 记录
- **AND** 系统 SHALL 自动将转写文本提交为 Agent 消息

#### Scenario: 确认后发送模式
- **WHEN** 语音输入模式为 `confirmBeforeSend`
- **AND** ASR 返回非空转写文本
- **THEN** 系统 SHALL 创建待确认 utterance
- **AND** 系统 SHALL 等待前端调用提交、编辑后提交或丢弃命令

### Requirement: 管理 utterance 生命周期
系统 SHALL 为每段完成的语音转写分配 `utteranceId`，并允许前端提交、编辑后提交或丢弃。

#### Scenario: 提交已转写语音
- **WHEN** 前端调用 `submit_transcript_to_agent` 并传入有效 `utteranceId`
- **THEN** 系统 SHALL 将该 utterance 的转写文本发送为 Agent 消息
- **AND** 系统 SHALL 将 utterance 状态更新为 `submittedToAgent`

#### Scenario: 编辑后提交语音
- **WHEN** 前端调用 `edit_and_submit_transcript` 并传入有效 `utteranceId` 和新文本
- **THEN** 系统 SHALL 使用新文本发送 Agent 消息
- **AND** 系统 SHALL 保留原始转写文本用于调试或 UI 展示

#### Scenario: 丢弃语音片段
- **WHEN** 前端调用 `discard_transcript` 并传入有效 `utteranceId`
- **THEN** 系统 SHALL 将该 utterance 状态更新为 `discarded`
- **AND** 系统 SHALL NOT 将该文本发送给 Agent
