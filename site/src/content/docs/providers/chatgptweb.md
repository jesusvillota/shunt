---
title: "ChatGPT Web (text only)"
description: "ChatGPT Web (text only)"
---

Use `kind = "chatgptweb_browser"` with the Codex Responses endpoint. One logged-in browser profile and the separate Node helper are required. Tools and images return errors. This stage does not rotate accounts.

## Setup

The configured browser routes appear in the Codex model picker. `upstream_model` must equal the selected browser model label. Use `auth = "none"` and one account with `profile_dir`. Set `SHUNT_CHATGPTWEB_HELPER` to the helper’s absolute `cli.mjs` path. The legacy `base_url` field is required but unused. Send the complete text history each turn. Shunt never retries an uncertain browser send.

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)
