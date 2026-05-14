## Context

MOSS Rust 运行时的外部流式合成已经在 TTS frame generation loop 中把生成的 audio frames 送入 codec `decode_step`，并将每个 PCM batch 映射为 `AudioChunk`。当前 `FrameBudget` 使用 `stream.audioChunkMs` 或默认 PCM 目标来粗略决定 `1/2/4/8` frame batch，无法反映本机实时生成速度，也不会在接近或慢于实时的情况下主动增大后续 batch。

## Goals / Non-Goals

**Goals:**

- 首包前固定攒约 1.0s audio frames，避免首包被动态策略无限拖长。
- 首包后依据窗口 RTF 和已产音频 lead 调整目标缓冲和下一批 frame 数，在后续包上用更大 batch 换取更低 RTF。
- 保持公开配置、流式事件和最终播放契约不变。

**Non-Goals:**

- 不实现新的浏览器 WebAudio 排程层。
- 不把阈值和 batch 档位暴露为用户配置。
- 不改变非流式 `synthesize` 的 full decode 优先与 step fallback 行为。

## Decisions

1. 以 codec metadata 推导首包帧数。使用 `sample_rate / downsample_rate` 得到 frames per second，metadata 缺失或无效时回退到 `48000 / 3840 = 12.5`；`ceil(1.0s * fps)` 得到首包目标，并 clamp 到 codec decode step batch 上限。
2. 将自适应策略封装在 MOSS 内部 frame budget 结构中。该结构记录首批完成时间、上一批完成时间、累计输出音频时长、`adaptive_target_buffer_seconds` 和上一窗口 RTF，调用方只询问 `next_batch_size(flushing)` 并在每批完成后记录实际 PCM 样本数。
3. 首包后使用固定档位控制下一批 frame 数。`rtf_window >= 1.08` 或 lead 很低时倾向 32 frames；`rtf_window >= 0.98` 或 lead 偏低时倾向 24 frames；lead 恢复但仍不足时使用 8/16/24；lead 充足时使用 32。所有档位都 clamp 到 metadata batch 上限。
4. 目标缓冲只影响后续包等待策略。RTF 慢或 lead 低时把目标推向 1.5s、2.0s、3.0s；明显快于实时且 lead 充足时缓慢降低，但最低 0.8s。

## Risks / Trade-offs

- [首包延迟上升] → 首包固定为约 1.0s，换取减少后续断续；不会被慢 RTF 再次扩大。
- [短文本不足首包帧数] → 生成结束时 flush pending frames，短文本仍会发出最终 chunk。
- [metadata 字段差异] → metadata 不可用时使用已知默认 rate，避免策略初始化失败。
- [不同机器阈值不完美] → 先固化保守阈值并保留真实模型 ignored perf 测试用于本机调参。
