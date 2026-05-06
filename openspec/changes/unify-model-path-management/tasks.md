## 1. 路径解析核心

- [x] 1.1 新增 Tauri 后端模型路径解析模块，定义模型类型、模型 ID、路径来源、模型包目录、引擎目录、旧布局标记、缺失文件和错误诊断结构
- [x] 1.2 实现环境变量、应用数据目录和开发目录可注入的解析上下文，避免单元测试依赖真实平台目录
- [x] 1.3 实现 ASR 路径解析优先级：`STT_MODEL_DIR`、`VOICE_CODING_MODEL_HOME`、应用数据目录、标准开发目录、旧 `./models` 布局
- [x] 1.4 实现 MOSS TTS 路径解析优先级：`MOSS_TTS_MODEL_DIR`、`VOICE_CODING_MODEL_HOME`、应用数据目录、标准开发目录、旧 `./models/moss-tts` 布局
- [x] 1.5 实现 Qwen3 ASR 和 MOSS TTS 必需文件诊断，返回结构化 `missingFiles` 与可读错误信息
- [x] 1.6 为路径优先级、标准布局、旧布局兼容、显式环境变量和缺失文件诊断添加 Rust 单元测试

## 2. 后端 Runtime 集成

- [x] 2.1 更新 ASR runtime，使 `Qwen3AsrEngine` 从统一路径解析结果创建，并保留顶层 `modelDir` 兼容字段
- [x] 2.2 扩展 ASR 状态快照和 `asr-status` 事件 payload，加入结构化模型路径诊断字段
- [x] 2.3 更新 TTS runtime 默认引擎创建逻辑，使 `MossOnnxTtsEngine` 从统一路径解析结果创建，而不是直接依赖 crate 默认环境解析
- [x] 2.4 扩展 TTS 状态快照和 `tts-state` 事件 payload，加入 `engineName` 与结构化模型路径诊断字段
- [x] 2.5 保留 `MossModelConfig::from_env()` 等 crate-level 兼容入口，确保已有 engine 级测试和直接使用场景不被破坏
- [x] 2.6 为 ASR/TTS runtime 状态序列化、失败快照和 startup error fallback 添加或更新 Rust 测试

## 3. 前端状态契约

- [x] 3.1 更新 TypeScript ASR 状态类型，增加模型路径诊断结构，并保持现有 `modelDir` 字段兼容
- [x] 3.2 更新 TTS 状态类型和消费逻辑，支持展示或保存 `engineName` 与模型路径诊断信息
- [x] 3.3 更新 `useAsrStatus`、TTS 状态加载相关测试和必要组件测试，覆盖新增字段不会破坏完整快照替换语义

## 4. 下载脚本与文档

- [x] 4.1 更新 `scripts/download_model.sh` 默认目标为 `${VOICE_CODING_MODEL_HOME:-models}/asr/qwen3-asr-0.6b-onnx`，并更新完成提示
- [x] 4.2 更新 `scripts/download_moss_tts_models.sh` 默认目标为 `${VOICE_CODING_MODEL_HOME:-models}/tts/moss-tts-nano-100m-onnx`，并保持 TTS/codec 兄弟目录结构
- [x] 4.3 确认两个下载脚本仍支持显式目标目录参数，且不会强制改写用户传入路径
- [x] 4.4 更新模型路径相关文档，写明 `VOICE_CODING_MODEL_HOME`、`STT_MODEL_DIR`、`MOSS_TTS_MODEL_DIR`、标准布局、旧布局兼容和完整路径契约

## 5. 验证与质量门禁

- [ ] 5.1 运行 `nix develop -c cargo test -p voice-coding`，验证 Tauri 后端路径解析与 runtime 集成测试
- [ ] 5.2 运行 `nix develop -c cargo test -p stt-qwen3`，确认 Qwen3 ASR crate 行为未被路径管理改造破坏
- [ ] 5.3 运行 `nix develop -c cargo test -p tts-moss`，确认 MOSS TTS crate 兼容入口和资产校验仍正常
- [ ] 5.4 运行 `nix develop -c cargo clippy`，修复 Rust lint 问题
- [ ] 5.5 运行 `pnpm test`，验证前端 hook 与组件测试
- [ ] 5.6 运行 `pnpm build`，验证 TypeScript 类型和前端构建
- [ ] 5.7 运行 `nix develop -c pnpm tauri build`，验证桌面应用集成构建
- [ ] 5.8 手动执行或 dry-run 检查两个下载脚本的默认路径和显式路径提示，记录无法执行的外部网络依赖原因
