## Why

当前项目的 TTS 后端仍使用 mock 引擎，缺少可落地的本地离线语音合成能力。为支持稳定、低依赖的桌面端播报体验，需要接入 MOSS ONNX 模型并与现有 TTS runtime、播放链路和 VAD 暂停/恢复机制打通。

## What Changes

- 新增 MOSS ONNX TTS 引擎实现，替换默认 mock 合成路径为可配置的真实推理路径。
- 新增模型资产加载与校验流程，覆盖 manifest、meta、onnx、external data、tokenizer 的一致性检查。
- 新增 `TtsConfig.voice` 到内置音色的映射策略（含默认音色与未知音色回退/报错策略）。
- 新增“文本 -> 音频 token -> codec decode_full -> 48kHz 立体声 PCM”的非流式推理闭环。
- 新增可测试的错误分类与健康检查能力，确保模型缺失、路径错误、推理失败时可观测。

## Capabilities

### New Capabilities
- `moss-onnx-tts-engine`: 在 Rust 后端提供基于 MOSS ONNX 的本地 TTS 推理能力，包含模型校验、音色映射、非流式合成与运行时集成。

### Modified Capabilities
- 无。

## Impact

- 后端 Rust 代码将新增 MOSS 引擎模块，并调整 `src-tauri/src/tts.rs` 的引擎注入与初始化逻辑。
- 可能新增 ONNX Runtime 相关依赖与模型配置读取逻辑。
- 测试面将扩展到模型资产校验、推理输出契约（48kHz/2ch）、错误路径与 runtime 状态转换。
- 实现完成后需运行 `cargo test`、`cargo clippy`、`pnpm tauri build`，并执行前端 `pnpm build`、`pnpm test` 做回归确认。
