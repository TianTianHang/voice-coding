## Context

当前后端有两套模型路径入口：ASR runtime 在 `src-tauri/src/asr.rs` 读取 `STT_MODEL_DIR`，默认值为 `models`；MOSS TTS engine 在 `src-tauri/tts-moss/src/lib.rs` 读取 `MOSS_TTS_MODEL_DIR`，默认值为 `../models/moss-tts/MOSS-TTS-Nano-100M-ONNX`。两者的“模型目录”含义不同：ASR 指向 Qwen3 模型根，TTS 指向 MOSS TTS 子组件目录，并通过官方 manifest 相对引用 codec 目录。

这会带来几个长期问题：
- 默认路径依赖运行时工作目录，尤其 TTS crate 的 `../models/...` 在不同 Cargo/Tauri 启动路径下容易产生歧义。
- ASR 模型文件直接位于 `models/` 根目录，TTS 位于 `models/moss-tts/`，未来增加更多 ASR/TTS/VAD/LLM 模型时目录会混乱。
- ASR 状态暴露 `modelDir`，TTS 状态没有等价路径诊断字段，前端和用户无法一致理解模型缺失原因。
- 下载脚本默认输出目录和运行时默认解析目录没有统一抽象。

本设计把模型路径决策收敛到 Tauri app 层，让 engine crate 只负责“给定路径后的资产验证与推理”。

## Goals / Non-Goals

**Goals:**
- 建立统一的模型根目录契约，以 `VOICE_CODING_MODEL_HOME` 作为新推荐入口。
- 定义稳定、可扩展的标准目录布局，覆盖当前 Qwen3 ASR 和 MOSS TTS。
- 集中实现模型路径解析优先级，并保留现有 `STT_MODEL_DIR` 与 `MOSS_TTS_MODEL_DIR` 兼容行为。
- 让 ASR/TTS 状态都能暴露一致的模型诊断信息，便于前端展示、日志排查和未来设置页使用。
- 将路径选择从 `stt-qwen3` / `tts-moss` 引擎 crate 中剥离，降低 crate 对环境变量和工作目录的隐式依赖。
- 更新下载脚本默认输出布局，并保留旧脚本参数的可用性。

**Non-Goals:**
- 不改变 Qwen3 ASR 或 MOSS TTS 的推理流程、ONNX 输入输出、采样策略或音频处理逻辑。
- 不在本次变更中实现模型自动下载 UI、模型市场、远端模型服务或多模型热切换。
- 不强制迁移用户磁盘上的旧模型目录；旧目录应继续可被解析。
- 不把大型模型文件打包进桌面应用安装包。
- 不改变 `ORT_DYLIB_PATH` 的职责；ONNX Runtime 动态库仍由 Nix/运行环境提供。

## Decisions

### 1. 引入 `VOICE_CODING_MODEL_HOME` 作为统一模型根

推荐的新根目录为：

```text
models/
├── asr/
│   └── qwen3-asr-0.6b-onnx/
└── tts/
    └── moss-tts-nano-100m-onnx/
```

解析优先级为：

```text
1. 引擎级显式环境变量
   - ASR: STT_MODEL_DIR
   - TTS: MOSS_TTS_MODEL_DIR

2. 统一模型根环境变量
   - VOICE_CODING_MODEL_HOME

3. Tauri 应用数据目录
   - <app-data>/models

4. 仓库开发目录
   - ./models
```

理由：引擎级环境变量保留最高优先级，避免破坏现有开发、CI 和手动调试；统一模型根是未来推荐入口；应用数据目录适合正式桌面分发；仓库 `./models` 仅作为开发兜底。

备选方案：只保留 `STT_MODEL_DIR` / `MOSS_TTS_MODEL_DIR`。该方案改动少，但无法解决路径语义不一致，也不利于未来多个模型族扩展。

### 2. 标准目录布局按能力和模型 ID 分层

完整路径契约如下：

```text
<model-home>/
├── asr/
│   └── qwen3-asr-0.6b-onnx/
│       ├── tokenizer.json
│       ├── config.json
│       ├── embed_tokens.bin
│       └── onnx_models/
│           ├── encoder.int4.onnx 或 encoder.onnx
│           ├── decoder_init.int4.onnx 或 decoder_init.onnx
│           ├── decoder_step.int4.onnx 或 decoder_step.onnx
│           └── decoder_weights.int4.data
│
└── tts/
    └── moss-tts-nano-100m-onnx/
        ├── MOSS-TTS-Nano-100M-ONNX/
        │   ├── browser_poc_manifest.json
        │   ├── tts_browser_onnx_meta.json
        │   ├── tokenizer.model
        │   ├── moss_tts_prefill.onnx
        │   ├── moss_tts_decode_step.onnx
        │   ├── moss_tts_global_shared.data
        │   ├── moss_tts_local_decoder.onnx
        │   ├── moss_tts_local_cached_step.onnx
        │   ├── moss_tts_local_fixed_sampled_frame.onnx
        │   └── moss_tts_local_shared.data
        └── MOSS-Audio-Tokenizer-Nano-ONNX/
            ├── codec_browser_onnx_meta.json
            ├── moss_audio_tokenizer_encode.onnx
            ├── moss_audio_tokenizer_encode.data
            ├── moss_audio_tokenizer_decode_full.onnx
            ├── moss_audio_tokenizer_decode_step.onnx
            └── moss_audio_tokenizer_decode_shared.data
```

模型 ID 使用小写 kebab-case，避免把上游仓库名大小写直接暴露为应用级路径契约。上游原始目录名可保留在模型包内部，尤其 MOSS TTS 需要官方 `browser_poc_manifest.json` 的相对路径关系。

备选方案：继续让 ASR 占用 `<model-home>/` 根目录。该方案兼容当前脚本，但会阻碍未来增加更多 ASR 模型或其他模型类型。

### 3. TTS 的应用级模型目录指向“模型包根”，引擎入口仍可接收 TTS 组件目录

MOSS TTS 实际由 TTS 组件和 codec 组件共同组成。应用级契约使用模型包根：

```text
<model-home>/tts/moss-tts-nano-100m-onnx/
```

解析器从模型包根派生当前 `MossModelConfig.model_dir`：

```text
<model-home>/tts/moss-tts-nano-100m-onnx/MOSS-TTS-Nano-100M-ONNX
```

`MOSS_TTS_MODEL_DIR` 作为兼容变量时继续表示“直接的 TTS 组件目录”，因为这是当前实现和测试已经使用的语义。

理由：应用层需要表达完整 MOSS 模型包，才能准确诊断 codec 依赖；但不应一次性打破 `tts-moss` 现有 manifest 解析方式。

备选方案：把 `MOSS_TTS_MODEL_DIR` 改成模型包根。语义更统一，但属于破坏性变更，会让已有命令、测试和用户配置失效。

### 4. 在 Tauri app 层新增模型路径解析模块

后端新增集中模块，例如 `src-tauri/src/model_paths.rs`，负责：
- 读取环境变量和 app data dir。
- 计算 ASR/TTS 的候选路径。
- 判断标准布局和旧布局。
- 返回带来源和诊断信息的解析结果。

建议核心类型：

```text
ModelKind = Asr | Tts
ModelPathSource = EngineEnv | ModelHomeEnv | AppData | DevFallback | LegacyDevFallback

ResolvedModelPath
├── kind
├── model_id
├── engine_name
├── package_dir
├── engine_model_dir
├── source
├── is_legacy_layout
├── required_files
├── missing_files
└── error
```

`package_dir` 表示应用级模型包目录；`engine_model_dir` 表示传给当前 engine crate 的目录。对 ASR 两者通常相同；对 MOSS TTS，`package_dir` 是 `tts/moss-tts-nano-100m-onnx`，`engine_model_dir` 是其中的 `MOSS-TTS-Nano-100M-ONNX`。

备选方案：在 `stt-qwen3` 和 `tts-moss` 各自实现路径解析。该方案局部简单，但会重复优先级规则，也难以输出一致的前端状态。

### 5. 引擎 crate 不再直接决定默认路径

目标结构是：
- `src-tauri/src/asr.rs` 调用模型路径解析器，拿到 Qwen3 `engine_model_dir`，再创建 `Qwen3AsrEngine`。
- `src-tauri/src/tts.rs` 调用模型路径解析器，拿到 MOSS TTS `engine_model_dir`，再创建 `MossOnnxTtsEngine`。
- `stt-qwen3` 只校验给定 Qwen3 目录下的 tokenizer、embedding 和 ONNX 文件。
- `tts-moss` 只校验给定 MOSS TTS 组件目录及 manifest 相对引用。

为了平滑迁移，可以先保留 `MossModelConfig::from_env()`，但默认 Tauri runtime 不再使用它；crate-level 集成测试仍可继续使用引擎级环境变量。

备选方案：立即移除所有 env 读取函数。语义更干净，但对测试和外部直接使用 crate 的场景不够温和。

### 6. 状态快照扩展统一模型诊断字段

ASR 当前 `AsrStatusSnapshot` 已包含 `engineName` 和 `modelDir`。TTS 当前 `TtsStatusSnapshot` 只有状态、错误和缓冲标记。本变更建议引入共享的序列化诊断结构：

```text
ModelPathSnapshot
├── kind: "asr" | "tts"
├── modelId
├── engineName
├── packageDir
├── modelDir
├── source
├── legacyLayout
├── missingFiles
└── error
```

ASR 可以保留顶层 `modelDir` 兼容字段，同时新增 `model` 字段；TTS 新增 `engineName` 和 `model` 字段。前端优先消费 `model`，旧字段继续可用。

备选方案：只把更多信息拼进 `error` 字符串。实现最少，但前端无法结构化展示，也不利于测试。

### 7. 旧布局兼容策略

ASR 兼容：

```text
标准：<model-home>/asr/qwen3-asr-0.6b-onnx/
旧版：<repo>/models/
显式：STT_MODEL_DIR 直接指向 Qwen3 模型根
```

当没有引擎级环境变量时，解析器先检查标准路径；如果标准路径缺失但旧版 `models/tokenizer.json` 或 `models/onnx_models/` 存在，则使用旧布局并标记 `legacyLayout=true`。

TTS 兼容：

```text
标准：<model-home>/tts/moss-tts-nano-100m-onnx/MOSS-TTS-Nano-100M-ONNX
旧版：<repo>/models/moss-tts/MOSS-TTS-Nano-100M-ONNX
显式：MOSS_TTS_MODEL_DIR 直接指向 MOSS-TTS-Nano-100M-ONNX
```

旧布局只作为解析兼容，不再作为下载脚本默认输出。

### 8. 下载脚本默认写入标准布局

建议更新脚本默认值：

```text
scripts/download_model.sh
  默认目标：${VOICE_CODING_MODEL_HOME:-models}/asr/qwen3-asr-0.6b-onnx

scripts/download_moss_tts_models.sh
  默认目标：${VOICE_CODING_MODEL_HOME:-models}/tts/moss-tts-nano-100m-onnx
```

脚本仍接受第一个参数作为显式目标目录。ASR 脚本参数表示 Qwen3 模型根；TTS 脚本参数表示 MOSS 模型包根。脚本完成后输出推荐环境变量：

```text
export VOICE_CODING_MODEL_HOME="<model-home>"
```

对于用户指定旧式目标参数，脚本不强制拒绝，但输出中应说明标准布局推荐路径。

备选方案：新增一个总脚本并废弃旧脚本。长期更统一，但本次可以先调整默认值和提示，减少迁移成本。

## Risks / Trade-offs

- [路径优先级过多导致调试复杂] → 在 `ModelPathSnapshot.source` 中暴露实际命中的来源，并在缺失文件错误中列出候选路径摘要。
- [旧用户模型目录没有自动迁移] → 保留旧布局解析；文档和脚本提示推荐新目录，不移动用户文件。
- [TTS 同时存在 package dir 和 engine model dir 容易混淆] → 类型和状态字段明确区分 `packageDir` 与 `modelDir`，文档中固定 `modelDir` 表示传给引擎的目录。
- [前端类型变化影响现有测试] → 保留 ASR 顶层 `modelDir`，新增字段采用向后兼容方式，并更新 hook 测试。
- [应用数据目录在测试环境不稳定] → 路径解析核心逻辑设计为可注入 app data/dev root/env map，单测不依赖真实平台目录。
- [下载脚本默认目录变化让已有文档过时] → 同步更新 README/AGENTS 或模型相关文档中的示例命令。

## Migration Plan

1. 新增 Tauri app 层模型路径解析模块和单元测试，先不接入 runtime。
2. 接入 ASR runtime：使用解析器结果创建 `Qwen3AsrEngine`，保持 `STT_MODEL_DIR` 优先级和旧 `models/` 兼容。
3. 接入 TTS runtime：使用解析器结果创建 `MossOnnxTtsEngine`，保持 `MOSS_TTS_MODEL_DIR` 直接组件目录语义。
4. 扩展 ASR/TTS 状态快照和前端类型，增加结构化模型诊断字段。
5. 更新下载脚本默认输出目录和完成提示。
6. 更新文档，写明完整路径契约、优先级、旧布局兼容和推荐环境变量。
7. 执行 Rust/前端/脚本相关测试与构建。

回滚策略：如果新解析器导致运行时加载问题，可暂时让 ASR/TTS runtime 回退到原有 `STT_MODEL_DIR` / `MOSS_TTS_MODEL_DIR` 读取逻辑；因为旧环境变量语义保留，回滚不需要迁移用户模型文件。

## Open Questions

- 是否要在本次变更中新增一个 `get_model_status` Tauri 命令，统一查询 ASR/TTS 模型状态，还是仅扩展现有 ASR/TTS 状态快照？倾向于先扩展现有状态，避免增加前端范围。
- 是否需要为模型包新增应用级 `manifest.json`？倾向于本次不强制新增文件，仅在代码中定义路径契约；未来做模型下载 UI 时再引入应用 manifest。
- 正式打包环境的默认模型根是否直接使用 Tauri app data dir，还是仍允许 repo `./models` fallback？倾向于保留 fallback，但状态中明确标记来源。
