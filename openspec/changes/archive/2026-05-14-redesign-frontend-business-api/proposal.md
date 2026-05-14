## Why

当前前端直接面对 ASR、VAD、TTS、ACP 等后端内部模块命令，主流程需要理解“先合成再播放”“VAD 事件后自动发 Agent”“TTS 播放期间暂停录音”等实现细节。准备完全重做前端时，需要先提供稳定、业务化的后端命令面，让新 UI 只围绕语音会话、Agent 回合、语音输出和应用状态构建。

## What Changes

- 新增面向前端的后端业务 API，将主流程抽象为应用运行时、语音输入会话、Agent 会话/回合、语音输出四个域。
- 新增统一状态快照与状态变更事件，使前端启动、恢复、重连和多面板展示都能从一个稳定契约读取状态。
- 将语音输入从“开始监听后自动转写并发送 Agent”的固定行为改为可配置输入模式：仅转写、自动发送、确认后发送。
- 将 TTS 从 `synthesize_tts` + `play_tts` 的底层两段式命令包装为 `speak_text`、`speak_agent_result`、`stop_speech` 等业务命令。
- 增加转写草稿提交、丢弃、编辑后提交的业务命令，支持新前端实现“听到的内容先确认再发送”。
- 保留现有底层命令作为兼容/调试入口，但新前端主流程应优先使用新的业务 API。
- **BREAKING**：新前端不再以旧事件 `transcript`、`error`、`tts-state` 作为唯一主状态源；它应改用新的业务状态事件。旧事件可在过渡期保留。

## Capabilities

### New Capabilities
- `frontend-business-api`: 面向重做前端的后端业务命令、状态快照和事件契约。

### Modified Capabilities
- `backend-vad`: 语音输入会话需要支持业务状态、暂停/恢复原因、输入模式和转写草稿生命周期。
- `real-time-vad-events`: VAD/STT 事件需要升级为语音会话与语音片段事件，携带 session 和 utterance 身份。
- `acp-client-runtime`: Agent 发送入口需要支持来源、回合状态、取消回合，以及从语音转写提交触发 Agent 回合。
- `backend-auto-tts`: 自动口播需要通过业务化语音输出命令和状态管理，避免前端直接编排合成与播放。

## Impact

- `src-tauri/src/lib.rs`：注册新的 Tauri 业务命令，并在过渡期保留旧调试命令。
- `src-tauri/src/vad_commands.rs`：拆出或包装语音会话业务层，补充输入模式、utterance 身份和暂停/恢复状态。
- `src-tauri/src/asr.rs`：继续负责模型加载和转写，向业务 API 提供状态与转写能力。
- `src-tauri/src/acp/session.rs`：支持带来源的消息发送、Agent 回合状态、取消回合和语音转写提交入口。
- `src-tauri/src/tts.rs`：提供 `speak_text`、`speak_agent_result`、`stop_speech` 等业务命令，修正播放期间暂停/恢复录音的编排。
- `src/`：重做前端时只消费新业务 API；旧 hooks 可删除或迁移为新 API 的适配层。
- 验证：实现阶段需要运行 Rust 测试与 clippy；前端接入后需要运行 `pnpm build`、`pnpm test`，并在影响 Tauri 集成时运行 `pnpm tauri build`。
