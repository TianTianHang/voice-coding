## ADDED Requirements

### Requirement: 后端维护 Agent timeline
系统 SHALL 在后端维护 Agent 内容流的 authoritative timeline，用于表达当前 ACP session 和 Agent turn 中的 thinking、message、tool、diff、plan、confirmation、session state、fallback 和 error。

#### Scenario: 首帧读取 timeline 快照
- **WHEN** 前端 Agent stream hook 挂载
- **THEN** 系统 SHALL 提供命令读取当前 `AgentTimelineSnapshot`
- **AND** 快照 SHALL 包含当前 session id、当前 turn id、最新 sequence、timeline items、plan、session state 和 pending confirmations

#### Scenario: ACP 事件归并到 timeline
- **WHEN** 后端收到 ACP SDK session notification
- **THEN** 系统 SHALL 将其归并到后端 Agent timeline
- **AND** 系统 SHALL NOT 要求主控制台前端理解 ACP operation 才能得到正确展示状态

### Requirement: Timeline 事件带有 session、turn 和 sequence 归属
系统 SHALL 为 Agent stream 输出维护稳定的 session、turn 和 sequence 归属，以避免跨 turn 污染和乱序展示。

#### Scenario: 当前 turn 事件归属
- **WHEN** 业务 Agent turn 正在运行并产生 ACP 输出
- **THEN** timeline item 或 patch SHALL 带有对应 turn id
- **AND** 同一 turn 内输出 SHALL 按后端 sequence 单调递增

#### Scenario: session 级事件归属
- **WHEN** ACP 输出描述连接、session info、available commands、current mode 或 config options
- **THEN** 系统 SHALL 将其归属为 session 级状态
- **AND** 系统 SHALL NOT 强行把 session 级事件归并到某个 message 或 tool item

#### Scenario: late event 不污染当前 turn
- **WHEN** 已取消、失败或完成的 turn 后续到达 late ACP 输出
- **THEN** 系统 SHALL 丢弃该 late 输出或标记为 ignored
- **AND** 系统 SHALL NOT 将其追加到当前活跃 turn 的 timeline

### Requirement: 后端归并流式文本和工具更新
系统 SHALL 在后端完成 message/thinking 文本增量合并和 tool create/update 归并。

#### Scenario: 合并同一 message 文本
- **WHEN** 后端连续收到相同 message id 的 result 或 thinking chunk
- **THEN** 系统 SHALL 更新同一个 timeline item 的文本
- **AND** 系统 SHALL 通过 patch 通知前端该 item 已更新

#### Scenario: 合并缺少 message id 的连续文本
- **WHEN** 后端连续收到缺少 message id 但属于同一 turn、同一文本类型的 append chunk
- **THEN** 系统 SHALL 将其合并到最近的兼容 timeline item
- **AND** 系统 SHALL NOT 为每个 chunk 创建独立可见 item

#### Scenario: 更新同一 tool item
- **WHEN** 后端收到相同 tool call id 的 tool update
- **THEN** 系统 SHALL 更新同一个 tool timeline item
- **AND** 工具状态、内容、locations、raw input 和 raw output SHALL 反映最新可用值

### Requirement: 后端发布 timeline snapshot 和 patch
系统 SHALL 通过 Tauri 命令和事件向前端发布 UI-ready Agent timeline 状态。

#### Scenario: 发布 reset patch
- **WHEN** 前端需要初始化或后端需要恢复完整状态
- **THEN** 系统 SHALL 发布或返回包含完整 snapshot 的 reset 语义
- **AND** 前端 SHALL 能用该 snapshot 重建完整 Agent stream UI

#### Scenario: 发布增量 patch
- **WHEN** 单个 timeline item、plan、session state 或 confirmation 状态变化
- **THEN** 系统 SHALL 发布对应 patch
- **AND** patch SHALL 足以让前端通过机械 upsert 或替换更新 UI

### Requirement: 确认状态以后端为准
系统 SHALL 以后端 permission response 结果更新 confirmation timeline item 的状态。

#### Scenario: 创建确认请求
- **WHEN** ACP SDK 请求用户权限
- **THEN** 系统 SHALL 创建 confirmation timeline item
- **AND** item SHALL 包含 confirmation id、turn id、可读请求内容和 pending 状态

#### Scenario: 用户接受或拒绝确认
- **WHEN** 前端提交 confirmation id 和接受或拒绝选择
- **THEN** 系统 SHALL 将选择提交给 ACP SDK permission response
- **AND** SDK 响应完成后系统 SHALL 更新 confirmation item 状态并发布 patch

#### Scenario: 确认响应失败
- **WHEN** confirmation id 不存在、已处理或 SDK 响应失败
- **THEN** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 在前端显示已完成确认状态

### Requirement: 保留 legacy Agent event 兼容入口
系统 SHALL 在迁移期间保留 legacy `agent-event` 内容流入口，直到主控制台完成迁移并有单独清理计划。

#### Scenario: 新旧事件并存
- **WHEN** 后端发布新的 Agent timeline patch
- **THEN** 系统 MAY 同时发布 legacy `agent-event`
- **AND** legacy 事件 SHALL NOT 成为主控制台的事实源

#### Scenario: Debug 或兼容视图继续工作
- **WHEN** 兼容 hook 或 debug 视图仍订阅 legacy `agent-event`
- **THEN** 系统 SHALL 保持现有事件字段尽量兼容
- **AND** 系统 SHALL NOT 因新增 timeline runtime 立即删除旧事件
