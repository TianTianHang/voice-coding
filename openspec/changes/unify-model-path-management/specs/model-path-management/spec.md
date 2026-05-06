## ADDED Requirements

### Requirement: 统一模型根目录
系统 SHALL 支持以 `VOICE_CODING_MODEL_HOME` 表示本地模型资产的统一根目录。

#### Scenario: 使用统一模型根环境变量
- **WHEN** `VOICE_CODING_MODEL_HOME` 被设置且没有对应引擎级环境变量覆盖当前模型
- **THEN** 系统 SHALL 从该目录解析当前 ASR 与 TTS 模型路径
- **AND** 系统 SHALL 将解析来源标记为 `modelHomeEnv`

#### Scenario: 使用应用数据目录兜底
- **WHEN** 没有设置对应引擎级环境变量且没有设置 `VOICE_CODING_MODEL_HOME`
- **THEN** 系统 SHALL 尝试使用 Tauri 应用数据目录下的 `models` 目录作为模型根
- **AND** 系统 SHALL 将解析来源标记为 `appData`

#### Scenario: 使用开发目录兜底
- **WHEN** 应用数据目录下没有可识别的当前模型资产且仓库开发目录 `./models` 存在可识别模型资产
- **THEN** 系统 SHALL 使用仓库开发目录 `./models` 作为模型根
- **AND** 系统 SHALL 将解析来源标记为 `devFallback` 或 `legacyDevFallback`

### Requirement: 模型路径解析优先级
系统 SHALL 使用稳定优先级解析每个模型的实际路径。

#### Scenario: ASR 引擎级环境变量优先
- **WHEN** `STT_MODEL_DIR` 被设置
- **THEN** 系统 SHALL 将其作为当前 Qwen3 ASR 模型根目录
- **AND** 系统 SHALL 忽略 `VOICE_CODING_MODEL_HOME` 中的 ASR 标准路径
- **AND** 系统 SHALL 将解析来源标记为 `engineEnv`

#### Scenario: TTS 引擎级环境变量优先
- **WHEN** `MOSS_TTS_MODEL_DIR` 被设置
- **THEN** 系统 SHALL 将其作为当前 MOSS TTS 组件目录
- **AND** 系统 SHALL 忽略 `VOICE_CODING_MODEL_HOME` 中的 TTS 标准路径
- **AND** 系统 SHALL 将解析来源标记为 `engineEnv`

#### Scenario: 统一模型根优先于兜底目录
- **WHEN** 没有设置当前模型的引擎级环境变量且 `VOICE_CODING_MODEL_HOME` 被设置
- **THEN** 系统 SHALL 优先从 `VOICE_CODING_MODEL_HOME` 解析当前模型
- **AND** 系统 MUST NOT 优先使用应用数据目录或仓库开发目录中的同名模型

### Requirement: 标准模型目录布局
系统 SHALL 支持按模型类型和模型 ID 分层的标准目录布局。

#### Scenario: 解析标准 Qwen3 ASR 模型目录
- **WHEN** 当前模型根为 `<model-home>`
- **THEN** 系统 SHALL 将标准 Qwen3 ASR 模型包目录解析为 `<model-home>/asr/qwen3-asr-0.6b-onnx`
- **AND** 系统 SHALL 将传给 Qwen3 ASR 引擎的模型目录解析为同一路径

#### Scenario: 解析标准 MOSS TTS 模型目录
- **WHEN** 当前模型根为 `<model-home>`
- **THEN** 系统 SHALL 将标准 MOSS TTS 模型包目录解析为 `<model-home>/tts/moss-tts-nano-100m-onnx`
- **AND** 系统 SHALL 将传给 MOSS TTS 引擎的模型目录解析为 `<model-home>/tts/moss-tts-nano-100m-onnx/MOSS-TTS-Nano-100M-ONNX`

#### Scenario: 区分模型包目录和引擎目录
- **WHEN** 系统返回 MOSS TTS 模型路径诊断信息
- **THEN** 系统 SHALL 分别暴露模型包目录和传给引擎的模型目录
- **AND** 系统 MUST NOT 将 TTS 组件目录误标记为完整模型包目录

### Requirement: 兼容旧模型目录布局
系统 SHALL 在未显式覆盖时兼容现有开发目录中的旧模型布局。

#### Scenario: 兼容旧 ASR 根目录布局
- **WHEN** 标准 ASR 路径不存在且 `./models/tokenizer.json` 或 `./models/onnx_models` 表明旧 ASR 模型根存在
- **THEN** 系统 SHALL 将 `./models` 解析为当前 Qwen3 ASR 模型目录
- **AND** 系统 SHALL 将 `legacyLayout` 标记为 `true`

#### Scenario: 兼容旧 MOSS TTS 布局
- **WHEN** 标准 TTS 路径不存在且 `./models/moss-tts/MOSS-TTS-Nano-100M-ONNX` 存在可识别 MOSS manifest
- **THEN** 系统 SHALL 将 `./models/moss-tts` 解析为旧 MOSS TTS 模型包目录
- **AND** 系统 SHALL 将 `./models/moss-tts/MOSS-TTS-Nano-100M-ONNX` 解析为传给引擎的模型目录
- **AND** 系统 SHALL 将 `legacyLayout` 标记为 `true`

#### Scenario: 显式环境变量不标记为旧布局
- **WHEN** 用户通过 `STT_MODEL_DIR` 或 `MOSS_TTS_MODEL_DIR` 显式指定模型目录
- **THEN** 系统 SHALL 使用该显式目录
- **AND** 系统 MUST NOT 因该目录不符合标准布局而自动标记为旧开发布局

### Requirement: 模型资产诊断信息
系统 SHALL 为每个当前本地模型提供结构化路径诊断信息。

#### Scenario: 返回成功解析的模型诊断
- **WHEN** 系统成功解析当前模型路径
- **THEN** 诊断信息 SHALL 包含模型类型、模型 ID、引擎名称、模型包目录、引擎模型目录、解析来源和旧布局标记
- **AND** 缺失文件列表 SHALL 为空

#### Scenario: 返回缺失文件诊断
- **WHEN** 当前模型目录存在但缺少必需资产
- **THEN** 诊断信息 SHALL 包含缺失文件列表
- **AND** 每个缺失文件 SHALL 使用相对于模型包目录或引擎模型目录的可读路径
- **AND** 系统 SHALL 保留足够错误信息供前端展示和日志排查

#### Scenario: 返回完全缺失的模型诊断
- **WHEN** 当前模型无法从任何候选路径解析到可识别目录
- **THEN** 诊断信息 SHALL 包含预期标准路径
- **AND** 诊断信息 SHALL 包含模型不可用的错误说明

### Requirement: 下载脚本使用标准布局
模型下载脚本 SHALL 默认写入标准模型目录布局。

#### Scenario: ASR 下载脚本默认路径
- **WHEN** 用户运行 `scripts/download_model.sh` 且没有传入目标目录参数
- **THEN** 脚本 SHALL 默认下载到 `${VOICE_CODING_MODEL_HOME:-models}/asr/qwen3-asr-0.6b-onnx`
- **AND** 脚本完成提示 SHALL 推荐设置 `VOICE_CODING_MODEL_HOME`

#### Scenario: MOSS TTS 下载脚本默认路径
- **WHEN** 用户运行 `scripts/download_moss_tts_models.sh` 且没有传入目标目录参数
- **THEN** 脚本 SHALL 默认下载到 `${VOICE_CODING_MODEL_HOME:-models}/tts/moss-tts-nano-100m-onnx`
- **AND** 脚本 SHALL 在该目录下创建 `MOSS-TTS-Nano-100M-ONNX` 和 `MOSS-Audio-Tokenizer-Nano-ONNX`
- **AND** 脚本完成提示 SHALL 推荐设置 `VOICE_CODING_MODEL_HOME`

#### Scenario: 下载脚本显式目标目录
- **WHEN** 用户向下载脚本传入目标目录参数
- **THEN** 脚本 SHALL 使用该目标目录
- **AND** 脚本 MUST NOT 强制改写用户传入的目录为标准布局
