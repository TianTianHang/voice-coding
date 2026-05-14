## 1. 业务 hook 扩展

- [x] 1.1 补齐 `useBusinessApi` 的业务类型，覆盖 Agent turn、转写草稿请求、speech preference 和缺失的业务命令返回值。
- [x] 1.2 订阅并合并 `agent-turn-changed`，让 hook 对外暴露当前 Agent turn 状态。
- [x] 1.3 增加 `pauseVoiceSession`、`resumeVoiceSession`、`discardTranscript`、`editAndSubmitTranscript`、`cancelAgentTurn`、`speakAgentResult` 和 `setSpeechPreferences` 等业务动作封装。
- [x] 1.4 为业务 hook 的事件合并和命令封装补充 vitest 覆盖。

## 2. 主控制台状态迁移

- [x] 2.1 将主控制台的语音体验状态派生改为基于 `useBusinessApi` 的 `AppStatus`、`VoiceSessionStatus`、`AgentStatus`、`AgentTurnStatus`、`SpeechOutputStatus` 和 `RuntimeErrorEvent`。
- [x] 2.2 从 `AssistantConsole` 主流程移除 `useBackendVAD` 和 `useAsrStatus` 依赖，准备、语音控制和错误展示改用业务状态。
- [x] 2.3 将转写展示、编辑提交、丢弃、手动发送和取消 turn 的动作改为调用业务 hook。
- [x] 2.4 将自动朗读开关、朗读 Agent 结果、朗读文本和停止朗读改为使用业务 speech 状态与业务命令。

## 3. Agent 内容流边界

- [x] 3.1 保留 `useAgentEvents` 用于 thinking、tool、result、diff、confirm 和 plan 渲染，并确保确认按钮行为不回退。
- [x] 3.2 将主连接状态和当前 turn 展示改为优先读取业务 Agent 状态，`useAgentEvents` 不再作为主连接事实源。

## 4. 文档与守护

- [x] 4.1 更新 `src/hooks/AGENTS.md`，标明 `useBusinessApi` 是主产品流程 facade，旧 VAD/ASR/TTS hook 属于调试或兼容入口。
- [x] 4.2 更新 `src/components/AGENTS.md`，标明 `AssistantConsole` 主流程不得重新依赖 debug/compat hook。
- [x] 4.3 检查前端 imports 和调用点，确认 DebugToolsWindow 以外的主控制台路径没有回退到旧 debug 命令。

## 5. 验证

- [x] 5.1 运行 `pnpm test src/hooks`，确认 hook 行为测试通过。
- [x] 5.2 运行 `pnpm test src/components`，确认主控制台派生和渲染测试通过。
- [x] 5.3 运行 `pnpm build`，确认 TypeScript 与前端构建通过。
- [x] 5.4 运行 `openspec validate migrate-assistant-console-to-business-api --strict`，确认提案 artifacts 可用于实现。
