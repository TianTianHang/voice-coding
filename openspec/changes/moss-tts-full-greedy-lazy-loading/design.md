## Context

MOSS TTS 当前实现加载 `prefill`、`decode_step`、`local_fixed_sampled_frame`、`codec.encode` 和 `codec.decode_step`，并且非流式 `synthesize` 也使用 decode step 拼接完整音频。这降低了内存，但与规格中“非流式优先 `decode_full`”和“支持 greedy 采样”的要求不一致。

## Decisions

### 1. 懒加载 session 容器

`MossSessions` 保持单 mutex 串行访问，但每个 ONNX session 存为 `Option<Session>`。公共 TTS session、fixed session、greedy local decoder、codec encode、codec full 和 codec step 分别在首次使用时创建并校验 I/O。`health_check` 只校验资产和 tokenizer 已加载，不预热 ONNX session。

### 2. Greedy 使用 local_decoder

`samplingMode: "greedy"` 不依赖 `local_greedy_frame`。实现使用 `local_decoder` 对当前 `global_hidden` 运行本地解码，并对 logits 执行 argmax，生成 deterministic audio frame。若模型包缺少 `local_decoder`，请求 greedy 时返回包含 `tts.local_decoder` 的明确错误。

### 3. Full decode 只用于非流式

非流式 `synthesize` 先生成完整 audio frames，再优先调用 codec `decode_full`。如果 `decode_full` 缺失或推理失败，则 fallback 到 `decode_step_buffered`。外部流式 session 继续使用 codec `decode_step`，不回退到 full decode。

### 4. 资产校验分层

资产加载阶段只要求核心文件可用：`prefill`、`decode_step`、`local_fixed_sampled_frame` 和 codec `decode_step`。`local_decoder`、codec `encode` 与 codec `decode_full` 允许存在于文件映射中但不强制要求；对应能力首次使用时再校验并加载。

## Risks

- `local_decoder` 的具体输入输出命名依赖 meta；实现必须基于 session I/O 和保守错误处理，避免 silent fallback 到 fixed。
- `decode_full` 和 `decode_step` 的音频时长可能有数值差异；测试关注可播放契约、非空和 deterministic greedy。
