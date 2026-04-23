# VAD-Based Voice Recorder - Technical Design

## Context

**Current State**:
- Tauri + React 应用，后端已集成 Qwen3 ASR 引擎
- 现有 `transcribe(audio_path: String)` 命令支持文件路径输入
- 前端使用模板代码，无录音功能
- 项目已有 VAD 相关 specs：`audio-preprocessing`, `onnx-inference`, `stt-engine`, `stt-qwen3`

**Constraints**:
- Tauri 桌面应用环境（可访问浏览器 API）
- 后端 ASR 要求 16kHz WAV 格式输入
- 需要处理浏览器自动播放策略和麦克风权限
- 内存管理严格（WASM 手动分配/释放）

**Stakeholders**:
- 最终用户：需要流畅的语音转文字体验
- 开发团队：需要可维护的代码和清晰的架构

## Goals / Non-Goals

**Goals:**
- 实现基于 VAD 的自动语音检测和录音
- 提供流畅的用户体验（"说就开始，停就转录"）
- 优化性能（CPU <2%, 内存 <5MB）
- 保持代码可维护性和可测试性

**Non-Goals:**
- 实时转录（说话过程中逐字显示）
- 多语言音频支持（仅支持 16kHz 单声道）
- 长时间录音支持（超过 30 秒）
- 音频可视化波形（仅状态指示）

## Decisions

### 1. VAD 引擎选择: 官方 ten-vad WASM

**Decision**: 使用 TEN-framework/ten-vad 官方仓库的 WebAssembly 版本

**Rationale**:
- ✅ 官方维护，更新及时
- ✅ 完整文档和示例代码（test_browser.html）
- ✅ 性能优异（RTF < 0.02）
- ✅ 包含 TypeScript 类型定义
- ✅ 文件大小合理（~278KB）

**Alternatives Considered**:
- `@gooney-001/ten-vad-lib`: 第三方封装，维护不确定
- `defuss-vad`: 嵌入在大型框架中，不够独立
- `silero-vad`: 性能不如 ten-vad

### 2. 录音方案: VAD 驱动的选择性录音

**Decision**: 实现状态机，仅在检测到语音时录制音频到内存缓冲区

**Rationale**:
- ✅ 性能优化：空闲时 CPU 接近 0%
- ✅ 隐私友好：用户知道仅在"检测到语音"后录音
- ✅ 简化实现：无需复杂的裁剪逻辑

**Alternatives Considered**:
- **持续录音 + 后裁剪**: 完美流畅，但 CPU 持续 3-5%，隐私担忧
- **预缓冲 + 触发录音**: 降低资源消耗，但丢失 32-48ms 开头

**Trade-off**: 接受 ~32ms 开头丢失以换取性能和隐私优势

### 3. 音频格式: WAV (PCM, 16kHz, mono, 16-bit)

**Decision**: 使用 WAV 格式，前端直接编码

**Rationale**:
- ✅ 零 CPU 编码开销（内存→文件直接拷贝）
- ✅ 后端 Qwen3 直接支持，无需转换
- ✅ 实现简单（~50 行代码）

**Alternatives Considered**:
- **WebM (Opus)**: 压缩率高，但需要编码 CPU，后端需解码

### 4. 文件传递: Tauri 临时文件路径

**Decision**: 前端将 WAV 数据发送给后端，后端保存为临时文件后调用现有 transcribe

**Rationale**:
- ✅ 复用现有 `transcribe(audio_path)` 逻辑
- ✅ 避免在前端处理文件系统（Tauri 安全限制）
- ✅ 自动清理临时文件

**Alternatives Considered**:
- **Base64 字符串传递**: 内存占用高（Base64 增大 33%）

### 5. 状态管理: React Hooks

**Decision**: 使用自定义 Hooks 封装 VAD 和录音逻辑

**Rationale**:
- ✅ 符合 React 最佳实践
- ✅ 逻辑复用和测试简单
- ✅ 轻量级，无需 Redux

**Hooks 设计**:
- `useVAD`: 封装 ten-vad WASM 初始化和帧处理
- `useAudioRecorder`: 封装 MediaRecorder 和 WAV 编码
- `useTranscription`: 封装 Tauri invoke 调用

### 6. VAD 参数配置

**Decision**: 使用官方推荐的默认参数

```typescript
const HOP_SIZE = 256;        // 16ms @ 16kHz
const SAMPLE_RATE = 16000;   // 16kHz
const THRESHOLD = 0.5;       // 平衡的检测阈值
const SILENCE_FRAMES = 30;   // 480ms 静音判定
```

**Rationale**: 官方测试验证过这些参数的准确性

## Risks / Trade-offs

### Risk 1: WASM 内存泄漏
**Risk**: 手动内存管理（`_malloc`/`_free`）可能导致内存泄漏
**Mitigation**:
- 使用 React `useEffect` cleanup 确保释放
- 封装 RAII 风格的 VAD wrapper 类
- 添加内存使用监控（开发环境）

### Risk 2: 浏览器兼容性
**Risk**: Web Audio API 和 WASM 在旧浏览器不支持
**Mitigation**:
- Tauri 使用系统 WebView，版本可控
- 添加特性检测和优雅降级提示

### Risk 3: 假阳性触发（误录音）
**Risk**: 咳嗽、噪音可能触发录音
**Mitigation**:
- 可调节 `THRESHOLD` 参数（0.3-0.7）
- 添加最小录音时长限制（如 500ms）
- 提供用户取消按钮

### Risk 4: 开头音频丢失
**Risk**: VAD 检测延迟导致丢失 ~32ms 开头
**Mitigation**:
- 接受权衡（性能 vs 完美性）
- 文档说明已知限制
- 未来可优化预缓冲方案

### Risk 5: 麦克风权限拒绝
**Risk**: 用户拒绝麦克风权限导致功能不可用
**Mitigation**:
- 友好的 UI 提示和重试机制
- 文档说明权限要求

## Migration Plan

### Phase 1: 前端基础（核心功能）
1. 集成 ten-vad WASM 文件
2. 实现 `useVAD` hook
3. 实现 VAD 状态机
4. 实现 WAV 编码

### Phase 2: UI 和用户反馈
1. 实现 `useAudioRecorder` hook
2. 创建 VoiceRecorder 组件
3. 添加状态可视化（监听中、录音中、处理中）

### Phase 3: 后端集成
1. 添加 `transcribe_audio_data` Tauri 命令
2. 实现 `useTranscription` hook
3. 连接完整流程（录音 → 转录 → 显示）

### Phase 4: 测试和优化
1. 单元测试（VAD hook, WAV 编码）
2. 集成测试（完整流程）
3. 性能优化和内存泄漏检测

### Rollback Strategy
- 每个阶段独立可回滚
- 保留现有手动录音功能（如果存在）
- Feature flag 控制 VAD 功能开关

## Open Questions

1. **临时文件清理策略**: 是否立即清理还是延迟清理？
   - **倾向**: 转录完成后立即清理，避免磁盘占用

2. **错误重试机制**: VAD 初始化失败或麦克风被占用时的处理？
   - **倾向**: 显示错误提示，提供重试按钮

3. **音频质量调整**: 是否允许用户调整采样率或阈值？
   - **倾向**: 初始版本固定参数，后续版本可配置
