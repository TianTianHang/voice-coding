## ADDED Requirements

### Requirement: 使用官方 Rust SDK 建立 ACP 连接
系统 SHALL 使用官方 Rust ACP SDK 作为唯一 ACP 协议实现，连接本地兼容 ACP 的 agent 进程。

#### Scenario: 连接默认 agent profile
- **WHEN** 用户请求连接 ACP agent
- **AND** JSON 配置文件中存在有效默认 profile
- **THEN** 系统 SHALL 启动该 profile 对应的本地 agent 进程
- **AND** 系统 SHALL 使用官方 Rust ACP SDK 建立 client-side connection

#### Scenario: 不再使用手写 JSON-RPC 协议层
- **WHEN** 系统发送 initialize、new session 或 prompt 请求
- **THEN** 系统 SHALL 调用官方 Rust ACP SDK 的 typed API
- **AND** 系统 SHALL NOT 通过本地手写 JSON-RPC envelope 构造协议消息

### Requirement: 初始化并维护单一活动 ACP 会话
系统 SHALL 通过官方 SDK 初始化 agent 并维护一个活动 ACP session。

#### Scenario: 初始化会话
- **WHEN** agent 进程启动并建立连接
- **THEN** 系统 SHALL 执行 SDK 初始化流程
- **AND** 系统 SHALL 创建新的 ACP session
- **AND** 系统 SHALL 保存 agent 返回的 session id

#### Scenario: 复用活动会话
- **WHEN** 当前已有活动 ACP session
- **AND** 用户继续产生新的语音转写文本
- **THEN** 系统 SHALL 将新文本发送到同一个 session
- **AND** 系统 SHALL NOT 为每一句语音重新启动 agent 进程

#### Scenario: 拒绝第二个活动 agent
- **WHEN** 当前已有活动 ACP agent
- **AND** 用户再次请求连接 agent
- **THEN** 系统 SHALL 拒绝第二个连接请求
- **AND** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL 保持当前活动 agent 不变

### Requirement: 发送语音 prompt 到 ACP session
系统 SHALL 将完成转写的语音文本作为 prompt 发送到当前活动 ACP session。

#### Scenario: 自动发送转写文本
- **WHEN** VAD 判定语义段结束
- **AND** ASR 返回非空转写文本
- **AND** 存在活动 ACP session
- **THEN** 系统 SHALL 使用 SDK typed prompt request 将文本发送给当前 session

#### Scenario: 无活动 session 时发送失败
- **WHEN** 系统尝试发送 prompt
- **AND** 当前不存在活动 ACP session
- **THEN** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL 向前端发布 error 事件

### Requirement: 归一化 SDK notification 为前端事件
系统 SHALL 将官方 SDK 提供的 typed session notification 映射为稳定的内部 `AgentEvent`。

#### Scenario: 输出文本归一化
- **WHEN** agent 发送文本、阶段性结果或最终结果 notification
- **THEN** 系统 SHALL 发布 `result` 或 `status` 类型的 `AgentEvent`
- **AND** 事件内容 SHALL 保留可读文本

#### Scenario: 工具事件归一化
- **WHEN** agent 发送工具调用或工具结果 notification
- **THEN** 系统 SHALL 发布 `tool` 类型的 `AgentEvent`

#### Scenario: 差异事件归一化
- **WHEN** agent 发送 diff 或 patch 相关 notification
- **THEN** 系统 SHALL 发布 `diff` 类型的 `AgentEvent`

#### Scenario: 错误事件归一化
- **WHEN** SDK、agent 或 runtime 报告错误
- **THEN** 系统 SHALL 发布 `error` 类型的 `AgentEvent`
- **AND** 事件内容 SHALL 包含可读错误原因

### Requirement: 通过 SDK permission request 处理确认流
系统 SHALL 使用官方 SDK 的 permission request 回调驱动前端确认流程。

#### Scenario: 创建待处理确认
- **WHEN** agent 通过 SDK 请求用户权限
- **THEN** 系统 SHALL 创建唯一 confirmation id
- **AND** 系统 SHALL 发布 `confirm` 类型的 `AgentEvent`
- **AND** 事件 SHALL 包含 confirmation id 和可读请求内容

#### Scenario: 用户接受确认
- **WHEN** 前端提交某个 confirmation id 的接受选择
- **THEN** 系统 SHALL 通过 SDK permission response 返回允许结果
- **AND** 系统 SHALL 更新该确认的处理状态

#### Scenario: 用户拒绝确认
- **WHEN** 前端提交某个 confirmation id 的拒绝选择
- **THEN** 系统 SHALL 通过 SDK permission response 返回拒绝结果
- **AND** 系统 SHALL 更新该确认的处理状态

#### Scenario: 未知确认 id
- **WHEN** 前端提交不存在或已处理的 confirmation id
- **THEN** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 影响当前活动 session

### Requirement: 保守声明和处理 client capabilities
系统 SHALL 只声明第一版实际支持的 ACP client capabilities，并明确拒绝不支持的能力请求。

#### Scenario: 初始化时声明保守能力
- **WHEN** 系统初始化 ACP connection
- **THEN** 系统 SHALL 只声明 prompt、session notification 和 permission confirmation 所需的能力
- **AND** 系统 SHALL NOT 声明文件系统或终端能力

#### Scenario: 收到不支持的能力请求
- **WHEN** agent 请求未支持的 client capability
- **THEN** 系统 SHALL 返回明确拒绝或 unsupported 错误
- **AND** 系统 SHALL 发布可读 error 或 status 事件

### Requirement: 管理 agent 生命周期和断开状态
系统 SHALL 在断开、退出和 agent 异常结束时清理 ACP runtime 资源。

#### Scenario: 用户断开 agent
- **WHEN** 用户请求断开当前 agent
- **THEN** 系统 SHALL 关闭 SDK connection
- **AND** 系统 SHALL 停止或终止对应 agent 进程
- **AND** 系统 SHALL 清理活动 session 状态
- **AND** 系统 SHALL 发布 disconnected 状态

#### Scenario: 应用退出清理
- **WHEN** 应用执行退出流程
- **THEN** 系统 SHALL 停止监听
- **AND** 系统 SHALL 断开活动 ACP agent
- **AND** 系统 SHALL 清理 agent 子进程

#### Scenario: agent 异常退出
- **WHEN** agent 进程或 SDK connection 异常关闭
- **THEN** 系统 SHALL 清理活动 session 状态
- **AND** 系统 SHALL 发布 disconnected 状态
- **AND** 系统 SHALL 保留可读错误信息
