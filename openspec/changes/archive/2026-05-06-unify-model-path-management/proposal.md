## Why

当前 ASR 与 TTS 模型路径由各自模块分散解析：ASR 使用 `STT_MODEL_DIR` 并默认指向 `models`，TTS 使用 `MOSS_TTS_MODEL_DIR` 并默认指向 MOSS TTS 子目录。这导致“模型根目录”的含义不一致，开发、打包、脚本和前端诊断都难以形成稳定契约。

随着本地 ASR/TTS 都进入真实模型推理阶段，应用需要统一的模型路径管理能力，让后端、下载脚本、状态快照和未来设置页共享同一套路径契约，同时兼容现有环境变量与目录布局。

## What Changes

- 引入统一模型根目录概念：`VOICE_CODING_MODEL_HOME` 作为新的主入口，管理 ASR/TTS 等本地模型资产。
- 定义标准模型目录布局：`asr/<model-id>/` 与 `tts/<model-id>/` 分层存放，避免 ASR 文件直接散落在 `models/` 根目录。
- 在 Tauri 后端集中解析模型路径，按优先级处理引擎级环境变量、统一模型根、应用数据目录和开发目录兜底。
- 保留 `STT_MODEL_DIR` 与 `MOSS_TTS_MODEL_DIR` 的兼容语义，避免破坏现有开发、测试和手动配置流程。
- 为 ASR/TTS 状态提供一致的模型路径诊断信息，包括模型 ID、解析来源、目录、缺失文件和错误信息。
- 调整下载脚本与文档，使默认下载位置符合新的标准目录布局，并说明旧布局兼容方式。
- 不改变 ASR/TTS 推理算法、ONNX session 执行逻辑或前端核心交互流程。

## Capabilities

### New Capabilities
- `model-path-management`: 统一管理本地 ASR/TTS 模型目录、路径解析优先级、标准布局、兼容旧路径和状态诊断契约。

### Modified Capabilities
- `asr-model-loading`: ASR 加载状态需要暴露统一模型解析信息，并从集中路径解析结果加载当前 Qwen3 ASR 模型。
- `onnx-inference`: Qwen3 ASR ONNX 模型文件路径要求需要支持新的标准模型包根目录，同时兼容旧 `{model_dir}/onnx_models` 结构。

## Impact

- 后端：影响 `src-tauri/src/asr.rs`、`src-tauri/src/tts.rs`、`src-tauri/stt-qwen3` 和 `src-tauri/tts-moss` 的模型路径入口与状态快照。
- 前端：影响 `src/hooks/useAsrStatus.ts` 以及 TTS 状态消费位置，新增或扩展模型路径诊断字段。
- 脚本：影响 `scripts/download_model.sh` 与 `scripts/download_moss_tts_models.sh` 的默认输出目录和提示信息。
- 文档与配置：新增统一路径契约说明，保留 `STT_MODEL_DIR`、`MOSS_TTS_MODEL_DIR`、`ORT_DYLIB_PATH` 的现有说明，并新增 `VOICE_CODING_MODEL_HOME`。
- 测试：需要覆盖路径解析优先级、标准布局、旧布局兼容、缺失文件诊断、ASR/TTS 状态序列化和下载脚本目标路径。
