---
title: "ChatGPT Web (テキスト専用)"
description: "ChatGPT Web (テキスト専用)"
---

Codex Responses エンドポイントで `kind = "chatgptweb_browser"` を使います。ログイン済みのブラウザープロファイル1つと、別の Node ヘルパーが必要です。ツールと画像はエラーになります。この段階ではアカウントを切り替えません。

## 設定

設定したブラウザー経路は Codex のモデル選択に表示されます。`upstream_model` はブラウザーで選択したモデル名と一致させます。`auth = "none"` と `profile_dir` を持つアカウント1つを使います。`SHUNT_CHATGPTWEB_HELPER` にヘルパーの `cli.mjs` の絶対パスを指定します。既存の `base_url` は必須ですが使いません。各リクエストにテキスト履歴全体を送ります。送信結果が不明な場合、再試行しません。

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)

ヘルパーを直接実行する場合、`--chrome` でインストール済みの Google Chrome を選びます。Playwright の既定フラグを使います。別のプロファイル `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` を使ってください。既定ではウィンドウが開き、`--headless` で非表示になります。Shunt はこれらのフラグを渡しません。
