# VAD-Based Voice Recorder

## Why

当前语音转文字应用需要用户手动点击开始/停止录音按钮，体验不流畅。我们需要实现一个基于语音活动检测（VAD）的自动录音系统，让用户只需说话，系统自动检测语音开始和结束，实现真正的"说就开始，停就转录"的无缝体验。

## What Changes

- **前端集成 ten-vad WASM**: 从 TEN-framework/ten-vad 官方仓库集成 WebAssembly 版本的 VAD 引擎
- **VAD 驱动的选择性录音**: 实现 VAD 状态机，仅在检测到语音时录制音频，优化性能和资源消耗
- **自动音频处理**: 检测到停顿（默认 480ms 静音）后自动停止录音并触发转录
- **音频格式标准化**: 将音频编码为 WAV 格式（PCM, 16kHz, mono, 16-bit）以兼容后端 Qwen3 ASR
- **实时用户反馈**: 提供可视化反馈显示录音状态（监听中、录音中、处理中）
- **Tauri 后端扩展**: 添加新的 Tauri 命令支持接收音频数据并保存为临时文件

## Capabilities

### New Capabilities
- `frontend-vad`: 前端语音活动检测能力，包括 VAD 初始化、音频帧处理、状态机管理

### Modified Capabilities
- `stt-qwen3`: 可能需要扩展转录命令以支持直接接收音频数据（而非仅文件路径）

## Impact

**前端依赖**:
- 新增 ten-vad WASM 文件（~278KB）
- 需要麦克风权限（`navigator.mediaDevices.getUserMedia`）
- 使用 Web Audio API 处理音频流

**后端变更**:
- 新增 Tauri 命令: `transcribe_audio_data(audio_data: Vec<u8>, language: Option<String>)`
- 复用现有 `transcribe` 命令逻辑
- 临时文件管理（自动清理）

**性能影响**:
- CPU: ~2% (VAD 1% + 音频编码 1%)
- 内存: ~5MB (VAD WASM + 30秒音频缓冲)
- 延迟: <32ms 开头丢失（VAD 检测累积）

**用户体验**:
- 无需手动控制录音，真正自动化的语音转文字体验
- 可能丢失前 32ms 音频（约 1 个音节），可通过预缓冲优化
