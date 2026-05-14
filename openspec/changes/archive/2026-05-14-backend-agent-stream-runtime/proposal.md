## Why

主助手控制台已经迁移到业务 API 作为主流程事实源，但 Agent 内容流仍由前端 `useAgentEvents` 直接消费和归并 ACP 事件。这样会让前端继续理解 ACP 的增量合并、工具更新、计划替换、确认状态和 session state 细节，也难以在取消 turn、late event、跨 turn 历史等场景中保持一致。

本变更将 Agent 事件流的业务处理前移到 Rust 后端：后端维护 authoritative Agent timeline，前端只消费已经处理好的快照或补丁并负责渲染。

## What Changes

- 新增后端 Agent stream runtime，用于接收 ACP session updates 并归并为 UI-ready timeline。
- Agent stream 事件 SHALL 带有 session、turn 和 sequence 归属，后端负责丢弃 stale/late 事件并保证同一 turn 内顺序稳定。
- 后端 SHALL 维护 thinking、message/result、tool、diff、plan、confirmation、session state 和 stream error 的规范化 timeline model。
- 前端新增轻量 `useAgentStream` facade，订阅后端 timeline snapshot/patch；前端 reducer 仅做机械 upsert/reset，不再承担 ACP 语义归并。
- `AssistantConsole` SHALL 从 `useAgentStream` 渲染事件时间线、计划、确认按钮和结果内容；`useAgentEvents` 降级为兼容/debug 入口。
- 确认响应 SHALL 通过 stream/business 封装动作携带 confirmation/turn 上下文，后端更新确认状态后再推送 timeline 变更。
- 不改变视觉设计；保留现有 thinking、tool、result、diff、confirm 和 plan 展示能力。

## Capabilities

### New Capabilities
- `agent-stream-runtime`: 后端维护 Agent 内容流的 authoritative timeline，并向前端发布 UI-ready snapshot/patch。

### Modified Capabilities
- `assistant-console-ui`: 主控制台 Agent 内容流改为消费后端处理好的 timeline，不再在主流程中归并 legacy ACP raw event。

## Impact

- 后端：`src-tauri/src/acp/` 事件映射与 session runtime，可能新增 `agent_stream` 模块和 Tauri command/event。
- 前端 hooks：新增 `src/hooks/useAgentStream.ts`，保留 `useAgentEvents.ts` 作为兼容/debug 入口。
- 前端组件：`AssistantConsole.tsx`、`AgentEventStream.tsx` 需要改为消费 timeline model；确认按钮行为不得回退。
- 测试：补充 Rust timeline reducer/ordering/stale-event 测试、TypeScript hook reducer 测试、组件渲染测试。
- 验证：运行 `pnpm test src/hooks`、`pnpm test src/components`、`pnpm build`，以及相关 Rust 测试如 `nix develop -c cargo test` 或更窄的 ACP/stream 测试。
