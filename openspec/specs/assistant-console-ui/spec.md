# 语音助手控制台能力规格

## ADDED Requirements

### Requirement: 托盘优先的助手面板
系统 SHALL 提供一个托盘优先的桌面助手面板，用于展示当前会话状态、接收用户语音输入并显示 agent 输出流。

#### Scenario: 从托盘唤起面板
- **WHEN** 用户点击托盘图标或使用唤起快捷键
- **THEN** 系统 SHALL 显示助手面板
- **AND** 系统 SHALL 保持当前会话状态不丢失

#### Scenario: 关闭窗口隐藏到托盘
- **WHEN** 用户选择关闭窗口时隐藏到托盘
- **AND** 用户关闭助手面板窗口
- **THEN** 系统 SHALL 将窗口隐藏到托盘
- **AND** 系统 SHALL 继续保持后台会话可用

#### Scenario: 关闭窗口退出应用
- **WHEN** 用户选择关闭窗口时退出应用
- **AND** 用户关闭助手面板窗口
- **THEN** 系统 SHALL 停止后台监听
- **AND** 系统 SHALL 请求后端清理当前 agent 会话
- **AND** 系统 SHALL 退出应用

### Requirement: 单句输入与自动清空
系统 SHALL 仅展示当前一句语音的实时转写，并在该句提交后自动清空输入区。

#### Scenario: 当前一句实时展示
- **WHEN** 用户正在说话或当前语义段尚未完成
- **THEN** 系统 SHALL 在输入区展示当前一句的实时转写
- **AND** 系统 SHALL NOT 在输入区保留长历史列表

#### Scenario: 提交后清空输入区
- **WHEN** 当前语义段完成转写并已发送给 ACP runtime
- **THEN** 系统 SHALL 清空输入区
- **AND** 系统 SHALL 准备接收下一句语音

### Requirement: 输出流按事件类型区分
系统 SHALL 将 agent 的输出流按事件类型区分展示，同时保留完整内容。

#### Scenario: 工具调用与结果区分
- **WHEN** agent 输出工具调用或工具结果
- **THEN** 系统 SHALL 将其分别标记为 `tool` 或 `result`
- **AND** 系统 SHALL 使用不同的标题或视觉样式展示

#### Scenario: 确认请求高亮
- **WHEN** agent 输出需要用户确认的内容
- **THEN** 系统 SHALL 将其标记为 `confirm`
- **AND** 系统 SHALL 以明显样式提示用户该项需要处理

#### Scenario: 确认请求提供操作按钮
- **WHEN** 输出流中存在待处理的 `confirm` 事件
- **THEN** 系统 SHALL 为该事件显示确认和拒绝操作按钮
- **AND** 用户点击任一按钮后，系统 SHALL 将用户选择发送给后端 runtime

#### Scenario: 错误和状态区分
- **WHEN** agent 或后台运行时发生错误或状态变化
- **THEN** 系统 SHALL 将其分别标记为 `error` 或 `status`
- **AND** 系统 SHALL 保留该条消息在输出流中

### Requirement: 显示连续工作状态
系统 SHALL 清晰展示监听、录音、转写、发送和连接状态。

#### Scenario: 等待下一句
- **WHEN** 系统处于空闲监听状态
- **THEN** 系统 SHALL 显示正在监听的状态
- **AND** 系统 SHALL 提示当前会话仍然活跃

#### Scenario: 录音中
- **WHEN** VAD 检测到语音并进入录音状态
- **THEN** 系统 SHALL 显示录音中状态
- **AND** 系统 SHALL 强调当前语义段仍在收集

#### Scenario: 转写或发送中
- **WHEN** 当前语义段正在转写或已转发给 agent
- **THEN** 系统 SHALL 显示处理中或已发送状态
- **AND** 系统 SHALL 保持输出流可见

#### Scenario: 连接失败
- **WHEN** 后台 agent 或 ACP 连接失败
- **THEN** 系统 SHALL 显示失败状态
- **AND** 系统 SHALL 提供可见错误信息

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
