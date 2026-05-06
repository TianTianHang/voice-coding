## 1. 模型资产与配置契约

- [x] 1.1 定义 MOSS 模型目录配置与加载入口（包含 manifest、tts meta、codec meta、tokenizer 路径）。
- [x] 1.2 实现模型资产完整性校验（必需文件、relative path、external data 依赖）。
- [x] 1.3 实现并测试错误分类（缺文件、路径解析失败、元信息不匹配）。

## 2. MOSS ONNX 推理引擎实现

- [x] 2.1 新增 `MossOnnxTtsEngine` 模块并实现 `TtsEngine` 接口。
- [x] 2.2 实现 `health_check`：最小化 session 初始化、I/O 名称与关键维度校验。
- [x] 2.3 实现非流式 `synthesize`：文本分词 -> TTS ONNX -> audio codes -> codec `decode_full` -> PCM。
- [x] 2.4 实现 `TtsConfig.voice` 映射策略（默认音色、未知音色错误提示）。
- [x] 2.5 确保输出严格满足 48kHz/2ch 契约，不满足时返回明确错误。

## 3. TTS runtime 与播放联动接入

- [x] 3.1 在 `src-tauri/src/tts.rs` 中引入可配置引擎注入（mock 与 MOSS 可切换）。
- [x] 3.2 保持现有状态机行为：`idle -> synthesizing -> ready -> playing -> idle` 与失败态处理。
- [x] 3.3 验证播放期间 VAD/录音暂停与恢复逻辑在 MOSS 引擎下行为一致。

## 4. 测试与验证

- [x] 4.1 为模型加载、路径解析、音色映射与错误分类补齐 Rust 单元测试。
- [x] 4.2 为推理输出契约与 runtime 状态转换补齐 Rust 集成测试。
- [x] 4.3 运行 `cargo test` 并修复失败项。
- [x] 4.4 运行 `cargo clippy` 并修复告警/错误。
- [x] 4.5 运行 `pnpm tauri build` 验证桌面端后端集成构建。
- [x] 4.6 运行 `pnpm build` 与 `pnpm test` 完成前端回归验证。
- [x] 4.7 记录最终验证结果（执行命令、通过项、阻塞项与原因）。
