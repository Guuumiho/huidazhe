# ARCHITECTURE

## 系统目标

这个项目的目标是做一个本地、轻量、适合个人开发者使用的桌面问答系统，并在问答之上逐步扩展两条能力：

- 带短期上下文的记忆问答
- 自动整理的知识地图

系统强调：

- 本地运行
- 低资源占用
- 问答与知识整理解耦
- 便于后续继续演进

## 主要模块

前端模块：

- [web/app.js](/D:/Learning/agent/vibecoding/codex/question/web/app.js)
  前端启动入口
- [web/state.js](/D:/Learning/agent/vibecoding/codex/question/web/state.js)
  共享状态
- [web/ui.js](/D:/Learning/agent/vibecoding/codex/question/web/ui.js)
  通用 UI 行为
- [web/settings.js](/D:/Learning/agent/vibecoding/codex/question/web/settings.js)
  设置区逻辑
- [web/chat.js](/D:/Learning/agent/vibecoding/codex/question/web/chat.js)
  多对话窗口、单点问答、记忆问答
- [web/knowledge.js](/D:/Learning/agent/vibecoding/codex/question/web/knowledge.js)
  知识地图逻辑

后端模块：

- [src-tauri/src/lib.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/lib.rs)
  后端入口、模块声明、命令注册、后台调度入口
- [src-tauri/src/settings.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/settings.rs)
  设置读写
- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs)
  问答主流程、多对话窗口、短期记忆、模型请求
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs)
  知识整理、节点、边、状态
- [src-tauri/src/storage.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/storage.rs)
  SQLite、路径、日志落盘

数据与配置：

- `settings.json`
- `qa_records.db`
- `model_calls.jsonl`

## 模块之间依赖关系

```text
前端
├─ app.js
│  ├─ settings.js
│  ├─ chat.js
│  ├─ knowledge.js
│  └─ ui.js / state.js
└─ 各模块通过 Tauri invoke 调后端命令

后端
├─ lib.rs
│  ├─ settings.rs
│  ├─ chat.rs
│  ├─ knowledge.rs
│  └─ storage.rs
└─ chat.rs / knowledge.rs 依赖 storage.rs
```

关系原则：

- `settings.js` / `settings.rs` 只管设置
- `chat.js` / `chat.rs` 只管问答主流程
- `knowledge.js` / `knowledge.rs` 只管知识地图
- `storage.rs` 是底层支持层，不反向依赖业务模块

## 数据流 / 请求流

### 1. 单点问答

```text
问题区输入
→ web/chat.js
→ invoke('ask')
→ src-tauri/src/chat.rs
→ 模型接口
→ 写入 qa_records
→ 返回前端渲染
```

### 2. 记忆问答

```text
问题区输入
→ web/chat.js 传 useShortTermMemory
→ src-tauri/src/chat.rs
→ 读取当前对话窗口最近几轮问答
→ 组装短期上下文后请求模型
→ 写入 qa_records
→ 返回前端渲染
```

### 3. 多对话窗口

```text
左边栏切换窗口
→ web/chat.js 更新 currentConversationId
→ invoke('list_history_records')
→ src-tauri/src/chat.rs
→ 按 conversation_id 读取 qa_records
→ 前端重绘消息区
```

### 4. 知识地图

```text
问答成功写入 qa_records
→ 后台小时级检查
→ src-tauri/src/knowledge.rs 整理未处理问答
→ 写入 knowledge_nodes / knowledge_edges / knowledge_sources
→ web/knowledge.js 读取并展示
```

### 5. 设置加载

```text
应用启动
→ web/settings.js
→ invoke('load_settings')
→ src-tauri/src/settings.rs
→ settings.json
→ 前端应用到设置区和主题
```

## 核心文件位置

最重要的入口文件：

- [web/app.js](/D:/Learning/agent/vibecoding/codex/question/web/app.js)
- [src-tauri/src/lib.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/lib.rs)

问答核心：

- [web/chat.js](/D:/Learning/agent/vibecoding/codex/question/web/chat.js)
- [src-tauri/src/chat.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/chat.rs)

知识地图核心：

- [web/knowledge.js](/D:/Learning/agent/vibecoding/codex/question/web/knowledge.js)
- [src-tauri/src/knowledge.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/knowledge.rs)

存储与配置：

- [src-tauri/src/settings.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/settings.rs)
- [src-tauri/src/storage.rs](/D:/Learning/agent/vibecoding/codex/question/src-tauri/src/storage.rs)

## 关键抽象与设计决策

### 1. 单点问答与记忆问答共用主流程

不是复制两套完整问答系统，而是共用问答主链路，只在“是否带短期记忆”这个策略上分叉。

### 2. 多对话窗口作为一级数据实体

对话窗口不是纯前端概念，后端有 `conversations` 表，`qa_records` 用 `conversation_id` 归属到具体窗口。这样模式、标题、历史范围都能自然跟窗口绑定。

### 3. 默认简洁回复在后端统一处理

简洁提示词不再由前端按钮控制，而是在 Rust 后端统一拼接，保证不同窗口和不同模式下的行为一致。

### 4. 知识地图与问答主流程解耦

知识整理不阻塞问答发送。问答先写原始记录，知识整理在后台批量进行。

### 5. 本地优先

设置、问答历史、知识节点、模型请求日志都保存在本地，不依赖外部数据库服务。

## 已完成到什么程度

已经完成：

- 本地设置区
- 多对话窗口与窗口切换
- 单点问答
- 记忆问答第一版
- 默认简洁回复
- 历史消息展示
- 模型调用日志落盘
- 知识地图第一版
- 后台按小时检查知识整理
- 前后端第一阶段按功能域拆分

## 半成品 / 待重构 / 已知坑

半成品：

- 记忆问答目前只有短期记忆，没有长期检索
- 知识地图还是第一版，关系抽取与节点归并仍偏粗糙

待重构：

- `chat.rs` 仍然偏大，后续可以再拆成模型客户端、上下文策略、历史查询
- `knowledge.rs` 仍然承担较多整理逻辑，后续可拆成聚类、抽取、持久化子模块

已知问题：

- 左边栏切换窗口的交互刚做过修复，需要继续观察是否还有偶发切换不稳定
- 问题区布局此前有过被挤出视口的问题，样式层仍需继续稳定
- 项目里仍有少量历史中文编码污染，后续需要逐步清理

这个文档解决的问题是：

> 这个项目整体长什么样，现在做到哪了，还有哪些地方不能误判成“已经完全稳定”。
