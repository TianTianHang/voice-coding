## 1. 后端自动播报协调器

- [x] 1.1 在后端增加自动播报状态与去重记录，保存启用状态、播放中状态、最近 result 内容/标识、最近播报状态。
- [x] 1.2 在 Agent 最终 result 事件路径上接入自动播报触发，只对 `resultEvent.content` 生效。
- [x] 1.3 复用现有 `TtsRuntime::synthesize` 与 `play_tts` 路径完成合成与播放，并确保播放时暂停/恢复 VAD。

## 2. 前端介入命令与状态

- [x] 2.1 暴露自动播报启停命令与状态查询命令，供前端控制与展示。
- [x] 2.2 暴露停止当前播报命令与重播最近 result 命令，支持前端介入。
- [x] 2.3 在 `AssistantConsole` 中增加自动播报状态与控制入口，保留现有手动 TTS 测试控件。

## 3. 事件与命令接线

- [x] 3.1 在 `src-tauri/src/lib.rs` 注册新增 TTS/自动播报命令与相关 state。
- [x] 3.2 如有必要，补充 Agent/TTS 状态事件，让前端能区分“自动播报关闭”和“当前未播放”。
- [x] 3.3 确认现有 `agent-event`、`tts-state`、`transcript` 事件契约未被破坏。

## 4. 测试与验证

- [x] 4.1 补充 Rust 单测，验证仅 result 触发播报、重复 result 不重复播报、停止后能恢复。
- [x] 4.2 补充前端测试，验证自动播报状态显示与控制入口逻辑。
- [x] 4.3 运行 `cargo test`、`cargo clippy`、`pnpm test`、`pnpm build`，并记录结果。
- [x] 4.4 如桌面集成受影响，运行 `pnpm tauri build` 做最终验证。
