## 1. SDK 依赖与 API 确认

- [x] 1.1 在 `src-tauri/Cargo.toml` 中加入 `agent-client-protocol` 和 `agent-client-protocol-tokio` 依赖，并更新锁文件
- [x] 1.2 通过本地 docs、crate 源码或小型编译 spike 确认 initialize、new session、prompt、session notification 和 permission response 的实际类型字段
- [x] 1.3 确认 `agent-client-protocol-tokio` 是否能覆盖 command、args、cwd、env 的 agent 进程启动需求，并记录是否保留薄 transport helper

## 2. JSON Agent 配置

- [x] 2.1 扩展 `src-tauri/src/acp/profile.rs`，定义 `AgentConfigFile`、JSON profile 结构和默认 profile 解析逻辑
- [x] 2.2 实现配置文件查找顺序：`ACP_AGENT_CONFIG` 指定路径优先，其次当前工作目录 `acp-agents.json`
- [x] 2.3 实现 profile 字段校验，覆盖缺少 id、name、command、默认 profile 不存在和多 profile 无默认值等错误
- [x] 2.4 实现 `args` 数组透传，移除对 agent 参数的空白切分依赖
- [x] 2.5 实现 env 字面量传递和 `${ENV_NAME}` 形式的环境变量插值，缺失变量时返回明确错误
- [x] 2.6 为配置加载、默认 profile 解析、参数数组和 env 插值添加 Rust 单元测试

## 3. SDK Client Adapter

- [x] 3.1 新增 `src-tauri/src/acp/client.rs`，实现本应用的 `VoiceCodingAcpClient`
- [x] 3.2 在 client adapter 中实现 session notification 到 `AgentEvent` 的映射，覆盖 result、tool、diff、status 和 error
- [x] 3.3 在 client adapter 中实现 permission request 到 `confirm` 事件的映射，并生成唯一 confirmation id
- [x] 3.4 建立 pending permission map，使用 oneshot 或等价机制等待前端确认结果
- [x] 3.5 对未支持的文件系统、终端或其他 client capability 返回明确拒绝或 unsupported 错误
- [x] 3.6 为事件映射、pending confirmation、接受/拒绝和未知 confirmation id 添加 Rust 单元测试

## 4. ACP Runtime 重写

- [x] 4.1 重写 `AcpRuntime::connect_from_environment` 或替代入口，使其从 JSON 默认 profile 加载 agent 配置
- [x] 4.2 重写连接流程，使用官方 SDK 建立 client-side connection、初始化 agent 并创建 session
- [x] 4.3 将活动 session id 改为使用 agent 返回值，并更新 `AgentStatus`
- [x] 4.4 重写 `send_prompt`，使用 SDK typed prompt request 发送当前语音文本
- [x] 4.5 重写 `respond_confirmation`，通过 pending permission response 返回接受或拒绝结果
- [x] 4.6 保持单一活动 agent 约束，重复连接时返回明确错误并保留原会话
- [x] 4.7 重写断开和退出清理流程，关闭 SDK connection、清理子进程、清空 session 和 pending permission 状态
- [x] 4.8 确保 `vad_commands` 自动投递路径无需前端改动即可继续调用 `AcpRuntime::send_prompt`

## 5. 删除自研协议层

- [x] 5.1 删除或停用 `src-tauri/src/acp/json_rpc.rs`，并移除所有手写 JSON-RPC request/notification 调用
- [x] 5.2 根据 SDK Tokio helper 的覆盖范围删除或压薄 `src-tauri/src/acp/transport.rs`
- [x] 5.3 更新 `src-tauri/src/acp/mod.rs` 和相关 imports，确保模块边界只暴露 runtime、profile、events 和 SDK adapter
- [x] 5.4 删除或改写依赖旧 JSON-RPC envelope 的单元测试

## 6. 前端兼容性检查

- [x] 6.1 确认现有 `useAgentEvents`、`AgentEventStream` 和确认按钮仍能消费后端 `AgentEvent` / `AgentStatus`
- [x] 6.2 如后端确认状态事件字段发生变化，做最小前端适配并保持 UI 行为不变
- [x] 6.3 验证 `connect_agent`、`disconnect_agent`、`send_agent_prompt` 和 `respond_agent_confirmation` 的前端调用名称保持兼容

## 7. 验证

- [x] 7.1 运行 `nix develop -c cargo test`
- [x] 7.2 运行 `nix develop -c cargo clippy`
- [x] 7.3 运行 `pnpm build`
- [x] 7.4 运行 `pnpm test`
- [x] 7.5 运行 `nix develop -c pnpm tauri build`
- [x] 7.6 记录任何因网络、依赖下载、模型文件或本地环境导致无法运行的检查及明确原因
