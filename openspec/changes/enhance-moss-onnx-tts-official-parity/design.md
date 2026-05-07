## Context

当前 `tts-moss` 已经实现 MOSS ONNX 基本链路：读取 manifest/meta、加载 tokenizer 与 ONNX session、使用内置音色 prompt 构造输入、执行 prefill + fixed local sampling + global decode step，最后通过 codec `decode_full` 生成 48kHz 立体声 PCM。该实现能满足“短文本可播报”，但与官方 ONNX 推理路径相比仍有几个明显缺口：

- 文本直接分词，没有官方推理中的文本归一化和长文本切块。
- 采样模式固定为 `local_fixed_sampled_frame`，缺少可复现的 greedy 模式和后续 full sampling 扩展点。
- 下载脚本已包含 codec encode 与 decode step 模型，但运行时没有使用 reference audio encode，也没有内部流式 decode。
- ONNX session I/O 校验只覆盖了部分模型，模型版本漂移时可能在推理中途才失败。
- 合成在 async 调用路径中同步执行 CPU 密集推理，长文本场景可能影响 Tauri 后端响应性。

本变更以官方 ONNX 推理代码为行为参照，但保持当前桌面应用的外部交互模型：TTS runtime 仍先合成完整音频，再进入播放状态；播放期间继续暂停 VAD/录音。

## Goals / Non-Goals

**Goals:**

- 对齐官方 ONNX 文本准备行为：归一化文本、过滤空片段、按 token budget 切分长文本，并将多段音频安全拼接。
- 支持 MOSS 采样模式配置，默认 fixed sampling，提供 greedy 模式用于确定性测试和问题复现。
- 支持 reference audio voice clone：通过 codec encode 将参考音频转成 prompt audio codes，并复用现有 prompt construction。
- 支持内部 streaming codec decode，使引擎可以分批产出音频 chunk 并缓存，降低未来边合成边播的改造成本。
- 扩展模型资产和 session I/O 校验，覆盖本次使用的所有 ONNX 图和关键输入输出名称。
- 将完整 MOSS 推理放入阻塞 worker 或单线程推理执行器，避免阻塞 async runtime。
- 为无模型和真实模型两类环境提供测试路径。

**Non-Goals:**

- 本次不改变用户主流程为“边合成边播放”；外部播放契约仍是完整音频准备好后播放。
- 本次不实现完整 full sampling 的逐 codebook 自回归采样，除非实现 greedy 时发现已有模型接口可低风险复用。
- 本次不引入 GPU/provider 选择 UI，也不改变 ONNX Runtime 动态库加载策略。
- 本次不重做 Assistant Console 视觉设计；前端仅补充必要的调试/测试入口。

## Decisions

### 1) 文本准备独立成 MOSS 专用 pipeline

新增 `MossTextPreprocessor` 或等效模块，负责：

- trim、空白折叠、标点规范化；
- 对中英文、数字和常见符号做与官方 `normalize_tts_text` 等价的处理；
- 分词后按 `max_tokens_per_chunk` 切块，优先在句号、问号、感叹号、逗号、换行等自然边界切分；
- 对每个 chunk 独立构造 prompt 并合成，最后拼接 PCM。

选择该方案是因为 Rust 端需要可测试、可维护的行为边界，而不是把文本处理散落在 `synthesize` 中。备选方案是直接复刻 Python 逻辑为一大段函数，短期更快，但后续调试长文本和多语言输入会比较痛。

### 2) 采样配置进入 TTS 配置扩展层，默认 fixed

在不破坏现有 `TtsConfig` 调用方的前提下，为 MOSS 增加引擎特定选项。实现上可以选择：

- 扩展 `TtsConfig` 增加可序列化的 `options` 字段；
- 或在 Tauri 命令层引入 `MossTtsConfig`，再映射到核心 config。

默认值保持 `fixed`，因为它是当前实现和官方 ONNX CPU 快路径的共同交集。`greedy` 用于确定性测试、回归复现和性能基准；未知采样模式 MUST 返回可读配置错误。

### 3) reference audio 先作为显式调试/命令能力接入

reference audio clone 需要音频加载、重采样、声道规范化、codec encode 和 prompt construction。为控制风险，本次先提供明确的后端能力和可选前端测试入口：

```
reference audio file/data
  -> 48kHz stereo float32
  -> codec encode
  -> prompt_audio_codes
  -> existing prompt rows
  -> TTS generation
```

内置音色仍是默认路径；传入 reference audio 时，reference prompt 优先于 `voice` 字段。若 reference audio 无法解码或 codec encode 失败，错误必须标识在 `codec_encode` 或 `reference_audio` 阶段。

### 4) 内部 streaming decode 先用于缓存与延迟优化，不直接改变播放契约

当前 `streaming-tts-playback` spec 要求最终播放前形成完整音频。为了降低风险，本次新增内部 codec decode step 管线，但 TTS runtime 仍等待完整 `TtsResult` 后进入 `ready`。

实现路径：

- frame 生成循环按固定批次收集 audio frames；
- 当达到 batch 大小或生成结束时调用 `moss_audio_tokenizer_decode_step.onnx`；
- 将 chunk 追加到内部 PCM buffer；
- 合成结束后返回完整 PCM。

若 decode step 状态维护或模型输出不满足契约，可配置回退到 `decode_full`，但必须记录状态并让测试覆盖 fallback。

### 5) session 校验覆盖所有被加载模型

`MossSessions::load` 应校验 prefill、decode step、local fixed、local greedy/decoder、codec encode、codec decode full、codec decode step 的关键输入输出。校验基于 meta 文件声明和实现实际使用名称双重检查：meta 缺失时报 metadata mismatch，模型缺输入输出时报 session I/O mismatch。

这样做的代价是初始化更严格，但模型资产问题能在 `prepare_tts`/health check 阶段暴露，而不是在用户首次播放长回复时失败。

### 6) 推理执行放入阻塞 worker

MOSS ONNX session 不是轻量异步操作。`TtsEngine::synthesize` 应通过 `spawn_blocking` 或专用 worker 执行完整推理，worker 内部继续串行访问 `MossSessions`。这样保持 session 缓存和线程安全，同时避免占用 async runtime。

备选方案是给每次请求新建 session，能简化锁和 Send/Sync 问题，但模型加载成本高且内存压力大，不适合桌面语音助手。

## Risks / Trade-offs

- [Rust 文本归一化与官方 Python 行为不完全一致] -> 建立 fixtures，覆盖数字、英文、中文标点、空白和长文本切块；无法完全复刻的规则在设计中显式记录。
- [reference audio 增加输入格式复杂度] -> 复用现有音频解码/重采样能力，先限定为本地文件或已解码 PCM，不在首版支持任意远程 URL。
- [codec decode step 状态维护复杂] -> 保留 `decode_full` fallback，并用真实模型 ignore 集成测试验证 decode step 输出格式和拼接结果。
- [配置扩展影响现有前后端类型] -> 保持默认值兼容旧调用，新增字段必须 optional。
- [阻塞 worker 与 ONNX value 生命周期冲突] -> 保持 session 和中间 tensor 都在 worker 线程内创建和消费，不跨线程传递 `DynValue`。
- [真实模型测试耗时长] -> 默认单元测试使用 fixture/mock；真实模型测试继续 `#[ignore]`，通过环境变量显式开启。
