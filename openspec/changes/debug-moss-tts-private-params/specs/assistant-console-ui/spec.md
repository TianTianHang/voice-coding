## ADDED Requirements

### Requirement: Debug TTS 暴露 MOSS 私有参数
Debug 工具窗口 SHALL 允许开发者为 `debug_synthesize_tts` 配置 MOSS 私有参数，并只发送有效的可解析字段。

#### Scenario: 构造 Debug TTS 配置
- **WHEN** 开发者在 Debug TTS 面板输入 voice、seed、maxNewFrames、参考音频路径或采样常量字段
- **THEN** 前端 SHALL 将有效字段映射到 `TtsConfig` 的 camelCase invoke payload
- **AND** 空字符串字段 SHALL 被省略
- **AND** 非有限数字字段 SHALL 被省略

#### Scenario: 标注 fixed 图限制
- **WHEN** Debug TTS 面板展示 temperature、top-p、top-k 或 repetition penalty 输入
- **THEN** 界面 SHALL 提示这些字段当前已发送但 fixed ONNX 图中仍为 baked constants
- **AND** 界面 SHALL 提示 `seed` 与 `maxNewFrames` 当前生效

### Requirement: Debug TTS 支持流式播放进度
Debug 工具窗口 SHALL 支持直接触发 TTS 流式合成播放，并展示播放进度条。

#### Scenario: 流式播放音频 chunk
- **WHEN** 开发者点击 Debug TTS 流式播放入口
- **THEN** 后端 SHALL 使用当前 TTS 配置启动流式合成
- **AND** 每个可播放音频 chunk 可用时 SHALL 立即进入音频输出队列
- **AND** 前端 SHALL 根据 Debug TTS 流式事件更新播放条

#### Scenario: 流式播放完成
- **WHEN** 后端完成流式合成并播放完已入队音频
- **THEN** 前端 SHALL 将播放进度显示为完成
- **AND** TTS runtime 状态 SHALL 回到 idle 或可再次触发 Debug 播放的状态
