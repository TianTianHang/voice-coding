# Backend VAD Recording

## Why

前端录音方案因 Tauri WebView 权限限制无法获取麦克风访问权限，导致 `navigator.mediaDevices.getUserMedia()` 失败。需要将录音+VAD功能完全迁移到后端，通过 Rust 原生代码直接访问系统音频设备，绕过前端权限限制。

## What Changes

- **移除前端 VAD 实现**
  - 删除 `src/hooks/useVAD.ts` (ten-vad WASM 封装)
  - 删除 `src/hooks/useAudioRecorder.ts` (WAV 编码)
  - 删除 `src/lib/ten_vad.*` (WASM 库文件)
  - 删除前端 `useTranscription` hook (改用后端事件驱动)

- **新增后端录音+VAD模块**
  - 新增 `src-tauri/src/vad/` 模块 (ten-vad FFI 封装)
  - 新增 `src-tauri/src/audio/` 模块 (cpal 录音机)
  - 新增 Tauri 命令: `start_listening`, `stop_listening`, `get_vad_state`
  - 后端直接调用 ASR，通过 Tauri Events 发送转录结果

- **前端改为纯 UI 层**
  - 新增 `src/hooks/useBackendVAD.ts` (监听后端事件，显示状态)
  - 更新 `VoiceRecorder.tsx` 使用新的 backend hook
  - 移除所有音频处理逻辑

- **集成 ten-vad 原生库**
  - 下载 Linux x64 预编译库 `libten_vad.so` 到 `src-tauri/libs/`
  - 添加 Rust 依赖: `cpal`, `libloading`, `crossbeam-channel`, `parking_lot`
  - 配置 `tauri.conf.json` 打包库文件

## Capabilities

### New Capabilities
- `backend-audio-recording`: 后端音频录制能力，通过 cpal 访问系统麦克风设备，支持 16kHz mono i16 格式
- `backend-vad`: 后端语音活动检测，通过 ten-vad 原生库 (FFI) 检测语音开始/结束，状态机管理录音缓冲区
- `real-time-vad-events`: 实时 VAD 状态事件，通过 Tauri Events 向前端推送 `vad-state`, `transcript`, `error` 事件

### Modified Capabilities
- `stt-qwen3`: 扩展为支持内部音频数据转录 (直接从内存缓冲区调用，无需临时文件)

## Impact

**前端依赖变化**:
- ❌ 移除: ten-vad WASM (~278KB)
- ✅ 无新增依赖

**后端依赖变化**:
- ✅ 新增: cpal 0.15 (音频输入)
- ✅ 新增: libloading 0.8 (动态库加载)
- ✅ 新增: crossbeam-channel 0.5 (跨线程通信)
- ✅ 新增: parking_lot 0.12 (高性能互斥锁)
- ✅ 新增: libten_vad.so (~300KB, Linux x64)

**架构影响**:
- 前端从"录音+VAD+UI"简化为"纯UI层"
- 后端从"纯ASR服务"扩展为"录音+VAD+ASR完整流水线"
- 通信方式从"命令式invoke"改为"事件驱动" (Tauri Events)

**性能影响**:
- CPU: ~2-5% (后端 VAD + 录音，vs 前端 WASM ~2%)
- 内存: ~50MB (后端音频缓冲，vs 前端 ~5MB)
- 延迟: <100ms 状态更新延迟

**跨平台支持**:
- ✅ Linux x64: 完全支持 (使用预编译 libten_vad.so)
- ⚠️ macOS/Windows: 需额外下载对应平台的 ten-vad 库 (当前未实现)
