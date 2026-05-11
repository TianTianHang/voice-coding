## Why

`tts-core` 已经具备真正的流式 session 契约，但 MOSS ONNX TTS 目前仍只对外提供批量合成。官方 `ort_cpu_runtime.py` 展示了更低延迟的实现路径：TTS 每生成 audio frame 后可回调推动 codec streaming decode，从而在完整文本合成结束前产出 PCM chunk。

## What Changes

- 为 MOSS ONNX TTS 增加对 `StreamingTts` / `StreamingTtsSession` 的真实实现。
- 按官方 runtime 的思路，将 TTS frame generation loop 与 codec decode step 连接起来，边生成 audio frames 边产出 `AudioChunk` 事件。
- 引入 MOSS 流式 session 内部状态，包括文本 flush 策略、推理 worker、事件队列、取消标记、PCM chunk 聚合和最终 `TtsResult`。
- 使用官方动态 frame budget 思路控制 codec decode step 的 batch 大小，在首包延迟和吞吐之间取得平衡。
- 保持现有批量 `synthesize` 和默认播放策略不变；上层 runtime 仍可选择等待完整 `TtsResult` 后播放。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `moss-onnx-tts-engine`: MOSS 引擎需要从“尚未实现外部流式时返回 unsupported”推进为“按官方 frame callback + codec streaming decode 模式产出外部流式音频事件”。
- `streaming-tts-playback`: 流式 session 的 MOSS 实现需要满足 core 层事件生命周期、最终结果一致性、取消和错误语义。

## Impact

- Rust 后端：
  - `src-tauri/tts-moss/src/engine.rs`
  - `src-tauri/tts-moss/src/sessions.rs`
  - `src-tauri/tts-moss/src/codec_buffer.rs`
  - 可能新增 MOSS stream session 辅助模块。
- Core API 不预期破坏性变更；如发现现有事件字段不足，应优先通过 MOSS 内部元数据补齐，而不是扩大 core 契约。
- 需要增加 Rust 单元测试覆盖事件顺序、chunk 格式、finish/End 一致性、取消、codec streaming state 更新和 fallback/错误行为。
- 验证期望：
  - `openspec validate add-moss-official-streaming-tts --strict`
  - `nix develop -c cargo test -p tts-core`
  - `nix develop -c cargo test -p tts-moss`
  - `nix develop -c cargo check -p voice-coding --features tts-moss-onnx`
