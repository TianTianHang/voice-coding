## Context

当前自动播报能力在 ACP turn 收到 `StopReason` 后，从 `AgentResultTracker` 读取最新 `result` 的完整内容，并直接调用 `TtsRuntime::speak_agent_result()` 合成播放。这个设计简单，但把“给 UI 看的最终回答”和“适合朗读的短口语回复”绑定在同一段文本上。

新的产品意图是让 agent 显式决定是否发声以及说什么：只有最终 `result` 中恰好一对 `<tts>...</tts>` 标签内的内容能进入 TTS。UI 仍展示 agent 的完整回答语义，但不显示 `<tts>` 协议块。

## Goals / Non-Goals

**Goals:**
- 自动 TTS 只播报最终 result 中唯一一对有效 `<tts>...</tts>` 标签内的文本。
- 未提供有效单个 `<tts>` 块时严格跳过播报，不回退到完整 result。
- UI 展示 result 时隐藏所有 `<tts>...</tts>` 块。
- 保持现有后端托管自动播报、非流式合成、播放控制、停止和重播入口。
- 为 agent prompt/profile 增加明确输出契约，要求 `<tts>` 内只能包含短自然口语文本。

**Non-Goals:**
- 不实现流式 TTS。
- 不改变 ACP SDK 或外部协议结构。
- 不把 TTS 标签作为 Markdown/HTML 渲染能力开放给 UI。
- 不在后端自动总结、改写或清洗完整 result 作为兜底播报。

## Decisions

### 1. 使用严格 `<tts>` 单块协议

后端只接受恰好一对完整 `<tts>...</tts>` 标签。没有标签、多对标签、嵌套标签、标签不完整、大小写不匹配或标签内容为空时，自动 TTS 跳过播报。

**Rationale:** 自动播报是高打扰通道，宁可少说也不要误说。严格协议能避免把代码、日志、工具输出和长结果误送入 TTS。

**Alternatives considered:**
- 无标签时回退播报完整 result：迁移平滑，但保留了当前问题。
- 多个标签时取第一个：容错更高，但会掩盖 agent 违反协议的问题。
- 使用 JSON content block：结构更强，但当前 ACP 消息链路主要以文本 chunk 进入前端和 tracker，会扩大协议改造范围。

### 2. 分离三种文本视图

同一个 agent result 需要保留三种视图：
- 原始 result 文本：包含 agent 实际输出，用于解析、调试和必要的 raw payload。
- UI 展示文本：从原始 result 中移除所有完整 `<tts>...</tts>` 块。
- TTS 文本：仅当原始 result 中存在唯一有效 `<tts>` 块时，为标签内 trim 后的内容。

```
raw result
   │
   ├── strip complete <tts> blocks ──▶ UI display result
   │
   └── validate exactly one block ───▶ spoken TTS text or skip
```

**Rationale:** UI 和 TTS 的文本目标不同。分离视图后，UI 不泄漏协议标签，TTS 也不依赖展示格式。

**Alternatives considered:**
- 在前端隐藏标签，后端仍播完整 result：UI 干净但语音仍不纯净。
- 后端修改 result event content 为隐藏后的文本：简单，但可能丢失调试原文，需要确保 raw 或内部 tracker 仍能访问原始文本。

### 3. 在后端 TTS 触发路径执行严格提取

自动播报的提取和校验应位于后端自动 TTS 触发路径，而不是前端。ACP result tracker 可以继续累积最终文本；触发播报时从完整 result 中提取 `<tts>` 文本，只有有效时才合成播放。

**Rationale:** 自动播报已经由后端托管，提取逻辑放在后端能保证所有入口一致，包括自动播报和“重播最近结果”。

**Alternatives considered:**
- 前端监听 result 后提取并调用 TTS：会把播报编排拆回 UI。
- 要求 agent 额外发送独立 TTS event：协议更清楚，但需要改变 agent/ACP 输出结构。

### 4. UI 隐藏所有完整 `<tts>` 块

前端展示 result 时移除所有完整 `<tts>...</tts>` 块。即使因为多对标签导致后端跳过播报，UI 也不显示这些协议块。

**Rationale:** `<tts>` 是控制协议，不是用户内容。多标签场景下隐藏所有完整块可以避免 UI 泄漏协议细节。

**Alternatives considered:**
- 只隐藏有效单块：多标签时用户会看到协议残留。
- 完全由后端发送已清洗 display content：可以做，但需要确保增量流式 UI 合并时不会因为标签跨 chunk 出现闪烁或残留。

### 5. 去重基于 TTS 文本

自动播报的 latest spoken key 应基于 result id 和提取出的 TTS 文本生成。没有有效 TTS 文本的 result 不应更新已播报 key。

**Rationale:** UI 正文可能变化但口播内容相同；去重应以真正发声的文本为核心。

## Risks / Trade-offs

- [Agent 未遵守提示词导致不播报] → 在 agent profile/system prompt 中明确 `<tts>` 规则，并在自动 TTS 状态中保留“跳过”原因供调试。
- [标签跨 chunk 导致 UI 短暂显示协议块] → 优先在最终合并展示模型层清洗；若当前 UI 必须实时显示 chunk，可在合并后的块内容上持续应用隐藏规则。
- [用户本意输出字面 `<tts>` 文本] → 该标签被保留为控制协议；如确需展示，应转义或避免使用裸标签。
- [解析规则过严造成静默] → 严格模式是产品选择；通过状态和测试保证静默是可观察、可解释的。
- [大小写或属性兼容性] → 第一版只接受精确小写 `<tts>` 和 `</tts>`，不支持属性，避免协议歧义。

## Migration Plan

1. 增加 `<tts>` 提取/隐藏辅助逻辑和测试。
2. 将自动 TTS 触发路径改为严格提取有效 TTS 文本。
3. 调整 UI result 展示逻辑，隐藏完整 `<tts>` 块。
4. 更新 agent profile/system prompt，让 agent 仅在需要播报时输出恰好一对 `<tts>` 标签。
5. 更新状态和测试，确认未带标签的 result 不再播报。

回滚方式：恢复自动 TTS 使用完整 result content 的旧逻辑，并移除或忽略 UI 隐藏规则。

## Open Questions

- 自动 TTS 状态是否需要新增细分原因，例如 `SkippedMissingTag`、`SkippedInvalidTag`、`SkippedEmptyTag`、`SkippedDuplicate`，还是先复用现有跳过/空闲状态并仅在日志中记录原因？
