## Why

当前 ACP runtime 已经验证了“语音转写后自动投递给 coding agent”的产品闭环，但协议层由手写 JSON-RPC、手写 stdio transport 和基于 method 字符串的事件猜测组成，长期维护风险较高。现在决定完全采用官方 Rust ACP 库作为协议边界，并顺手把 agent 配置从环境变量迁移为 JSON profile 文件，让桌面应用的接入方式更稳定、可读和可复用。

## What Changes

- **BREAKING**: 废弃当前自研 ACP JSON-RPC 协议层，不再维护 `json_rpc.rs` 中的 envelope、request id 和 parse 逻辑。
- **BREAKING**: ACP 会话 id 不再由本地 UUID 生成，改为使用官方 SDK 建立 session 后由 agent 返回的 session id。
- 使用 `agent-client-protocol` 和 `agent-client-protocol-tokio` 作为 ACP client runtime 的唯一协议实现。
- 保留现有 Tauri command 名称和前端 `AgentEvent` / `AgentStatus` 契约，避免前端直接依赖 ACP schema。
- 将 `connect_agent` 的默认 agent 来源改为 JSON 配置文件，支持多 profile、默认 profile、命令参数数组、工作目录和环境变量映射。
- 保守第一版仅支持本地 stdio ACP agent、单一活动 agent、单一活动 session、prompt 发送、session notification 归一化和 permission confirmation flow。
- 文件系统、终端和其他高风险 client capability 第一版默认不开放；如果 agent 请求未声明或不支持的能力，runtime 返回明确拒绝或错误。
- `request_permission` 改为官方 SDK 的 typed client 回调驱动，并通过前端确认按钮异步返回用户选择。

## Capabilities

### New Capabilities
- `acp-client-runtime`: 覆盖通过官方 Rust ACP SDK 连接本地 agent、建立会话、发送语音 prompt、接收 typed notification、处理 permission confirmation、管理单一活动 agent 和生命周期清理。
- `acp-agent-config`: 覆盖通过 JSON 文件配置 ACP agent profiles、选择默认 profile、解析命令参数、工作目录和环境变量插值。

### Modified Capabilities
- 无。

## Impact

- Rust 后端：重写 `src-tauri/src/acp/session.rs` 的协议调用路径，新增 SDK client adapter，删除或停用 `src-tauri/src/acp/json_rpc.rs`，并视 `agent-client-protocol-tokio` 能力删除或压薄 `src-tauri/src/acp/transport.rs`。
- Rust 配置：扩展 `src-tauri/src/acp/profile.rs`，从 env-only 改为 JSON profile 文件加载，并保留少量环境变量用于配置文件路径和 secret 插值。
- 依赖：在 `src-tauri/Cargo.toml` 引入 `agent-client-protocol` 和 `agent-client-protocol-tokio`。
- 前端：优先不改 UI 事件契约；如果需要选择 profile，后续再扩展 command 参数和 UI。
- 自动投递：`vad_commands` 调用 `AcpRuntime::send_prompt` 的路径保持不变，但内部使用 typed SDK request。
- 验证：实现后需要运行 `nix develop -c cargo test`、`nix develop -c cargo clippy`、`pnpm build` 和相关前端测试；如果依赖下载受限，需要在允许网络的环境中完成首次锁定。
