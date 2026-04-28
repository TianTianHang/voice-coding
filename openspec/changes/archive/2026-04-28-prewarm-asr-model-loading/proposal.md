## Why

当前 Qwen3 ASR 引擎通过全局懒加载在首次转写时初始化，用户第一次开始语音识别时会同时承担 ONNX session 创建、embedding 读取、tokenizer 加载等成本，导致首次识别延迟不可预测。

需要将模型加载从首次转写路径中前移到后台预热，并通过状态事件和分阶段耗时记录让加载过程可观测、可诊断。

## What Changes

- 新增 ASR 模型后台预热能力，在应用启动后或语音功能初始化时异步触发当前 Qwen3 模型加载。
- 新增 ASR 加载状态查询与事件通知，暴露 `unloaded`、`loading`、`ready`、`failed` 等状态。
- 修改转写入口，在模型尚未 ready 时等待同一个加载任务完成或返回清晰错误，避免重复初始化。
- 为当前 Qwen3 模型加载记录分阶段耗时，包括 ONNX sessions、embedding、tokenizer、mel filterbank 和总耗时。
- 加载成功和失败状态均应包含可用于诊断的信息；成功状态包含 timing，失败状态包含错误消息。
- 不改变当前模型文件布局、ONNX graph、embedding 表示方式或转写输出语义。

## Capabilities

### New Capabilities

- `asr-model-loading`: 描述 ASR 模型后台预热、加载状态事件、状态查询和加载耗时记录。

### Modified Capabilities

无。

## Impact

- 后端 Tauri/Rust：
  - `src-tauri/src/asr.rs` 需要从直接 `Lazy<Qwen3AsrEngine>` 转为可观测的加载 runtime。
  - Tauri command registration 需要增加预热和状态查询命令。
  - Qwen3 引擎构造路径需要记录分阶段加载耗时。
- 前端 React/TypeScript：
  - 新增或扩展 hook 监听 ASR 加载状态事件。
  - 语音 UI 可根据 loading/ready/failed 状态调整可用性和错误展示。
- 事件/API：
  - 新增 `asr-status` 后端事件。
  - 新增 `prepare_asr` 与 `get_asr_status` 命令。
- 验证：
  - Rust 单元测试覆盖加载状态转换、并发预热复用、加载失败状态。
  - 前端测试覆盖状态事件解析和 UI 状态映射。
  - 运行 `cargo test`、`cargo clippy`、`pnpm test`、`pnpm build`。
