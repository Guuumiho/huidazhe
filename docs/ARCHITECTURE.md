# ARCHITECTURE

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
  当前窗口右侧思维地图侧栏

### 后端

- [src-tauri/src/lib.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/lib.rs)
  常量、结构体、模块注册、Tauri 命令汇总
- [src-tauri/src/settings.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/settings.rs)
  设置读写
- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs)
  问答主流程、多对话窗口、短期记忆、中期记忆、失败兜底
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs)
  旧知识地图接口 + conversation map 增量更新逻辑
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
- `knowledge.rs` 既保留旧知识地图接口，也负责新的 conversation map
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

### 3. 思维地图

```text
问答成功
→ chat.rs 异步触发 refresh_conversation_map_internal
→ knowledge.rs 读取当前窗口已有节点和边
→ gpt-5.4-mini 返回本轮增量 JSON
→ 写入 conversation_map_nodes / edges / events
→ 前端 thought-map.js 拉取当前窗口图并渲染右侧圆形节点
```

思维地图特点：
- 每个 conversation 独立维护
- 不跨窗口共享
- 用户节点为实心
- 助手节点更透明
- 每轮最多新增 3 个助手节点

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
- 思维地图 V1：右侧侧栏、conversation map 数据层、异步增量更新
- 模型调用日志落盘
- 一键打包脚本 `build-exe.cmd`
- 前后端第一阶段模块拆分
- 旧知识地图冻结页
- 失败自动重试 / mini 降级 / 前端重发按钮

## 哪些地方是半成品 / 待重构 / 已知坑

### 半成品

- 思维地图 V1 已可用，但布局和抽取质量仍然是第一版
- 旧知识地图独立页面仍保留占位逻辑，后续可能删掉或并入思维地图

### 待重构

- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs) 仍然偏大，后续可以再拆成：
  - 模型客户端
  - 记忆策略
  - 记录写入
  - 失败兜底
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs) 同时承载旧知识地图和新思维地图，后续适合继续拆开
- [web/chat.js](/D:/Learning/agent/vibecoding/codex/question/web/chat.js) 仍然是前端最复杂文件

### 已知坑

- 项目里仍有历史中文编码污染，需要继续清理
- PowerShell 某些输出会显示乱码，但不等于文件本身损坏
- 思维地图增量更新失败不会阻塞聊天，但仍依赖结构化 LLM 输出质量

这个文档回答的问题是：
> 这个项目整体长什么样、模块怎么分、请求怎么流动、做到哪了、哪些地方还不能算稳定终态。
