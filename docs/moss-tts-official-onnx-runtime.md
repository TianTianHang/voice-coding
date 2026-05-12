# MOSS-TTS-Nano 官方 ONNX 推理流程摘要

本文总结 OpenMOSS 官方 `onnx_tts_runtime.py` 与 `ort_cpu_runtime.py` 的推理实现，重点用于对照本项目 Rust/Tauri 版 MOSS TTS 运行时。

参考源码：

- `onnx_tts_runtime.py`: <https://github.com/OpenMOSS/MOSS-TTS-Nano/blob/main/onnx_tts_runtime.py>
- `ort_cpu_runtime.py`: <https://github.com/OpenMOSS/MOSS-TTS-Nano/blob/main/ort_cpu_runtime.py>

## 模块分工

官方实现分为两层：

- `OnnxTtsRuntime`：面向用户的高层封装，负责模型目录检查/下载、文本归一化、分句分块、SentencePiece 编码、参考音频加载/编码、full decode fallback、流式 chunk 组织和最终 WAV 写出。
- `OrtCpuRuntime`：核心 ONNX 推理层，负责创建 ORT sessions、构造 prompt rows、生成音频 token frames、维护 codec streaming decode state。

`OnnxTtsRuntime` 继承 `OrtCpuRuntime`。因此 TTS token 生成、prompt rows 构造、codec step session 状态维护等核心逻辑在 `ort_cpu_runtime.py`；文本归一化、文本分块、参考音频加载/编码、full decode fallback、流式 chunk 组织和 WAV 写出在 `onnx_tts_runtime.py`。

## 模型与 Session

官方从 `browser_poc_manifest.json` 读取模型清单，再加载 TTS 与 codec 的 meta 文件。核心 sessions 包括：

- TTS:
  - `prefill`
  - `decode`，对应文件清单里的 `decode_step`
  - `local_decoder`
  - 可选 `local_greedy_frame`
  - 可选 `local_fixed_sampled_frame`
  - 可选 `local_cached_step`
- Codec:
  - `codec_encode`
  - `codec_decode`，即 full decode
  - `codec_decode_step`，即 streaming/incremental decode

ORT session 默认启用 `ORT_ENABLE_ALL` 图优化，CPU 下使用 `CPUExecutionProvider`，CUDA 下优先 `CUDAExecutionProvider` 并回退 CPU。

## 文本处理与分块

高层 `synthesize()` 会先调用文本归一化流程：

1. 可选 WeText 规范化。
2. 可选 TTS 文本归一化。
3. 按句末标点、分句标点、token budget 切分文本。
4. 每个文本 chunk 单独合成。
5. 多个文本 chunk 之间插入静音 pause。

默认 voice clone 文本 chunk token 上限是 `75`。短英文会做首字母大写、补句号、短句前置空格等处理；CJK 文本会补中文句号。

## Prompt Rows 构造

`build_voice_clone_request_rows()` 把文本 token 与参考音频 codes 拼成 TTS 输入 rows。

每一行宽度为：

```text
n_vq + 1
```

第 0 列是文本 token 或音频 slot token，后续列是音频 codebook token。流程是：

1. `user_prompt_prefix_token_ids`
2. `audio_start_token_id`
3. 参考音频 codes，使用 `audio_user_slot_token_id`
4. `audio_end_token_id`
5. `user_prompt_after_reference_token_ids`
6. 当前文本 token ids
7. `assistant_prompt_prefix_token_ids`
8. `audio_start_token_id`

`attentionMask` 是单行二维数组，长度等于 rows 数量，全部为 1。

## 音频 Token 生成

`generate_audio_frames()` 是 TTS token 生成主循环。

1. 先运行 `prefill(input_ids, attention_mask)`。
2. 取 `global_hidden` 的最后位置作为当前 hidden。
3. 从 prefill outputs 中把 `present_*` 转成下一步需要的 `past_*`。
4. 循环最多 `max_new_frames` 次：
   - 通过 local decoder 生成一帧音频 token。
   - 如果 local decoder 返回停止标志或采样到非 assistant slot，则结束。
   - 把生成的 frame 作为一个新的 assistant audio row 输入 `decode_step`。
   - 更新 `global_hidden` 与 TTS KV cache。
   - 如果传入 `on_frame` 回调，则在当前帧已追加到 generated frames，且 TTS `decode` 已更新 hidden/KV 后回调。

官方支持几种 local 生成路径，优先级大致是：

- 有 `local_greedy_frame` 且非采样模式：直接用图内 greedy frame。
- 有 `local_fixed_sampled_frame` 且 sample mode 是 fixed：直接用图内 fixed sampled frame。
- 有 `local_cached_step`：逐 codebook channel 使用 cached local step。
- 否则使用普通 `local_decoder`，逐 channel host-side 采样。

所有路径都会维护每个 codebook channel 已出现 token 集合，用于 repetition penalty。

## 非流式 Codec Decode

非流式路径调用 `decode_full_audio()`：

1. 将所有 generated frames 组成形状 `[1, frame_count, num_quantizers]` 的 `int32 audio_codes`。
2. `audio_code_lengths = [frame_count]`，dtype 为 `int32`。
3. 运行 `codec_decode`。
4. 读取输出：
   - `audio`
   - `audio_lengths`
5. `audio` 是 channel-major 格式，形状类似 `[1, channels, samples]`。
6. 只取 `audio_lengths` 指定范围内的样本。
7. 高层再把多个 channel merge 成 `[samples, channels]` 的 waveform。

`OnnxTtsRuntime.decode_full_audio_safe()` 会优先使用 full decode。如果 full decode 抛错，则 fallback 到 streaming decode，每 8 帧一批跑 `codec_decode_step`，最后拼接。

## 流式 Codec Decode 状态

官方封装了 `CodecStreamingDecodeSession` 来维护 `codec_decode_step` 的状态。

初始化时读取 `codec_meta["streaming_decode"]`：

- `transformer_offsets`
- `attention_caches`

`reset()` 初始化 state feeds：

- transformer offset: `int32` 全 0
- attention offset: `int32` 全 0
- cached keys: `float32` 全 0
- cached values: `float32` 全 0
- cached positions: `int32` 全 -1

`cached_positions` 初始为 `-1` 是关键行为。它表示 cache 位置尚未有效；如果误初始化为 0，streaming decoder 可能把空 cache 当成有效位置。

每次 `run_frames(frame_rows)`：

1. 构造 `audio_codes`，形状 `[1, frame_count, num_quantizers]`。
2. 构造 `audio_code_lengths = [frame_count]`，dtype 为 `int32`。
3. 合并当前 `state_feeds` 作为输入。
4. 运行 `codec_decode_step`。
5. 根据 session output names 组装 named outputs。
6. 用输出的 offset、keys、values、positions 覆盖下一次 state feeds。
7. 返回 `(audio, audio_length)`。

官方 streaming state 是跨 frame batch 连续复用的，但每个文本 chunk 合成前后会 reset。

## 流式合成流程

`synthesize_single_chunk(streaming=True)` 的核心流程：

1. 创建 `pending_decode_frames`。
2. 创建 `emitted_chunks`。
3. `codec_streaming_session.reset()`。
4. 调用 `generate_audio_frames(request_rows, on_frame=on_frame)`。
5. 每次 `on_frame` 收到一帧：
   - append 到 `pending_decode_frames`
   - 调用 `decode_pending_frames(False)`
6. 生成结束后调用 `decode_pending_frames(True)` 强制 flush。
7. finally 中再次 reset streaming session。
8. 将 emitted chunks concat 成最终 waveform。

官方流式 batch 大小不是固定值，而是根据“已发音频时长 - 已耗实时”的 lead time 动态决定：

- 尚未发出首段音频，或 lead < 0.20s：1 帧
- lead < 0.55s：2 帧
- lead < 1.10s：4 帧
- 否则：8 帧

也就是说官方先尽快出声，随后随着缓冲领先量变大而逐步增大 decode batch。

## 流式 PCM 拼接

官方每次 `codec_decode_step` 返回：

- `audio`
- `audio_length`

处理方式：

1. 如果 `audio_length <= 0`，丢弃。
2. 首次成功发出音频时记录当前时间。
3. `emitted_samples_total += audio_length`。
4. 对每个 channel 取 `audio[0, channel_index, :audio_length]`。
5. merge channels 得到 `[samples, channels]`。
6. append 到 `emitted_chunks`。

官方代码没有显式 crossfade 或 overlap-add。它依赖 `codec_decode_step` 的 cache state 正确维护，使每次返回的 `audio[:audio_length]` 可以直接拼接。

## 参考音频编码

如果提供 `prompt_audio_path`：

1. 用 torchaudio 加载。
2. 转 `float32`。
3. 重采样到 codec meta 中的 sample rate。
4. 调整声道数：
   - mono 到 stereo：repeat
   - multi 到 mono：mean
   - 其他不支持的转换报错
5. 输入 `codec_encode(waveform, input_lengths)`。
6. 按 `audio_code_lengths` 取有效 code frames。

如果没有参考音频，则使用 manifest 里的 builtin voice prompt codes。

## 与 Rust 实现对照时的关键注意点

1. `cached_positions` 初始值必须是 `-1`，不是 `0`。
2. `audio_code_lengths` 在 full decode 与 step decode 中都是 `int32 [frame_count]`。
3. `audio` 输出是 channel-major，需要按 `[channel][sample]` 转成交错 PCM。
4. 每次 step decode 后，所有 state 输出都必须完整覆盖下一次输入 state。
5. streaming session 应在每个独立文本 chunk 开始前 reset，结束后也 reset。
6. 官方没有对 streaming chunk 做显式裁剪或 crossfade；如果声音糊，优先检查 cache state、positions、offsets、output name 映射和 dtype。
7. 官方动态 batch 依据 realtime lead，而不是单纯按总已生成 PCM 或固定 chunk 毫秒。
8. 多文本 chunk 之间官方会插入静音 pause；这不同于单 chunk 内部的 streaming PCM 拼接。

## 排查流式音质问题的优先级

若非流式正常而流式模糊，建议按以下顺序排查：

1. 对齐 `CodecStreamingDecodeSession.reset()`，尤其是 cached positions = -1。
2. 确认 Rust 读取的 `streaming_decode` metadata 与官方 meta 完全一致。
3. 确认 state output name 到下一轮 input name 的映射没有错位。
4. 确认 `audio_lengths` 使用的是 step decode 输出的有效长度，而不是完整 tensor 长度。
5. 用同一组 generated frames 比较：
   - 官方 Python `codec_decode_step`
   - Rust `codec_decode_step`
   - 官方 `codec_decode`
   - Rust `codec_decode_full`
6. 如果 step decode 单独仍与 full decode 差异明显，再考虑 batch 策略或模型导出本身的限制。
