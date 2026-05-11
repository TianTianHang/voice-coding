## 背景

当前 `tts-core` 已有批量合成接口，也有 `TtsSynthesisEvent` 和 `synthesize_stream` 的雏形，但 `synthesize_stream` 返回 `Vec<TtsSynthesisEvent>`，调用方只能在合成结束后一次性拿到事件，不能表达真正的边合成边消费。MOSS 引擎内部已经具备 codec decode step 分批产生 PCM chunk 的能力，但这些 chunk 仍只用于拼接最终 `TtsResult`。

## 目标

- 为 `tts-core` 增加真正的流式 TTS session 抽象。
- 支持调用方推入完整文本或增量文本，并通过 `next_event` 拉取进度、文本边界和音频 chunk。
- 保持现有 `TtsEngine::synthesize` 批量接口兼容。
- 明确默认播放路径仍可继续使用完整合成后播放，不强制 Tauri runtime、自动播报或前端改为边合成边播放。

## 非目标

- 本次不接入 Tauri 命令、播放队列、前端 UI 或自动播报流程。
- 本次不要求 `tts-moss` 实现外部可消费的流式音频输出。
- 本次不改变 48kHz 立体声 PCM 的播放格式契约。

## 验证

- `openspec validate add-streaming-tts-core-api --strict`
- `nix develop -c cargo test -p tts-core`
- `nix develop -c cargo check -p tts-moss`
