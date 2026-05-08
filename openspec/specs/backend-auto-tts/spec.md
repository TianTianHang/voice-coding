# backend-auto-tts

## 要求

### 要求 1：自动播报最终回复
系统 MUST 在 Agent 产生最终 `result` 事件后，自动将 `resultEvent.content` 准备为适合朗读的文本，并对准备后的文本进行语音合成和播放。

#### 场景：仅对 result 发声
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

### 要求 2：后端托管播报状态
系统必须在后端维护自动播报开关与当前播报状态，前端可以查询和控制。

#### 场景：前端可关闭自动播报
- **WHEN** 前端关闭自动播报
- **THEN** 后端必须停止对后续 result 的自动播报
- **AND** 当前播报可以被停止

#### 场景：前端可重播最近结果
- **WHEN** 前端请求重播最近一次 result
- **THEN** 系统必须重新使用最近一次 result 的内容进行合成与播放

### 要求 3：复用现有非流式 TTS
系统必须复用现有非流式 TTS 合成与播放路径，不引入流式合成作为本次变更的一部分。

#### 场景：非流式合成
- **WHEN** 系统执行自动播报
- **THEN** 系统必须先完成整段文本合成，再执行播放
- **AND** 系统无需逐段输出音频流

### 要求 4：避免重复播报
系统必须避免对同一条 result 重复播报。

#### 场景：重复 result 不重复播报
- **WHEN** 系统再次观察到同一条 result 事件或等价内容
- **THEN** 系统必须忽略该事件，不再次发声

### Requirement: 严格提取 agent 口播文本
系统 SHALL 只从 agent 最终 `result` 文本中唯一一对完整 `<tts>...</tts>` 标签内提取自动播报文本。

#### Scenario: 有效单个 TTS 标签触发播报
- **WHEN** agent 最终 `result` 包含恰好一对完整 `<tts>...</tts>` 标签
- **AND** 标签内文本 trim 后非空
- **THEN** 系统 SHALL 使用标签内 trim 后文本进行 TTS 合成和播放
- **AND** 系统 SHALL NOT 使用标签外的 result 文本进行播报

#### Scenario: 缺少 TTS 标签时跳过播报
- **WHEN** agent 最终 `result` 不包含 `<tts>...</tts>` 标签
- **THEN** 系统 SHALL 跳过自动播报
- **AND** 系统 SHALL NOT 回退播报完整 result 文本

#### Scenario: 多个 TTS 标签时跳过播报
- **WHEN** agent 最终 `result` 包含多于一对完整 `<tts>...</tts>` 标签
- **THEN** 系统 SHALL 跳过自动播报
- **AND** 系统 SHALL NOT 播报第一对或任意一对标签内容

#### Scenario: 标签不完整时跳过播报
- **WHEN** agent 最终 `result` 包含未闭合 `<tts>` 或没有起始标签的 `</tts>`
- **THEN** 系统 SHALL 跳过自动播报
- **AND** 系统 SHALL NOT 猜测或补全标签边界

#### Scenario: 空 TTS 标签时跳过播报
- **WHEN** agent 最终 `result` 包含恰好一对完整 `<tts>...</tts>` 标签
- **AND** 标签内文本 trim 后为空
- **THEN** 系统 SHALL 跳过自动播报

#### Scenario: 嵌套 TTS 标签时跳过播报
- **WHEN** agent 最终 `result` 的 `<tts>...</tts>` 标签内部再次出现 `<tts>` 或 `</tts>`
- **THEN** 系统 SHALL 跳过自动播报

### Requirement: 自动播报状态和去重基于提取后的口播文本
系统 SHALL 使用提取后的 TTS 文本管理自动播报去重、最近结果和重播行为。

#### Scenario: 有效口播文本更新最近结果
- **WHEN** agent 最终 `result` 包含有效单个 `<tts>...</tts>` 标签
- **THEN** 系统 SHALL 将标签内 trim 后文本保存为最近可播报结果
- **AND** 系统 SHALL 使用该文本参与 latest result key 计算

#### Scenario: 无效标签不更新已播报结果
- **WHEN** agent 最终 `result` 不包含有效单个 `<tts>...</tts>` 标签
- **THEN** 系统 SHALL NOT 将完整 result 保存为最近可播报结果
- **AND** 系统 SHALL NOT 更新 latest spoken result key

#### Scenario: 重复口播文本不重复播报
- **WHEN** agent 最终 `result` 提取出的 TTS 文本和已播报 key 匹配
- **THEN** 系统 SHALL 跳过重复播报
- **AND** 系统 SHALL 保持当前自动播报控制面可查询该跳过状态

### Requirement: Agent 提示词声明 TTS 标签契约
系统 SHALL 在 agent 会话提示或 profile 约束中声明 `<tts>` 单块口播协议。

#### Scenario: 需要口播时提示 agent 输出单个标签
- **WHEN** 系统构造 agent 会话提示词或配置 agent profile
- **THEN** 系统 SHALL 要求 agent 在需要短口语播报时输出恰好一对 `<tts>...</tts>` 标签
- **AND** 系统 SHALL 要求标签内只包含自然口语文本

#### Scenario: 不需要口播时提示 agent 省略标签
- **WHEN** 系统构造 agent 会话提示词或配置 agent profile
- **THEN** 系统 SHALL 要求 agent 在没有必要发声时省略 `<tts>` 标签
- **AND** 系统 SHALL 要求 agent 不输出多对 `<tts>` 标签
