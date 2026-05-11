## Context

`tts-core` 已经归档了流式 session 契约，`TtsSynthesisEvent::AudioChunk` 可以表达可即时消费的 PCM chunk。当前 MOSS ONNX TTS 引擎已经加载 TTS 与 codec 两套 ONNX sessions，并具备两类解码路径：

- `decode_full`：完整 audio frames 一次性解码为完整 PCM。
- `decode_step_buffered`：将已生成的完整 audio frames 按 batch 调用 codec decode step，内部得到 PCM chunk，最终再拼成完整 `TtsResult`。

官方 `ort_cpu_runtime.py` 的实现更进一步：TTS frame generation loop 支持在每生成一个 audio frame 后触发回调，codec streaming decode session 持有 cache/state，并可对新增 frames 调用 decode step 产出 PCM。这个设计将 MOSS 实现从“完整 frames 后分批解码”推进为“边生成 frames 边流式解码”。

## Goals / Non-Goals

**Goals:**

- 为 `MossOnnxTtsEngine` 实现 `StreamingTts`，让调用方可以启动 MOSS 流式 session。
- 复刻官方 runtime 的关键数据流：TTS 每生成 audio frame 后进入 pending frame buffer，满足 frame budget 时调用 codec decode step 并发送 `AudioChunk`。
- 保持 `finish` 返回的最终 `TtsResult` 与事件流 `End(TtsResult)` 内容一致。
- 保持 ONNX 推理在 blocking worker 或专用推理线程中执行，不阻塞 Tauri async runtime。
- 支持取消，避免取消后继续发送新事件或继续处理后续 frame batch。

**Non-Goals:**

- 不改变 `tts-core` trait 形状，除非实现中证明当前字段无法表达必要语义。
- 不要求上层 Tauri runtime 立即边合成边播放；默认播放策略仍可等待完整结果。
- 不改变批量 `synthesize` 的外部行为。
- 不新增模型资产格式，继续依赖当前 manifest/meta 中的 codec streaming metadata。

## Decisions

### 1. 采用官方 frame callback 风格，而不是“完整 frames 后再切 chunk”

实现应在 `MossSessions` 中抽出一个流式合成入口，结构上复用 `generate_audio_frames` 的 prefill、local sampling 和 `tts_decode_step` 逻辑，但在每个 frame 生成并完成 TTS decode state 更新后，将 frame 交给流式 sink。

可选方案是先生成完整 `generated_frames`，再调用 `decode_step_buffered` 映射事件。该方案改动更少，但首包延迟仍包含完整 frame generation 时间，不能体现官方 runtime 的低延迟路径。因此本变更选择官方 callback 风格。

### 2. codec streaming state 独立于批量 decode buffer

流式入口应为每个文本合成片段创建新的 `CodecDecodeStepState`，并在每次 codec decode step 后用输出 state 更新下一次输入。PCM chunk 同时写入事件队列和 `PcmChunkBuffer`，最终由 buffer 拼接成 `TtsResult`。

批量 `synthesize` 可继续使用现有 fallback 行为；外部流式 session 不应在已经开始发送 `AudioChunk` 后 fallback 到 `decode_full`，否则调用方无法得到一致的事件语义。

### 3. 使用动态 frame budget 控制 chunk 大小

参考官方 runtime 的思路，根据当前已产出音频时长相对生成进度的领先量选择 codec decode step batch 大小：

- 启动阶段偏向 1 frame，以降低首包延迟。
- 缓冲领先增加后逐步扩大到 2、4、8 frames 或 metadata 支持的上限，以改善吞吐。
- 最终 flush 时必须解码所有 pending frames。

具体阈值可以先采用官方近似值，并限制在 `CodecDecodeStepState.batch_size` 与 `TtsStreamConfig.audio_chunk_ms` 可表达范围内。

### 4. session 使用 channel 桥接 async 调用方和 blocking 推理 worker

`StreamingTtsSession` 保持 async 侧对象，内部维护：

- 文本输入 buffer 和 final/flush 状态。
- `tokio::sync::mpsc` 事件接收端。
- blocking worker join handle。
- `Arc<AtomicBool>` cancel flag。
- 最终结果共享槽，用于 `finish` 与 `End` 一致性。

worker 侧串行持有 MOSS ONNX sessions，并通过事件 sender 发送 `Started`、`TextBoundary`、`Progress`、`AudioChunk` 和 `End`。

### 5. 文本 flush 以 MOSS chunk 为执行边界

`push_text` 收到增量文本后，应根据 `TtsStreamConfig`、标点、`flush` 和 `is_final` 决定何时提交可朗读文本。提交后仍经过现有 MOSS 文本归一化、空片段过滤、长文本切块、voice/reference audio 解析和 tokenization。每个实际进入 ONNX 推理的 `PreparedTextChunk` 都应产生对应 `TextBoundary`。

## Risks / Trade-offs

- [Risk] ONNX 单次 `run` 无法中断，取消不会立即停止当前推理调用 → 在每次 frame loop 和 codec batch 边界检查 cancel flag，并保证取消后不再发送新音频事件。
- [Risk] 动态 frame budget 过小会增加 ONNX 调用次数，过大会提高首包延迟 → 初始采用官方阈值并加配置约束，后续通过实测调参。
- [Risk] codec decode step 和 full decode 在数值上可能有细微差异 → 流式路径以 decode step 输出为准，测试关注格式、顺序、非空、最终拼接一致性，而不是与 full decode bit-exact。
- [Risk] 文本增量 flush 可能导致过短片段发音不自然 → 默认仍优先自然边界和最小字符数，`flush=true` 才强制提交当前 buffer。
- [Risk] 共享 ONNX sessions 串行化会限制并发 session → 与当前批量合成保持一致，先保证线程安全和 tensor 生命周期正确；后续如需要可设计 session pool。

## Migration Plan

1. 保留现有批量合成路径和测试，新增流式入口与测试。
2. 在 MOSS 引擎上实现 `StreamingTts`，让原本 unsupported 的流式调用变为真实 session。
3. 默认 Tauri 播放 runtime 不变，后续再单独接入边合成边播放。
4. 如流式路径出现运行时问题，可临时让 `start_stream` 返回明确 unsupported；批量播放不受影响。

## Open Questions

- 是否需要将官方动态 frame budget 的阈值暴露为 MOSS 专属配置，还是先固定为内部策略？
- `TextBoundary` 的 `start/end` 是否应指向原始输入文本 byte range，还是归一化后文本 range？建议先使用原始输入 range，并在实现中记录映射能力不足的场景。
- 是否需要为 `next_event` 增加等待式 wrapper，避免上层轮询？本变更不改变 core trait，可在 runtime 层后续补充。
