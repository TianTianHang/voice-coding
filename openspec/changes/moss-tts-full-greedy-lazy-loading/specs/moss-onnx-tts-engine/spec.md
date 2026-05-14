## MODIFIED Requirements

### Requirement: 非流式推理闭环必须输出可播放 PCM
系统 MUST 支持非流式推理链路：文本输入经分词与 TTS ONNX 推理后，优先使用 codec `decode_full` 产出可播放 PCM，并符合现有播放契约；当 `decode_full` 不可用或失败时，系统 SHALL 可以回退到 codec `decode_step` 分批解码。

#### Scenario: 正常文本合成返回有效音频
- **WHEN** 调用 `synthesize` 并提供非空文本
- **THEN** 系统必须返回 `TtsResult`，且音频数据长度大于零

#### Scenario: 非流式优先使用 full decode
- **WHEN** 调用非流式 `synthesize` 且 codec `decode_full` 可用
- **THEN** 系统 SHALL 使用 `decode_full` 将完整 audio frames 解码为 PCM

#### Scenario: full decode 不可用时回退 step decode
- **WHEN** 调用非流式 `synthesize` 且 codec `decode_full` 缺失或推理失败
- **THEN** 系统 SHALL 回退到 codec `decode_step` 分批解码或返回包含失败阶段的错误

#### Scenario: 输出格式不满足播放契约时失败
- **WHEN** 推理输出的采样率或声道数不等于 48kHz/2ch
- **THEN** 系统必须拒绝该结果并返回明确的格式约束错误

#### Scenario: 推理阶段失败时错误可区分
- **WHEN** tokenizer、TTS 推理或 codec 解码任一阶段发生错误
- **THEN** 返回错误必须包含失败阶段标识，便于定位问题

### Requirement: MOSS 采样模式可配置
MOSS ONNX TTS 引擎 SHALL 支持调用方选择 MOSS 音频 token 采样模式，并在未指定时使用 fixed sampling。

#### Scenario: 未指定采样模式时使用 fixed
- **WHEN** 调用 `synthesize` 且没有提供 MOSS 采样模式
- **THEN** 系统 SHALL 使用 fixed sampling 生成音频 frames

#### Scenario: 指定 fixed 采样模式
- **WHEN** 调用 `synthesize` 且采样模式为 `fixed`
- **THEN** 系统 SHALL 使用 `moss_tts_local_fixed_sampled_frame.onnx` 生成音频 frames

#### Scenario: 指定 greedy 采样模式
- **WHEN** 调用 `synthesize` 且采样模式为 `greedy`
- **THEN** 系统 SHALL 使用 `moss_tts_local_decoder.onnx` 执行确定性 argmax frame 生成
- **AND** 对相同输入、相同模型和相同配置 MUST 产生可复现的 frame 序列

#### Scenario: greedy 模型文件缺失
- **WHEN** 调用 `synthesize` 且采样模式为 `greedy`
- **AND** 模型资产未提供 `local_decoder`
- **THEN** 系统 MUST 拒绝该请求
- **AND** 错误信息 SHALL 包含 `tts.local_decoder`

#### Scenario: 指定未知采样模式
- **WHEN** 调用 `synthesize` 且采样模式不是系统支持的 MOSS 采样模式
- **THEN** 系统 MUST 拒绝合成
- **AND** 错误信息 SHALL 包含未知采样模式和可用模式列表

### Requirement: MOSS 模型资产加载与一致性校验
系统 MUST 在引擎初始化阶段校验 MOSS TTS 与 Codec 两套模型资产的完整性和引用一致性，且在校验失败时返回可定位的错误信息。可选能力模型 SHALL 允许延迟到首次使用时加载和校验。

#### Scenario: 核心资产完整且引用一致时初始化成功
- **WHEN** 后端按约定目录加载 `browser_poc_manifest.json`、`tts_browser_onnx_meta.json`、`codec_browser_onnx_meta.json` 及核心 ONNX/data/tokenizer 文件
- **THEN** 引擎初始化必须成功并可进入健康状态

#### Scenario: 可选能力文件缺失不阻止健康检查
- **WHEN** codec `decode_full` 或 TTS `local_decoder` 文件缺失
- **THEN** 引擎初始化和 `health_check` SHALL 成功
- **AND** 首次使用对应能力时 SHALL 返回明确错误或按定义 fallback

#### Scenario: 关键文件缺失时初始化失败
- **WHEN** manifest、meta、tokenizer 或核心 onnx/external data 中任意必需文件缺失
- **THEN** 引擎初始化必须失败并返回包含缺失文件路径的错误

#### Scenario: manifest 相对路径不可解析时初始化失败
- **WHEN** manifest 中 `codec_meta` 或其他相对路径指向不存在位置
- **THEN** 引擎初始化必须失败并返回包含字段名与原始相对路径的错误
