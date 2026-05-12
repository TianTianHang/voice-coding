## MODIFIED Requirements

### Requirement: 输出流按事件类型区分
系统 SHALL 从后端 Agent timeline snapshot 或 patch 渲染 agent 输出流，并按 timeline item 类型区分展示，同时保留完整内容。

#### Scenario: 工具调用与结果区分
- **WHEN** 后端 Agent timeline 包含 tool 或 message item
- **THEN** 系统 SHALL 将其分别展示为工具调用块或结果块
- **AND** 系统 SHALL 使用不同的标题或视觉样式展示

#### Scenario: 确认请求高亮
- **WHEN** 后端 Agent timeline 包含 pending confirmation item
- **THEN** 系统 SHALL 以明显样式提示用户该项需要处理
- **AND** 系统 SHALL 展示该 confirmation item 的可读请求内容

#### Scenario: 确认请求提供操作按钮
- **WHEN** 输出流中存在待处理的 confirmation item
- **THEN** 系统 SHALL 为该 item 显示确认和拒绝操作按钮
- **AND** 用户点击任一按钮后，系统 SHALL 通过 Agent stream hook 将用户选择发送给后端 runtime

#### Scenario: 错误和状态区分
- **WHEN** 后端 Agent timeline 包含 error、fallback 或 session-level status
- **THEN** 系统 SHALL 将其以对应样式或状态区域展示
- **AND** 系统 SHALL 保留可读消息内容

### Requirement: 输出流按协议身份增量更新
系统 SHALL 根据后端 Agent timeline patch 更新已有输出块，而不是在主控制台前端解释 ACP operation 或把每个增量都渲染为新块。

#### Scenario: 合并同一 agent message
- **WHEN** 前端收到后端针对同一 message timeline item 的 upsert patch
- **THEN** 系统 SHALL 在同一个结果块中展示更新后的文本
- **AND** 系统 SHALL NOT 为每个 chunk 创建新的可见块

#### Scenario: 实时渲染 agent message 增量
- **WHEN** 前端收到类型为 message 的 timeline item patch
- **THEN** 系统 SHALL 立即更新对应结果块的可见文本
- **AND** 系统 SHALL NOT 等待 stop reason、turn 完成或完整消息结束后才刷新 UI

#### Scenario: 合并同一 thought message
- **WHEN** 前端收到后端针对同一 thinking timeline item 的 upsert patch
- **THEN** 系统 SHALL 在同一个思考块中展示更新后的文本
- **AND** 系统 SHALL NOT 为每个 chunk 创建新的可见块

#### Scenario: 实时渲染 thought 增量
- **WHEN** 前端收到类型为 thinking 的 timeline item patch
- **THEN** 系统 SHALL 立即更新对应思考块的可见文本
- **AND** 系统 SHALL NOT 等待完整 thought message 结束后才刷新 UI

#### Scenario: 前端不解释缺失 messageId 的合并规则
- **WHEN** 后端 ACP 输入缺少 message id 但已经归并为 timeline item
- **THEN** 主控制台 SHALL 按后端提供的 item id 更新可见块
- **AND** 主控制台 SHALL NOT 自行根据最近同类事件推断合并目标

#### Scenario: 非流式或非追加消息
- **WHEN** 后端发布新的独立 timeline item
- **THEN** 系统 SHALL 将其作为独立块追加
- **AND** 系统 SHALL NOT 错误合并到其他消息

### Requirement: 工具调用块反映最新状态
系统 SHALL 使用后端 Agent timeline 中的 tool item 将工具调用创建和更新渲染为同一个持续变化的工具块。

#### Scenario: 更新已有工具块
- **WHEN** 前端收到后端针对已存在 tool item id 的 upsert patch
- **THEN** 系统 SHALL 更新对应工具块的 title、kind、status、content、locations、raw input 或 raw output 中变化的字段
- **AND** 系统 SHALL NOT 追加重复工具块

#### Scenario: 展示工具执行状态
- **WHEN** 工具块状态为 pending、in progress、completed 或 failed
- **THEN** 系统 SHALL 在工具块中展示对应状态
- **AND** failed 状态 SHALL 以错误样式或明确文本区分

#### Scenario: 工具更新没有已有工具块
- **WHEN** 后端将未知 tool update 归并为降级 tool item
- **THEN** 系统 SHALL 创建或更新对应工具块
- **AND** 工具块 SHALL 展示后端 item 中已有的信息

### Requirement: 当前计划以快照方式展示
系统 SHALL 将后端 Agent timeline 的 plan snapshot 展示为当前计划快照，并在后续 patch 更新时替换旧快照。

#### Scenario: 首次展示计划
- **WHEN** 前端收到包含 plan 的 Agent timeline snapshot 或 updatePlan patch
- **THEN** 系统 SHALL 展示所有 plan entries
- **AND** 每个 entry SHALL 展示 content、priority 和 status

#### Scenario: 替换已有计划
- **WHEN** 前端收到新的 updatePlan patch
- **THEN** 系统 SHALL 替换当前计划块
- **AND** 系统 SHALL NOT 在输出流中追加重复计划历史

### Requirement: 会话级 update 分流到状态区
系统 SHALL 将后端 Agent timeline 的 available commands、current mode、config options 和 session info 展示为当前会话状态，而不是普通输出日志。

#### Scenario: 展示当前模式和 session info
- **WHEN** 前端收到包含 current mode 或 session info 的 timeline snapshot 或 session state patch
- **THEN** 系统 SHALL 在状态区域展示当前 mode、session title 或更新时间中可用的信息
- **AND** 系统 SHALL 保持输出流聚焦于 agent 执行内容

#### Scenario: 展示可用命令和配置
- **WHEN** 前端收到 available commands 或 config options 状态
- **THEN** 系统 SHALL 保存最新快照并提供可见摘要
- **AND** 系统 SHALL NOT 把每次快照更新作为普通输出块追加

#### Scenario: 未知 update 仍可见
- **WHEN** 后端 Agent timeline 包含 fallback 或 unknown item
- **THEN** 系统 SHALL 在输出流中展示可读 fallback
- **AND** 系统 SHALL 明确标记该事件为 status 或 unknown update

## ADDED Requirements

### Requirement: 主控制台只渲染后端 Agent timeline
主助手控制台 SHALL 使用后端 Agent timeline snapshot 和 patch 作为 Agent 内容流事实源，不再在主流程中归并 legacy ACP raw event。

#### Scenario: 启动后读取 Agent timeline
- **WHEN** 主助手控制台挂载
- **THEN** 控制台 SHALL 通过 Agent stream hook 读取后端 `AgentTimelineSnapshot`
- **AND** 控制台 SHALL 使用该 snapshot 恢复输出流、计划、session state 和 pending confirmations

#### Scenario: 订阅 timeline patch
- **WHEN** 后端发布 Agent timeline patch
- **THEN** 控制台 SHALL 根据 patch 机械更新本地渲染状态
- **AND** 控制台 SHALL NOT 使用 legacy `useAgentEvents` reducer 解释 ACP operation

#### Scenario: legacy hook 不作为主流程入口
- **WHEN** 开发者检查主控制台 imports 和调用点
- **THEN** 主控制台 SHALL NOT import `useAgentEvents` 作为 Agent 内容流事实源
- **AND** legacy hook MAY 继续存在于 debug 或兼容入口
