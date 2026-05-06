## MODIFIED Requirements

### Requirement: ASR 加载状态可查询

系统 SHALL 提供 ASR 加载状态快照，供前端和转写路径判断模型是否可用，并 SHALL 暴露当前 ASR 模型的统一路径诊断信息。

#### Scenario: 查询未加载状态

- **WHEN** `get_asr_status` 在尚未开始加载模型时被调用
- **THEN** 系统 SHALL 返回 `unloaded` 状态
- **AND** 响应 SHALL 包含当前 ASR 模型路径诊断信息

#### Scenario: 查询加载中状态

- **WHEN** `get_asr_status` 在模型加载过程中被调用
- **THEN** 系统 SHALL 返回 `loading` 状态
- **AND** 响应 MAY 包含当前加载阶段名称
- **AND** 响应 SHALL 包含用于本次加载的 ASR 模型路径诊断信息

#### Scenario: 查询就绪状态

- **WHEN** `get_asr_status` 在模型加载成功后被调用
- **THEN** 系统 SHALL 返回 `ready` 状态
- **AND** 响应 SHALL 包含模型加载耗时信息
- **AND** 响应 SHALL 包含成功加载的 ASR 模型路径诊断信息

#### Scenario: 查询失败状态

- **WHEN** `get_asr_status` 在模型加载失败后被调用
- **THEN** 系统 SHALL 返回 `failed` 状态
- **AND** 响应 SHALL 包含失败错误消息
- **AND** 响应 SHALL 包含失败加载尝试对应的 ASR 模型路径诊断信息

#### Scenario: 保留 ASR 模型目录兼容字段

- **WHEN** `get_asr_status` 返回任意 ASR 状态快照
- **THEN** 响应 SHALL 继续包含顶层 `modelDir` 字段
- **AND** 顶层 `modelDir` SHALL 等于当前传给 ASR 引擎的模型目录
- **AND** 响应 SHALL 同时包含结构化模型路径诊断字段

### Requirement: ASR 加载状态通过事件广播

系统 SHALL 在 ASR 加载状态变化时向前端广播 `asr-status` 事件，并 SHALL 在事件中包含完整模型路径诊断信息。

#### Scenario: 加载开始事件

- **WHEN** ASR 模型加载开始
- **THEN** 系统 SHALL 发出 `asr-status` 事件
- **AND** 事件 payload SHALL 包含 `state: "loading"`
- **AND** 事件 payload SHALL 包含用于本次加载的 ASR 模型路径诊断信息

#### Scenario: 加载成功事件

- **WHEN** ASR 模型加载成功
- **THEN** 系统 SHALL 发出 `asr-status` 事件
- **AND** 事件 payload SHALL 包含 `state: "ready"`
- **AND** 事件 payload SHALL 包含加载耗时信息
- **AND** 事件 payload SHALL 包含成功加载的 ASR 模型路径诊断信息

#### Scenario: 加载失败事件

- **WHEN** ASR 模型加载失败
- **THEN** 系统 SHALL 发出 `asr-status` 事件
- **AND** 事件 payload SHALL 包含 `state: "failed"`
- **AND** 事件 payload SHALL 包含错误消息
- **AND** 事件 payload SHALL 包含失败加载尝试对应的 ASR 模型路径诊断信息

#### Scenario: 状态事件是完整快照

- **WHEN** 前端收到任意 `asr-status` 事件
- **THEN** 事件 payload SHALL 包含足够信息以替换前端当前 ASR 状态
- **AND** 前端 MUST NOT 依赖增量事件推断状态
- **AND** 事件 payload SHALL 包含当前 ASR 模型路径诊断信息

## ADDED Requirements

### Requirement: ASR 使用统一路径解析结果加载模型
系统 SHALL 通过统一模型路径解析结果创建当前 Qwen3 ASR 引擎。

#### Scenario: 从标准布局加载 ASR 模型
- **WHEN** 当前 ASR 模型解析到标准目录 `<model-home>/asr/qwen3-asr-0.6b-onnx`
- **THEN** 系统 SHALL 使用该目录创建 Qwen3 ASR 引擎
- **AND** ASR 状态快照 SHALL 将模型来源标记为对应解析来源

#### Scenario: 从引擎级环境变量加载 ASR 模型
- **WHEN** `STT_MODEL_DIR` 被设置
- **THEN** 系统 SHALL 使用 `STT_MODEL_DIR` 指向的目录创建 Qwen3 ASR 引擎
- **AND** ASR 状态快照 SHALL 将模型来源标记为 `engineEnv`

#### Scenario: 从旧开发布局加载 ASR 模型
- **WHEN** 当前 ASR 模型解析到旧开发目录 `./models`
- **THEN** 系统 SHALL 使用 `./models` 创建 Qwen3 ASR 引擎
- **AND** ASR 状态快照 SHALL 将 `legacyLayout` 标记为 `true`
