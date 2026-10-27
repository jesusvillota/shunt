---
title: "ChatGPT Web（纯文本）"
description: "ChatGPT Web（纯文本）"
---

在 Codex Responses 端点中使用 `kind = "chatgptweb_browser"`。需要一个已登录的浏览器配置文件和独立的 Node 辅助程序。工具和图片请求会返回错误。此阶段不轮换账号。

## 配置

配置的浏览器路由会显示在 Codex 模型选择器中。`upstream_model` 为可选项。设置时必须与页面显示的思考强度标签（如 `Medium` 或 `High`）完全一致。省略时不检查思考强度。辅助程序不会更改所选强度。使用 `auth = "none"` 和一个带有 `profile_dir` 的账号。将 `SHUNT_CHATGPTWEB_HELPER` 设为辅助程序 `cli.mjs` 的绝对路径。旧版 `base_url` 字段为必填项，但不会被使用。每次请求都应发送完整的文本历史。发送结果不确定时，shunt 不会重试。

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)

直接运行辅助程序时，`--chrome` 连接到在 `http://127.0.0.1:9333` 运行的 Chrome。`--cdp-url` 仅接受不含身份验证信息的 HTTP 回环地址。请使用独立配置文件 `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` 和调试端口手动启动 Chrome。辅助程序不添加浏览器参数，退出时不会关闭 Chrome。`--headless` 仅适用于辅助程序启动的 Chromium，不能与 `--chrome` 一起使用。Shunt 不传递这些参数。
