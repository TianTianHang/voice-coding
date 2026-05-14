## 1. 后端 timeline 模型

- [x] 1.1 定义 `AgentTimelineSnapshot`、`AgentTimelinePatch`、`AgentTimelineItem`、plan、session state 和 confirmation DTO，保持 camelCase 序列化契约。
- [x] 1.2 新增后端 timeline reducer/helper，支持 reset、upsert item、update plan、update session state、resolve confirmation 和 stream error。
- [x] 1.3 为 reducer 补充 Rust 单元测试，覆盖 message/thinking 合并、tool update、plan replace、confirmation resolve、sequence 递增和 late event 处理。

## 2. ACP stream runtime 集成

- [x] 2.1 新增 `AgentStreamRuntime`，维护当前 session、active turn、sequence、items、plan、session state 和 pending confirmations。
- [x] 2.2 将 ACP SDK notification 归一化结果接入 `AgentStreamRuntime`，由 runtime 生成 timeline patch。
- [x] 2.3 保留 legacy `agent-event` emit，确保兼容 hook/debug 入口暂不破坏。
- [x] 2.4 将业务 Agent turn 的开始、完成、失败和取消状态同步给 stream runtime，用于 turn 归属和 stale event 丢弃。

## 3. Tauri 命令与事件

- [x] 3.1 增加 `get_agent_timeline` 命令，返回当前 `AgentTimelineSnapshot`。
- [x] 3.2 增加 `agent-timeline-changed` 事件，发布 reset 和增量 patch。
- [x] 3.3 增加或封装 confirmation 响应命令，使确认接受/拒绝以后端 SDK response 结果更新 timeline 状态。
- [x] 3.4 注册新增命令和 runtime state，并补充后端命令/事件测试或集成测试。

## 4. 前端 Agent stream facade

- [x] 4.1 新增 `src/hooks/useAgentStream.ts` 类型和 hook，启动时先订阅 patch 再读取 `get_agent_timeline` 快照。
- [x] 4.2 实现前端轻量 patch reducer，仅做 reset、upsert、replace plan、replace/merge session state、resolve confirmation 等机械更新。
- [x] 4.3 为 `useAgentStream` 的 patch reducer、订阅合并和 confirmation 动作补充 vitest 覆盖。

## 5. 主控制台迁移

- [x] 5.1 将 `AssistantConsole` 的 Agent 内容流、计划、选中事件和确认按钮迁移到 `useAgentStream`。
- [x] 5.2 调整 `AgentEventStream` 或新增 adapter，使其渲染后端 timeline item，同时保留 thinking、tool、result/message、diff、confirm、error 和 fallback 展示能力。
- [x] 5.3 确认主控制台不再 import `useAgentEvents` 作为 Agent 内容流事实源，legacy hook 仅留作 debug/compat。
- [x] 5.4 更新 `src/hooks/AGENTS.md`、`src/components/AGENTS.md` 和必要的 `src-tauri/src/acp/AGENTS.md`，记录新旧 stream 边界。

## 6. 验证

- [x] 6.1 运行 `pnpm test src/hooks`，确认新旧 hook 行为测试通过。
- [x] 6.2 运行 `pnpm test src/components`，确认 Agent timeline 渲染和主控制台测试通过。
- [x] 6.3 运行 `pnpm build`，确认 TypeScript 与前端构建通过。
- [x] 6.4 运行 `nix develop -c cargo test` 或更窄的 ACP/agent stream Rust 测试，确认后端 timeline runtime 行为通过。
- [x] 6.5 运行 `openspec validate backend-agent-stream-runtime --strict`，确认提案 artifacts 可用于实现。
