---
title: "ChatGPT Web"
description: "ChatGPT Web"
---

Codex Responses エンドポイントで `kind = "chatgptweb_browser"` を使います。ログイン済みのブラウザープロファイル1つと、別の Node ヘルパーが必要です。シェルとファイル編集ツールを順番に実行できます。画像と未対応のツールはエラーになります。この段階ではアカウントを切り替えません。

## 設定

設定したブラウザー経路は Codex のモデル選択に表示されます。`upstream_model` は任意です。指定する場合、`Medium` や `High` など画面の思考努力ラベルと完全に一致させます。省略すると努力を確認しません。ヘルパーは選択された努力を変更しません。`auth = "none"` と `profile_dir` を持つアカウント1つを使います。`SHUNT_CHATGPTWEB_HELPER` にヘルパーの `cli.mjs` の絶対パスを指定します。既存の `base_url` は必須ですが使いません。各リクエストにツール結果を含む履歴全体を送ります。送信結果が不明な場合、再試行しません。

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)

ヘルパーを直接実行する場合、`--chrome` は `http://127.0.0.1:9333` で実行中の Chrome に接続します。`--cdp-url` は認証情報のない HTTP ループバックアドレスのみ受け付けます。別のプロファイル `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi` とデバッグポートを指定して Chrome を手動で起動してください。ヘルパーはブラウザーフラグを追加せず、終了時も Chrome を閉じません。`--headless` はヘルパーが起動する Chromium のみに適用され、`--chrome` と併用できません。Shunt はこれらのフラグを渡しません。

オフラインデモはブラウザーやアカウントなしで実際の Codex を実行します。ブラウザーの信頼性は別のライブ検証が必要です。ライブテストは明示的なプロファイル/CDP 入力と Chat モードの状態・送信チェックを要求します。15分で最大6回送信し、再試行せず、所有者の Chrome を維持します。コマンドと制限は上記の設定文書を参照してください。
