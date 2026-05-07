## ADDED Requirements

### Requirement: 隐藏 TTS 控制标签
助手控制台 SHALL 在展示 agent result 时隐藏完整 `<tts>...</tts>` 控制标签块。

#### Scenario: 单个 TTS 标签不显示
- **WHEN** 前端展示包含一对完整 `<tts>...</tts>` 标签的 agent result
- **THEN** 控制台 SHALL 展示标签外的 result 文本
- **AND** 控制台 SHALL NOT 展示 `<tts>`、`</tts>` 或标签内文本

#### Scenario: 多个 TTS 标签全部隐藏
- **WHEN** 前端展示包含多对完整 `<tts>...</tts>` 标签的 agent result
- **THEN** 控制台 SHALL 隐藏所有完整 `<tts>...</tts>` 标签块
- **AND** 控制台 SHALL 展示标签块之外的 result 文本

#### Scenario: 隐藏标签后保留流式合并语义
- **WHEN** 前端连续收到相同 `messageId` 且类型为 `result` 的输出事件
- **AND** 合并后的文本包含完整 `<tts>...</tts>` 标签块
- **THEN** 控制台 SHALL 在同一个结果块中展示隐藏标签后的文本
- **AND** 控制台 SHALL NOT 因隐藏标签而创建新的可见输出块

#### Scenario: 不完整标签不破坏普通展示
- **WHEN** 前端展示的 agent result 包含未闭合 `<tts>` 或孤立 `</tts>`
- **THEN** 控制台 SHALL 保持标签外可读文本可见
- **AND** 控制台 SHALL NOT 崩溃或空白渲染整个 result
