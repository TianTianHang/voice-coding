## ADDED Requirements

### Requirement: 提供业务化语音输出命令
系统 SHALL 提供面向前端的语音输出业务命令，用于朗读文本、朗读 Agent 结果、停止朗读和查询朗读状态。

#### Scenario: 朗读任意文本
- **WHEN** 前端调用 `speak_text` 并传入非空文本
- **THEN** 系统 SHALL 创建 speech id
- **AND** 系统 SHALL 执行 TTS 合成和播放
- **AND** 系统 SHALL 发布 `speech-output-changed` 事件

#### Scenario: 朗读 Agent 结果
- **WHEN** 前端或 Agent 自动口播流程调用 `speak_agent_result`
- **THEN** 系统 SHALL 按既有 `<tts>...</tts>` 契约提取口播文本
- **AND** 系统 SHALL 使用提取后的文本进行合成和播放

#### Scenario: 停止朗读
- **WHEN** 前端调用 `stop_speech`
- **THEN** 系统 SHALL 停止当前播放
- **AND** 系统 SHALL 将语音输出状态更新为 `idle` 或 `stopping`
- **AND** 如果语音输入因本次播放暂停，系统 SHALL 恢复原语音输入会话

### Requirement: 管理语音输出状态
系统 SHALL 维护 `SpeechOutputStatus`，包含当前状态、speech id、来源、自动朗读开关和错误。

#### Scenario: 合成播放状态变化
- **WHEN** 系统开始合成、开始播放、播放结束或播放失败
- **THEN** 系统 SHALL 更新 `SpeechOutputStatus`
- **AND** 系统 SHALL 发布 `speech-output-changed` 事件

#### Scenario: 查询当前语音输出状态
- **WHEN** 前端调用 `get_speech_status`
- **THEN** 系统 SHALL 返回当前 `SpeechOutputStatus`
- **AND** 返回值 SHALL 足以让前端恢复朗读按钮、停止按钮和自动朗读开关状态

### Requirement: 后端负责播放期间语音输入编排
系统 SHALL 在语音输出播放期间负责暂停和恢复语音输入，前端不需要手动调用语音输入暂停/恢复命令。

#### Scenario: 播放期间暂停录音
- **WHEN** 语音输出开始播放
- **AND** 存在活动语音会话
- **THEN** 系统 SHALL 暂停语音输入并记录被暂停的 session
- **AND** 系统 SHALL NOT 让 TTS 输出被麦克风重新捕获为用户语音

#### Scenario: 播放结束恢复录音
- **WHEN** 语音输出播放结束、失败或被停止
- **AND** 存在由本次播放暂停的语音会话
- **THEN** 系统 SHALL 恢复同一个语音会话
- **AND** 系统 SHALL NOT 调用会分配新 session 的开始监听路径
