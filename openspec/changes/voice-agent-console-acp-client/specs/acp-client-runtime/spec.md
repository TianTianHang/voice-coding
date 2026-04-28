# ACP Client Runtime Capability Specification

## ADDED Requirements

### Requirement: 配置驱动接入 ACP 代理进程
系统 SHALL 通过可配置命令启动兼容 ACP 的 agent 子进程，并通过 stdio 与其通信。

#### Scenario: 启动兼容 agent
- **WHEN** 用户选择一个配置好的 ACP agent
- **THEN** 系统 SHALL 启动对应子进程
- **AND** 系统 SHALL 将标准输入输出作为协议通道

#### Scenario: 代理实现无关
- **WHEN** 配置中的 agent 命令来自不同实现
- **THEN** 系统 SHALL 采用同一套 client runtime 接入
- **AND** 系统 SHALL NOT 绑定单一 vendor 或二进制名称

### Requirement: 建立和维护 ACP 会话
系统 SHALL 在子进程启动后建立 ACP 会话，并在同一活动会话中处理连续的语音语义段。

#### Scenario: 初始化会话
- **WHEN** agent 子进程启动完成
- **THEN** 系统 SHALL 执行协议初始化流程
- **AND** 系统 SHALL 将结果记录为当前活动会话

#### Scenario: 连续语义段复用会话
- **WHEN** 用户在同一工作流中继续发言
- **THEN** 系统 SHALL 复用当前活动会话
- **AND** 系统 SHALL NOT 为每一句创建完全独立的进程

#### Scenario: 单一活动 agent
- **WHEN** 已存在活动 ACP agent 会话
- **AND** 用户尝试连接另一个 agent profile
- **THEN** 系统 SHALL 拒绝第二个连接请求并返回明确错误
- **AND** 系统 SHALL 保持当前活动 agent 会话不变

#### Scenario: 切换 agent 前断开当前会话
- **WHEN** 用户先断开当前活动 agent 会话
- **AND** 用户连接另一个 agent profile
- **THEN** 系统 SHALL 启动新的 agent 子进程
- **AND** 系统 SHALL 将新的会话记录为唯一活动会话

### Requirement: 自动转发完成的语音句子
系统 SHALL 将每个完成转写的语义段自动发送到当前活动 ACP 会话。

#### Scenario: 语义段完成后发送
- **WHEN** VAD 判定当前语义段结束且 ASR 返回转写文本
- **THEN** 系统 SHALL 自动将该文本转发给 ACP 会话
- **AND** 系统 SHALL NOT 额外要求用户确认后再发送

#### Scenario: 发送失败可见化
- **WHEN** 语义段发送给 ACP 会话失败
- **THEN** 系统 SHALL 记录错误原因
- **AND** 系统 SHALL 向前端发布错误状态事件

### Requirement: 归一化 ACP 输出为内部事件流
系统 SHALL 将 ACP 输出转换为稳定的内部事件类型，供前端统一渲染。

#### Scenario: 工具调用归一化
- **WHEN** ACP agent 发出工具调用相关消息
- **THEN** 系统 SHALL 归一化为 `tool` 事件

#### Scenario: 结果归一化
- **WHEN** ACP agent 返回工具执行结果或阶段性结论
- **THEN** 系统 SHALL 归一化为 `result` 事件

#### Scenario: 确认请求归一化
- **WHEN** ACP agent 请求用户确认
- **THEN** 系统 SHALL 归一化为 `confirm` 事件
- **AND** 事件 SHALL 包含可用于回传用户选择的确认 id

#### Scenario: 用户确认回传
- **WHEN** 前端提交某个确认 id 的确认或拒绝选择
- **THEN** 系统 SHALL 将该选择发送回当前活动 ACP 会话
- **AND** 系统 SHALL 更新该确认事件的处理状态

#### Scenario: 错误与状态归一化
- **WHEN** ACP agent 或 runtime 发生错误、连接变化或恢复状态变化
- **THEN** 系统 SHALL 归一化为 `error` 或 `status` 事件

### Requirement: 正确管理子进程生命周期
系统 SHALL 在停止、退出或 agent 异常终止时正确清理 ACP 子进程和相关资源。

#### Scenario: 正常停止
- **WHEN** 用户停止当前会话或退出应用
- **THEN** 系统 SHALL 终止或关闭 ACP 子进程
- **AND** 系统 SHALL 清理 stdio 管道和会话状态

#### Scenario: 异常退出
- **WHEN** ACP 子进程意外退出
- **THEN** 系统 SHALL 向前端发布断开状态
- **AND** 系统 SHALL 保留可读错误信息以便恢复
