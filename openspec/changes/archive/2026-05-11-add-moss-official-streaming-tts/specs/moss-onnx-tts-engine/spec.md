## MODIFIED Requirements

### Requirement: 官方等价文本准备
MOSS ONNX TTS 引擎 SHALL 在分词和推理前执行官方 ONNX 推理等价的文本准备流程，包括文本归一化、空片段过滤和长文本切块。

#### Scenario: 文本归一化后进入分词
- **WHEN** 调用 `synthesize` 且文本包含多余空白、常见中英文标点、数字或英文混排
- **THEN** 系统 SHALL 先对文本执行 MOSS 文本归一化
- **AND** 系统 SHALL 使用归一化后的文本生成 token ids

#### Scenario: 官方字符处理进入分词前生效
- **WHEN** 合成文本包含全角 ASCII、全角标点、弯引号、零宽字符、控制字符、百分号、常见连接运算符或英文短句
- **THEN** 系统 SHALL 将字符归一化为稳定的半角或可朗读形式
- **AND** 系统 SHALL 删除不适合朗读的零宽字符和控制字符
- **AND** 英文短句按官方运行时策略需要前导空格时，最终进入 tokenizer 的第一个文本 chunk MUST 保留该前导空格

#### Scenario: 中文连字符和负数保护
- **WHEN** 中文文本包含中文词之间的连字符、数字前负号或数字范围样式
- **THEN** 系统 SHALL 在通用符号清理前保护负数或数字范围语义
- **AND** 中文词之间的连字符 SHALL 转换为自然停顿
- **AND** 系统 MUST NOT 将负数前的 `-` 误转换为句子边界

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

### Requirement: MOSS 外部流式合成 SHALL 映射 codec decode step chunk
MOSS ONNX TTS 引擎 SHALL 在实现 core 流式 TTS session 时，按官方 runtime 的 frame callback 与 codec streaming decode 模式，将合成过程中产生的 PCM chunk 按顺序映射为对外可消费的 TTS 流式音频事件。

#### Scenario: TTS frame 生成驱动 codec streaming decode
- **WHEN** MOSS 流式 session 开始处理一个可朗读文本片段
- **THEN** 系统 SHALL 在 TTS frame generation loop 中逐步生成 audio frames
- **AND** 每个 frame 完成 TTS decode state 更新后 SHALL 可进入 codec streaming decode 的 pending frame buffer
- **AND** 系统 MUST NOT 等待该文本片段的全部 frames 生成完毕后才允许 codec decode step 产出首个音频 chunk

#### Scenario: codec decode step chunk 映射为流式音频事件
- **WHEN** MOSS 引擎在流式 session 中使用 codec decode step 分批解码 PCM
- **THEN** 每个可用 PCM chunk SHALL 被包装为 `AudioChunk` 事件
- **AND** 事件 SHALL 保留递增 sequence
- **AND** 事件音频 SHALL 满足 48kHz 立体声 PCM 格式
- **AND** chunk 时间范围 SHOULD 根据已产出样本数推导

#### Scenario: codec streaming state 按 batch 更新
- **WHEN** codec decode step 完成一个 pending frame batch
- **THEN** 系统 SHALL 从 ONNX 输出中提取 transformer offsets、attention cache keys、attention cache values 和 cached positions
- **AND** 下一次 codec decode step SHALL 使用更新后的 streaming state
- **AND** 缺失必要 state 输出时 SHALL 返回包含 `codec_decode_step` 阶段标识的错误

#### Scenario: 动态 frame budget 控制首包延迟和吞吐
- **WHEN** 流式 session 正在生成并解码 audio frames
- **THEN** 系统 SHALL 使用 frame budget 决定何时对 pending frames 运行 codec decode step
- **AND** 启动阶段 SHALL 允许较小 frame budget 以降低首包延迟
- **AND** 已产出音频缓冲增加后 SHALL 允许扩大 frame budget 以提高吞吐
- **AND** 文本片段结束时 MUST flush 所有 pending frames

#### Scenario: 流式路径不可静默回退为 full decode
- **WHEN** 外部流式 session 已经启动并依赖 codec decode step 产出事件
- **AND** codec decode step 初始化或推理失败
- **THEN** 系统 SHALL 返回可定位的流式合成错误
- **AND** 系统 MUST NOT 在已承诺外部流式事件语义后静默回退到 `decode_full`
- **AND** 现有批量 `synthesize` 的 fallback 行为 MAY 保持不变

#### Scenario: V1 未实现外部流式时保持明确错误
- **WHEN** MOSS 引擎尚未实现外部流式 TTS session
- **THEN** 调用流式能力 SHALL 返回明确的 unsupported 错误
- **AND** 现有批量 `synthesize` 行为 MUST 保持不变
