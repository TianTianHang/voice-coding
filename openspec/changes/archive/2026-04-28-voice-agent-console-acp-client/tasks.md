## 1. 前端助手面板

- [x] 1.1 重构 `App` 和主布局，改为托盘优先的助手面板，保留当前一句输入区和输出流区。
- [x] 1.2 调整现有 VAD/ASR 状态呈现，让监听、录音、转写、发送和连接状态在面板中清晰可见。
- [x] 1.3 实现输出流的事件分区样式，区分 `thinking`、`tool`、`result`、`diff`、`confirm`、`error` 和 `status`。
- [x] 1.4 删除或隐藏不再需要的命令/skill 建议入口，确保输入区只显示当前一句。
- [x] 1.5 在不引入 UI 组件库的前提下建立助手面板 CSS 变量、状态色和紧凑布局。
- [x] 1.6 为 `confirm` 事件实现单独高亮区和确认/拒绝操作按钮。

## 2. ACP Client Runtime

- [x] 2.1 为 `tokio` 补充 ACP 子进程通信所需 feature，并确认不引入大型 JSON-RPC 框架。
- [x] 2.2 在 Rust 后端新增 `acp` 模块结构，包括 profile、JSON-RPC envelope、transport、session 和 event 类型。
- [x] 2.3 实现 ACP 子进程启动与 stdio 管道管理。
- [x] 2.4 实现 ACP 初始化、单一活动 agent 约束、活动会话维护和连续语义段复用同一会话的逻辑。
- [x] 2.5 将 ACP 输出归一化为内部事件流，并通过 Tauri 事件发送给前端。
- [x] 2.6 实现 `confirm` 事件确认 id、用户选择回传和处理状态更新。
- [x] 2.7 处理 agent 异常退出、连接失败和关闭时的清理逻辑。

## 3. 自动投递与会话串联

- [x] 3.1 将 VAD 结束后的转写结果自动发送到 ACP runtime。
- [x] 3.2 让发送失败、转写失败和 agent 错误在前端输出区可见。
- [x] 3.3 保持“开始一次后持续监听、自动切段、自动发送、回到下一句”的循环体验。

## 4. 托盘与窗口生命周期

- [x] 4.1 启用 Tauri tray 支持，并在 Rust 侧配置托盘菜单、显示窗口、隐藏窗口和退出动作。
- [x] 4.2 支持用户选择关闭窗口时隐藏到托盘或关闭即退出。
- [x] 4.3 明确退出动作负责停止监听、清理 agent 子进程并退出应用。
- [x] 4.4 保持 ASR 预热和后台运行逻辑在托盘模式下可用。
- [x] 4.5 评估全局快捷键是否进入本轮实现；若进入，则添加 `tauri-plugin-global-shortcut` 和对应权限配置。

## 5. 验证

- [x] 5.1 运行 `pnpm build` 和 `pnpm test` 验证前端重构。
- [x] 5.2 运行 `cargo test` 和 `cargo clippy` 验证 Rust 后端与 ACP runtime。
- [x] 5.3 运行 `pnpm tauri build` 验证桌面打包与窗口生命周期配置。
