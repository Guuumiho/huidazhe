# 回答者

一个运行在本地的轻量桌面问答工具，面向个人开发者的日常零散提问场景。项目基于 Tauri，当前支持：

- 多对话窗口切换
- 单点问答模式
- 记忆问答模式
- 知识地图视图
- 本地 SQLite 历史记录

## 怎么启动

前置要求：

- Windows
- Rust / Cargo
- Visual Studio Build Tools
- Node.js

在项目根目录运行：

```bat
.\start.cmd
```

只做 Rust 编译检查：

```bat
cargo check --manifest-path src-tauri/Cargo.toml
```

## 核心目录结构

```text
question/
├─ web/                     前端页面与交互逻辑
│  ├─ app.js                前端启动入口
│  ├─ state.js              共享状态
│  ├─ ui.js                 通用 UI 行为
│  ├─ settings.js           设置区逻辑
│  ├─ chat.js               问答主流程与多窗口切换
│  ├─ knowledge.js          知识地图视图
│  ├─ index.html            页面骨架
│  └─ styles.css            样式
├─ src-tauri/
│  └─ src/
│     ├─ lib.rs             后端入口与命令注册
│     ├─ settings.rs        设置读写
│     ├─ chat.rs            问答、多窗口、短期记忆
│     ├─ knowledge.rs       知识地图与后台整理
│     └─ storage.rs         SQLite、路径、日志
├─ docs/
│  └─ ARCHITECTURE.md       架构说明
├─ README.md                项目稳定说明
└─ start.cmd                Windows 启动脚本
```

## 主要技术栈

- Tauri
- Rust
- SQLite
- 原生 HTML / CSS / JavaScript
- OpenAI 兼容接口

## 最基本使用方式

1. 启动应用
2. 在设置区填写 `API URL` 和 `API Key`
3. 点击左侧“新增对话窗口”创建新窗口，或切换已有窗口
4. 在问题区输入问题并发送
5. 用问题区左下角的模式按钮切换：
   - `单点`
   - `记忆`
6. 如需查看知识整理结果，切换到“知识地图”

## 更多信息

- 架构全景见 [docs/ARCHITECTURE.md](/D:/Learning/agent/vibecoding/codex/question/docs/ARCHITECTURE.md)
