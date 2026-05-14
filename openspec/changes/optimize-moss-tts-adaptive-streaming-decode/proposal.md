## Why

当前 MOSS Rust 外部流式合成已经可以在 frame callback 中分批 codec decode 并发出 `AudioChunk`，但 batch 策略仍偏向按已产 PCM 粗略递增，不能根据真实生成速度和播放水位趋势调节。慢于实时或接近实时的机器容易出现后续碎片式断续；过早使用小 batch 也会牺牲吞吐。

## What Changes

- 将 MOSS 外部流式 codec decode batch 策略改为两段式自适应：首包前固定按约 1.0s 音频帧启动，首包后按窗口 RTF 和 lead 调整目标缓冲与下一批帧数。
- 从 codec metadata 推导每秒 frame 数，并将首包目标和后续 `8/16/24/32` 档位统一限制在 codec decode step 支持的 batch 上限内。
- 保持 `TtsConfig`、`AudioChunk` 事件、sequence、最终 `TtsResult` 和现有播放层契约不变。
- 增加策略单元测试与真实模型性能验证入口，确保首包不被动态扩张拖长，后续 batch 能随 RTF/lead 变化。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `moss-onnx-tts-engine`: 明确 MOSS 外部流式 codec decode 的两段式自适应 frame budget 行为。

## Impact

- 影响代码：`src-tauri/tts-moss` 的外部流式 codec decode 策略和相关测试。
- 不新增公开 API，不改变前端配置结构，不引入新依赖。
- 验证包括 OpenSpec strict 校验、`tts-core`/`tts-moss` Rust 测试、`tts-moss` clippy，以及可选真实模型实时性能测试。
