# 回答者

- 是否有当前对话窗口提问细节问题扰乱上下文信息的困扰？回答者为了不污染IDE工作对话上下文窗口而生。
- 适用于提问无上下文联系的单点问题。
- 私有AI对话窗口，沉淀所有提问信息成个人知识库。

## 特性

- Tauri 桌面应用，资源占用相对轻
- 前端使用原生 `HTML/CSS/JS`
- 支持填写并保存 `API URL`、`API Key`、`Model`
- 每次提问只发送当前消息，不带历史上下文
- 历史记录保存到本地 SQLite
- 历史可查看，但不会影响后续回答

## 运行

先安装 Rust 和 Visual Studio Build Tools，然后执行：

```bat
start.cmd
```

## 接口说明

- `API URL` 可以填完整端点，也可以填 base URL
- 如果填的是 base URL，程序会自动补成 `/chat/completions`
- 如果 URL 以 `/responses` 结尾，会自动走 Responses API

## 隐私

- 不上传你的本地配置、数据库、构建产物和安装器
- 前端不会直接暴露你的 API Key，请求由 Tauri 后端发起
