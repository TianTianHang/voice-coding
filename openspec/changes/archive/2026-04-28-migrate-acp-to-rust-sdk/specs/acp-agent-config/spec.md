## ADDED Requirements

### Requirement: 通过 JSON 文件配置 ACP agent profiles
系统 SHALL 从 JSON 配置文件加载 ACP agent profiles，并使用默认 profile 连接 agent。

#### Scenario: 加载指定配置文件
- **WHEN** 环境变量 `ACP_AGENT_CONFIG` 指向一个存在的 JSON 配置文件
- **THEN** 系统 SHALL 从该路径加载 ACP agent 配置

#### Scenario: 加载默认配置文件
- **WHEN** 环境变量 `ACP_AGENT_CONFIG` 未设置
- **AND** 当前工作目录存在 `acp-agents.json`
- **THEN** 系统 SHALL 从当前工作目录的 `acp-agents.json` 加载 ACP agent 配置

#### Scenario: 配置文件不存在
- **WHEN** 系统无法找到 ACP agent 配置文件
- **THEN** 系统 SHALL 返回明确的未配置错误
- **AND** 系统 SHALL NOT 尝试启动 agent 进程

### Requirement: 支持多 profile 和默认 profile
系统 SHALL 支持在同一个配置文件中定义多个 agent profile，并通过默认 profile 决定自动连接目标。

#### Scenario: 使用 defaultProfile
- **WHEN** 配置文件包含 `defaultProfile`
- **AND** `profiles` 中存在匹配该 id 的 profile
- **THEN** 系统 SHALL 使用该 profile 作为 `connect_agent` 的默认连接目标

#### Scenario: 未指定 defaultProfile
- **WHEN** 配置文件未包含 `defaultProfile`
- **AND** `profiles` 中只包含一个 profile
- **THEN** 系统 SHALL 使用唯一 profile 作为默认连接目标

#### Scenario: 默认 profile 无法解析
- **WHEN** 配置文件未包含可解析的默认 profile
- **THEN** 系统 SHALL 返回明确配置错误
- **AND** 系统 SHALL NOT 尝试启动 agent 进程

### Requirement: profile 描述 agent 启动参数
系统 SHALL 使用 profile 中的字段描述 agent 子进程的启动方式。

#### Scenario: 启动参数完整
- **WHEN** profile 包含 `command`、`args`、`cwd` 和 `env`
- **THEN** 系统 SHALL 使用 `command` 作为可执行命令
- **AND** 系统 SHALL 使用 `args` 数组作为命令参数
- **AND** 系统 SHALL 使用 `cwd` 作为子进程工作目录
- **AND** 系统 SHALL 将 `env` 映射传给子进程环境

#### Scenario: 参数包含空格
- **WHEN** profile 的 `args` 数组中某个参数包含空格
- **THEN** 系统 SHALL 将该数组项作为单个参数传给子进程
- **AND** 系统 SHALL NOT 对该参数执行空白切分

#### Scenario: 缺少必填字段
- **WHEN** profile 缺少 id、name 或 command
- **THEN** 系统 SHALL 返回明确配置错误
- **AND** 系统 SHALL NOT 启动 agent 进程

### Requirement: 支持环境变量插值
系统 SHALL 支持在 profile env 值中引用当前进程环境变量，以避免在配置文件中明文保存 secret。

#### Scenario: 展开环境变量引用
- **WHEN** profile env 值为 `${NAME}` 格式
- **AND** 当前进程环境变量中存在 `NAME`
- **THEN** 系统 SHALL 将该 env 值展开为环境变量 `NAME` 的值

#### Scenario: 环境变量引用缺失
- **WHEN** profile env 值为 `${NAME}` 格式
- **AND** 当前进程环境变量中不存在 `NAME`
- **THEN** 系统 SHALL 返回明确配置错误
- **AND** 系统 SHALL NOT 启动 agent 进程

#### Scenario: 字面量环境变量
- **WHEN** profile env 值不是完整的 `${NAME}` 格式
- **THEN** 系统 SHALL 将该值作为字面量传给 agent 子进程

### Requirement: 保留最小环境变量入口
系统 SHALL 仅保留必要的环境变量入口用于定位配置文件和展开 secret。

#### Scenario: 不再依赖 env-only agent 配置
- **WHEN** 用户请求连接 agent
- **THEN** 系统 SHALL 从 JSON profile 配置解析 agent
- **AND** 系统 SHALL NOT 要求设置 `ACP_AGENT_CMD` 或 `ACP_AGENT_ARGS`

#### Scenario: 配置路径覆盖
- **WHEN** 用户设置 `ACP_AGENT_CONFIG`
- **THEN** 系统 SHALL 将其视为配置文件路径覆盖
- **AND** 系统 SHALL NOT 将其作为 agent 命令或参数
