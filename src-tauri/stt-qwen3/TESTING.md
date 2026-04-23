# ASR 测试文档

## 测试概览

本项目包含以下测试：

| 测试类型 | 数量 | 文件 | 说明 |
|---------|------|------|------|
| **单元测试** | 73 | `src/**/*.rs` | 音频处理、编码器、解码器等模块测试 |
| **引擎测试** | 21 | `tests/engine_test.rs` | 主引擎集成测试 |
| **边界测试** | 15 | `tests/boundary_test.rs` | 边界条件和异常情况测试 |
| **总计** | **109** | - | **+36 新测试** |

## 快速开始

### 运行所有测试

```bash
# 使用提供的测试脚本（推荐）
./run_tests.sh
```

### 运行特定测试

```bash
# 只运行单元测试（快速，不需要模型）
cargo test --lib

# 运行引擎测试（需要模型）
export ORT_DYLIB_PATH=/path/to/libonnxruntime.so
cargo test --test engine_test

# 运行边界测试（需要模型）
cargo test --test boundary_test

# 运行单个测试
cargo test --test engine_test test_engine_initialization_success
```

## 测试说明

### 1. 单元测试 (73个)

快速运行的测试，覆盖：
- ✅ 梅尔频谱计算 (7个测试)
- ✅ VAD语音活动检测 (7个测试)
- ✅ 音频加载和重采样 (7个测试)
- ✅ 编码器分块 (8个测试)
- ✅ 解码器和嵌入融合 (20个测试)
- ✅ Prompt构建 (9个测试，含4个属性测试)
- ✅ Tokenizer多语言支持 (10个测试)

### 2. 引擎测试 (21个)

测试主引擎的核心功能：

#### 初始化测试
- `test_engine_initialization_success` - 引擎初始化
- `test_engine_initialization_invalid_path` - 无效路径处理

#### 语言验证测试
- `test_supported_language_accepted` - 支持的语言
- `test_unsupported_language_rejected` - 不支持的语言拒绝
- `test_unsupported_language_another_case` - 另一种无效语言

#### 音频输入测试
- `test_audio_input_filepath_valid` - 文件路径输入
- `test_audio_input_filepath_missing` - 缺失文件处理
- `test_audio_input_bytes_valid` - 字节输入
- `test_audio_input_bytes_invalid` - 无效字节处理
- `test_audio_input_samples_16khz` - 16kHz采样率
- `test_audio_input_samples_resampling` - 重采样（48kHz→16kHz）
- `test_audio_input_samples_8khz` - 8kHz采样率

#### VAD测试
- `test_vad_not_triggered_under_threshold` - VAD未触发（<45秒）
- `test_vad_triggered_at_threshold` - VAD触发（≥45秒）
- `test_vad_disabled` - VAD禁用

#### 配置测试
- `test_config_max_new_tokens` - 最大token限制
- `test_config_chunk_seconds` - 分块大小配置

#### 健康检查测试
- `test_health_check_success` - 健康检查成功
- `test_health_check_missing_files` - 文件缺失检测

#### 结果验证测试
- `test_transcription_result_structure` - 结果结构验证
- `test_multiple_languages` - 多语言支持

### 3. 边界测试 (15个)

测试极端和异常情况：

#### 时长边界测试
- `test_minimum_duration_boundary` - 最小时长边界
- `test_below_minimum_duration` - 低于最小时长
- `test_empty_samples` - 空音频
- `test_single_sample` - 单样本

#### 音频质量测试
- `test_silence_only_audio` - 纯静音
- `test_clipping_audio` - 削波音频
- `test_negative_amplitude_audio` - 负振幅

#### 采样率测试
- `test_extreme_low_sample_rate` - 8kHz
- `test_extreme_high_sample_rate` - 96kHz

#### 配置边界测试
- `test_very_large_max_tokens` - 极大token数
- `test_zero_max_tokens` - 零token

#### VAD压力测试
- `test_long_audio_handling` - 长音频处理
- `test_vad_with_very_small_chunk` - 极小分块
- `test_vad_with_very_large_chunk` - 极大分块

#### 其他
- `test_very_short_duration` - 极短音频

## 环境要求

### 必需

1. **模型文件** (2.5GB)
   ```
   models/
   ├── onnx_models/
   │   ├── encoder_conv.onnx
   │   ├── encoder_transformer.onnx
   │   ├── decoder_init.int8.onnx
   │   └── decoder_step.int8.onnx
   ├── embed_tokens.bin
   └── tokenizer.json
   ```

2. **ONNX Runtime 动态库**
   ```bash
   export ORT_DYLIB_PATH=/path/to/libonnxruntime.so
   ```

3. **测试音频** (可选，仅集成测试需要)
   ```
   test_audio/
   ├── librispeech_0_1089_0.wav
   ├── librispeech_1_1089_1.wav
   └── librispeech_2_1089_2.wav
   ```

## 性能指标

### 预期测试运行时间

| 测试套件 | 时间 | 说明 |
|---------|------|------|
| 单元测试 | ~7秒 | 无需模型 |
| 引擎测试 | ~10-15分钟 | 需要加载模型（21个测试 × ~30秒） |
| 边界测试 | ~8-12分钟 | 需要加载模型（15个测试 × ~30秒） |
| **总计** | **25-35分钟** | 使用 `--test-threads=1` |

### 优化建议

1. **并行运行**（如果内存充足）
   ```bash
   cargo test --test engine_test -- --test-threads=4
   ```

2. **只运行快速测试**
   ```bash
   cargo test --lib
   ```

3. **CI/CD优化**
   - 单元测试每次PR都运行
   - 引擎/边界测试仅在特定分支运行

## 故障排除

### 测试失败

1. **模型未找到**
   ```
   Error: Failed to initialize STT engine
   ```
   解决：确保 `models/` 目录存在且包含所有文件

2. **ORT库未找到**
   ```
   Error: ONNX Runtime library not found
   ```
   解决：设置 `ORT_DYLIB_PATH` 环境变量

3. **测试超时**
   ```
   Error: test timed out after 300 seconds
   ```
   解决：增加超时或使用 `--test-threads=1`

### 调试单个测试

```bash
# 显示输出
cargo test --test engine_test test_name -- --nocapture

# 查看日志
RUST_BACKTRACE=1 cargo test --test engine_test test_name
```

## 添加新测试

1. 在相应的测试文件中添加测试函数
2. 使用 `async fn` 和 `#[tokio::test]`
3. 使用 `setup_test_engine()` 创建引擎实例
4. 使用 `create_mock_samples()` 生成测试音频

示例：
```rust
#[tokio::test]
async fn test_my_new_feature() {
    let engine = setup_test_engine().await;
    let samples = create_mock_samples(2, 16000);
    let input = AudioInput::Samples(samples, 16000);
    let config = SttConfig::default();

    let result = engine.transcribe(input, config).await;
    assert!(result.is_ok());
}
```

## 覆盖率报告

生成覆盖率报告：
```bash
cargo install cargo-llvm-cov
cargo llvm-cov --lib --tests
```

当前覆盖率：
- **整体**: ~80%
- **lib.rs (主引擎)**: ~75% (新增)
- **音频处理**: ~90%
- **编码器/解码器**: ~85%
