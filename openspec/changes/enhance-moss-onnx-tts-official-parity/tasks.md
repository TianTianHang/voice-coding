## 1. 文本准备与长文本切块

- [ ] 1.1 在 `tts-moss` 中新增 MOSS 文本准备模块，覆盖 trim、空白折叠、标点规范化和常见数字/英文混排处理。
- [ ] 1.2 实现按 token budget 的长文本切块，优先在自然语言边界切分，并跳过空 chunk。
- [ ] 1.3 将 `MossOnnxTtsEngine::synthesize` 改为逐 chunk 合成并按顺序拼接 PCM。
- [ ] 1.4 为文本归一化、chunk token 上限、空 chunk 跳过和多 chunk 拼接补充无模型单元测试。

## 2. 采样模式配置

- [ ] 2.1 设计并实现兼容旧调用的 MOSS 采样模式配置入口，默认值为 `fixed`。
- [ ] 2.2 保留现有 `fixed` 快路径，并将未知采样模式映射为可读配置错误。
- [ ] 2.3 接入 deterministic `greedy` 采样路径，确保相同输入和配置可复现。
- [ ] 2.4 为默认模式、显式 fixed、显式 greedy 和未知模式补充单元测试或可选真实模型测试。

## 3. 参考音频克隆

- [ ] 3.1 加载并校验 `moss_audio_tokenizer_encode.onnx` session 及其关键输入输出。
- [ ] 3.2 复用或新增音频解码/重采样逻辑，将参考音频规范化为 codec encode 所需 48kHz 立体声 PCM。
- [ ] 3.3 实现 reference audio 到 prompt audio codes 的 codec encode 管线。
- [ ] 3.4 扩展 prompt construction，使 reference audio prompt 优先于内置音色 prompt。
- [ ] 3.5 为参考音频优先级、无效参考音频错误阶段和 codec encode 输出形状补充测试。

## 4. 内部 Streaming Codec Decode

- [ ] 4.1 加载并校验 `moss_audio_tokenizer_decode_step.onnx` session 及其状态输入输出。
- [ ] 4.2 实现按 frame batch 调用 codec decode step 的内部 PCM chunk 缓存。
- [ ] 4.3 将内部 chunk 拼接为完整 `TtsResult`，并保持 TTS runtime 仍在完整音频准备后进入 `ready`。
- [ ] 4.4 实现 decode step 不可用或失败时的 `decode_full` fallback 或明确错误返回。
- [ ] 4.5 为 streaming decode 成功、fallback 和失败不进入播放状态补充测试。

## 5. Session 校验与推理执行

- [ ] 5.1 扩展 `MossSessions::load` 的 session I/O 校验，覆盖 prefill、decode step、local sampling、codec encode、decode full 和 decode step。
- [ ] 5.2 将 MOSS ONNX CPU 推理移入 `spawn_blocking` 或专用串行 worker，保持共享 sessions 串行访问。
- [ ] 5.3 确保 ONNX `DynValue` 和中间 tensor 生命周期不跨线程泄露。
- [ ] 5.4 为 session I/O mismatch、并发合成串行化和 worker 错误传播补充测试。

## 6. Tauri 与前端调试入口

- [ ] 6.1 如需扩展 `TtsConfig` 或新增 MOSS 专用 config，更新 Tauri 命令参数和序列化类型。
- [ ] 6.2 在现有调试工具或 TTS 测试入口中增加采样模式选择和参考音频输入能力。
- [ ] 6.3 保持自动播报主流程默认配置不变，并验证 `auto-tts-state` 与 `tts-state` 事件契约未破坏。
- [ ] 6.4 为前端配置映射和状态展示补充 vitest 覆盖。

## 7. 验证与质量门禁

- [ ] 7.1 执行 `nix develop -c cargo test`，记录结果或阻塞原因。
- [ ] 7.2 执行 `nix develop -c cargo clippy`，记录结果或阻塞原因。
- [ ] 7.3 在已下载 MOSS 模型时执行可选真实模型集成测试，覆盖 fixed、greedy、长文本和 reference audio 场景。
- [ ] 7.4 执行 `pnpm build`，记录结果或阻塞原因。
- [ ] 7.5 执行 `pnpm test`，记录结果或阻塞原因。
- [ ] 7.6 若改动影响桌面集成或模型打包路径，执行 `nix develop -c pnpm tauri build`，记录结果或阻塞原因。
