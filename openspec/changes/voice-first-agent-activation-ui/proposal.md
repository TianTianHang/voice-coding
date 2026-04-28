## Why

当前助手控制台已经具备事件流与状态展示能力，但整体交互仍偏向“按钮驱动的控制台”，与用户期望的“语音优先、被唤醒的助手感”存在明显差距。需要通过一次以体验为核心的界面重构，将主交互从显式点击转向语音唤醒与连续对话，降低操作负担并提升语音代理的在场感与信任感。

## What Changes

- 将现有助手主界面重构为“语音优先”的舞台式布局，以语音存在感组件（Presence Orb）和状态文案作为第一视觉焦点。
- 引入标准化语音状态机（Dormant、WakeDetected、Listening、Processing、Responding、Error），统一状态语义与界面反馈。
- 增加“激活仪式”与状态过渡动效规范，确保用户可感知助手从待机到唤醒、处理到回应的完整过程。
- 将实时反馈收敛为最小可信闭环：Heard（听到内容）、Intent（理解意图）、Status（执行状态）、Response（最新回应）。
- 将低频配置与兜底控制降级到次要区域，保留纯语音主路径下的最小点击依赖。
- 调整输出流与时间线的展示优先级：默认聚焦当前轮交互，按需展开完整事件历史。

## Capabilities

### New Capabilities
- `voice-agent-activation-experience`: 定义语音助手在场感、唤醒感、状态机与动效反馈的体验要求。

### Modified Capabilities
- `assistant-console-ui`: 将现有控制台能力从“信息并列展示”升级为“语音优先的层级化交互”，并更新主界面信息架构与可见反馈要求。

## Impact

- 前端：`src/components/AssistantConsole.tsx`、`src/components/AgentEventStream.tsx`、`src/components/AudioVisualizer.tsx`、`src/App.css` 等 UI 相关模块将进行结构与视觉层级调整。
- 状态与事件：`useBackendVAD`、`useAgentEvents` 的状态映射和展示语义需要对齐新的体验状态机与文案规范。
- 可用性与可访问性：需要验证键盘可达、可读性对比度、`prefers-reduced-motion` 降级行为，以及窄屏下的语音主路径一致性。
- 质量验证：实施后需要执行 `pnpm build` 与 `pnpm test`，并补充或更新前端交互测试以覆盖关键状态迁移与展示逻辑。
