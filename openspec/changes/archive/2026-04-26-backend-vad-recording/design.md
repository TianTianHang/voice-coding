# Backend VAD Recording - Technical Design

## Context

**Current State:**
- 前端使用 ten-vad WASM 进行语音活动检测 (VAD)
- 前端通过 `navigator.mediaDevices.getUserMedia()` 获取麦克风权限
- Tauri WebView 环境下麦克风权限请求被拒绝，导致录音功能不可用
- 后端仅有 `stt-qwen3` ASR 引擎，通过 `transcribe` 和 `transcribe_audio_data` 命令提供转录服务
- 前端负责音频编码 (WAV) 并通过 Tauri invoke 发送给后端

**Constraints:**
- Tauri capability 配置无法解决前端权限问题 (已尝试配置 `core:webview:allow-get-user-media`)
- 目标平台为 Linux x64 (暂时不考虑 macOS/Windows)
- ten-vad 官方提供预编译库 (libten_vad.so)
- 需保持与现有 ASR 引擎的兼容性

**Stakeholders:**
- 最终用户：需要可用的语音转文字功能
- 开发团队：需要可维护的代码和清晰的架构

## Goals / Non-Goals

**Goals:**
- 实现后端录音+VAD，绕过前端权限限制
- 提供与前端 VAD 相同的用户体验 (自动检测语音开始/结束)
- 保持响应式状态反馈 (listening → recording → processing → idle)
- 复用现有 ASR 引擎，最小化改动
- 支持事件驱动的状态推送

**Non-Goals:**
- 实时逐字转录 (说话过程中显示文字)
- 多平台支持 (仅支持 Linux x64)
- 长时间录音支持 (限制 30 秒，防止内存溢出)
- 前端录音方案保留 (完全移除前端 VAD 代码)

## Decisions

### 1. 音频输入方案: cpal 库

**Decision**: 使用 `cpal` (Cross-platform Audio Library) 进行系统音频采集

**Rationale:**
- ✅ Tauri 生态常用音频库，文档丰富
- ✅ 跨平台抽象层，支持 Linux/macOS/Windows
- ✅ 零拷贝架构，性能优异
- ✅ 支持 16kHz mono i16 格式 (符合 ten-vad 要求)

**Alternatives Considered:**
- **rodio**: 基于 cpal 的高层封装，但更侧重播放而非录音
- **alsa-sys**: Linux 原生 ALSA 绑定，但无法跨平台

### 2. VAD 引擎: ten-vad 原生库 (FFI)

**Decision**: 使用 `libloading` 动态加载 ten-vad 预编译库 (libten_vad.so)，通过 FFI 调用

**Rationale:**
- ✅ 与前端 WASM 使用相同的 VAD 引擎，检测结果一致
- ✅ 零依赖 (无需 ONNX Runtime)
- ✅ 性能最优 (原生代码，RTF < 0.02)
- ✅ 文件体积小 (~300KB)

**Alternatives Considered:**
- **ONNX Runtime + ten-vad.onnx**: 需要额外依赖 (onnxruntime-rs ~10MB)，性能开销大
- **复用现有 vad.rs**: 基于 RMS 能量的简单 VAD，准确率低于 ten-vad

**FFI 接口设计:**
```rust
// C API
ten_vad_create(hop_size: i32, threshold: f32) -> *mut TenVad
ten_vad_process(handle, audio: *const i16, hop_size, prob: *mut f32, flag: *mut i32) -> i32
ten_vad_destroy(handle: *mut TenVad)
```

### 3. 线程模型: 单线程音频流 + 后台事件处理

**Decision**:
- cpal 音频流在独立线程运行 (cpal 内部管理)
- VAD 状态机在音频回调中同步执行
- ASR 转录在后台异步任务中执行 (tokio spawn)
- 通过 `crossbeam-channel` 在线程间传递事件

**Rationale:**
- ✅ 音频回调必须在 cpal 线程执行 (低延迟要求)
- ✅ ASR 转录是耗时操作，不能阻塞音频回调
- ✅ `crossbeam-channel` 提供高性能无锁队列
- ✅ Tauri Events 从后台线程发送到主线程 (自动序列化)

**数据流:**
```
cpal audio thread → VAD process → state_machine → channel → background task → ASR → Tauri Event → Frontend
```

### 4. 状态同步: Tauri Events

**Decision**: 后端通过 Tauri Events 向前端推送状态和转录结果

**Events:**
- `vad-state`: `{ "Idle" | "Listening" | "Recording" | "Processing" }`
- `transcript`: `{ "text": "..." }`
- `error`: `{ "message": "..." }`

**Rationale:**
- ✅ 单向数据流，前端无需轮询
- ✅ Tauri 自动处理线程安全和序列化
- ✅ 支持多窗口监听 (未来扩展)

**Alternatives Considered:**
- **命令式 invoke + 轮询**: 前端需要定期调用 `get_vad_state()`，延迟高
- **WebSocket**: 过度设计，Tauri Events 已满足需求

### 5. 内存管理: RAII + Arc<Mutex<>>

**Decision**:
- `VadEngine` 使用 RAII (Drop trait) 自动释放 FFI 资源
- 状态共享使用 `Arc<Mutex<>>` (parking_lot 提供高性能互斥锁)
- 音频缓冲区预分配 (Vec::with_capacity)，避免频繁分配

**Rationale:**
- ✅ FFI 资源必须手动释放，RAII 确保不泄漏
- ✅ `parking_lot::Mutex` 比 `std::sync::Mutex` 性能高 2-3x
- ✅ `Arc` 允许多线程共享所有权 (音频线程 + 事件处理线程)

### 6. 错误处理: Result<T, String>

**Decision**: 所有 Tauri 命令返回 `Result<(), String>`，错误通过 `error` event 发送

**Rationale:**
- ✅ Tauri 命令要求错误可序列化 (String 最简单)
- ✅ 区分"命令失败" (返回 Err) 和"运行时错误" (发送 event)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (React)                                                │
├─────────────────────────────────────────────────────────────────┤
│  useBackendVAD.ts:                                               │
│    - listen("vad-state", ...)     → update state                │
│    - listen("transcript", ...)    → update transcript           │
│    - listen("error", ...)         → show error                  │
│    - invoke("start_listening")    → start backend               │
│    - invoke("stop_listening")     → stop backend                │
│                                                                   │
│  VoiceRecorder.tsx: 纯 UI 渲染                                    │
└────────────────────────────┬────────────────────────────────────┘
                             │ Tauri IPC
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  Backend (Rust)                                                  │
├─────────────────────────────────────────────────────────────────┤
│  Tauri Commands (lib.rs):                                       │
│    - start_listening()     → initialize recorder & spawn task   │
│    - stop_listening()      → cleanup recorder                   │
│    - get_vad_state()       → query current state                │
│                                                                   │
│  audio/recorder.rs:                                              │
│    - AudioRecorder::new()   → setup cpal stream                 │
│    - cpal callback         → process audio frames              │
│                                                                   │
│  vad/engine.rs:                                                   │
│    - VadEngine::new()       → dlopen libten_vad.so              │
│    - process(audio)         → return (prob, is_speech)          │
│                                                                   │
│  vad/state_machine.rs:                                           │
│    - VadStateMachine        → manage state transitions          │
│    - process_frame()        → buffer audio, detect silence      │
│    - send events            → push to crossbeam-channel         │
│                                                                   │
│  Background Task (tokio::spawn):                                │
│    - recv from channel      → get VadEvent                     │
│    - match VadEvent         → handle StateChange/SpeechDetected │
│    - transcribe audio       → call asr::transcribe_audio_data   │
│    - emit("transcript")     → send result to frontend           │
└─────────────────────────────────────────────────────────────────┘
```

## Risks / Trade-offs

### Risk 1: libten_vad.so 依赖缺失
**Risk**: 用户系统缺少必要的共享库 (如 libc++), 导致动态加载失败

**Mitigation**:
- 构建时检查 `ldd libten_vad.so`，列出所有依赖
- 在 `README.md` 文档说明系统要求
- 提供友好的错误提示："缺少依赖库，请安装 libc++"

### Risk 2: cpal 设备权限问题
**Risk**: 用户拒绝麦克风权限或设备被占用

**Mitigation**:
- 提供清晰的错误提示："麦克风被占用，请关闭其他应用"
- 添加设备枚举功能，允许用户选择输入设备 (未来)
- 提供重试按钮

### Risk 3: FFI 内存泄漏
**Risk**: ten-vad handle 未正确释放

**Mitigation**:
- 使用 RAII (Drop trait) 确保释放
- 添加 `#[cfg(test)]` 单元测试，验证资源释放
- 使用 Valgrind 检测内存泄漏 (开发阶段)

### Risk 4: 音频缓冲区溢出
**Risk**: 长时间录音导致内存耗尽

**Mitigation**:
- 设置 `MAX_RECORDING_SECONDS = 30`
- 在状态机中检查缓冲区大小，超过限制自动截断
- 使用 `Vec::with_capacity` 预分配，避免频繁分配

### Risk 5: 线程安全
**Risk**: 多线程访问共享状态导致数据竞争

**Mitigation**:
- 所有共享状态使用 `Arc<Mutex<>>` 保护
- 音频回调中锁持有时间极短 (仅复制数据)
- 使用 `parking_lot::Mutex` (性能优于 std::sync::Mutex)

## Migration Plan

### Phase 1: 依赖添加 (5分钟)
```bash
cargo add cpal libloading crossbeam-channel parking_lot
mkdir -p src-tauri/libs/Linux/x64
cp /path/to/libten_vad.so src-tauri/libs/Linux/x64/
```

### Phase 2: 后端实现 (2-3小时)
1. 创建 `vad/` 和 `audio/` 模块
2. 实现 FFI 封装 (`vad/engine.rs`)
3. 实现状态机 (`vad/state_machine.rs`)
4. 实现 cpal 录音机 (`audio/recorder.rs`)
5. 实现 Tauri 命令 (`vad_commands.rs`)
6. 集成到 `lib.rs`

### Phase 3: 前端重构 (1小时)
1. 删除前端 VAD 文件 (`useVAD.ts`, `useAudioRecorder.ts`, `ten_vad.*`)
2. 创建 `useBackendVAD.ts` (监听事件)
3. 更新 `VoiceRecorder.tsx` (使用新 hook)
4. 移除 `useTranscription` (后端直接发送 transcript event)

### Phase 4: 集成测试 (30分钟)
1. 编译检查: `cargo check`
2. 运行测试: `pnpm tauri dev`
3. 测试场景: 启动/停止/说话/静音/错误处理

### Rollback Strategy
- 保留现有 `asr.rs` 不变，确保转录功能可用
- 如果后端 VAD 失败，前端仍可通过手动触发 ASR 使用基本功能
- Git commit 分阶段，可随时回滚

## Open Questions

1. **ten-vad 库分发方式**:
   - ❓ 提交到 Git 仓库 (增加 repo 体积)
   - ❓ 构建时从 GitHub releases 下载 (需要网络连接)
   - **倾向**: 提交到 Git，简单直接

2. **静音阈值参数可调性**:
   - ❓ 暴露给用户配置 (增加 UI 复杂度)
   - ❓ 固定默认值 (SILENCE_FRAMES = 30, 480ms)
   - **倾向**: 固定默认值，未来版本可配置

3. **音频格式兼容性**:
   - ❓ 是否需要支持其他采样率 (8kHz, 44.1kHz)
   - **倾向**: 仅支持 16kHz (ten-vad 和 ASR 要求一致)

4. **多麦克风支持**:
   - ❓ 是否需要设备枚举和选择功能
   - **倾向**: 使用默认设备，未来版本可扩展

## Performance Targets

| 指标 | 目标值 | 测量方法 |
|------|--------|---------|
| CPU 占用 | <5% | `top` 命令观察 |
| 内存占用 | <50MB | `/proc/<pid>/status` |
| 状态延迟 | <100ms | 时间戳差值测量 |
| VAD 检测延迟 | <32ms | ten-vad 官方数据 |
| ASR 转录延迟 | <500ms | 端到端测量 |
