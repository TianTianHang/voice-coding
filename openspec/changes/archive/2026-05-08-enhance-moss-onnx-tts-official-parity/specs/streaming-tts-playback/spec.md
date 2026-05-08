## ADDED Requirements

### Requirement: 内部流式解码保持完整音频播放契约
系统 SHALL 允许 TTS 引擎在内部使用流式音频解码优化合成，但对播放层暴露的契约仍为合成完成后提供完整音频。

#### Scenario: 内部 chunk 用于缓存
- **WHEN** TTS 引擎在合成过程中产生内部音频 chunk
- **THEN** 系统 SHALL 将 chunk 缓存在 TTS 合成结果中
- **AND** 系统 MUST NOT 因 chunk 可用而提前进入播放状态

#### Scenario: 完整音频形成后进入 Ready
- **WHEN** 所有内部 chunk 已拼接为完整 PCM
- **THEN** 系统 SHALL 验证完整音频满足播放格式
- **AND** 系统 SHALL 允许 TTS runtime 从 `synthesizing` 转为 `ready`

#### Scenario: 内部流式失败不破坏恢复策略
- **WHEN** TTS 引擎内部流式解码失败
- **THEN** 系统 SHALL 返回合成错误或使用已定义 fallback
- **AND** 系统 MUST NOT 进入播放状态
- **AND** 系统 SHALL 保持录音恢复策略可执行
