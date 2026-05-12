## MODIFIED Requirements

### Requirement: MOSS 采样模式可配置
MOSS ONNX TTS 引擎 SHALL 支持调用方选择 MOSS 音频 token 采样模式，并在未指定时使用 fixed sampling。系统还 SHALL 支持 Debug 链路传入 MOSS 私有采样配置，其中 `seed` 与 `maxNewFrames` 在当前 fixed sampling 实现中生效，其他采样常量字段作为未来参数化采样契约保留。

#### Scenario: 未指定采样模式时使用 fixed
- **WHEN** 调用 `synthesize` 且没有提供 MOSS 采样模式
- **THEN** 系统 SHALL 使用 fixed sampling 生成音频 frames

#### Scenario: 指定 fixed 采样模式
- **WHEN** 调用 `synthesize` 且采样模式为 `fixed`
- **THEN** 系统 SHALL 使用 `moss_tts_local_fixed_sampled_frame.onnx` 生成音频 frames

#### Scenario: 指定 greedy 采样模式
- **WHEN** 调用 `synthesize` 且采样模式为 `greedy`
- **THEN** 系统 SHALL 使用确定性采样路径生成音频 frames
- **AND** 对相同输入、相同模型和相同配置 MUST 产生可复现的 frame 序列

#### Scenario: 指定未知采样模式
- **WHEN** 调用 `synthesize` 且采样模式不是系统支持的 MOSS 采样模式
- **THEN** 系统 MUST 拒绝合成
- **AND** 错误信息 SHALL 包含未知采样模式和可用模式列表

#### Scenario: 指定 fixed sampling seed
- **WHEN** 调用 `synthesize` 且 `TtsConfig.moss.seed` 有值
- **THEN** 系统 SHALL 使用该 seed 初始化 fixed sampling 随机数生成器
- **AND** 相同文本、模型、音色和 seed 在 fixed sampling 路径中 SHALL 产生可复现的随机输入序列

#### Scenario: 指定 maxNewFrames
- **WHEN** 调用 `synthesize` 且 `TtsConfig.moss.maxNewFrames` 有值
- **THEN** 系统 SHALL 使用该值作为本次 frame generation loop 的最大 frame 数
- **AND** 未指定时 SHALL 继续使用模型 manifest 中的默认上限

#### Scenario: 预留采样常量字段
- **WHEN** 调用 `synthesize` 且提供 temperature、top-p、top-k 或 repetition penalty 字段
- **THEN** 系统 SHALL 能反序列化并保留这些字段
- **AND** 当前 fixed ONNX 图未参数化时，系统 MAY 不改变推理结果
