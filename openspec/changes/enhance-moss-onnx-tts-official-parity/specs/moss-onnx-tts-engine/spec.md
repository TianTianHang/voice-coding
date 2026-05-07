## ADDED Requirements

### Requirement: 官方等价文本准备
MOSS ONNX TTS 引擎 SHALL 在分词和推理前执行官方 ONNX 推理等价的文本准备流程，包括文本归一化、空片段过滤和长文本切块。

#### Scenario: 文本归一化后进入分词
- **WHEN** 调用 `synthesize` 且文本包含多余空白、常见中英文标点、数字或英文混排
- **THEN** 系统 SHALL 先对文本执行 MOSS 文本归一化
- **AND** 系统 SHALL 使用归一化后的文本生成 token ids

#### Scenario: 空片段不会触发推理
- **WHEN** 文本准备流程产生空 chunk
- **THEN** 系统 MUST 跳过该 chunk
- **AND** 系统 MUST NOT 对空 chunk 调用 MOSS ONNX 推理

#### Scenario: 长文本按 token budget 切块
- **WHEN** 归一化后的文本超过单次 MOSS 合成 token budget
- **THEN** 系统 SHALL 将文本切分为多个 chunk
- **AND** 每个 chunk 的 token 数 MUST 不超过配置的最大 token 数
- **AND** 系统 SHALL 优先在自然语言边界切分

#### Scenario: 多 chunk 音频拼接
- **WHEN** 长文本被切分为多个 chunk
- **THEN** 系统 SHALL 对每个 chunk 独立完成 MOSS 合成
- **AND** 系统 SHALL 将所有 chunk 的 PCM 按文本顺序拼接为一个 `TtsResult`
- **AND** 拼接后的音频 MUST 仍满足 48kHz 立体声播放契约

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
- **THEN** 系统 SHALL 使用确定性采样路径生成音频 frames
- **AND** 对相同输入、相同模型和相同配置 MUST 产生可复现的 frame 序列

#### Scenario: 指定未知采样模式
- **WHEN** 调用 `synthesize` 且采样模式不是系统支持的 MOSS 采样模式
- **THEN** 系统 MUST 拒绝合成
- **AND** 错误信息 SHALL 包含未知采样模式和可用模式列表

### Requirement: 参考音频克隆
MOSS ONNX TTS 引擎 SHALL 支持将参考音频编码为 prompt audio codes，并用其作为本次合成的音色提示。

#### Scenario: 使用参考音频合成
- **WHEN** 调用 TTS 合成并提供有效参考音频
- **THEN** 系统 SHALL 将参考音频解码并规范化为 MOSS codec 所需格式
- **AND** 系统 SHALL 使用 `moss_audio_tokenizer_encode.onnx` 生成 prompt audio codes
- **AND** 系统 SHALL 使用这些 prompt audio codes 构造 TTS prompt

#### Scenario: 参考音频优先于内置音色
- **WHEN** 同一次合成同时提供参考音频和内置音色名称
- **THEN** 系统 SHALL 使用参考音频生成的 prompt audio codes
- **AND** 系统 MUST NOT 使用内置音色的 prompt audio codes 作为本次合成音色来源

#### Scenario: 参考音频无效
- **WHEN** 参考音频无法解码、重采样或通过 codec encode
- **THEN** 系统 MUST 拒绝合成
- **AND** 错误信息 SHALL 包含 `reference_audio` 或 `codec_encode` 阶段标识

### Requirement: MOSS 内部流式 codec 解码
MOSS ONNX TTS 引擎 SHALL 支持使用 codec decode step 在合成过程中分批解码音频 chunk，并在外部返回完整音频结果。

#### Scenario: 使用 decode step 分批解码
- **WHEN** MOSS 引擎启用内部 streaming decode
- **THEN** 系统 SHALL 在生成音频 frames 的过程中按批次调用 `moss_audio_tokenizer_decode_step.onnx`
- **AND** 系统 SHALL 将每批输出追加到内部 PCM buffer
- **AND** 合成完成后 SHALL 返回包含完整 PCM 的 `TtsResult`

#### Scenario: decode step 不改变外部播放契约
- **WHEN** MOSS 引擎使用内部 streaming decode 完成合成
- **THEN** TTS runtime SHALL 仍在完整 `TtsResult` 准备好后进入 `ready`
- **AND** 系统 MUST NOT 在合成未完成时开始播放

#### Scenario: decode step 不可用时回退
- **WHEN** 内部 streaming decode 被启用但 codec decode step 初始化或推理失败
- **THEN** 系统 SHALL 能回退到 `decode_full` 路径或返回包含 `codec_decode_step` 阶段标识的错误
- **AND** 回退成功时最终音频 MUST 满足播放契约

### Requirement: 完整 ONNX 会话校验
MOSS ONNX TTS 引擎 SHALL 在健康检查或 session 初始化阶段校验所有使用到的 ONNX 模型输入输出。

#### Scenario: 校验 TTS local sampling 模型
- **WHEN** 系统初始化 MOSS ONNX sessions
- **THEN** 系统 SHALL 校验 local sampling 模型包含实现所需输入输出
- **AND** 缺失时 MUST 返回 metadata 或 session I/O mismatch 错误

#### Scenario: 校验 codec encode 模型
- **WHEN** 系统支持参考音频克隆
- **THEN** 系统 SHALL 校验 codec encode 模型包含实现所需输入输出
- **AND** 缺失时 MUST 在健康检查或首次使用前返回可定位错误

#### Scenario: 校验 codec decode step 模型
- **WHEN** 系统支持内部 streaming decode
- **THEN** 系统 SHALL 校验 codec decode step 模型包含实现所需输入输出
- **AND** 缺失时 MUST 在健康检查或启用 streaming decode 前返回可定位错误

### Requirement: MOSS 推理不得阻塞 async runtime
MOSS ONNX TTS 引擎 SHALL 将 CPU 密集型推理放入阻塞 worker 或等效串行执行器，避免长文本合成阻塞 Tauri async runtime。

#### Scenario: 合成请求进入阻塞执行器
- **WHEN** 调用 `synthesize` 执行 MOSS ONNX 推理
- **THEN** 系统 SHALL 在阻塞 worker 或专用推理线程中运行 ONNX session 推理
- **AND** async 调用方 SHALL 通过 await 接收最终结果

#### Scenario: session 缓存保持串行一致性
- **WHEN** 多个 TTS 合成请求接近同时到达
- **THEN** 系统 SHALL 串行访问共享 MOSS ONNX sessions
- **AND** 系统 MUST NOT 跨线程传递不安全的 ONNX 中间 tensor 生命周期
