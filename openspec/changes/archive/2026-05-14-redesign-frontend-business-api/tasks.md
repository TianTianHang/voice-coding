## 1. 后端业务 API 基础

- [x] 1.1 定义 `AppStatus`、`VoiceSessionStatus`、`VoiceUtteranceEvent`、`AgentTurnStatus`、`SpeechOutputStatus`、`RuntimeErrorEvent` 等业务 DTO，并补充 serde 序列化测试。
- [x] 1.2 新增或整理后端业务 API 模块，注册 `get_app_status`、`prepare_app`、`get_app_preferences`、`set_app_preferences` 等应用运行时命令。
- [x] 1.3 为业务状态事件增加统一 emit helper，覆盖 `app-status-changed`、`voice-session-changed`、`agent-status-changed`、`agent-turn-changed`、`speech-output-changed` 和 `runtime-error`。

## 2. 语音输入会话

- [x] 2.1 在现有 VAD 录音状态之上实现 `start_voice_session`、`stop_voice_session`、`pause_voice_session`、`resume_voice_session`、`get_voice_session_status` 和 `update_voice_session_config`。
- [x] 2.2 增加语音输入模式配置：`dictationOnly`、`autoSendToAgent`、`confirmBeforeSend`。
- [x] 2.3 为每段完成转写生成 `utteranceId`，发布 `voice-utterance` 生命周期事件，并保留旧 `transcript`/`vad-state` 兼容事件。
- [x] 2.4 实现 `submit_transcript_to_agent`、`edit_and_submit_transcript`、`discard_transcript`，覆盖自动发送、确认后发送、编辑后发送和丢弃路径。

## 3. Agent 会话和回合

- [x] 3.1 将现有 `send_agent_prompt` 包装为 `send_agent_message`，支持 `source`、可选 `utteranceId` 和返回 `turnId`。
- [x] 3.2 维护 Agent turn 状态，发布 `agent-turn-changed`，并尽量把后续 `agent-event` 关联到当前 `turnId`。
- [x] 3.3 实现 `cancel_agent_turn`，第一版至少支持标记取消并忽略该回合后续结果；如 SDK 支持原生取消则接入原生取消。
- [x] 3.4 保持 `connect_agent`、`disconnect_agent`、`get_agent_status` 和 `respond_agent_confirmation` 兼容，同时让状态进入新的业务快照。

## 4. 语音输出业务层

- [x] 4.1 实现 `speak_text`、`speak_agent_result`、`stop_speech`、`get_speech_status` 和 `set_speech_preferences`。
- [x] 4.2 将现有自动口播状态映射为 `SpeechOutputStatus`，并发布 `speech-output-changed`。
- [x] 4.3 修正 TTS 播放停止或失败后的 VAD 恢复逻辑，恢复同一个暂停会话，不通过重新 `start` 分配新 session。
- [x] 4.4 保留 `synthesize_tts`、`play_tts`、`cancel_tts_playback` 等底层命令作为调试入口。

## 5. 前端适配准备

- [x] 5.1 增加新业务 API 的 TypeScript 类型定义或 hook 草稿，供重做前端调用。
- [x] 5.2 标记旧 hooks 的兼容用途，避免新前端继续依赖旧 `transcript`、`error`、`tts-state` 作为主状态源。
- [x] 5.3 如本轮包含前端重做入口，迁移启动流程为 `get_app_status`、`prepare_app` 和业务事件订阅。

## 6. 测试和验证

- [x] 6.1 增加 Rust 单元测试，覆盖状态 DTO 序列化、语音输入模式、utterance 提交/丢弃、Agent turn 状态和 SpeechOutput 状态。
- [x] 6.2 增加 Rust 集成或命令测试，覆盖语音转写到 Agent 提交流程、TTS 播放期间暂停/恢复 VAD、停止语音输出恢复原会话。
- [x] 6.3 运行 `nix develop -c cargo test` 并记录结果。
- [x] 6.4 运行 `nix develop -c cargo clippy` 并记录结果。
- [x] 6.5 若修改前端代码，运行 `pnpm build` 和 `pnpm test` 并记录结果。
- [x] 6.6 若命令注册或 Tauri 集成发生变化，运行 `nix develop -c pnpm tauri build`，或记录无法运行的明确阻塞原因。
- [x] 6.7 运行 `openspec validate redesign-frontend-business-api --strict`，确保规格可归档。
