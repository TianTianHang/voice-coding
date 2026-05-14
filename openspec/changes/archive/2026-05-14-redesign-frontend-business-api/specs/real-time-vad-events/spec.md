## ADDED Requirements

### Requirement: 发布语音会话事件
系统 SHALL 发布业务化语音会话事件，携带完整 session 身份和当前状态。

#### Scenario: 语音状态变更
- **WHEN** 语音会话进入 `starting`、`listening`、`recording`、`transcribing`、`paused`、`stopping`、`idle` 或 `failed`
- **THEN** 系统 SHALL 发布 `voice-session-changed` 事件
- **AND** 事件 payload SHALL 包含 `sessionId`、`state`、可选 `pauseReason` 和可选错误

#### Scenario: 过渡期保留旧 VAD 事件
- **WHEN** 底层 VAD 状态变化
- **THEN** 系统 MAY 继续发布旧 `vad-state` 事件用于兼容
- **AND** 新前端 SHALL 以 `voice-session-changed` 作为主状态源

### Requirement: 发布语音片段事件
系统 SHALL 为每段语音片段发布 `voice-utterance` 事件，描述从检测、转写到提交或丢弃的生命周期。

#### Scenario: 转写成功
- **WHEN** ASR 对某段语音返回非空转写文本
- **THEN** 系统 SHALL 发布 `voice-utterance` 事件，kind 为 `transcribed`
- **AND** 事件 payload SHALL 包含 `sessionId`、`utteranceId` 和 `transcript`

#### Scenario: 转写失败
- **WHEN** ASR 对某段语音转写失败
- **THEN** 系统 SHALL 发布 `voice-utterance` 事件，kind 为 `failed`
- **AND** 事件 payload SHALL 包含 `sessionId`、`utteranceId` 和可读错误
- **AND** 系统 SHALL 发布 `runtime-error` 事件，scope 为 `voice`

#### Scenario: 语音片段提交给 Agent
- **WHEN** utterance 被自动提交或由前端确认提交给 Agent
- **THEN** 系统 SHALL 发布 `voice-utterance` 事件，kind 为 `submittedToAgent`
- **AND** 事件 payload SHALL 包含关联的 `turnId`
