## ADDED Requirements

### Requirement: 覆盖所有 ACP session update 类型
系统 SHALL 对官方 Rust SDK 当前暴露的所有 `SessionUpdate` 类型进行显式归一化，并为未来未知类型提供可读兜底。

#### Scenario: 归一化所有已知 session update
- **WHEN** 后端收到 `user_message_chunk`、`agent_message_chunk`、`agent_thought_chunk`、`tool_call`、`tool_call_update`、`plan`、`available_commands_update`、`current_mode_update`、`config_option_update` 或 `session_info_update`
- **THEN** 系统 SHALL 将其转换为稳定的内部输出事件或会话状态更新
- **AND** 系统 SHALL NOT 静默丢弃该 update

#### Scenario: 归一化未知 session update
- **WHEN** 后端收到 SDK 未来新增或未显式处理的 session update
- **THEN** 系统 SHALL 发布可读的 `status` 或调试事件
- **AND** 事件内容 SHALL 包含原始 update 的可读摘要

### Requirement: 保留流式消息身份
系统 SHALL 使用 ACP `messageId` 表达流式消息 chunk 的归属关系。

#### Scenario: agent 文本 chunk 携带 messageId
- **WHEN** 后端收到携带 `messageId` 的 `agent_message_chunk`
- **THEN** 系统 SHALL 发布 `result` 类型输出事件
- **AND** 事件 SHALL 包含同一个 `messageId`
- **AND** 事件内容 SHALL 保留该 chunk 的文本增量

#### Scenario: agent thought chunk 携带 messageId
- **WHEN** 后端收到携带 `messageId` 的 `agent_thought_chunk`
- **THEN** 系统 SHALL 发布 `thinking` 类型输出事件
- **AND** 事件 SHALL 包含同一个 `messageId`
- **AND** 事件内容 SHALL 保留该 chunk 的文本增量

#### Scenario: messageId 缺失
- **WHEN** 后端收到未携带 `messageId` 的文本 chunk
- **THEN** 系统 SHALL 保留该 chunk 为独立可读事件
- **AND** 系统 SHALL NOT 基于相邻顺序猜测消息归属

#### Scenario: 文本 chunk 追加语义透传
- **WHEN** 后端收到 `agent_message_chunk` 或 `agent_thought_chunk`
- **THEN** 系统 SHALL 将输出事件标记为 `operation=append`
- **AND** 事件 SHALL 保留该 chunk 的原始文本内容以支持前端增量合并

### Requirement: 归一化工具调用生命周期
系统 SHALL 使用 `toolCallId` 表达工具调用和工具更新的身份，并保留工具状态、类型、内容和位置。

#### Scenario: 创建工具调用事件
- **WHEN** 后端收到 `tool_call`
- **THEN** 系统 SHALL 发布 `tool` 类型输出事件
- **AND** 事件 SHALL 包含 `toolCallId`、title、kind、status、content、locations、raw input 和 raw output 中可用的信息

#### Scenario: 更新工具调用事件
- **WHEN** 后端收到 `tool_call_update`
- **THEN** 系统 SHALL 发布针对同一 `toolCallId` 的工具更新事件
- **AND** 更新事件 SHALL 只改变协议 payload 中声明变化的字段

#### Scenario: 工具更新先于工具创建
- **WHEN** 后端收到没有已知 `tool_call` 的 `tool_call_update`
- **THEN** 系统 SHALL 仍发布可读工具事件
- **AND** 事件 SHALL 包含该 update 中已有的 `toolCallId` 和字段

### Requirement: 识别工具内容中的 diff 和 terminal
系统 SHALL 从工具调用内容中识别 diff、terminal 引用和标准 content block，并保留其类型语义。

#### Scenario: 工具内容包含 diff
- **WHEN** `tool_call` 或 `tool_call_update` 的 content 包含 `Diff`
- **THEN** 系统 SHALL 将该内容标记为 `diff`
- **AND** 系统 SHALL 保留 path、old text 和 new text 中可用的信息

#### Scenario: 工具内容包含 terminal 引用
- **WHEN** `tool_call` 或 `tool_call_update` 的 content 包含 `Terminal`
- **THEN** 系统 SHALL 将该内容标记为 terminal 引用
- **AND** 系统 SHALL 保留 terminal id 的可读表示

#### Scenario: 工具内容包含非文本 content block
- **WHEN** 工具内容或消息 chunk 包含 image、audio、resource link 或 embedded resource
- **THEN** 系统 SHALL 生成可读摘要或结构化 payload
- **AND** 系统 SHALL NOT 丢弃该内容

### Requirement: 归一化计划和会话状态快照
系统 SHALL 将 plan 和 session-level update 表达为可替换的当前状态，而不是普通追加日志。

#### Scenario: 收到 plan update
- **WHEN** 后端收到 `plan`
- **THEN** 系统 SHALL 发布当前计划快照
- **AND** 快照 SHALL 包含所有 plan entries 的 content、priority 和 status

#### Scenario: 收到 available commands update
- **WHEN** 后端收到 `available_commands_update`
- **THEN** 系统 SHALL 更新当前会话可用命令列表
- **AND** 每个命令 SHALL 保留 name、description 和 input hint 中可用的信息

#### Scenario: 收到 mode/config/session info update
- **WHEN** 后端收到 `current_mode_update`、`config_option_update` 或 `session_info_update`
- **THEN** 系统 SHALL 更新对应的会话状态字段
- **AND** 系统 SHALL 保留当前 mode id、配置项快照、session title 和 updated time 中可用的信息
