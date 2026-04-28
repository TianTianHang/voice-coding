# asr-model-loading Specification

## Purpose
TBD - created by archiving change prewarm-asr-model-loading. Update Purpose after archive.
## Requirements
### Requirement: ASR 模型支持后台预热

系统 SHALL 支持在首次转写前后台加载当前 Qwen3 ASR 模型。

#### Scenario: 启动预热

- **WHEN** 应用后端初始化或语音功能初始化触发 ASR 预热
- **THEN** 系统 SHALL 异步开始加载当前 Qwen3 ASR 模型
- **AND** 预热 SHALL 不阻塞前端应用渲染

#### Scenario: 幂等预热

- **WHEN** `prepare_asr` 在模型已经 `ready` 时被调用
- **THEN** 系统 SHALL 返回当前 ready 状态
- **AND** 系统 MUST NOT 创建新的 ASR 引擎实例

#### Scenario: 复用进行中的加载任务

- **WHEN** `prepare_asr` 在模型处于 `loading` 状态时被多次调用
- **THEN** 系统 SHALL 复用同一个加载任务
- **AND** 系统 MUST NOT 并发创建多个 ASR 引擎实例

#### Scenario: 失败后重试

- **WHEN** `prepare_asr` 在模型处于 `failed` 状态时被调用
- **THEN** 系统 SHALL 尝试重新加载模型
- **AND** 新的加载结果 SHALL 更新 ASR 状态快照

### Requirement: ASR 加载状态可查询

系统 SHALL 提供 ASR 加载状态快照，供前端和转写路径判断模型是否可用。

#### Scenario: 查询未加载状态

- **WHEN** `get_asr_status` 在尚未开始加载模型时被调用
- **THEN** 系统 SHALL 返回 `unloaded` 状态

#### Scenario: 查询加载中状态

- **WHEN** `get_asr_status` 在模型加载过程中被调用
- **THEN** 系统 SHALL 返回 `loading` 状态
- **AND** 响应 MAY 包含当前加载阶段名称

#### Scenario: 查询就绪状态

- **WHEN** `get_asr_status` 在模型加载成功后被调用
- **THEN** 系统 SHALL 返回 `ready` 状态
- **AND** 响应 SHALL 包含模型加载耗时信息

#### Scenario: 查询失败状态

- **WHEN** `get_asr_status` 在模型加载失败后被调用
- **THEN** 系统 SHALL 返回 `failed` 状态
- **AND** 响应 SHALL 包含失败错误消息

### Requirement: ASR 加载状态通过事件广播

系统 SHALL 在 ASR 加载状态变化时向前端广播 `asr-status` 事件。

#### Scenario: 加载开始事件

- **WHEN** ASR 模型加载开始
- **THEN** 系统 SHALL 发出 `asr-status` 事件
- **AND** 事件 payload SHALL 包含 `state: "loading"`

#### Scenario: 加载成功事件

- **WHEN** ASR 模型加载成功
- **THEN** 系统 SHALL 发出 `asr-status` 事件
- **AND** 事件 payload SHALL 包含 `state: "ready"`
- **AND** 事件 payload SHALL 包含加载耗时信息

#### Scenario: 加载失败事件

- **WHEN** ASR 模型加载失败
- **THEN** 系统 SHALL 发出 `asr-status` 事件
- **AND** 事件 payload SHALL 包含 `state: "failed"`
- **AND** 事件 payload SHALL 包含错误消息

#### Scenario: 状态事件是完整快照

- **WHEN** 前端收到任意 `asr-status` 事件
- **THEN** 事件 payload SHALL 包含足够信息以替换前端当前 ASR 状态
- **AND** 前端 MUST NOT 依赖增量事件推断状态

### Requirement: 转写路径等待或复用 ASR 加载结果

系统 SHALL 确保转写命令和 VAD 转写路径通过同一个 ASR runtime 获取已加载模型。

#### Scenario: ready 后转写

- **WHEN** 用户在 ASR 状态为 `ready` 时触发转写
- **THEN** 系统 SHALL 使用已加载的 ASR 引擎执行转写
- **AND** 系统 MUST NOT 重新初始化 ASR 引擎

#### Scenario: loading 期间转写

- **WHEN** 用户在 ASR 状态为 `loading` 时触发转写
- **THEN** 系统 SHALL 等待当前加载任务完成
- **AND** 如果加载成功，系统 SHALL 使用加载完成的 ASR 引擎执行转写
- **AND** 如果加载失败，系统 SHALL 返回包含加载失败原因的错误

#### Scenario: unloaded 时转写

- **WHEN** 用户在 ASR 状态为 `unloaded` 时触发转写
- **THEN** 系统 SHALL 启动 ASR 加载
- **AND** 系统 SHALL 复用该加载结果继续转写或返回加载错误

### Requirement: Qwen3 模型加载耗时被分阶段记录

系统 SHALL 记录当前 Qwen3 ASR 模型加载的分阶段耗时。

#### Scenario: 加载成功 timing

- **WHEN** Qwen3 ASR 模型加载成功
- **THEN** 系统 SHALL 记录总加载耗时
- **AND** 系统 SHALL 记录 ONNX sessions 加载耗时
- **AND** 系统 SHALL 记录 embedding 加载耗时
- **AND** 系统 SHALL 记录 tokenizer 加载耗时
- **AND** 系统 SHALL 记录 mel filterbank 构造耗时

#### Scenario: timing 随 ready 状态返回

- **WHEN** `get_asr_status` 返回 `ready` 状态
- **THEN** 响应 SHALL 包含最近一次成功加载的 timing 数据

#### Scenario: timing 随 ready 事件广播

- **WHEN** 系统广播 `state: "ready"` 的 `asr-status` 事件
- **THEN** 事件 payload SHALL 包含最近一次成功加载的 timing 数据

#### Scenario: 加载失败保留错误诊断

- **WHEN** Qwen3 ASR 模型加载失败
- **THEN** 系统 SHALL 记录失败错误消息
- **AND** 系统 SHALL 在 `failed` 状态快照和事件中暴露该错误消息

