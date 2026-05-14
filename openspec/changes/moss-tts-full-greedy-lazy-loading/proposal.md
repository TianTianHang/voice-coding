## Why

当前 Rust 版 MOSS TTS 为降低内存只保留 fixed token 生成与 codec decode step 路径，导致现有规格中要求的非流式 `decode_full` 闭环和 `greedy` 采样模式不完整。用户希望恢复完整能力，同时避免初始化时一次性加载所有 ONNX sessions。

## What Changes

- 为 MOSS TTS 恢复非流式 codec `decode_full` 路径，流式路径继续使用 codec `decode_step`。
- 支持 `samplingMode: "greedy"`，通过 `local_decoder` 执行确定性 frame 生成；未指定时继续默认 `fixed`。
- 将 MOSS ONNX sessions 改为按需加载：首次使用某个生成/解码/参考音频能力时才创建对应 session。
- 更新资产校验、文档和测试，明确 `local_decoder` 与 `decode_full` 是可选能力文件，缺失时按调用路径报错或 fallback。

## Impact

- 影响规格：`moss-onnx-tts-engine`
- 影响代码：`src-tauri/tts-moss`、MOSS runtime 文档与真实模型测试
