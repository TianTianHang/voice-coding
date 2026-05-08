## MODIFIED Requirements

### Requirement: 自动播报最终回复
系统 MUST 在 Agent 产生最终 `result` 事件后，自动将 `resultEvent.content` 准备为适合朗读的文本，并对准备后的文本进行语音合成和播放。

#### Scenario: 仅对 result 发声
- **WHEN** Agent 发出 `result` 事件
- **THEN** 系统必须将 `resultEvent.content` 作为自动播报文本的来源
- **AND** 系统必须先生成适合 TTS 的 speakable text
- **AND** 系统必须自动合成并播放该 speakable text
- **AND** 系统不得对 `thinking`、`tool`、`status` 等非 result 事件发声

#### Scenario: 保持 UI 展示文本不变
- **WHEN** 自动播报对 Agent result 执行 Markdown、代码、路径或符号清理
- **THEN** 前端 Agent 事件流 SHALL 继续展示原始 `resultEvent.content`
- **AND** 清理后的 speakable text SHALL NOT replace the displayed Agent result

#### Scenario: 不朗读不可语音化的大块技术内容
- **WHEN** Agent result 包含 fenced code block、diff、长 JSON、终端日志或命令输出
- **THEN** 自动播报 SHALL skip or summarize that block instead of reading it verbatim
- **AND** 自动播报 SHALL continue speaking remaining natural-language explanation when present
