## ADDED Requirements

### Requirement: MOSS 模型资产加载与一致性校验
系统必须在引擎初始化阶段校验 MOSS TTS 与 Codec 两套模型资产的完整性和引用一致性，且在校验失败时返回可定位的错误信息。

#### Scenario: 资产完整且引用一致时初始化成功
- **WHEN** 后端按约定目录加载 `browser_poc_manifest.json`、`tts_browser_onnx_meta.json`、`codec_browser_onnx_meta.json` 及其声明的 ONNX/data/tokenizer 文件
- **THEN** 引擎初始化必须成功并可进入健康状态

#### Scenario: 关键文件缺失时初始化失败
- **WHEN** manifest、meta、onnx 或 external data 中任意必需文件缺失
- **THEN** 引擎初始化必须失败并返回包含缺失文件路径的错误

#### Scenario: manifest 相对路径不可解析时初始化失败
- **WHEN** manifest 中 `codec_meta` 或其他相对路径指向不存在位置
- **THEN** 引擎初始化必须失败并返回包含字段名与原始相对路径的错误

### Requirement: 非流式推理闭环必须输出可播放 PCM
系统必须支持非流式推理链路：文本输入经分词与 TTS ONNX 推理后，使用 codec `decode_full` 产出可播放 PCM，并符合现有播放契约。

#### Scenario: 正常文本合成返回有效音频
- **WHEN** 调用 `synthesize` 并提供非空文本
- **THEN** 系统必须返回 `TtsResult`，且音频数据长度大于零

#### Scenario: 输出格式不满足播放契约时失败
- **WHEN** 推理输出的采样率或声道数不等于 48kHz/2ch
- **THEN** 系统必须拒绝该结果并返回明确的格式约束错误

#### Scenario: 推理阶段失败时错误可区分
- **WHEN** tokenizer、TTS 推理或 codec 解码任一阶段发生错误
- **THEN** 返回错误必须包含失败阶段标识，便于定位问题

### Requirement: 音色配置映射行为可预测
系统必须支持 `TtsConfig.voice` 到 MOSS 内置音色的确定性映射，并定义默认音色与未知音色处理策略。

#### Scenario: 未指定音色时使用默认音色
- **WHEN** 调用 `synthesize` 且 `TtsConfig.voice` 为空
- **THEN** 系统必须使用预设默认音色完成合成

#### Scenario: 指定有效内置音色时按指定生效
- **WHEN** 调用 `synthesize` 且 `TtsConfig.voice` 匹配内置音色名
- **THEN** 系统必须使用该音色完成合成

#### Scenario: 指定未知音色时返回可读错误
- **WHEN** 调用 `synthesize` 且 `TtsConfig.voice` 不匹配任何内置音色
- **THEN** 系统必须返回未知音色错误，并包含可用音色提示信息

### Requirement: 与现有 TTS runtime 和播放联动兼容
系统必须在接入 MOSS 引擎后保持现有 TTS runtime 生命周期与播放联动行为，包括播放期间暂停录音/VAD、播放结束恢复监听。

#### Scenario: 合成后状态进入 Ready
- **WHEN** `synthesize_tts` 成功完成
- **THEN** TTS runtime 状态必须从 `synthesizing` 转为 `ready`

#### Scenario: 播放期间暂停录音并在结束后恢复
- **WHEN** `play_tts` 在存在活动录音会话时执行
- **THEN** 系统必须先停止监听，再开始播放，并在播放结束后恢复监听

#### Scenario: 播放取消后状态回到 Idle
- **WHEN** 播放过程中触发 `cancel_tts_playback`
- **THEN** 系统必须清理输出缓冲并将状态恢复到 `idle`

### Requirement: 变更必须通过完整质量门禁
系统必须为该能力提供可重复的测试与验证流程，覆盖 Rust 逻辑、桌面集成与前端回归。

#### Scenario: Rust 质量门禁通过
- **WHEN** 实现完成后执行 `cargo test` 与 `cargo clippy`
- **THEN** 命令必须通过或记录清晰阻塞原因

#### Scenario: 桌面构建验证通过
- **WHEN** 实现完成后执行 `pnpm tauri build`
- **THEN** 构建必须成功或记录清晰阻塞原因

#### Scenario: 前端回归验证通过
- **WHEN** 实现完成后执行 `pnpm build` 与 `pnpm test`
- **THEN** 命令必须通过或记录清晰阻塞原因
