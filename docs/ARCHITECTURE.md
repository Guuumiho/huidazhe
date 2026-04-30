# ARCHITECTURE

## 思维链 V2 目标架构（待实现）

旧右侧思维地图 / 知识地图 V1 已暂停使用。旧 `conversation_map_nodes`、`conversation_map_edges`、`conversation_map_events`、`knowledge_*` 数据不删除，后续可用于对比或迁移，但新实现不继续复用这套数据结构。

后端 [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs) 已空壳化：保留原 Tauri 命令签名，但不再访问数据库、不再调用模型、不再聚类、不再写入旧知识图或 conversation map。右侧 [web/thought-map.js](/D:/Learning/agent/vibecoding/codex/question/web/thought-map.js) 只保留占位告示。

思维链 V2 目标是服务 `记忆` 对话窗口：根据用户最近的连续问答，维护一份可用于右栏展示的思维链状态。

### V2 第一版模块拆分

V2 第一版暂定拆分为 6 个模块：

1. 上下文组装：已确认，负责把最近 2 轮对话、旧 `state_json` 和系统提示词组合成 LLM 输入。
2. LLM 抽取：负责问题识别、显式任务识别、隐式任务识别。
3. JSON 校验与降级：负责校验模型输出、处理解析失败、自动重试 1 次、避免坏结果覆盖旧状态。
4. 状态存储：负责每个记忆窗口一份完整 `state_json`，并记录更新事件。
5. 右侧栏渲染：负责把 `state_json.nodes` 展示为问题、显式任务、隐式任务节点。
6. 调试日志：负责记录 prompt、模型返回、解析结果和失败原因。

### 触发范围

- 只在 `记忆` 窗口的成功问答后触发。
- `单点` 窗口不触发。
- 失败不影响主问答结果。

### 输入与输出

输入暂定为：

- 最近 2 轮对话。
- 当前窗口旧 `state_json`。
- 系统提示词，包含任务说明、返回 JSON 格式和抽取规则。

输出为完整新版 `state_json`，不是增删改操作列表。

### state_json 核心结构

```json
{
  "nodes": [
    {
      "id": 1,
      "type": "问题",
      "keyword": "",
      "description": "",
      "confidence": 0,
      "parent_id": -1
    }
  ]
}
```

节点类型第一版固定为：

- `问题`
- `显式任务`
- `隐式任务`

`confidence` 范围为 `0-10`。

V2 第一版只解决节点抽取、节点价值、去重、父子关系和右侧栏节点渲染。任务跟踪相关状态不进入第一版。

### 后续迭代：任务跟踪

任务确认触发模块第一版目标不是“管理所有任务”，而是“在合适时机邀请用户把一个任务升级为导航追踪”。

后续任务跟踪再接入 `active_task`、`active_task_id`、`current_focus`、`next_step`、`conclusion`、`prompt_state`、右边栏下方任务确认弹窗，以及多任务小任务条轻量展示。任务跟踪不是废弃，而是放到节点展示稳定后的下一步。

### JSON 校验与降级

思维链 V2 接收 LLM 输出后，必须先校验，再决定是否写入正式 `state_json`。

校验内容：

- 合法 JSON：必须能解析，不能带 markdown、解释文字或代码块。
- 结构完整：顶层必须包含 `nodes`。
- 字段类型：`nodes` 必须是 array。
- 节点字段：每个节点必须包含 `id`、`type`、`keyword`、`description`、`confidence`、`parent_id`。
- 业务规则：`type` 只能是 `问题` / `显式任务` / `隐式任务`；`confidence` 必须在 `0-10`；`parent_id` 必须是 `-1` 或真实存在节点；LLM 不能改旧节点 `id` 和 `keyword`。

降级策略：

- 第一次解析或校验失败后，自动重试 1 次。
- 重试只要求模型修正 JSON 格式和校验错误，不重新改变业务意图。
- 第二次仍失败，则跳过本轮思维链更新。
- 任何失败都不覆盖旧 `state_json`。
- 聊天主流程不受思维链失败影响。

事件存档：

- 采用事件全量存档。
- 成功与失败都记录 prompt 摘要、原始返回、旧状态、候选状态、应用状态、错误原因、重试次数和最终状态。

## 系统目标

这是一个本地、轻量、自用导向的桌面问答工具。

当前系统围绕三条主线演进：
- 单点问答
- 记忆问答
- 思维地图

系统设计优先级：
- 本地运行
- 低资源占用
- 多对话窗口隔离
- 数据默认保留在本机
- 便于持续迭代

## 主要模块

### 前端

- [web/app.js](/D:/Learning/agent/vibecoding/codex/question/web/app.js)
  前端启动入口
- [web/state.js](/D:/Learning/agent/vibecoding/codex/question/web/state.js)
  前端共享状态
- [web/ui.js](/D:/Learning/agent/vibecoding/codex/question/web/ui.js)
  通用 UI 行为和 DOM 工具
- [web/settings.js](/D:/Learning/agent/vibecoding/codex/question/web/settings.js)
  设置区逻辑
- [web/chat.js](/D:/Learning/agent/vibecoding/codex/question/web/chat.js)
  多对话窗口、单点问答、记忆问答、失败重发
- [web/knowledge.js](/D:/Learning/agent/vibecoding/codex/question/web/knowledge.js)
  旧知识地图冻结页
- [web/thought-map.js](/D:/Learning/agent/vibecoding/codex/question/web/thought-map.js)
  当前窗口右侧思维地图占位侧栏

### 后端

- [src-tauri/src/lib.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/lib.rs)
  常量、结构体、模块注册、Tauri 命令汇总
- [src-tauri/src/settings.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/settings.rs)
  设置读写
- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs)
  问答主流程、多对话窗口、短期记忆、中期记忆、失败兜底
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs)
  旧知识地图 / conversation map 的后端空壳命令，当前只返回空数据或 `disabled`
- [src-tauri/src/storage.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/storage.rs)
  SQLite、路径、日志文件

## 模块之间依赖关系

```text
前端
├─ app.js
├─ settings.js
├─ chat.js
├─ thought-map.js
├─ knowledge.js
└─ ui.js / state.js
   └─ 通过 Tauri invoke 调后端命令

后端
├─ lib.rs
├─ settings.rs
├─ chat.rs
├─ knowledge.rs
└─ storage.rs
   └─ chat.rs / knowledge.rs 依赖 storage.rs
```

原则：
- 设置、问答、思维地图按功能域拆开
- `chat.rs` 管问答
- `knowledge.rs` 当前只保留旧命令空壳，V2 不复用其中实现
- `storage.rs` 只做底层数据支持

## 数据流 / 请求流

### 1. 单点问答

```text
问题区输入
→ web/chat.js
→ invoke("ask")
→ src-tauri/src/chat.rs
→ gpt-5.4
→ 成功后写入 qa_records
→ 异步触发 refresh_conversation_map_internal
→ 返回前端渲染
```

单点模式特点：
- 不带短期记忆
- 不更新中期记忆
- 仍然会更新当前窗口的思维地图

### 2. 记忆问答

```text
问题区输入
→ web/chat.js
→ invoke("ask", useShortTermMemory=true)
→ src-tauri/src/chat.rs
→ 读取当前 conversation 的中期记忆
→ 读取当前 conversation 下最近 6 轮、且 prompt_mode=memory 的历史问答
→ 发送给 gpt-5.4
→ 成功后写入 qa_records
→ 更新 conversation_session_memory
→ 异步触发 refresh_conversation_map_internal
→ 返回前端渲染
```

记忆模式特点：
- 短期记忆只取当前窗口、记忆模式下的历史
- 中期记忆按窗口单独保存

当前已知限制：
- 中期记忆提取器仍是 V1，存在废话多、关键信息遗漏的问题。
- 该提取器后续应独立优化 prompt、校验和写入策略，不应和右侧思维链 V2 的节点抽取混用。
- 记忆模块的目标是压缩对后续回答有用的会话状态，而不是生成可视化节点。

### 3. 思维地图

```text
当前状态
→ V1 后端 refresh_conversation_map_internal 已空壳化
→ knowledge.rs 不再读取旧节点、不再调用模型、不再写入旧图数据
→ thought-map.js 只渲染右侧占位告示
→ V2 后续将新建 thought_chain 数据层和渲染链路
```

思维地图特点：
- V1 数据表暂留但不再运行
- V2 第一版仍计划按 conversation 隔离
- 当前右侧栏只保留页面框架和告示

### 4. 失败兜底链路

```text
gpt-5.4 第一次失败
→ 自动重试一次 gpt-5.4
→ 再失败则切到 gpt-5.4-mini
→ 若 mini 成功，记录 fallback_notice
→ 若 mini 也失败，不写数据库，只在前端显示本地失败消息和“重新发送”
```

### 5. 多对话窗口

```text
左边栏创建窗口
→ create_conversation(mode)
→ conversations 表新增一条记录
→ 对应 conversation_session_memory 初始化空记录

左边栏切换窗口
→ currentConversationId 改变
→ list_history_records(conversation_id)
→ get_conversation_map(conversation_id)
→ 只加载该窗口自己的问答和思维地图
```

## 核心文件位置

问答主链路：
- [web/chat.js](/D:/Learning/agent/vibecoding/codex/question/web/chat.js)
- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs)

思维地图：
- [web/thought-map.js](/D:/Learning/agent/vibecoding/codex/question/web/thought-map.js)
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs)

设置与本地配置：
- [web/settings.js](/D:/Learning/agent/vibecoding/codex/question/web/settings.js)
- [src-tauri/src/settings.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/settings.rs)

数据层：
- [src-tauri/src/storage.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/storage.rs)

## 关键抽象与设计决策

### 1. 单点问答和记忆问答共用 ask 主链路

不是复制两套系统，而是：
- 共用一个 `ask`
- 由 `useShortTermMemory` 和 conversation mode 决定是否带记忆

### 2. conversation 是一等实体

每个对话窗口有自己的：
- title
- mode
- updated_at
- session memory
- qa_records 范围
- conversation map

所以窗口之间是逻辑隔离的，不是前端假切换。

### 3. 非主问答模型统一走 mini

当前约定：
- 当前问答窗口点击发送：`gpt-5.4`
- 其他辅助调用：`gpt-5.4-mini`

辅助调用包括：
- 会话标题生成
- 中期记忆更新
- 思维地图增量更新

### 4. 思维地图只做增量更新，不做全图重算

每轮只处理：
- 本轮用户问题命中哪个节点
- 是否要把助手节点转正
- 新增哪些助手节点
- 节点怎么连线

这样可以降低 token 和布局抖动。

## 已完成到什么程度

已完成：
- 设置区与本地配置
- 多对话窗口
- 单点问答
- 记忆问答第一版
- 短期记忆按窗口隔离
- 中期记忆按窗口保存
- 思维地图 V1 已停用：右侧侧栏保留占位，后端旧逻辑已空壳化
- 模型调用日志落盘
- 一键打包脚本 `build-exe.cmd`
- 前后端第一阶段模块拆分
- 旧知识地图冻结页
- 失败自动重试 / mini 降级 / 前端重发按钮

## 哪些地方是半成品 / 待重构 / 已知坑

### 半成品

- 思维链 V2 尚未实现；旧思维地图 V1 已停用
- 旧知识地图独立页面仍保留占位逻辑，后续可能删掉或并入思维地图

### 待重构

- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs) 仍然偏大，后续可以再拆成：
  - 模型客户端
  - 记忆策略
  - 记录写入
  - 失败兜底
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs) 当前只是旧命令空壳；V2 应新建独立实现，不要在旧 V1 逻辑上修补
- [web/chat.js](/D:/Learning/agent/vibecoding/codex/question/web/chat.js) 仍然是前端最复杂文件

### 已知坑

- PowerShell 某些输出会显示乱码，但不等于文件本身损坏。维护文档当前已确认是正常 UTF-8。
- 后续若看到中文乱码，先用 Node 按 UTF-8 读取文件确认真实内容；不要只凭终端显示结果反复修文档。
- 记忆问答的中期记忆提取器 V1 存在废话多、关键信息遗漏的问题，需要后续单独优化。
- 思维地图增量更新失败不会阻塞聊天，但仍依赖结构化 LLM 输出质量

这个文档回答的问题是：
> 这个项目整体长什么样、模块怎么分、请求怎么流动、做到哪了、哪些地方还不能算稳定终态。
