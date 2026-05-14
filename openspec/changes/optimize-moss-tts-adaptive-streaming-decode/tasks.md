## 1. 规格与策略

- [x] 1.1 校验 OpenSpec change，确认 MOSS 外部流式 frame budget 需求有效。
- [x] 1.2 从 codec metadata 推导 frame rate，并设计内部两段式 budget 状态。

## 2. Rust 实现

- [x] 2.1 替换 `tts-moss` 当前 `FrameBudget` 的 `audioChunkMs` 主导逻辑，实现首包约 1.0s 固定启动。
- [x] 2.2 实现首包后的窗口 RTF、lead、目标缓冲和 `8/16/24/32` batch 档位调节。
- [x] 2.3 保持流式事件、最终 `TtsResult` 和非流式 fallback 行为不变。

## 3. 验证

- [x] 3.1 增加 frame budget 单元测试，覆盖首包 13 frames、慢 RTF、接近实时、低 lead、快生成降缓冲和 flush。
- [x] 3.2 运行 `openspec validate optimize-moss-tts-adaptive-streaming-decode --strict`。
- [x] 3.3 运行 `nix develop -c cargo test -p tts-core`。
- [x] 3.4 运行 `nix develop -c cargo test -p tts-moss`。
- [x] 3.5 运行 `nix develop -c cargo clippy -p tts-moss --all-targets`。
- [x] 3.6 记录可选真实模型性能测试命令及是否执行：未执行 ignored 真实模型性能测试；命令为 `nix develop -c cargo test -p tts-moss --test inference streaming_synthesis_is_realtime_on_this_machine -- --ignored --nocapture --exact`。
