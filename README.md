# 回答者

一个本地运行的 Tauri 问答工具，面向个人开发者的高频零碎提问场景。

当前项目主要有三条主线：
- 单点问答：每个问题互不关联，不污染主工作流上下文
- 记忆问答：同一对话窗口内保留上下文，适合连续追问
- 思维地图：每个对话窗口右侧自动生成一张思维链图，展示问题推进和回答关键点

## 怎么启动

开发运行：

```bat
.\start.cmd
```

一键打包 exe：

```bat
.\build-exe.cmd
```

打包产物默认在：

`src-tauri\target\release\local-qa-window.exe`

## 核心目录结构

```text
question/
├─ web/
│  ├─ index.html
│  ├─ styles.css
│  ├─ app.js
│  ├─ state.js
│  ├─ ui.js
│  ├─ settings.js
│  ├─ chat.js
│  ├─ knowledge.js
│  └─ thought-map.js
├─ src-tauri/
│  ├─ Cargo.toml
│  └─ src/
│     ├─ main.rs
│     ├─ lib.rs
│     ├─ settings.rs
│     ├─ chat.rs
│     ├─ knowledge.rs
│     └─ storage.rs
├─ docs/
│  └─ ARCHITECTURE.md
├─ start.cmd
└─ build-exe.cmd
```

## 主要技术栈

- Tauri 2
- Rust
- SQLite
- 原生 HTML / CSS / JavaScript
- Reqwest

## 最基本使用方式

1. 启动应用。
2. 在设置区填写并保存 `API URL` 和 `API Key`。
3. 点击左侧“新增对话窗口”。
4. 选择：
   - `单点`：适合互不相关的零碎问题
   - `记忆`：适合同一主题下连续追问
5. 在问题区输入问题并发送。
6. 右侧思维地图会按当前窗口逐步生成节点和关系。

## 接口说明

- `API URL` 可以填完整端点，也可以填 base URL。
- 如果填的是 base URL，程序会自动补成 `/chat/completions`。
- 如果 URL 以 `/responses` 结尾，会自动走 Responses API。

## 本地数据

应用运行数据默认保存在本机应用数据目录，例如：

- `settings.json`
- `qa_records.db`
- `model_calls.jsonl`

这些文件不会被正常的 git 提交带上远端仓库。
