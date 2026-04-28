## 1. 后端加载 Runtime

- [x] 1.1 定义 ASR 加载状态、状态快照和 timing 数据结构，覆盖 `unloaded`、`loading`、`ready`、`failed`
- [x] 1.2 用 ASR runtime 替换直接 `Lazy<Qwen3AsrEngine>` 访问，保存已加载引擎和最近一次状态快照
- [x] 1.3 实现幂等 `prepare_asr` 加载入口，确保 loading 期间复用同一个加载任务
- [x] 1.4 实现转写路径的 ready engine 获取逻辑，支持 unloaded 自动加载、loading 等待、failed 返回错误

## 2. 命令与事件

- [x] 2.1 新增 Tauri command `prepare_asr`
- [x] 2.2 新增 Tauri command `get_asr_status`
- [x] 2.3 在加载开始、成功、失败时广播 `asr-status` 完整状态快照事件
- [x] 2.4 在应用启动或语音功能初始化路径触发后台预热，不阻塞前端渲染

## 3. 加载耗时记录

- [x] 3.1 为 Qwen3 模型加载增加总耗时记录
- [x] 3.2 分别记录 ONNX sessions、embedding、tokenizer、mel filterbank 加载耗时
- [x] 3.3 在 ready 状态快照和 `asr-status` ready 事件中包含 timing 数据
- [x] 3.4 在 failed 状态快照和 `asr-status` failed 事件中包含加载错误消息

## 4. 前端状态集成

- [x] 4.1 新增或扩展前端 hook 监听 `asr-status` 事件并读取 `get_asr_status`
- [x] 4.2 在语音 UI 中根据 ASR 状态展示准备中、就绪和失败状态
- [x] 4.3 确保前端使用最新 `asr-status` payload 覆盖状态，而不是依赖增量推断

## 5. 测试与验证

- [x] 5.1 添加 Rust 单元测试覆盖状态转换、幂等预热、loading 期间复用加载结果和失败后重试
- [x] 5.2 添加 Rust 测试覆盖转写路径在 unloaded、loading、ready、failed 状态下的行为
- [x] 5.3 添加前端测试覆盖 ASR 状态事件解析和 UI 状态映射
- [x] 5.4 运行 `cargo test`
- [x] 5.5 运行 `cargo clippy`
- [x] 5.6 运行 `pnpm test`
- [x] 5.7 运行 `pnpm build`
