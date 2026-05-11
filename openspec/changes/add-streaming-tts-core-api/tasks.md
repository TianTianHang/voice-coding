## 1. OpenSpec

- [x] 1.1 新增 `streaming-tts-playback` 的流式 session 抽象 requirement。
- [x] 1.2 新增 `moss-onnx-tts-engine` 的外部流式事件映射 requirement。
- [x] 1.3 运行 `openspec validate add-streaming-tts-core-api --strict`。

## 2. tts-core API

- [x] 2.1 新增 `TtsStreamConfig` 并挂到 `TtsConfig::stream`，默认值为 `None`。
- [x] 2.2 新增 `StreamingTextChunk`、`TtsSynthesisStarted`、`TtsTextBoundary` 等流式事件类型。
- [x] 2.3 扩展 `TtsAudioChunk` 和 `TtsSynthesisEvent`，保留既有事件变体兼容。
- [x] 2.4 新增 `StreamingTts` 和 `StreamingTtsSession` trait，并从 `lib.rs` re-export。
- [x] 2.5 保留 `TtsEngine::synthesize_stream` 默认 unsupported 行为。

## 3. 测试与验证

- [x] 3.1 为 `TtsConfig::default().stream == None` 增加测试。
- [x] 3.2 为 `StreamingTextChunk` final flush 和普通增量语义增加测试。
- [x] 3.3 为扩展后的 `TtsAudioChunk` 音频格式校验增加测试。
- [x] 3.4 为默认 unsupported 流式接口错误增加测试。
- [x] 3.5 运行 `nix develop -c cargo test -p tts-core`。
- [x] 3.6 运行 `nix develop -c cargo check -p tts-moss`。
