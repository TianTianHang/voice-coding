## Why

当前 MOSS ONNX TTS 引擎已经打通了基本非流式合成，但与官方 ONNX 推理代码相比仍缺少文本归一化、长文本切块、参考音频克隆、采样模式选择和实时 codec 解码等关键能力。Agent 回复一旦变长或包含数字、英文、标点混排，现有实现更容易出现发音不稳定、延迟偏高或无法复现的问题。

官方 MOSS-TTS-Nano 已发布独立 ONNX CPU 推理路径，并明确支持内置音色、参考音频、长文本自动切块与 Realtime Streaming Decode；现在对齐这些推理契约，可以让本地语音助手从“能播报”升级到“可持续用于真实对话”。

## What Changes

- 为 MOSS ONNX TTS 增加官方推理等价的文本准备流程，包括文本归一化、空白/标点处理和长文本按 token budget 切块。
- 扩展合成配置，使调用方可以选择 MOSS 采样模式；默认继续使用官方推荐的 fixed sampling，同时支持 deterministic greedy 模式用于测试和复现。
- 增加参考音频克隆路径：加载 codec encode 模型，将参考音频转为 prompt audio codes，并与现有内置音色 prompt 共享输入构建逻辑。
- 增加 codec streaming decode 内部管线，允许合成过程中分批解码音频 chunk；本次仍保持现有“合成完成后播放”的外部播放契约。该管线需要显式维护 `moss_audio_tokenizer_decode_step.onnx` 的 transformer offsets 与 attention cache tensors，并在状态初始化、状态更新或输出形状异常时走清晰 fallback/error 路径。
- 加固 ONNX session I/O 与 manifest/meta 一致性校验，覆盖 local sampling、codec encode、codec decode step 等当前未严格校验的模型。
- 将 CPU 密集型 MOSS 推理从 async 热路径移入阻塞 worker 或等效串行推理执行器，避免长文本合成拖慢 Tauri async runtime。
- 增加真实模型可选集成测试和无模型环境可运行的单元/fixture 测试，验证官方推理契约与现有 TTS runtime 状态机兼容。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `moss-onnx-tts-engine`: 扩展 MOSS ONNX 引擎的文本准备、采样模式、参考音频克隆、模型校验与推理执行要求。
- `streaming-tts-playback`: 明确 TTS 引擎可使用内部流式 codec 解码，同时保持当前合成完成后再播放的外部契约。

## Impact

- 主要影响 Rust 后端：`src-tauri/tts-moss/`、`src-tauri/tts-core/`、`src-tauri/src/tts.rs`，以及必要的 Tauri 命令/状态结构。
- 可能需要轻量扩展前端 TTS 测试入口，用于选择采样模式和传入参考音频；主语音交互 UI 不作为本次重点重做。
- 模型下载脚本需要继续确保 TTS 与 codec 两套 ONNX 资产完整，尤其是 `moss_audio_tokenizer_encode.onnx` 与 `moss_audio_tokenizer_decode_step.onnx`。
- 质量验证包括 `nix develop -c cargo test`、`nix develop -c cargo clippy`、可选真实模型集成测试、`pnpm build` 与 `pnpm test`；完整桌面回归可执行 `nix develop -c pnpm tauri build`。内部 streaming decode 的成功路径 SHOULD 至少通过真实模型 `#[ignore]` 测试验证，因为状态 cache tensor 的形状和更新语义依赖官方 ONNX 图。
