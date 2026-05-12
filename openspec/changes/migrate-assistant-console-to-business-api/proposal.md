## Why

后端已经通过 `redesign-frontend-business-api` 提供面向前端的业务命令与状态事件，但主助手控制台仍以旧的 debug/compat hook 作为主状态源：语音输入依赖 `useBackendVAD`，ASR readiness 依赖 `useAsrStatus`，自动播报依赖 debug TTS 状态，Agent 连接状态由旧 ACP hook 决定。这样会让新前端继续理解后端内部模块细节，抵消业务 API 层的价值。

本变更准备对主助手控制台做一次清晰的一刀切迁移：主状态、语音会话、转写草稿、Agent turn 和语音输出全部改由业务 API 驱动；旧 hook 与 debug 命令只留给调试窗口或兼容测试。Agent 内容流暂时保持现状，因为 thinking/tool/result/diff/confirm 等细粒度内容仍由 `agent-event` 提供。

## What Changes

- 扩展 `useBusinessApi`，让它成为主界面的业务状态中枢，覆盖 `agent-turn-changed`、转写提交/编辑/丢弃、speech preference 和朗读命令。
- 将 `AssistantConsole` 的主流程状态来源切换为 `get_app_status`、`prepare_app` 和业务状态事件。
- 从主助手控制台移除对 `useBackendVAD`、`useAsrStatus` 和 debug auto-TTS 状态的直接依赖。
- 保留 `useAgentEvents` 作为 Agent 内容流适配层，仅用于渲染事件时间线、计划、确认按钮和流式内容。
- 更新组件和 hook 的本地 AGENTS 文档，标明旧 hook 的调试/兼容定位，避免后续主流程回退到 debug 命令。
- 补充测试覆盖业务状态派生、业务命令封装、主界面迁移后的关键 UI 状态。

## Scope

本变更包含：

- `src/hooks/useBusinessApi.ts`
- `src/components/AssistantConsole.tsx`
- 必要时更新 `src/components/AudioVisualizer.tsx` 或增加轻量状态 mapper
- `src/components/AGENTS.md`、`src/hooks/AGENTS.md`、相关测试

本变更不包含：

- 重构 Agent 内容流协议或删除 `useAgentEvents`
- 移除后端 debug 命令
- 重做视觉设计
- 修改 Rust 后端业务 API，除非实现时发现现有命令参数或事件契约无法满足前端迁移

## Impact

- 主助手控制台会以业务快照和业务事件作为 source of truth。
- Debug 工具窗口继续使用 debug 命令，不受主流程迁移影响。
- 旧 `useBackendVAD`、`useAsrStatus`、`useTranscription` 仍可存在，但不得再作为主助手控制台状态源。
- 后续仍需要单独提案重构 Agent 内容流，把 `agent-event` 纳入业务 API 或建立专门的业务化 Agent stream 契约。
