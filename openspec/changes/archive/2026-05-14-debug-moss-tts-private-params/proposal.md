# Debug 链路开放 MOSS TTS 私有参数

## 背景
当前 Debug TTS 面板只暴露 MOSS 采样模式和参考音频路径，无法验证 MOSS fixed sampling 的随机性、frame 上限或未来采样参数契约。MOSS fixed ONNX 图中已有 temperature、top-p、top-k 与 repetition penalty 常量，但这些常量当前 baked 在图内，不是运行时输入。

## 目标
- 在 Debug TTS 链路中开放 MOSS 私有参数配置面。
- 让 `seed` 和 `maxNewFrames` 在当前 MOSS 引擎中立即生效。
- 让 temperature、top-p、top-k 和 repetition penalty 先进入前后端配置契约，并在 Debug UI 明确提示当前 fixed 图限制。
- 允许 Debug TTS 直接执行流式合成播放，并在页面展示播放进度条。

## 非目标
- 不修改 ONNX 文件。
- 不新增参数化 fixed 图。
- 不把高级参数接入正式自动播报业务流。
