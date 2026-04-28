## ADDED Requirements

### Requirement: 输出流按协议身份增量更新
系统 SHALL 根据后端提供的协议身份更新已有输出块，而不是把每个增量都渲染为新块。

#### Scenario: 合并同一 agent message
- **WHEN** 前端连续收到相同 `messageId` 且类型为 `result` 的输出事件
- **THEN** 系统 SHALL 在同一个结果块中追加文本
- **AND** 系统 SHALL NOT 为每个 chunk 创建新的可见块

#### Scenario: 实时渲染 agent message 增量
- **WHEN** 前端收到类型为 `result` 的流式 chunk
- **THEN** 系统 SHALL 立即更新对应结果块的可见文本
- **AND** 系统 SHALL NOT 等待 stop reason、turn 完成或完整消息结束后才刷新 UI

#### Scenario: 合并同一 thought message
- **WHEN** 前端连续收到相同 `messageId` 且类型为 `thinking` 的输出事件
- **THEN** 系统 SHALL 在同一个思考块中追加文本
- **AND** 系统 SHALL NOT 为每个 chunk 创建新的可见块

#### Scenario: 实时渲染 thought 增量
- **WHEN** 前端收到类型为 `thinking` 的流式 chunk
- **THEN** 系统 SHALL 立即更新对应思考块的可见文本
- **AND** 系统 SHALL NOT 等待完整 thought 消息结束后才刷新 UI

#### Scenario: 缺少 messageId 的流式文本增量
- **WHEN** 前端连续收到没有 `messageId`、但类型为 `thinking` 或 `result` 且 `operation=append` 的输出事件
- **THEN** 系统 SHALL 将其合并到最近的同类型流式文本块中
- **AND** 系统 SHALL 继续按 chunk 到达即时更新可见文本

#### Scenario: 缺少 messageId 的非流式或非追加消息
- **WHEN** 前端收到没有 `messageId` 且不满足流式文本追加条件的输出事件
- **THEN** 系统 SHALL 将其作为独立块追加
- **AND** 系统 SHALL NOT 错误合并到其他消息

### Requirement: 工具调用块反映最新状态
系统 SHALL 使用 `toolCallId` 将工具调用创建和更新渲染为同一个持续变化的工具块。

#### Scenario: 更新已有工具块
- **WHEN** 前端收到带有已存在 `toolCallId` 的工具更新
- **THEN** 系统 SHALL 更新对应工具块的 title、kind、status、content、locations、raw input 或 raw output 中变化的字段
- **AND** 系统 SHALL NOT 追加重复工具块

#### Scenario: 展示工具执行状态
- **WHEN** 工具块状态为 pending、in progress、completed 或 failed
- **THEN** 系统 SHALL 在工具块中展示对应状态
- **AND** failed 状态 SHALL 以错误样式或明确文本区分

#### Scenario: 工具更新没有已有工具块
- **WHEN** 前端收到未知 `toolCallId` 的工具更新
- **THEN** 系统 SHALL 创建一个降级工具块
- **AND** 工具块 SHALL 展示 update 中已有的信息

### Requirement: 展示 diff、terminal 和非文本内容
系统 SHALL 对工具内容和消息内容中的不同 content 类型提供可读展示。

#### Scenario: 展示 diff 内容
- **WHEN** 输出事件包含 diff 内容
- **THEN** 系统 SHALL 展示文件路径和变更内容摘要或完整文本
- **AND** 系统 SHALL 将该内容与普通文本结果视觉区分

#### Scenario: 展示 terminal 引用
- **WHEN** 输出事件包含 terminal 引用
- **THEN** 系统 SHALL 展示 terminal id 的可读占位
- **AND** 系统 SHALL NOT 声称已提供完整 terminal 交互能力

#### Scenario: 展示非文本 content block
- **WHEN** 输出事件包含 image、audio、resource link 或 embedded resource 摘要
- **THEN** 系统 SHALL 展示类型和可用元信息
- **AND** 系统 SHALL NOT 空白渲染该内容

### Requirement: 当前计划以快照方式展示
系统 SHALL 将 ACP plan update 展示为当前计划快照，并在后续更新时替换旧快照。

#### Scenario: 首次展示计划
- **WHEN** 前端收到 plan 快照
- **THEN** 系统 SHALL 展示所有 plan entries
- **AND** 每个 entry SHALL 展示 content、priority 和 status

#### Scenario: 替换已有计划
- **WHEN** 前端收到新的 plan 快照
- **THEN** 系统 SHALL 替换当前计划块
- **AND** 系统 SHALL NOT 在输出流中追加重复计划历史

### Requirement: 会话级 update 分流到状态区
系统 SHALL 将 available commands、current mode、config options 和 session info 展示为当前会话状态，而不是普通输出日志。

#### Scenario: 展示当前模式和 session info
- **WHEN** 前端收到 current mode 或 session info 状态
- **THEN** 系统 SHALL 在状态区域展示当前 mode、session title 或更新时间中可用的信息
- **AND** 系统 SHALL 保持输出流聚焦于 agent 执行内容

#### Scenario: 展示可用命令和配置
- **WHEN** 前端收到 available commands 或 config options 状态
- **THEN** 系统 SHALL 保存最新快照并提供可见摘要
- **AND** 系统 SHALL NOT 把每次快照更新作为普通输出块追加

#### Scenario: 未知 update 仍可见
- **WHEN** 前端收到后端归一化的未知 update 事件
- **THEN** 系统 SHALL 在输出流中展示可读 fallback
- **AND** 系统 SHALL 明确标记该事件为 status 或 unknown update
