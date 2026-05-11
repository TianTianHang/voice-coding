## 1. 流式推理基础结构

- [x] 1.1 梳理 `MossSessions::generate_audio_frames`，抽出可复用的 frame generation loop，支持在每个 frame 生成并更新 TTS decode state 后回调。
- [x] 1.2 为 codec decode step 增加流式 sink 路径，使 pending frames 可按 batch 解码为 PCM chunk，并继续更新 `CodecDecodeStepState`。
- [x] 1.3 增加动态 frame budget helper，覆盖启动小 batch、缓冲领先后扩大 batch、最终 flush pending frames 的策略。
- [x] 1.4 保留现有批量 `synthesize` 行为和 fallback 语义，确保外部流式路径不静默回退到 `decode_full`。

## 2. MOSS StreamingTts Session

- [x] 2.1 在 `tts-moss` 中实现 `MossOnnxTtsEngine: StreamingTts` 和 MOSS stream session 类型。
- [x] 2.2 实现 session 文本 buffer、flush/final/natural-boundary 提交策略，并复用现有文本归一化、切块、分词、voice/reference audio 准备逻辑。
- [x] 2.3 使用 blocking worker 执行 ONNX 推理，并通过 channel 发送 `Started`、`TextBoundary`、`Progress`、`AudioChunk`、`End` 事件。
- [x] 2.4 实现 `next_event` 非阻塞 drain、`finish` 等待最终 `TtsResult`、`cancel` 停止后续 frame/batch 并丢弃未消费事件。
- [x] 2.5 为 `AudioChunk` 填充递增 sequence、48kHz 立体声 PCM、final 标记和基于累计样本数的时间范围。

## 3. 测试覆盖

- [x] 3.1 增加 frame budget helper 单元测试，覆盖阈值、metadata batch 上限和最终 flush。
- [x] 3.2 增加 codec streaming state 更新测试，覆盖缺失 state 输出时返回 `codec_decode_step` 阶段错误。
- [x] 3.3 增加 MOSS stream session 事件顺序测试，验证 `AudioChunk` 可在 `End` 前产生且 sequence 单调递增。
- [x] 3.4 增加 `finish` 与 `End(TtsResult)` 一致性测试。
- [x] 3.5 增加取消测试，验证取消后不再发送新的 `AudioChunk` 或 `End`。
- [x] 3.6 增加外部流式路径 codec decode step 失败时不 fallback 到 `decode_full` 的测试。

## 4. 验证

- [x] 4.1 运行 `openspec validate add-moss-official-streaming-tts --strict`。
- [x] 4.2 运行 `nix develop -c cargo test -p tts-core`。
- [x] 4.3 运行 `nix develop -c cargo test -p tts-moss`。
- [x] 4.4 运行 `nix develop -c cargo clippy -p tts-moss --all-targets`。
- [x] 4.5 运行 `nix develop -c cargo check -p voice-coding --features tts-moss-onnx`。
- [x] 4.6 记录无法运行的检查及原因，确保实现阶段最终状态可审计。

验证记录：以上命令均已运行通过。`openspec validate` 退出码为 0，但 OpenSpec CLI 在退出时尝试刷新 PostHog 遥测，受当前网络/DNS 限制输出了 `edge.openspec.dev` 解析失败日志；该日志不影响本地严格校验结果。
