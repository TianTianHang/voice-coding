## Context

当前项目已经有稳定的 VAD 状态机、ASR 预热和转写管线，但前端仍是一个偏工具型的单窗口界面。新的方向要求它变成一个常驻托盘的语音前台：用户点击开始后持续监听，VAD 自动分段，ASR 完成后自动把当前一句送入 ACP agent，并在输出区展示 agent 的完整执行流。

这意味着产品的核心不再是“录音并显示文本”，而是“语音输入、会话路由、agent 执行流可视化”和“多 ACP agent 的统一接入”。

## Goals / Non-Goals

**Goals:**
- 把应用形态调整为托盘常驻的语音 agent 控制台。
- 保持当前 VAD 分段与自动转写的工作流，不要求用户手动管理每一段录音。
- 将每段完成的语音自动发送给 ACP agent。
- 为任意兼容 ACP 的 agent 提供统一接入方式，而不是绑定单一实现。
- 让输出区显示完整流式信息，但通过类型区分保持可读性。

**Non-Goals:**
- 不在本次变更中设计命令/skill 建议体系。
- 不在本次变更中实现编辑器深度插入或跨应用自动输入。
- 不在本次变更中引入远程 ACP 传输。
- 不在本次变更中重做 ASR/VAD 算法本身。

## Library / Dependency Choices

### 前端

- 继续使用现有 `React 19`、`TypeScript`、`Vite` 和 `@tauri-apps/api`。
- 不新增前端状态管理库。当前状态主要来自 Tauri events，适合继续用 React hooks 封装。
- 不新增 UI 组件库。助手面板需要高度定制的小型工作台界面，直接用本地组件和 CSS 更容易控制密度、状态色和输出流结构。
- 暂不引入图标库。第一版按钮数量少，可以优先用文本和少量 CSS 状态符号；如果后续需要更多工具按钮，再评估 `lucide-react`。

### Tauri / 桌面能力

- 使用 Tauri v2 自带的窗口管理能力处理显示、隐藏和关闭拦截。
- 为系统托盘启用 Tauri 的 tray 支持，优先使用 Rust 侧的 `tauri::tray::TrayIconBuilder` 和 menu API 管理托盘入口。
- 本次变更不强制引入全局快捷键插件。快捷键唤起可以作为后续增量；如果本轮决定实现全局快捷键，再加入 `tauri-plugin-global-shortcut` 及对应前端/权限配置。
- 保留现有 `tauri-plugin-opener`，但它不是本变更的核心依赖。

### Rust 后端

- 继续使用现有 `serde` / `serde_json` 定义 ACP JSON-RPC 请求、响应、通知和内部事件结构。
- 继续使用 `tokio` 作为异步运行时，并为 ACP 子进程通信补充所需 feature，例如 `process` 和 `io-util`，用于启动 agent 子进程、写入 stdin、按行读取 stdout。
- 继续使用 `parking_lot` 管理轻量同步状态，必要时使用 `tokio::sync` 管理异步会话任务。
- 暂不引入 `jsonrpsee`、`tower-lsp` 或其他大型 JSON-RPC 框架。第一版 ACP client 只需要一个窄接口：生成 request id、序列化 JSON-RPC 消息、路由 response/notification、把输出归一化给前端。
- 暂不引入数据库。agent profile、活动会话和输出事件先保存在内存中；持久化配置可以后续再加。

### ACP agent 进程

- 第一版以 stdio transport 为唯一传输方式。
- agent 启动命令采用配置驱动结构，例如命令、参数、工作目录和环境变量。
- `opencode acp` 可以作为首个验证目标，但运行时不得写死 opencode 名称或协议外行为。

## Decisions

1. **前端采用托盘优先的小面板，而不是全尺寸主窗口。**  
   这样更符合“常驻、随时说、自动投递”的使用习惯，也能避免输出流与控制区争抢空间。相比保留一个大聊天窗口，这种形态更轻，打断更少。  
   备选方案是维持当前 800x600 主窗口并增加侧栏，但会让常驻运行的心智负担更重。

2. **输入区只保留当前一句，不保留长输入历史。**  
   这是因为输入的责任只是把当前语义段稳定送给 agent；历史内容会转移到输出流或 agent 侧执行记录中。  
   备选方案是保留完整 transcript 历史，但会让“自动投递”的节奏显得拖沓，也会分散注意力。

3. **输出区保留全量信息，但按事件类型分层展示。**  
   将 `thinking`、`tool`、`result`、`diff`、`confirm`、`error`、`status` 这类事件映射成稳定的 UI 语义，可以保留透明度，同时避免纯文本流失去结构。  
   备选方案是完全按聊天消息渲染，但会丢失 tool call、确认请求和 diff 的类型感。

4. **ACP 适配层放在 Rust 后端，而不是前端。**  
   进程启动、stdio 管道、生命周期、权限与会话状态都更适合放在 Tauri 后端统一处理。前端只消费归一化后的事件流即可。  
   备选方案是把 ACP 直接嵌在前端，通过 Web API 管理子进程，但会让状态管理和安全边界变得更脆弱。

5. **ACP 连接以“配置驱动的 agent 进程”作为基本单位。**  
   运行时只假设“这里有一个兼容 ACP 的可执行命令”，不绑定具体厂商或实现。这样后续接入 opencode 等实现时，只需要更换配置，不必重写前端交互。

6. **JSON-RPC 先用轻量自实现，而不是引入完整 RPC 框架。**  
   ACP client runtime 的初始需求集中在本地子进程 stdio、请求/响应关联和通知分发。用 `serde_json` 定义协议 envelope 可以保持代码可读，也能减少新依赖带来的抽象成本。  
   备选方案是引入 `jsonrpsee`，但它更适合完整 client/server runtime，本项目第一版会显得偏重。

7. **托盘先放 Rust 侧管理。**  
   托盘菜单、窗口隐藏/显示、退出应用和 agent 子进程清理都在后端边界内，放在 Rust 侧可以更直接地处理生命周期。  
   备选方案是在前端通过 JS API 创建托盘，但关闭窗口、后台运行和进程清理会分散在更多地方。

8. **第一版仅支持单一活动 agent。**  
   运行时可以保存多个 agent profile，但任意时刻只有一个 agent 子进程和一个活动 ACP 会话接收语音输入。切换 agent 时必须先断开或停止当前活动 agent，再连接新的 profile。  
   备选方案是同时运行多个 agent，但这会立刻引入输入路由、输出流归属和资源占用问题，超出第一版需要。

9. **关闭窗口策略由用户选择：隐藏到托盘或关闭即退出。**  
   默认可以采用隐藏到托盘以符合常驻助手心智，但 UI/设置中需要允许用户选择“关闭即退出”。选择退出时，系统必须停止监听、清理 ACP 子进程并退出应用。  
   备选方案是固定隐藏到托盘，但这会让用户对后台运行缺少控制感。

10. **`confirm` 事件必须单独高亮并提供操作按钮。**  
   确认请求不应只是输出流中的普通块。前端需要把它作为待处理事项突出展示，并提供确认/拒绝等操作按钮，将用户选择回传给 ACP runtime。  
   备选方案是把确认请求作为普通文本输出，但用户容易错过关键决策点。

## Architecture Sketch

```text
React assistant panel
  │
  ├─ useBackendVAD / useAsrStatus
  ├─ useAgentEvents
  │
  ▼
Tauri commands + events
  │
  ├─ vad_commands
  ├─ asr runtime
  └─ acp runtime
       │
       ├─ AgentProfile { command, args, cwd, env }
       ├─ JsonRpcTransport { child, stdin, stdout reader }
       ├─ SessionManager { active session, request ids }
       └─ EventNormalizer { ACP message -> AgentEvent }
             │
             ▼
        compatible ACP agent process
```

前端只依赖内部 `AgentEvent`，不直接依赖 ACP 原始消息。后端负责把 ACP 初始化、会话消息、工具调用、确认请求、错误和状态变化翻译成稳定事件。

## Module Plan

### Frontend

- `src/components/AssistantConsole.tsx`: 主助手面板，组合状态栏、当前一句输入区和输出流。
- `src/components/AgentEventStream.tsx`: 分类型渲染 `thinking`、`tool`、`result`、`diff`、`confirm`、`error`、`status`。
- `src/hooks/useAgentEvents.ts`: 订阅后端 agent 事件并维护前端输出流。
- 现有 `useBackendVAD` 继续负责 VAD 状态和 transcript，但需要把“完整 transcript 历史”调整为“当前一句/最近发送一句”的语义。

### Backend

- `src-tauri/src/acp/mod.rs`: ACP runtime 入口和公开类型。
- `src-tauri/src/acp/profile.rs`: agent profile 配置结构。
- `src-tauri/src/acp/json_rpc.rs`: JSON-RPC envelope、request id 和基础解析。
- `src-tauri/src/acp/transport.rs`: 子进程启动、stdin 写入、stdout 读取。
- `src-tauri/src/acp/session.rs`: 初始化、活动会话和 prompt 发送。
- `src-tauri/src/acp/events.rs`: 内部 `AgentEvent` 类型与前端事件 payload。

Tauri command 建议从最小集合开始：`connect_agent`、`disconnect_agent`、`send_agent_prompt`、`get_agent_status`。自动发送路径可以在 VAD 转写完成后直接调用 runtime，而不是绕回前端再 invoke。

第一版 session manager 只维护一个活动 agent。profile 列表可以存在，但 `connect_agent` 在已有活动 agent 时必须返回明确错误，或先要求调用方断开当前 agent。`confirm` 事件需要携带可回传的确认 id，前端操作按钮通过专门 command 将用户选择发送回 runtime。

## Risks / Trade-offs

- [语音误切段导致任务过早发送] → 继续沿用 VAD 状态反馈，并把当前一句清晰展示出来，方便用户感知是否说完。
- [不同 ACP 实现的消息细节不一致] → 在后端做事件归一化，前端只依赖稳定的内部事件类型。
- [托盘常驻与后台进程带来生命周期复杂度] → 统一由 Rust 管理窗口、托盘和 agent 子进程，并明确关闭时是隐藏还是退出。
- [输出流过长影响可读性] → 保留全量内容，但用类型分段和固定样式区分，避免混成单段文本。
- [轻量 JSON-RPC 自实现可能遗漏协议细节] → 把 JSON-RPC envelope 和消息路由集中在 `acp/json_rpc.rs`，并用单元测试覆盖 request/response/notification 解析。
- [不引入 UI 库会增加局部样式工作量] → 组件范围保持小，先用稳定 CSS 变量和分类型样式建立视觉系统。

## Migration Plan

1. 先完成前端面板重构，让当前 VAD/ASR 流程在新 UI 下可见。
2. 再在 Rust 侧接入 ACP client runtime，打通一个本地 agent 子进程。
3. 然后把自动发送、输出归一化和会话生命周期串起来。
4. 最后补齐托盘行为、窗口隐藏策略和验证流程。

回滚时可以先保留现有 VAD/ASR 路径不变，只切回当前窗口式交互；ACP runtime 也可以通过配置关闭，退回到仅转写模式。

## Open Questions

- 未来是否需要把输出流进一步拆成“执行日志”和“结果摘要”两个层次？
