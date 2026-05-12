## ADDED Requirements

### Requirement: 主控制台使用业务状态作为主流程事实源
主助手控制台 SHALL 使用业务 API 的应用状态快照和业务状态事件作为主流程事实源，覆盖应用准备、语音会话、Agent 连接、Agent turn、语音输出和运行时错误。

#### Scenario: 启动后读取业务快照
- **WHEN** 主助手控制台挂载
- **THEN** 控制台 SHALL 通过业务 hook 读取 `get_app_status` 返回的完整业务状态
- **AND** 控制台 SHALL 使用该状态恢复应用准备、语音会话、Agent 连接、语音输出和偏好展示

#### Scenario: 业务事件更新主状态
- **WHEN** 控制台收到 `app-status-changed`、`voice-session-changed`、`agent-status-changed`、`agent-turn-changed`、`speech-output-changed` 或 `runtime-error`
- **THEN** 控制台 SHALL 根据事件 payload 更新对应 UI 状态
- **AND** 控制台 SHALL NOT 依赖旧 debug VAD、debug ASR 或 debug TTS hook 来覆盖主流程状态

### Requirement: 主控制台通过业务命令驱动用户动作
主助手控制台 SHALL 通过业务 API 命令封装执行准备、语音输入、转写处理、Agent turn 和语音输出动作。

#### Scenario: 准备应用能力
- **WHEN** 用户在主控制台触发模型或应用准备动作
- **THEN** 控制台 SHALL 调用业务 hook 的 `prepare` 动作
- **AND** 控制台 SHALL 使用返回的 `AppStatus` 更新主界面

#### Scenario: 控制语音会话
- **WHEN** 用户在主控制台开始、停止、暂停或恢复语音输入
- **THEN** 控制台 SHALL 调用业务 hook 的语音会话动作
- **AND** 控制台 SHALL NOT 直接调用 legacy VAD debug 命令作为主流程入口

#### Scenario: 处理转写草稿
- **WHEN** 用户提交、编辑提交或丢弃当前转写
- **THEN** 控制台 SHALL 调用业务 hook 的转写处理动作
- **AND** 控制台 SHALL 根据 `voice-utterance` 事件或返回状态更新输入区

#### Scenario: 控制 Agent turn
- **WHEN** 用户发送文本消息或取消当前 Agent turn
- **THEN** 控制台 SHALL 调用业务 hook 的 Agent 动作
- **AND** 控制台 SHALL 使用业务 Agent turn 状态展示运行、完成、失败或取消

#### Scenario: 控制语音输出
- **WHEN** 用户切换自动朗读、朗读文本、朗读 Agent 结果或停止朗读
- **THEN** 控制台 SHALL 调用业务 hook 的 speech 动作
- **AND** 控制台 SHALL 使用 `SpeechOutputStatus` 展示自动朗读开关、播放状态和错误

### Requirement: Agent 内容流保留专门事件适配
主助手控制台 SHALL 继续使用 Agent 内容流适配层渲染细粒度 Agent 输出，但连接和 turn 状态 SHALL 由业务 API 提供。

#### Scenario: 渲染细粒度 Agent 输出
- **WHEN** 控制台收到 thinking、tool、result、diff、confirm 或 plan 等 Agent 内容事件
- **THEN** 控制台 SHALL 继续将这些事件渲染到输出流
- **AND** 控制台 SHALL 保留现有确认按钮和内容合并语义

#### Scenario: 连接状态由业务状态提供
- **WHEN** 业务 Agent 状态与 Agent 内容流 hook 的连接状态同时存在
- **THEN** 控制台 SHALL 使用业务 Agent 状态展示主连接状态
- **AND** 控制台 SHALL 仅将内容流 hook 用于事件时间线和确认响应

### Requirement: 旧 debug hook 限定为调试和兼容用途
前端文档和主控制台实现 SHALL 明确旧 debug/compat hook 不再作为主助手控制台的主流程依赖。

#### Scenario: 文档标明 hook 边界
- **WHEN** 开发者查看 hooks 或 components 的本地 AGENTS 文档
- **THEN** 文档 SHALL 标明 `useBackendVAD`、`useAsrStatus` 和 debug TTS 状态属于调试或兼容入口
- **AND** 文档 SHALL 指引新的主产品流程使用 `useBusinessApi`

#### Scenario: 主控制台移除旧主流程依赖
- **WHEN** 开发者检查 `AssistantConsole`
- **THEN** 主控制台 SHALL NOT import `useBackendVAD` 或 `useAsrStatus` 作为主流程状态来源
- **AND** 主控制台 SHALL NOT 直接从 debug TTS 状态派生自动朗读主状态
