# 设计：Debug MOSS TTS 私有参数

## 配置契约
扩展 `TtsConfig.moss`，新增 `seed`、`maxNewFrames`、`textTemperature`、`textTopP`、`textTopK`、`audioTemperature`、`audioTopP`、`audioTopK`、`audioRepetitionPenalty`。字段使用 camelCase 序列化，未提供时保持当前行为。

## 推理行为
`seed` 用于初始化 fixed sampling 的 `SimpleRng`，未提供时继续使用系统时间。`maxNewFrames` 覆盖 manifest 中的生成 frame 上限，未提供时继续使用模型默认值。greedy 模式不使用随机数。

temperature、top-p、top-k 和 repetition penalty 只进入配置契约和 Debug UI；当前 `moss_tts_local_fixed_sampled_frame.onnx` 中这些值是 Constant，v1 不尝试 patch 图或重写采样器。

## Debug UI
Debug TTS 面板新增 voice、seed、max frame 和采样参数输入。构造 invoke config 时只发送有效输入：空字符串省略，整数取非负 floor，浮点数要求有限。界面展示说明：seed/max frames 当前生效，采样常量已发送但 fixed 图中暂不生效。

Debug TTS 面板新增流式播放入口。后端在流式合成过程中接收 `AudioChunk` 后立即写入现有 `AudioOutput` 队列，并通过 Debug 专用事件上报 started、chunk、end、error 状态。前端根据事件更新播放条；播放条以已播放时间和已入队音频时长的比例展示进度，在合成结束后收敛到 100%。
