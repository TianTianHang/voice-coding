## Context

当前链路已经具备：VAD 识别后把转写文本送入 Agent，Agent 事件流再回到前端展示，TTS 也已有非流式合成与播放能力。但 TTS 仍然是独立的手动测试路径，没有作为 Agent 回复的自然延伸，因此需要把“播报回复”提升为后端能力。

这个变更的核心约束是：只播报最终 `result`，播报内容直接取 `resultEvent.content`，不引入流式合成，不把前端变成播报编排者。

## Goals / Non-Goals

**Goals:**
- Agent 最终回复可自动进入 TTS 合成并播放。
- 播报只针对 `result` 事件，不对中间状态发声。
- 前端仍可介入：启停自动播报、停止当前播报、查看状态、重播最近结果。
- 复用现有非流式 TTS runtime、VAD 暂停/恢复播放逻辑与现有事件流。

**Non-Goals:**
- 不实现流式 TTS。
- 不改变 Agent 的消息协议结构。
- 不把 `thinking` / `tool` / `status` 事件纳入播报范围。
- 不重做前端整体语音交互状态机。

## Decisions

### 1. 自动播报托管在后端
自动播报由 Rust 后端负责，而不是由 React 前端监听 `agent-event` 后自行调度。这样可以把“收到最终回复 → 合成 → 播放 → 暂停/恢复录音”的生命周期放在一个地方管理，减少前端状态碎片化。

**Alternatives considered:**
- 前端监听 result 后直接调用 TTS：实现快，但把语音交互编排绑到 UI。
- Agent 层直接调用 TTS：耦合到协议层，后续扩展控制面不方便。

### 2. 触发条件只看最终 result
只有 `result` 事件会触发自动播报，且播报文本固定为 `resultEvent.content`。

**Alternatives considered:**
- 基于 raw payload 或 content blocks 提取更复杂的展示文本：灵活，但会扩大文本选择面，增加不确定性。
- 同时播报 status/tool：会产生噪音，不符合“只播报最终答案”的产品意图。

### 3. 继续使用非流式 TTS 合成 + 播放
自动播报复用现有 `TtsRuntime::synthesize` 和 `play_tts` 的能力，不引入 `synthesize_stream`。当前引擎已经提供完整音频结果，且播放层已负责暂停/恢复 VAD，适合直接复用。

**Alternatives considered:**
- 先做流式 TTS：与需求不符，且需要更多协议和状态管理。
- 前端本地播放：会绕开现有后端播放控制与 VAD 配合逻辑。

### 4. 增加独立的自动播报控制面
新增少量后端命令，让前端能够介入自动播报流程，但不接管整个编排。
建议的控制面：
- `set_auto_tts_enabled(enabled: bool)`
- `get_auto_tts_status()`
- `stop_auto_tts()`
- `speak_latest_result()`

其中 `stop_auto_tts()` 应优先停止当前播放；`speak_latest_result()` 允许前端在需要时重播最近一条 result。

### 5. 在后端保存最小状态
自动播报需要保存：
- 是否启用
- 当前是否正在播报
- 最近一次已播报的 result 标识或内容指纹
- 最近一次 result 内容
- 当前 TTS / 播放状态快照

这可以放在 `TtsRuntime` 内部，或拆一个很小的协调器结构与 `TtsRuntime` 并列持有。

## Risks / Trade-offs

- [重复播报] Agent 可能重新发出相同 result 或 UI 重连重复消费事件 → 用事件 id / 内容指纹去重。
- [自我打断] 播放时 VAD 可能重新识别到自己的声音 → 复用 `play_tts` 的暂停/恢复录音路径。
- [状态竞争] Agent 快速连续产出多个 result 时，自动播报与手动停止可能交错 → 通过单一后端状态锁串行化播报请求。
- [前端误解状态] 如果只暴露 TTS 状态，前端可能无法区分“自动播报已关”与“当前未说话” → 需要单独的 auto-speech 状态字段。
