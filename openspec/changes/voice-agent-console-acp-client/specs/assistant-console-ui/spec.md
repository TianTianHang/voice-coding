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
