## Requirements

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

### Requirement: 官方鲁棒文本归一化
MOSS ONNX TTS 引擎 SHALL 在分词和切块前执行与官方 MOSS robust normalizer 行为等价的鲁棒文本归一化，以避免符号密集文本导致漏读或异常声音。

#### Scenario: 清理 Markdown 展示语法
- **WHEN** 合成文本包含 Markdown 标题、引用、列表、强调、表格分隔符、链接、代码围栏或 inline code 标记
- **THEN** 系统 SHALL 移除不适合朗读的 Markdown 语法标记
- **AND** 系统 SHALL 保留可朗读的自然语言内容
- **AND** 系统 MUST NOT 将反引号、井号、列表连字符或表格竖线原样送入 MOSS 分词

#### Scenario: 规范符号密集技术文本
- **WHEN** 合成文本包含箭头、连续破折号、下划线、斜杠、重复标点、零宽字符或控制字符
- **THEN** 系统 SHALL 将其转换为稳定的空格、停顿或句子边界
- **AND** 系统 MUST 删除零宽字符和控制字符
- **AND** 系统 MUST collapse repeated punctuation into a bounded spoken punctuation form

#### Scenario: 保护可读技术片段
- **WHEN** 合成文本包含 URL、Email、mention、hashtag、文件名、扩展名、版本号或短技术标识符
- **THEN** 系统 SHALL 在通用符号替换前保护这些片段
- **AND** 系统 SHALL 避免把 `.env`、`app.js.map`、`v2.3.1` 等片段拆成会破坏语义或导致异常发音的字符序列

#### Scenario: 符号-only 文本不进入推理
- **WHEN** 文本经过鲁棒归一化后没有可朗读内容
- **THEN** 系统 MUST NOT 调用 MOSS ONNX 推理
- **AND** 系统 SHALL 返回可定位的 invalid input error

#### Scenario: Debug TTS 同样应用鲁棒归一化
- **WHEN** 用户在 debug 面板手动合成包含 Markdown、路径、URL 或符号密集内容的文本
- **THEN** 系统 SHALL 使用同一套 MOSS 鲁棒归一化流程
- **AND** 系统 SHALL 在归一化后继续执行现有 token budget 切块和多 chunk 拼接流程
