## ADDED Requirements

### Requirement: 支持业务化 Agent 消息发送
系统 SHALL 提供 `send_agent_message` 命令，将手动输入、语音转写、编辑后转写和重试统一为 Agent 用户消息。

#### Scenario: 发送带来源的消息
- **WHEN** 前端调用 `send_agent_message`
- **AND** 请求包含非空文本和 source
- **THEN** 系统 SHALL 将文本发送到当前活动 ACP session
- **AND** 系统 SHALL 创建 `turnId`
- **AND** 系统 SHALL 返回 Agent turn 状态

#### Scenario: 从 utterance 提交 Agent 消息
- **WHEN** 语音业务层提交有效 utterance
- **THEN** 系统 SHALL 通过同一消息发送路径创建 Agent turn
- **AND** Agent turn SHALL 记录来源为 `voice` 或 `editedTranscript`

### Requirement: 发布 Agent 回合状态
系统 SHALL 为每次用户消息维护 Agent turn 状态，并向前端发布状态变化。

#### Scenario: Agent 回合开始
- **WHEN** 用户消息成功发送给 ACP session
- **THEN** 系统 SHALL 发布 `agent-turn-changed` 事件
- **AND** 状态 SHALL 为 `running`
- **AND** 事件 SHALL 包含 `turnId`、source 和创建时间

#### Scenario: Agent 回合完成
- **WHEN** ACP session 返回 stop reason
- **THEN** 系统 SHALL 将当前 turn 状态更新为 `completed`
- **AND** 系统 SHALL 发布 `agent-turn-changed` 事件

#### Scenario: Agent 回合失败
- **WHEN** 发送消息、读取更新或 ACP connection 失败
- **THEN** 系统 SHALL 将当前 turn 状态更新为 `failed`
- **AND** 系统 SHALL 发布 `runtime-error` 事件，scope 为 `agent`

### Requirement: 支持取消 Agent 回合
系统 SHALL 提供 `cancel_agent_turn` 命令，用于前端请求停止或忽略当前 Agent 回合。

#### Scenario: 取消活动回合
- **WHEN** 前端调用 `cancel_agent_turn` 并指定当前活动 `turnId`
- **THEN** 系统 SHALL 尝试停止当前 Agent 回合
- **AND** 如果底层 SDK 无法原生取消，系统 SHALL 至少将该 turn 标记为 `cancelled` 并忽略后续结果事件

#### Scenario: 取消未知回合
- **WHEN** 前端调用 `cancel_agent_turn` 并指定未知或已结束 `turnId`
- **THEN** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 影响当前活动 session
