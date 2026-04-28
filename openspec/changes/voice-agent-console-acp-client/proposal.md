## Why

当前应用已经具备基于 VAD 的连续语音分段和 ASR 转写能力，但整体交互仍更像一个语音录入工具，而不是面向 coding agent 的常驻前台。项目目标已经转向 ACP，因此需要把产品形态收敛为“语音输入后自动投递到 agent”的控制台，并为所有兼容 ACP 的 agent 留出统一接入面。

## What Changes

- 将前端从普通桌面窗口调整为托盘优先的助手面板，支持显示、隐藏和持续后台运行。
- 保持“点击开始后持续监听、VAD 自动切段、转写后自动发送”的单句工作流。
- 输入区仅展示当前一句语音的实时转写，不保留长输入历史。
- 输出区保留 agent 的完整流式信息，并按事件类型区分展示。
- 在 Rust 后端新增 ACP client runtime，用于启动任意兼容 ACP 的 agent 子进程并与之通信。
- 将前端语音事件、ASR 结果和 ACP 流式事件串成统一会话。

## Capabilities

### New Capabilities
- `assistant-console-ui`: 托盘常驻的语音助手面板、当前一句输入、分类型输出流和基础状态反馈。
- `acp-client-runtime`: 通过 stdio 接入兼容 ACP 的 agent，管理会话、转发输入并归一化输出事件。

### Modified Capabilities
- 无。

## Impact

前端会影响 `src/App.tsx`、`src/components/`、`src/hooks/` 和全局样式；后端会影响 `src-tauri/src/` 中的命令注册、进程管理和事件分发。该变更还会引入 Tauri 托盘和快捷键相关依赖，以及 ACP 运行时所需的进程与 JSON-RPC 适配逻辑。测试层面将同时覆盖 React、Rust 和桌面打包流程。
