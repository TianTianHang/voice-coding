## MODIFIED Requirements

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
