---
title: "ChatGPT Web"
description: "ChatGPT Web"
---

Use `kind = "chatgptweb_browser"` with the Codex Responses endpoint. One logged-in browser profile and the separate Node helper are required. Serial shell and file-edit tools are supported. Images and unsupported tools return errors. This stage does not rotate accounts.

## Setup

The configured browser routes appear in the Codex model picker. `upstream_model` is optional; when set, it must exactly match the visible effort label, such as `Medium` or `High`. Omission leaves effort unchecked. The helper never changes effort. Use `auth = "none"` and one account with `profile_dir`. Set `SHUNT_CHATGPTWEB_HELPER` to the helper’s absolute `cli.mjs` path. The legacy `base_url` field is required but unused. Send the complete history, including tool results, each turn. Shunt never retries an uncertain browser send.

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)

Direct helper runs accept `--chrome` to attach to an existing Chrome at `http://127.0.0.1:9333`. `--cdp-url` accepts an HTTP loopback endpoint without authentication. Start Chrome manually with the separate profile `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` and the debugging port. The helper adds no browser flags and leaves Chrome open on shutdown. `--headless` applies only to launched Chromium and cannot be combined with `--chrome`. Shunt does not pass these flags.

The offline demo runs real Codex without a browser or account. Browser reliability requires the separate live gate. The live harness requires explicit profile/CDP inputs, Chat-only health and send guards, at most six sends in 15 minutes, no retries, and preserves owner Chrome. See the linked setup document for commands and limits.
