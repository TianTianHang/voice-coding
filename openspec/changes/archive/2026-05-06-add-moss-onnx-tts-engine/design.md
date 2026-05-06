## Context

当前后端 TTS runtime、播放输出和 VAD 播放期暂停/恢复链路已经存在，但合成引擎仍为 mock 实现，无法提供真实文本播报能力。本次变更需要在不破坏现有状态机与播放流程的前提下，引入 MOSS ONNX 本地推理能力，并确保模型目录契约、错误可观测性和测试完整性。

MOSS 方案由 TTS 模型与 Audio Tokenizer/Codec 模型共同组成，且 manifest 依赖相对路径引用 codec 元数据；因此实现重点不仅是推理调用，还包括模型资产一致性校验和目录布局约束。

## Goals / Non-Goals

**Goals:**
- 在 Rust 后端实现 `MossOnnxTtsEngine`，可接入现有 `TtsEngine` 抽象。
- 打通首版非流式推理：文本分词、音频 token 生成、`decode_full` 解码、返回 48kHz 立体声 PCM。
- 提供明确的模型加载/健康检查/错误分类，失败时可快速定位到文件缺失、路径错误或推理阶段。
- 与现有 TTS runtime 状态机兼容，保留播放期间暂停录音/VAD 的行为。
- 补齐单元与集成测试，并执行 Rust/Tauri/前端质量门禁。

**Non-Goals:**
- 首版不实现 `decode_step` 驱动的流式边合成边播放。
- 首版不实现参考音频克隆说话人，仅支持 MOSS 内置音色。
- 首版不改造前端交互模型，仅沿用现有 TTS 命令入口。

## Decisions

### 1) 引擎分层：模型资产层与推理执行层分离

将实现拆为两个内聚模块：
- 资产层：解析 `browser_poc_manifest.json`、`tts_browser_onnx_meta.json`、`codec_browser_onnx_meta.json`，完成文件存在性、相对路径可解析性、外部 data 文件绑定校验。
- 推理层：基于已校验资产创建 ONNX session，执行 token 生成与 codec 解码。

这样可以把“启动失败”与“运行时推理失败”分离，便于 health check 和测试覆盖。

**备选方案：** 初始化时直接散落读取文件并立刻推理。实现更快，但错误上下文不清晰，难定位 manifest/path 问题。

### 2) 先实现 `decode_full` 闭环，延后 `decode_step`

首版采用非流式闭环：
文本 -> tokenizer -> TTS ONNX -> audio codes -> codec `decode_full` -> PCM。

理由：`decode_step` 需要维护大量 transformer/attention cache，状态复杂，首版优先确保可靠可测交付。

**备选方案：** 一次性上流式。可降低首包延迟，但风险和调试成本显著上升。

### 3) 统一输出契约严格对齐 `tts-core`

引擎输出必须满足 `PLAYBACK_SAMPLE_RATE_HZ=48000`、`PLAYBACK_CHANNELS=2`。若模型输出元信息不符合，初始化直接失败，不在播放期再兜底隐式转换。

**备选方案：** 播放前隐式重采样/升降声道。更灵活但会掩盖模型资产问题，增加音质与性能不确定性。

### 4) 音色策略：显式映射 + 默认回退

`TtsConfig.voice` 采用“名称匹配内置音色表”；空值时使用默认音色；未知值返回可读错误并给出可选音色列表摘要。

**备选方案：** 未知音色静默回退默认值。体验更“容错”，但配置错误不可见，影响可控性。

### 5) runtime 注入策略：可配置选择 mock 或 MOSS

`TtsRuntime::default` 改为“按配置构造引擎”，本地开发可使用 mock，模型可用时使用 MOSS。这样保证现有测试与无模型环境可继续运行。

**备选方案：** 强制使用 MOSS。会导致 CI/开发环境在缺模型时普遍失败。

## ONNX Inference Pipeline

基于对 MOSS-TTS-Nano-100M-ONNX 官方 Python 推理代码（`infer_onnx.py`, `onnx_tts_runtime.py`, `ort_cpu_runtime.py`）的分析，MOSS ONNX 引擎采用 **Global + Local 混合 Transformer 架构**，推理链路分为 5 个阶段：

### Stage 0: 文本准备与预处理

```
输入文本
  ↓
1. 文本归一化 (WeTextProcessing / normalize_tts_text)
   → 处理多语言标点、大小写、空格
2. SentencePiece 分词
   → text_token_ids: List[int]
3. 长文本分块 (可选，max_tokens=75)
   → 按句子/分句切分，避免单次生成超长
```

### Stage 1: 参考音频编码 (Audio Tokenizer)

```
参考音频 (若提供，用于音色克隆)
  ↓
重采样 → 48kHz 立体声
  ↓
codec_encode.onnx (MOSS-Audio-Tokenizer-Nano)
  输入: waveform [1, channels, samples]
  输出: audio_codes [1, frames, n_quantizers=16]
  ↓
prompt_audio_codes: List[List[int]]  (RVQ 16 层 codebook)
```

**关键点**: Codec 与 TTS 是独立模型，通过 `codec_browser_onnx_meta.json` 引用。

### Stage 2: TTS 输入构建 (Prompt Construction)

```
构造特殊 token 序列:
[
  user_prompt_prefix_tokens,    # 来自 manifest
  audio_start_token_id,
  ... audio_prefix_rows ...     # 参考音频的 codes (若有)
  audio_end_token_id,
  user_prompt_after_reference_tokens,
  text_token_ids,               # 待合成文本的 token ids
  assistant_prompt_prefix_tokens,
  audio_start_token_id
]
  ↓
input_ids [batch=1, seq_len, n_vq+1]  # n_vq=8 (使用前 8 层 RVQ)
attention_mask [batch=1, seq_len]
```

**为什么是 n_vq+1?** 第 0 列存文本 token/slot token，第 1-8 列存 8 层 audio codebook tokens。

### Stage 3: TTS Prefill (Global Transformer)

```
moss_tts_prefill.onnx
  输入:
    - input_ids [1, seq, n_vq+1]
    - attention_mask [1, seq]
  输出:
    - global_hidden [1, seq, hidden_size]
    - present_key_values_* (各层的 KV cache)
  ↓
提取: global_hidden = global_hidden[:, -1, :]  # 最后一个隐藏状态
```

**作用**: 一次性处理完整 prompt，构建全局上下文表示和 KV cache，为后续自回归生成打基础。

### Stage 4: 自回归 Frame 生成循环 (核心)

```
FOR step_index in range(max_new_frames=375):
  │
  ├─ 4a: Local Transformer (帧预测)
  │    三种采样模式:
  │
  │    A) Greedy (do_sample=False)
  │        moss_tts_local_greedy_frame.onnx
  │        → 一次性生成 n_vq=8 个 token (确定性)
  │
  │    B) Fixed Sampling (默认, sample_mode="fixed")
  │        moss_tts_local_fixed_sampled_frame.onnx
  │        → 融合采样操作在 ONNX 图内，最快
  │
  │    C) Full Sampling (sample_mode="full")
  │        moss_tts_local_decoder.onnx / local_cached_step.onnx
  │        → 逐 channel 自回归采样，最灵活
  │
  │    输出:
  │      - should_continue: bool  (是否继续生成)
  │      - frame_token_ids [n_vq]  (8 个 codebook 各一个 token)
  │    ↓
  │  frame = [token_0, token_1, ..., token_7]
  │  generated_frames.append(frame)
  │
  ├─ 4b: Global Transformer Decode Step (更新 KV cache)
  │    moss_tts_decode_step.onnx
  │      输入:
  │        - input_ids [1, 1, n_vq+1]  # 当前生成的 frame
  │        - past_valid_lengths
  │        - past_key_values_* (上一步的 KV cache)
  │      输出:
  │        - global_hidden (更新后的最后隐藏状态)
  │        - present_key_values_* (更新的 KV cache)
  │    ↓
  │  更新 global_hidden, past_key_values
  │
  └─ IF should_continue=False: BREAK

输出: generated_frames: List[List[int]]
     [[t0_0, ..., t0_7],  # frame 0
      [t1_0, ..., t1_7],  # frame 1
      ...]
```

**Global + Local 设计优势**:
- Global 处理长距离依赖，但只在 prefill 和每步 decode 调用一次
- Local 是轻量级 4 层 decoder，专门负责帧预测，计算效率高
- 分离后支持流式：Local 可独立生成多帧后再更新 Global

### Stage 5: 音频解码 (Codec Decode)

```
generated_frames [num_frames, n_vq=8]
  ↓
A) Full Decode (首版实现)
   codec_decode_full.onnx
     输入: audio_codes [1, num_frames, n_quantizers=16]
     输出: audio [1, channels=2, samples]  # 48kHz 立体声

B) Streaming Decode (未来优化)
   codec_decode_step.onnx (带状态缓存)
     → 分批解码 (1-8 帧/批)，降低延迟
     → 维护 decoder state 跨调用

输出: waveform [channels, samples]  # float32 PCM
```

### ONNX 模型清单

**TTS 模型** (5 个):
1. `moss_tts_prefill.onnx` - Global transformer prefill
2. `moss_tts_decode_step.onnx` - Global transformer 自回归步
3. `moss_tts_local_decoder.onnx` - Local transformer (baseline)
4. `moss_tts_local_cached_step.onnx` - 带 KV cache 的优化 local step
5. `moss_tts_local_fixed_sampled_frame.onnx` - 融合采样的 fastest path

**Codec 模型** (2 个):
6. `codec_encode.onnx` - 音频编码 (参考音频 → codes)
7. `codec_decode_full.onnx` / `codec_decode_step.onnx` - 音频解码

### 数据流总结

```
text + (可选 ref_audio)
  → normalized_text
  → text_token_ids
  → input_ids (with special tokens)
  → prefill → global_hidden + KV cache
  → FOR each frame:
      local transform → frame [8 tokens]
      global decode step → update hidden + KV cache
  → generated_frames [N, 8]
  → codec decode → waveform [2, samples]
  → 48kHz stereo PCM
```

### 首版实现范围

- ✅ Stage 0-5 完整链路
- ✅ Greedy / Fixed 采样模式
- ✅ `decode_full` 批量解码
- ❌ 流式 `decode_step` (后续优化)
- ❌ 参考音频克隆 (后续扩展)

## Risks / Trade-offs

- [模型文件大、加载慢] -> 在 `prepare_tts` 做一次性健康检查并缓存 session，避免每次合成重复初始化。
- [manifest 相对路径耦合导致部署易错] -> 增加目录布局校验与错误提示，明确期望树形结构。
- [ONNX 输入输出名与 meta 不一致] -> 启动时对 session I/O 与 meta 声明做一致性比对，失败立即报错。
- [多平台 ORT 差异] -> 先锁定 CPU provider 路径，记录 provider 信息到日志，后续再扩展加速 provider。
- [测试依赖真实模型体积大] -> 单元测试以 mock/meta fixture 为主；真实模型集成测试标记为可选或按环境变量开启。
