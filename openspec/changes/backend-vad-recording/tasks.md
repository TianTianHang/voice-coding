# Implementation Tasks

## 1. 准备工作 (5分钟)

- [ ] 1.1 创建目录结构 `src-tauri/libs/Linux/x64/`
- [ ] 1.2 复制 ten-vad 预编译库 `libten_vad.so` 到 `src-tauri/libs/Linux/x64/`
- [ ] 1.3 验证库文件存在 `file src-tauri/libs/Linux/x64/libten_vad.so`
- [ ] 1.4 更新 `tauri.conf.json` 添加 `"resources": ["libs/**"]` 到 bundle 配置

## 2. 添加 Rust 依赖 (2分钟)

- [ ] 2.1 运行 `cargo add cpal` 添加音频库
- [ ] 2.2 运行 `cargo add libloading` 添加动态库加载
- [ ] 2.3 运行 `cargo add crossbeam-channel` 添加跨线程通信
- [ ] 2.4 运行 `cargo add parking_lot` 添加高性能互斥锁
- [ ] 2.5 验证 `Cargo.toml` 包含所有新依赖

## 3. 创建 Rust 模块结构 (5分钟)

- [ ] 3.1 创建目录 `src-tauri/src/vad/`
- [ ] 3.2 创建目录 `src-tauri/src/audio/`
- [ ] 3.3 创建文件 `src-tauri/src/vad/mod.rs` (模块导出)
- [ ] 3.4 创建文件 `src-tauri/src/vad/config.rs` (配置常量)
- [ ] 3.5 创建文件 `src-tauri/src/vad/engine.rs` (FFI 封装)
- [ ] 3.6 创建文件 `src-tauri/src/vad/state_machine.rs` (状态机)
- [ ] 3.7 创建文件 `src-tauri/src/audio/mod.rs` (模块导出)
- [ ] 3.8 创建文件 `src-tauri/src/audio/recorder.rs` (cpal 录音机)
- [ ] 3.9 创建文件 `src-tauri/src/vad_commands.rs` (Tauri 命令)

## 4. 实现 VAD 配置模块 (10分钟)

- [ ] 4.1 在 `vad/config.rs` 中定义常量: `HOP_SIZE`, `SAMPLE_RATE`, `THRESHOLD`, `SILENCE_FRAMES`, `MAX_RECORDING_SECONDS`
- [ ] 4.2 实现 `VadConfig` 结构体 (包含所有配置字段)
- [ ] 4.3 为 `VadConfig` 实现 `Default` trait
- [ ] 4.4 在 `vad/mod.rs` 中导出配置项和结构体

## 5. 实现 VAD FFI 引擎 (30分钟)

- [ ] 5.1 在 `vad/engine.rs` 中定义 FFI 类型别名 (`TenVadCreate`, `TenVadProcess`, `TenVadDestroy`)
- [ ] 5.2 定义 `VadError` 枚举 (LoadError, InitError, ProcessError)
- [ ] 5.3 实现 `VadEngine::new()` 函数 (dlopen + 符号解析 + ten_vad_create)
- [ ] 5.4 实现 `VadEngine::process()` 方法 (调用 ten_vad_process)
- [ ] 5.5 为 `VadEngine` 实现 `Drop` trait (调用 ten_vad_destroy)
- [ ] 5.6 添加 `unsafe impl Send for VadEngine` (跨线程安全)

## 6. 实现 VAD 状态机 (45分钟)

- [ ] 6.1 在 `vad/state_machine.rs` 中定义 `VadState` 枚举 (Idle, Listening, Recording, Processing)
- [ ] 6.2 为 `VadState` 派生 `serde::Serialize` (事件序列化)
- [ ] 6.3 定义 `VadEvent` 枚举 (StateChanged, SpeechDetected, Error)
- [ ] 6.4 实现 `VadStateMachine::new()` (初始化状态和缓冲区)
- [ ] 6.5 实现 `VadStateMachine::start()` (transition to Listening)
- [ ] 6.6 实现 `VadStateMachine::stop()` (cleanup and transition to Idle)
- [ ] 6.7 实现 `VadStateMachine::process_frame()` (核心状态逻辑)
  - [ ] 6.7.1 Listening → Recording transition (speech detected)
  - [ ] 6.7.2 Recording 状态缓冲区管理 (extend with audio)
  - [ ] 6.7.3 Silence detection (counter increment)
  - [ ] 6.7.4 Recording → Processing transition (silence threshold)
- [ ] 6.8 实现 `VadStateMachine::finish_transcription()` (return to Idle)
- [ ] 6.9 实现 `VadStateMachine::get_state()` (query current state)

## 7. 实现 cpal 录音机 (60分钟)

- [ ] 7.1 在 `audio/recorder.rs` 中定义 `RecorderError` 枚举
- [ ] 7.2 实现 `AudioRecorder::new()` 函数
  - [ ] 7.2.1 获取默认输入设备 `cpal::default_host().default_input_device()`
  - [ ] 7.2.2 配置流 (16kHz, mono, i16, buffer_size=256)
  - [ ] 7.2.3 创建 crossbeam channel (事件队列)
  - [ ] 7.2.4 初始化 `VadStateMachine`
  - [ ] 7.2.5 构建 cpal 输入流
- [ ] 7.3 实现音频回调 `data_callback`
  - [ ] 7.3.1 调用 `VadEngine::process()` 处理帧
  - [ ] 7.3.2 调用 `VadStateMachine::process_frame()` 更新状态
- [ ] 7.4 实现错误回调 `err_handler` (发送 Error event)
- [ ] 7.5 调用 `stream.play()` 启动音频流
- [ ] 7.6 实现 `AudioRecorder::start()` (委托给 state_machine.start())
- [ ] 7.7 实现 `AudioRecorder::stop()` (委托给 state_machine.stop())
- [ ] 7.8 实现 `AudioRecorder::get_state()` (查询状态)
- [ ] 7.9 实现 `AudioRecorder::recv_event()` (克隆 channel receiver)

## 8. 实现 Tauri 命令 (45分钟)

- [ ] 8.1 在 `vad_commands.rs` 中定义 `VadRecorderState` 结构体
- [ ] 8.2 实现 `get_vad_lib_path()` 函数 (平台特定路径)
- [ ] 8.3 实现 `encode_wav()` 辅助函数 (Vec<i16> → WAV Vec<u8>)
- [ ] 8.4 实现 `transcribe_audio_internal()` 异步函数 (调用 asr::transcribe_audio_data)
- [ ] 8.5 实现 `start_listening` Tauri 命令
  - [ ] 8.5.1 加载 VAD 库并初始化 `VadEngine`
  - [ ] 8.5.2 创建 `AudioRecorder`
  - [ ] 8.5.3 保存到全局状态 `VadRecorderState`
  - [ ] 8.5.4 启动录音
  - [ ] 8.5.5 启动后台 tokio 任务处理事件
- [ ] 8.6 实现后台事件循环 (tokio::spawn)
  - [ ] 8.6.1 接收 VadEvent from channel
  - [ ] 8.6.2 匹配事件类型 (StateChanged, SpeechDetected, Error)
  - [ ] 8.6.3 发送 Tauri Events (window.emit)
  - [ ] 8.6.4 调用 ASR 转录 (SpeechDetected)
  - [ ] 8.6.5 发送 transcript 事件
- [ ] 8.7 实现 `stop_listening` Tauri 命令 (cleanup recorder)
- [ ] 8.8 实现 `get_vad_state` Tauri 命令 (查询状态)

## 9. 集成到 lib.rs (10分钟)

- [ ] 9.1 在 `lib.rs` 中声明模块: `mod audio;`, `mod vad;`, `mod vad_commands;`
- [ ] 9.2 在 `tauri::Builder` 中添加 `.manage(VadRecorderState::new())`
- [ ] 9.3 在 `invoke_handler!` 中注册新命令
  - [ ] 9.3.1 `vad_commands::start_listening`
  - [ ] 9.3.2 `vad_commands::stop_listening`
  - [ ] 9.3.3 `vad_commands::get_vad_state`
- [ ] 9.4 验证编译通过 `cargo check`

## 10. 前端重构 (60分钟)

- [ ] 10.1 删除前端 VAD 文件
  - [ ] 10.1.1 删除 `src/hooks/useVAD.ts`
  - [ ] 10.1.2 删除 `src/hooks/useAudioRecorder.ts`
  - [ ] 10.1.3 删除 `src/lib/ten_vad.wasm`
  - [ ] 10.1.4 删除 `src/lib/ten_vad.js`
  - [ ] 10.1.5 删除 `src/lib/ten_vad.d.ts`
- [ ] 10.2 创建 `src/hooks/useBackendVAD.ts`
  - [ ] 10.2.1 定义 `VADState` 类型 (Idle, Listening, Recording, Processing)
  - [ ] 10.2.2 实现 `useBackendVAD()` hook
  - [ ] 10.2.3 监听 `vad-state` 事件 (useEffect + listen)
  - [ ] 10.2.4 监听 `transcript` 事件
  - [ ] 10.2.5 监听 `error` 事件
  - [ ] 10.2.6 实现 `startListening` 函数 (invoke start_listening)
  - [ ] 10.2.7 实现 `stopListening` 函数 (invoke stop_listening)
  - [ ] 10.2.8 返回 state, transcript, error, startListening, stopListening
- [ ] 10.3 更新 `src/components/VoiceRecorder.tsx`
  - [ ] 10.3.1 替换 `useVAD` → `useBackendVAD`
  - [ ] 10.3.2 移除 `useTranscription` hook
  - [ ] 10.3.3 更新 UI 绑定 (使用新的 hook 返回值)
  - [ ] 10.3.4 更新状态显示 (从 transcript 而非 transcribe result)

## 11. 编译和测试 (30分钟)

- [ ] 11.1 运行 `cargo check` 验证后端编译
- [ ] 11.2 运行 `pnpm build` 验证前端编译
- [ ] 11.3 启动开发服务器 `pnpm tauri dev`
- [ ] 11.4 测试场景: 启动应用 → 点击 "Start Listening"
- [ ] 11.5 验证状态变化: Idle → Listening (显示在 UI)
- [ ] 11.6 测试说话 → 验证状态变为 Recording
- [ ] 11.7 测试静音 480ms → 验证状态变为 Processing
- [ ] 11.8 验证转录结果显示在 UI
- [ ] 11.9 测试错误处理 (关闭麦克风权限)
- [ ] 11.10 测试停止功能 → 验证状态回到 Idle

## 12. 清理工作 (10分钟)

- [ ] 12.1 更新 `README.md` 架构说明
  - [ ] 12.1.1 更新 Architecture 部分
  - [ ] 12.1.2 添加 Backend VAD 说明
  - [ ] 12.1.3 移除前端 VAD 说明
- [ ] 12.2 运行 `cargo clippy` 检查代码质量
- [ ] 12.3 运行 `cargo fmt` 格式化代码
- [ ] 12.4 提交 Git commit (如果需要)
- [ ] 12.5 更新 OpenSpec 变更状态 (运行 `openspec archive` 如果完成)
