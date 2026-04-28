## Context

项目当前已经有一个完整的语音 agent 控制台闭环：VAD 自动分段，ASR 输出当前语义段，Rust 后端自动把文本发送给 ACP agent，前端消费统一的 `AgentEvent` / `AgentStatus` 事件流。但是 ACP runtime 的协议层由本地手写实现组成，包括 JSON-RPC envelope、request id、stdio transport、`initialize` / `session/new` / `session/prompt` 请求，以及基于 method 字符串的输出类型归一化。

这套实现适合验证产品方向，但不适合成为长期协议边界。ACP 已经提供官方 Rust SDK，应用应把协议细节交给 SDK，并只保留面向本产品的 UI 事件模型和 Tauri command 边界。

当前配置方式也偏临时：`AgentProfile::from_environment()` 从 `ACP_AGENT_CMD`、`ACP_AGENT_ARGS`、`ACP_AGENT_CWD`、`ACP_AGENT_NAME` 读取单个 agent。桌面应用需要更可读、更可维护的 profile 配置，因此本次迁移同时引入 JSON 配置文件。

## Goals / Non-Goals

**Goals:**

- 完全使用官方 Rust ACP SDK 处理 ACP 协议、连接、typed request、typed notification 和 permission request。
- 保留前端 `AgentEvent` / `AgentStatus` 契约，避免 React 层直接依赖 ACP schema。
- 保留现有 Tauri command 名称，让 `connect_agent`、`disconnect_agent`、`send_agent_prompt` 等调用点尽量稳定。
- 第一版仅支持本地 stdio agent、单一活动 agent 和单一活动 session。
- 使用 JSON 文件配置 ACP agent profiles，支持默认 profile、命令、参数数组、工作目录和环境变量映射。
- 用 SDK 的 `request_permission` 回调实现确认流，前端按钮通过现有确认 command 回传用户选择。
- 对文件系统、终端等高风险 client capability 采取保守策略：默认不声明支持，收到相关请求时明确拒绝。

**Non-Goals:**

- 不在本次变更中实现 profile 管理 UI 或 profile 持久化编辑器。
- 不支持多个 agent 并发连接或多 session 路由。
- 不开放远程 ACP transport。
- 不实现完整文件系统、终端或编辑器集成能力。
- 不重做 VAD、ASR、托盘窗口生命周期或前端输出流视觉设计。

## Decisions

1. **官方 SDK 是唯一 ACP 协议边界。**  
   Rust 后端不再生成原始 JSON-RPC request，也不再解析原始 line-based message 来判断业务类型。`agent-client-protocol` 负责 schema 和 typed API，`agent-client-protocol-tokio` 优先负责 Tokio 进程/stdio 集成。备选方案是保留手写 transport 只替换类型，但这仍会让协议生命周期、错误和 streaming 细节散在本地代码里。

2. **前端继续消费内部 `AgentEvent`，不直接消费 ACP 类型。**  
   UI 层需要的是 `thinking`、`tool`、`result`、`diff`、`confirm`、`error`、`status` 这样的稳定展示语义。ACP SDK 类型属于后端协议边界，应该由 Rust adapter 映射成内部事件。备选方案是把 ACP notification 原样传到前端，但会让前端随着协议字段变化而频繁调整。

3. **新增 `VoiceCodingAcpClient` 作为 SDK `Client` trait 实现。**  
   该 adapter 负责接收 agent 发给 client 的 session notification 和 permission request。notification 会被映射为 `AgentEvent` 并通过 Tauri event 发送给前端；permission request 会创建 pending confirmation，等待前端调用 `respond_agent_confirmation`。备选方案是在 `AcpRuntime` 中直接实现所有回调，但会让 runtime 同时承担状态管理、事件映射和权限等待三类职责。

4. **session id 使用 agent 返回值。**  
   当前实现本地生成 UUID 后传给 `session/new`。迁移后 `connect_agent` 应调用 SDK 的 typed new-session API，并把 response 中的 session id 记录为唯一活动 session。这样 runtime 遵循 ACP 的会话所有权，而不是把本地假设写进协议。备选方案是继续本地生成 id，但这会绕开 SDK 的协商结果。

5. **JSON profile 文件替代 env-only 配置。**  
   配置文件结构为单文件、多 profile、一个默认 profile。`args` 使用数组而不是 shell 字符串，避免空格和引号解析问题。保留 `ACP_AGENT_CONFIG` 用于指定配置路径，保留 `${ENV_NAME}` 插值用于 secret 值。备选方案是继续使用环境变量，但它无法自然表达多个 profile，也不适合桌面应用长期配置。

6. **第一版默认只连接 `defaultProfile`。**  
   现有前端不需要新增 profile 选择 UI。`connect_agent` 先读取默认 profile 并连接；未来如果要支持 UI 切换，可以扩展 command 参数为可选 `profileId`。备选方案是本次同步做 profile UI，但会扩大前端改动面，偏离协议层重建的主目标。

7. **保守声明 client capabilities。**  
   初始化时只声明本应用第一版真正支持的 capability。文件系统、终端、编辑器深度操作等能力不声明支持；如果 SDK trait 要求实现对应方法，则返回明确拒绝或 unsupported error。备选方案是先声明再内部拒绝，但这会误导 agent 规划能力。

8. **子进程生命周期仍由 Rust runtime 统一管理。**  
   `AcpRuntime` 负责连接、断开、退出清理和异常状态发布。若 `agent-client-protocol-tokio` 能完整覆盖 command/args/cwd/env 的 spawn 需求，则删除本地 `transport.rs`；若不能，则只保留极薄的 process-spawn helper，把协议读写交给 SDK。

## Architecture Sketch

```text
React assistant panel
  │
  ├─ useAgentEvents
  └─ existing Tauri commands
       │
       ▼
AcpRuntime
  │
  ├─ AgentConfigFile -> AgentProfile
  ├─ ActiveAgent
  │    ├─ profile
  │    ├─ SDK agent connection
  │    ├─ session_id returned by agent
  │    └─ process / connection task handles
  │
  └─ VoiceCodingAcpClient implements SDK Client
       ├─ session_notification -> AgentEvent
       ├─ request_permission -> pending confirmation -> frontend response
       └─ unsupported capabilities -> explicit rejection
```

## Configuration Shape

第一版配置文件建议命名为 `acp-agents.json`：

```json
{
  "defaultProfile": "opencode",
  "profiles": [
    {
      "id": "opencode",
      "name": "OpenCode",
      "command": "opencode",
      "args": ["acp"],
      "cwd": "/home/tiantian/project/voice-coding",
      "env": {
        "OPENAI_API_KEY": "${OPENAI_API_KEY}"
      }
    }
  ]
}
```

加载优先级：

```text
ACP_AGENT_CONFIG 指定路径
  ↓
当前工作目录 ./acp-agents.json
  ↓
返回未配置 agent 的明确错误
```

`env` 中的普通字符串按字面量传给子进程；`${NAME}` 形式从当前进程环境变量读取。第一版只支持整个字符串为单个变量引用，不做复杂模板拼接，避免 secret 展开规则过宽。

## Permission Flow

```text
Agent requests permission
  │
  ▼
VoiceCodingAcpClient::request_permission
  │
  ├─ allocate confirmation_id
  ├─ store oneshot sender in pending map
  ├─ emit AgentEvent { kind: confirm, confirmationId, content }
  │
  ▼
Frontend confirm/reject button
  │
  ▼
respond_agent_confirmation(confirmation_id, accepted)
  │
  ├─ remove pending sender
  ├─ send SDK permission response
  └─ emit confirm status update
```

如果前端返回未知或已完成的 confirmation id，后端返回明确错误，并不影响当前 agent session。

## Risks / Trade-offs

- [SDK API 字段与当前理解不同] → 实施第一步先做 SDK API spike，确认 `initialize`、`new_session`、`prompt`、notification 和 permission response 的确切类型，再写 runtime。
- [官方 Tokio helper 不覆盖现有 profile spawn 需求] → 保留一个薄的 process-spawn helper，但协议连接和消息处理仍交给 SDK。
- [保守 capability 导致某些 agent 功能不可用] → 第一版明确拒绝 unsupported capability，先保证语音 prompt 闭环正确；后续再针对文件系统或终端能力开独立 change。
- [JSON 配置引入 secret 泄露风险] → 支持 `${ENV_NAME}` 插值，文档建议不要把 secret 明文写进配置文件。
- [前端事件契约可能无法表达所有 typed notification] → 第一版采用最小稳定映射；无法精确分类的 notification 归入 `status` 或 `result`，并保留可读文本。
- [删除自研协议层可能影响现有 mock/test] → 迁移时同步重写 Rust 单元测试，使测试围绕 profile loading、event mapping、permission pending map 和 runtime 状态，而不是 JSON-RPC envelope。

## Migration Plan

1. 引入 SDK 依赖并做 API spike，确认官方类型字段和 Tokio spawn 能力。
2. 新增 JSON config loader，并让 `connect_agent` 从默认 profile 读取 agent 配置。
3. 新增 `VoiceCodingAcpClient` adapter，先实现 notification 映射和 permission pending flow。
4. 重写 `AcpRuntime` 连接、断开和 prompt 发送路径，使用 SDK typed API 建立 session 并发送 prompt。
5. 删除或停用 `json_rpc.rs`，压薄或删除 `transport.rs`。
6. 更新测试，覆盖配置加载、缺失配置、默认 profile、env 插值、单一活动 agent、permission response 和 lifecycle cleanup。
7. 运行 Rust 与前端验证命令。

回滚策略：保留前端事件契约和 Tauri command 名称可以降低回滚成本；如 SDK 迁移遇到阻塞，可在 Git 层回退本次 change，旧的手写 ACP runtime 不需要和新 SDK runtime 并存。

## Open Questions

- `agent-client-protocol-tokio` 是否能直接支持 command、args、cwd 和 env 的完整 profile spawn 需求？
- SDK 的 prompt content 是否要求 content block 结构，还是支持简单 text prompt helper？
- permission response 的 typed enum 是否能直接表达 confirm/reject，是否还需要携带 message 或 metadata？
