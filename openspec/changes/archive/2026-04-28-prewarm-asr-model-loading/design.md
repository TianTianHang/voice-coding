## Context

当前后端在 `src-tauri/src/asr.rs` 中使用 `once_cell::sync::Lazy<Qwen3AsrEngine>` 保存全局 ASR 引擎。第一次调用 `transcribe`、`transcribe_audio_data` 或 VAD 转写路径时才会触发 `Qwen3AsrEngine::new()`，因此首次识别会阻塞在模型加载上。

当前 Qwen3 模型加载包括 4 个 ONNX Runtime session、`embed_tokens.bin`、`tokenizer.json` 和 mel filterbank。模型文件较大，尤其是 embedding 文件约 594MB，但加载过程没有分阶段耗时记录，也没有对前端暴露 ready/failed 状态。

本变更只优化当前模型的加载时机和可观测性，不改变模型文件布局、推理图、embedding 存储格式或转写结果。

## Goals / Non-Goals

**Goals:**

- 在应用启动后或语音功能初始化时后台预热当前 Qwen3 ASR 引擎。
- 为 ASR 加载建立明确状态机：`unloaded`、`loading`、`ready`、`failed`。
- 提供 `prepare_asr` 命令触发预热，提供 `get_asr_status` 命令读取状态快照。
- 通过 `asr-status` 事件广播状态变化，前端可以无轮询地更新 UI。
- 记录 Qwen3 模型加载分阶段耗时，成功状态包含 timing 数据，失败状态包含错误消息。
- 并发调用预热或转写时复用同一个加载任务，不重复创建模型引擎。

**Non-Goals:**

- 不引入新的模型导出格式或模型目录 manifest。
- 不实现 embedding mmap、FP16 embedding、ONNX session 并行加载或 ORT format。
- 不改变 `SttEngine` trait 的核心转写接口。
- 不改变 VAD 录音状态事件和 transcript/error 事件语义。
- 不要求前端阻止用户操作；后端仍需正确处理 loading 和 failed 状态。

## Decisions

1. 使用 ASR runtime 管理全局引擎和加载状态。

   新增一个后端 runtime 状态容器，负责保存状态快照、可选的已加载 `Qwen3AsrEngine`、加载错误和加载 timing。所有命令和 VAD 转写路径都通过 runtime 获取 ready engine。

   备选方案：继续保留直接 `Lazy<Qwen3AsrEngine>`，只在启动时主动触发。该方案虽然简单，但难以表达失败状态、加载进度和并发加载复用。

2. 预热命令幂等。

   `prepare_asr` 在 `unloaded` 或 `failed` 状态下启动加载；在 `loading` 状态下复用当前加载任务；在 `ready` 状态下直接返回当前状态。多次调用不得创建多个引擎实例。

   备选方案：每次调用都尝试新建引擎。该方案会放大内存占用，并可能让 ONNX Runtime session 创建互相竞争。

3. 转写路径等待同一个加载结果。

   如果用户在 loading 期间触发转写，后端应等待当前加载完成后继续转写；如果加载失败，转写返回清晰错误。这样前端即使没有完全禁用按钮，后端行为仍然确定。

   备选方案：loading 期间立即拒绝转写。该方案实现简单，但可能让用户在模型即将 ready 时得到不必要的错误。

4. 状态事件使用完整快照。

   `asr-status` 事件 payload 应包含状态、可选错误消息、可选 timing、当前模型目录或引擎名。前端可以直接用最新事件覆盖本地状态，避免根据增量事件推断。

   备选方案：只发字符串状态。该方案不足以支持失败诊断和加载耗时展示。

5. 分阶段 timing 在 Qwen3 构造路径内收集。

   `Qwen3AsrEngine::new()` 或相邻构造 helper 应记录 `onnx_sessions_ms`、`embeddings_ms`、`tokenizer_ms`、`mel_filterbank_ms` 和 `total_ms`。记录结果不应改变加载顺序和错误语义。

   备选方案：只记录总耗时。该方案无法判断后续应优先优化 ONNX session 还是 embedding 加载。

## Risks / Trade-offs

- 后台预热会提前占用 CPU 和内存 -> 仅在应用初始化或语音功能初始化时触发一次，并保持命令幂等。
- 加载失败如果缓存为 `failed`，用户修复模型文件后可能需要重试 -> `prepare_asr` 在 `failed` 状态下允许重新加载。
- 转写等待 loading 可能让命令耗时较长 -> 前端通过 `asr-status` 可展示准备中状态，后端错误消息保持清晰。
- 状态容器需要跨 async 任务共享 -> 使用线程安全同步原语，避免在持锁状态下执行耗时加载。
- timing 数值受机器负载影响 -> 仅作为诊断信息和回归参考，不作为严格性能断言。
