## Context

当前后端 TTS runtime、播放输出和 VAD 播放期暂停/恢复链路已经存在，但合成引擎仍为 mock 实现，无法提供真实文本播报能力。本次变更需要在不破坏现有状态机与播放流程的前提下，引入 MOSS ONNX 本地推理能力，并确保模型目录契约、错误可观测性和测试完整性。

MOSS 方案由 TTS 模型与 Audio Tokenizer/Codec 模型共同组成，且 manifest 依赖相对路径引用 codec 元数据；因此实现重点不仅是推理调用，还包括模型资产一致性校验和目录布局约束。

## Goals / Non-Goals

**Goals:**
- 在 Rust 后端实现 `MossOnnxTtsEngine`，可接入现有 `TtsEngine` 抽象。
- 打通首版非流式推理：文本分词、音频 token 生成、`decode_full` 解码、返回 48kHz 立体声 PCM。
- 提供明确的模型加载/健康检查/错误分类，失败时可快速定位到文件缺失、路径错误或推理阶段。
- 与现有 TTS runtime 状态机兼容，保留播放期间暂停录音/VAD 的行为。
- 补齐单元与集成测试，并执行 Rust/Tauri/前端质量门禁。

**Non-Goals:**
- 首版不实现 `decode_step` 驱动的流式边合成边播放。
- 首版不实现参考音频克隆说话人，仅支持 MOSS 内置音色。
- 首版不改造前端交互模型，仅沿用现有 TTS 命令入口。

## Decisions

### 1) 引擎分层：模型资产层与推理执行层分离

将实现拆为两个内聚模块：
- 资产层：解析 `browser_poc_manifest.json`、`tts_browser_onnx_meta.json`、`codec_browser_onnx_meta.json`，完成文件存在性、相对路径可解析性、外部 data 文件绑定校验。
- 推理层：基于已校验资产创建 ONNX session，执行 token 生成与 codec 解码。

这样可以把“启动失败”与“运行时推理失败”分离，便于 health check 和测试覆盖。

**备选方案：** 初始化时直接散落读取文件并立刻推理。实现更快，但错误上下文不清晰，难定位 manifest/path 问题。

### 2) 先实现 `decode_full` 闭环，延后 `decode_step`

首版采用非流式闭环：
文本 -> tokenizer -> TTS ONNX -> audio codes -> codec `decode_full` -> PCM。

理由：`decode_step` 需要维护大量 transformer/attention cache，状态复杂，首版优先确保可靠可测交付。

**备选方案：** 一次性上流式。可降低首包延迟，但风险和调试成本显著上升。

### 3) 统一输出契约严格对齐 `tts-core`

引擎输出必须满足 `PLAYBACK_SAMPLE_RATE_HZ=48000`、`PLAYBACK_CHANNELS=2`。若模型输出元信息不符合，初始化直接失败，不在播放期再兜底隐式转换。

**备选方案：** 播放前隐式重采样/升降声道。更灵活但会掩盖模型资产问题，增加音质与性能不确定性。

### 4) 音色策略：显式映射 + 默认回退

`TtsConfig.voice` 采用“名称匹配内置音色表”；空值时使用默认音色；未知值返回可读错误并给出可选音色列表摘要。

**备选方案：** 未知音色静默回退默认值。体验更“容错”，但配置错误不可见，影响可控性。

### 5) runtime 注入策略：可配置选择 mock 或 MOSS

`TtsRuntime::default` 改为“按配置构造引擎”，本地开发可使用 mock，模型可用时使用 MOSS。这样保证现有测试与无模型环境可继续运行。

**备选方案：** 强制使用 MOSS。会导致 CI/开发环境在缺模型时普遍失败。

## Risks / Trade-offs

- [模型文件大、加载慢] -> 在 `prepare_tts` 做一次性健康检查并缓存 session，避免每次合成重复初始化。
- [manifest 相对路径耦合导致部署易错] -> 增加目录布局校验与错误提示，明确期望树形结构。
- [ONNX 输入输出名与 meta 不一致] -> 启动时对 session I/O 与 meta 声明做一致性比对，失败立即报错。
- [多平台 ORT 差异] -> 先锁定 CPU provider 路径，记录 provider 信息到日志，后续再扩展加速 provider。
- [测试依赖真实模型体积大] -> 单元测试以 mock/meta fixture 为主；真实模型集成测试标记为可选或按环境变量开启。
