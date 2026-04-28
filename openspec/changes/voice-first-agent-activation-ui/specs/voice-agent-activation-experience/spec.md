## ADDED Requirements

### Requirement: 语音助手在场感主舞台
系统 SHALL 在助手主界面提供以语音代理为中心的在场感主舞台，并将其作为最高视觉优先级区域。

#### Scenario: 待机在场感可见
- **WHEN** 助手已启动且未处于错误状态
- **THEN** 系统 SHALL 在主舞台显示待机在场状态
- **AND** 系统 SHALL 以低干扰方式提示助手可被唤醒

#### Scenario: 主舞台优先于次级配置
- **WHEN** 用户打开助手面板
- **THEN** 系统 SHALL 优先展示主舞台与当前状态文案
- **AND** 系统 SHALL NOT 让低频设置区成为首要视觉焦点

### Requirement: 激活仪式与状态迁移可感知
系统 SHALL 在语音激活与处理过程中提供可感知的状态迁移反馈，覆盖 Dormant、WakeDetected、Listening、Processing、Responding、Error 六个体验状态。

#### Scenario: 唤醒词命中反馈
- **WHEN** 系统检测到唤醒词或等效激活信号
- **THEN** 系统 SHALL 进入 WakeDetected 并给出短时确认反馈
- **AND** 系统 SHALL 在确认后进入 Listening

#### Scenario: 处理中与回应中可区分
- **WHEN** 用户语句提交后系统正在转写、执行或等待结果
- **THEN** 系统 SHALL 显示 Processing 状态
- **AND** 在系统生成并播报或展示回答时系统 SHALL 切换为 Responding

#### Scenario: 错误态可恢复
- **WHEN** 语音权限、连接或执行发生异常
- **THEN** 系统 SHALL 显示 Error 状态与可执行恢复提示
- **AND** 故障解除后系统 SHALL 可回到 Dormant 或 Listening

### Requirement: 纯语音最小可信闭环
系统 SHALL 提供纯语音交互所需的最小可见反馈闭环，包括 Heard、Intent、Status、Response 四类信息。

#### Scenario: 展示听到内容与理解意图
- **WHEN** 系统接收用户语音并完成基础解析
- **THEN** 系统 SHALL 展示 Heard（最近一句短转写）
- **AND** 系统 SHALL 展示 Intent（当前意图摘要）

#### Scenario: 展示执行状态与最新回应
- **WHEN** 系统执行用户请求并产生输出
- **THEN** 系统 SHALL 展示 Status（当前执行阶段）
- **AND** 系统 SHALL 展示 Response（最近一次回答摘要）

### Requirement: 语音优先且保留兜底控制
系统 SHALL 以语音作为默认主交互路径，同时保留低优先级的兜底手动控制。

#### Scenario: 纯语音可完成一轮交互
- **WHEN** 唤醒词可用且系统正常
- **THEN** 用户 SHALL 可以不点击按钮完成“唤醒、提问、获得回答”的完整一轮交互

#### Scenario: 手动控制降权展示
- **WHEN** 用户查看主界面
- **THEN** 系统 SHALL 保留按住说话、静音或停止等兜底入口
- **AND** 系统 SHALL 将其放置在低视觉优先级区域
