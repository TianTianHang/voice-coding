## ADDED Requirements

### Requirement: 官方鲁棒文本归一化
MOSS ONNX TTS 引擎 SHALL 在分词和切块前执行与官方 MOSS robust normalizer 行为等价的鲁棒文本归一化，以避免符号密集文本导致漏读或异常声音。

#### Scenario: 清理 Markdown 展示语法
- **WHEN** 合成文本包含 Markdown 标题、引用、列表、强调、表格分隔符、链接、代码围栏或 inline code 标记
- **THEN** 系统 SHALL 移除不适合朗读的 Markdown 语法标记
- **AND** 系统 SHALL 保留可朗读的自然语言内容
- **AND** 系统 MUST NOT 将反引号、井号、列表连字符或表格竖线原样送入 MOSS 分词

#### Scenario: 规范符号密集技术文本
- **WHEN** 合成文本包含箭头、连续破折号、下划线、斜杠、重复标点、零宽字符或控制字符
- **THEN** 系统 SHALL 将其转换为稳定的空格、停顿或句子边界
- **AND** 系统 MUST 删除零宽字符和控制字符
- **AND** 系统 MUST collapse repeated punctuation into a bounded spoken punctuation form

#### Scenario: 保护可读技术片段
- **WHEN** 合成文本包含 URL、Email、mention、hashtag、文件名、扩展名、版本号或短技术标识符
- **THEN** 系统 SHALL 在通用符号替换前保护这些片段
- **AND** 系统 SHALL 避免把 `.env`、`app.js.map`、`v2.3.1` 等片段拆成会破坏语义或导致异常发音的字符序列

#### Scenario: 符号-only 文本不进入推理
- **WHEN** 文本经过鲁棒归一化后没有可朗读内容
- **THEN** 系统 MUST NOT 调用 MOSS ONNX 推理
- **AND** 系统 SHALL 返回可定位的 invalid input error

#### Scenario: Debug TTS 同样应用鲁棒归一化
- **WHEN** 用户在 debug 面板手动合成包含 Markdown、路径、URL 或符号密集内容的文本
- **THEN** 系统 SHALL 使用同一套 MOSS 鲁棒归一化流程
- **AND** 系统 SHALL 在归一化后继续执行现有 token budget 切块和多 chunk 拼接流程

