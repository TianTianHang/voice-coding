# MOSS TTS Runtime Environment

本文记录 MOSS TTS 运行时、性能测试和 ONNX Runtime profiler 相关环境变量。

## 模型路径

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `VOICE_CODING_MODEL_HOME` | `models` | 推荐的本地模型根目录。TTS 标准布局为 `$VOICE_CODING_MODEL_HOME/tts/moss-tts-nano-100m-onnx/`。 |
| `MOSS_TTS_MODEL_DIR` | `../models/moss-tts/MOSS-TTS-Nano-100M-ONNX` | 兼容入口，必须指向直接的 `MOSS-TTS-Nano-100M-ONNX` 组件目录。真实推理测试要求显式设置该变量。 |

当前 Rust 版 MOSS TTS 只加载 fixed 生成路径需要的 ONNX sessions：

- `tts.prefill`
- `tts.decode_step`
- `tts.local_fixed_sampled_frame`
- `codec.encode`
- `codec.decode_step`

不再加载 `tts.local_decoder`、`tts.local_cached_step` 或 `codec.decode_full`。

## ORT 线程与执行策略

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `MOSS_TTS_INTRA_THREADS` | `4` | ORT intra-op 线程数，影响单个算子内部并行。 |
| `MOSS_TTS_INTER_THREADS` | `1` | ORT inter-op 线程数。仅在 parallel execution 下对可并行图分支有意义。 |
| `MOSS_TTS_PARALLEL_EXECUTION` | `false` | 设为 `1/true/yes/on` 时启用 ORT parallel execution。 |
| `MOSS_TTS_MEMORY_PATTERN` | `true` | 控制 ORT memory pattern 优化。输入形状高度变化时可尝试关闭。 |

布尔变量接受 `1/true/yes/on` 和 `0/false/no/off`。

## 运行时 Trace

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `MOSS_TTS_TRACE` | 关闭 | 设为 true 时把项目级阶段耗时打印到 stderr，例如 `tts_prefill`、`tts_decode_step`、`codec_decode_step_buffered`。 |

项目级 trace 是粗粒度计时，适合快速看端到端阶段耗时；它不是 ORT kernel profiler。

## ONNX Runtime Profiler

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `MOSS_TTS_ORT_PROFILE` | 关闭 | 设为 true 时为每个 ORT session 调用 `with_profiling(...)`，合成结束后调用 `Session::end_profiling()` 写出 JSON。 |
| `MOSS_TTS_ORT_PROFILE_DIR` | 系统临时目录下的 `voice-coding-moss-ort-profiles` | ORT profiler JSON 输出目录。 |

示例：

```bash
nix develop -c env \
  MOSS_TTS_ORT_PROFILE=1 \
  MOSS_TTS_ORT_PROFILE_DIR=/tmp/moss-ort-profiles \
  MOSS_TTS_MODEL_DIR=/home/tiantian/project/voice-coding/models/tts/moss-tts-nano-100m-onnx/MOSS-TTS-Nano-100M-ONNX \
  cargo test -p tts-moss --test inference synthesizes_playback_ready_audio -- --ignored --nocapture --exact
```

Profiler 输出为 Chrome trace 兼容 JSON。可用 `chrome://tracing`、Perfetto，或脚本按 `cat`、`args.op_name`、`name` 汇总。开启 profiler 会显著增加耗时和文件体积，只建议性能分析时使用。

## 推理测试变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `MOSS_TTS_TEXT` | `你好，欢迎使用语音编程。` | `synthesizes_playback_ready_audio` 使用的测试文本。 |
| `MOSS_TTS_OUTPUT_WAV` | 未设置 | 设置后把测试合成结果写为 WAV。 |
| `MOSS_TTS_PERF_TEXT` | `你好，当前测试用于确认本机是否支持实时语音合成播放。` | 流式实时性能测试文本。 |
| `MOSS_TTS_PERF_WARMUP_TEXT` | `预热。` | 流式实时性能测试预热文本。 |
| `MOSS_TTS_PERF_CHUNK_MS` | `240` | 流式测试请求的音频 chunk 毫秒数。 |
| `MOSS_TTS_PERF_SEED` | `42` | 流式实时性能测试使用的固定随机种子，用于减少生成帧数波动。 |
| `MOSS_TTS_REALTIME_MAX_RTF` | `1.0` | 流式测试允许的最大实时率。 |
