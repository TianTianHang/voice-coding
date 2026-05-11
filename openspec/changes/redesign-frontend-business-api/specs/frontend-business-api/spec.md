## ADDED Requirements

### Requirement: 提供统一应用状态快照
系统 SHALL 提供面向前端的统一应用状态快照，覆盖模型准备、语音输入、Agent 连接、语音输出和用户偏好。

#### Scenario: 前端启动读取完整状态
- **WHEN** 前端调用 `get_app_status`
- **THEN** 系统 SHALL 返回应用 readiness 状态
- **AND** 返回 ASR 模型状态、TTS 模型状态、语音会话状态、Agent 状态、语音输出状态和偏好设置
- **AND** 返回值 SHALL 足以让前端在没有历史事件的情况下恢复主界面

#### Scenario: 统一准备后端能力
- **WHEN** 前端调用 `prepare_app`
- **THEN** 系统 SHALL 准备主流程所需的 ASR 与 TTS 能力
- **AND** 系统 SHALL 发布应用状态变更事件
- **AND** 如果部分能力失败，系统 SHALL 返回 `degraded` 或 `failed` 状态并包含可读错误

### Requirement: 暴露业务化命令集合
系统 SHALL 为新前端暴露按业务域命名的 Tauri 命令，而不是要求前端直接编排内部 ASR、VAD、TTS、ACP 命令。

#### Scenario: 新前端使用语音会话命令
- **WHEN** 前端需要开始或停止麦克风语音输入
- **THEN** 前端 SHALL 能调用 `start_voice_session` 和 `stop_voice_session`
- **AND** 前端 SHALL NOT 需要直接调用 `start_listening` 或 `stop_listening` 作为主流程入口

#### Scenario: 新前端使用朗读命令
- **WHEN** 前端需要朗读文本或 Agent 结果
- **THEN** 前端 SHALL 能调用 `speak_text` 或 `speak_agent_result`
- **AND** 前端 SHALL NOT 需要先调用 `synthesize_tts` 再调用 `play_tts`

### Requirement: 发布业务状态事件
系统 SHALL 发布按业务域命名的状态事件，供前端增量更新 UI。

#### Scenario: 状态变化事件
- **WHEN** 应用、语音会话、Agent 或语音输出状态发生变化
- **THEN** 系统 SHALL 发布对应的 `app-status-changed`、`voice-session-changed`、`agent-status-changed` 或 `speech-output-changed` 事件
- **AND** 事件 payload SHALL 包含对应业务域的最新状态

#### Scenario: 运行时错误事件
- **WHEN** 任一业务域发生需要用户感知的错误
- **THEN** 系统 SHALL 发布 `runtime-error` 事件
- **AND** 事件 SHALL 包含错误 scope、可读 message 和 recoverable 标记

### Requirement: 保留旧命令作为兼容调试入口
系统 SHALL 在业务 API 可用后保留现有底层命令作为调试或迁移兼容入口，除非另有移除提案。

#### Scenario: 调试工具继续调用底层命令
- **WHEN** 调试窗口需要验证 ASR 文件转写或 TTS 合成播放
- **THEN** 系统 SHALL 允许继续调用 `transcribe_audio_data`、`synthesize_tts` 和 `play_tts`
- **AND** 新前端主流程 SHALL 优先使用业务 API
