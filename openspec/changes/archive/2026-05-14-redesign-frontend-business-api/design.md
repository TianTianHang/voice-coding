## Context

当前后端已经具备 VAD 录音、Qwen3 ASR、ACP Agent 会话、MOSS TTS 与自动口播能力，但前端调用面仍按内部模块暴露：`start_listening`、`transcribe_audio_data`、`synthesize_tts`、`play_tts`、`send_agent_prompt` 等命令要求前端理解多个后端子系统的编排细节。完全重做前端时，主 UI 需要一个更稳定的业务契约：应用是否准备好、是否正在听、听到了哪一句、Agent 正在处理哪个回合、是否正在朗读，以及用户能如何停止、确认或重试。

这次变更不要求删除现有底层命令。它在现有 Rust/Tauri 后端上增加业务 API 层，并让新前端优先消费该层。

## Goals / Non-Goals

**Goals:**

- 提供面向新前端的业务命令：应用运行时、语音输入会话、Agent 会话/回合、语音输出。
- 提供统一状态快照和事件，使前端能够在启动、刷新、窗口重建后恢复完整 UI 状态。
- 将语音输入行为配置化，支持仅转写、自动发送 Agent、确认后发送三种模式。
- 将 TTS 编排收敛为朗读业务命令，后端负责合成、播放、打断、暂停/恢复 VAD。
- 保留旧命令作为调试和过渡兼容入口。

**Non-Goals:**

- 不在本变更中实现全新的前端视觉设计。
- 不替换 Qwen3 ASR、MOSS TTS 或 ACP SDK。
- 不引入新的远程服务或外部依赖。
- 不要求一次性删除旧 hook、旧事件和调试窗口。

## Decisions

### 决策 1：新增业务 API 层，而不是直接重命名旧命令

后端 SHALL 保留 ASR/VAD/TTS/ACP 内部模块，新增一个面向前端的业务命令层。业务层可以调用现有模块，但前端主流程不再直接编排底层模块。

备选方案是直接重命名旧命令并改前端调用。该方案改动少，但无法解决两段式 TTS、语音转写自动发送 Agent、错误事件过泛等核心问题。新增业务层能让前端重做和后端内部演进解耦。

### 决策 2：状态快照优先，事件只表示状态变化

每个业务域都提供 `get_*_status` 或统一 `get_app_status`，事件 payload 携带完整或可合并的状态。前端启动时先查询快照，再订阅事件。

备选方案是让前端完全依赖事件重放。Tauri 本地事件没有持久队列，窗口关闭或未订阅时事件会丢弃，因此快照优先更可靠。

### 决策 3：语音输入使用 utterance 身份管理

每段完成的语音转写生成稳定 `utteranceId`，并带有 `sessionId`。转写可以被自动发送、等待确认、编辑后发送或丢弃。

备选方案是继续只发 `transcript` 字符串。该方案不支持确认前编辑，也无法可靠地把转写、Agent 回合和错误关联起来。

### 决策 4：Agent 消息使用 turn 身份管理

每次发送到 Agent 的用户消息生成 `turnId`，Agent 事件和状态更新尽量关联该 `turnId`。语音转写、手动输入、编辑后提交和重试都通过同一个 `send_agent_message` 业务入口进入。

备选方案是继续把语音 prompt 和手动 prompt 分成不同命令。该方案会让前端有两套消息状态机，长期更难维护。

### 决策 5：TTS 对前端暴露为 SpeechOutput

前端调用 `speak_text`、`speak_agent_result`、`stop_speech`，后端内部决定是否先合成再播放、是否暂停录音、是否打断已有播放。旧 `synthesize_tts` 和 `play_tts` 保留为调试入口。

备选方案是让前端继续先合成再播放。该方案适合调试模型，但不适合主业务 UI，因为前端会被迫处理缓冲音频、播放打断和 VAD 恢复。

## Risks / Trade-offs

- [Risk] 新旧命令和事件并存导致维护面变大 → 通过明确标记旧命令为兼容/调试入口，并让新前端只消费业务 API 降低复杂度。
- [Risk] 状态类型一次设计过大，初期实现成本上升 → 先实现主流程字段，保留可选字段用于后续扩展。
- [Risk] `confirmBeforeSend` 增加用户操作步骤 → 将输入模式设为配置项，允许用户选择 `autoSendToAgent` 获得更流畅体验。
- [Risk] Agent SDK 未直接支持取消当前回合 → `cancel_agent_turn` 可先实现为 best-effort 状态标记和后续结果忽略，后续再接入 SDK 原生取消能力。
- [Risk] 事件迁移影响旧前端测试 → 过渡期保留旧事件，并新增业务事件测试覆盖新前端契约。

## Migration Plan

1. 增加业务状态类型和命令注册，旧命令保持可用。
2. 将现有 VAD/STT 完成路径包装为 VoiceSession 事件和 utterance 管理。
3. 将现有 ACP 发送路径包装为 AgentMessage/AgentTurn 管理。
4. 将现有 TTS 自动播报和手动播报包装为 SpeechOutput 管理，并修正停止播放后的 VAD 恢复路径。
5. 新前端只接入业务 API；旧 hooks 在迁移完成后删除或降级为调试适配。
6. 当新前端稳定后，再单独提案移除旧主流程命令和事件。

## Open Questions

- `confirmBeforeSend` 是否作为默认模式，还是默认沿用 `autoSendToAgent`。
- `cancel_agent_turn` 第一版是否需要强制终止 agent 子进程，还是只忽略当前回合后续事件。
- 旧事件的兼容期持续多久。
