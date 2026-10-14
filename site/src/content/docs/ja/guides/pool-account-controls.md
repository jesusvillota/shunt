---
title: プールアカウント制御
description: 個々のプールアカウントを一時停止し、利用可能なアカウントをクォータのリセットが最も早い順に並べ替える — どちらも管理ダッシュボードからランタイムで、設定変更や再起動なしに。
---

アカウントプールの選択([Anthropic マルチアカウント](/ja/guides/anthropic-multi-account/)、[Codex マルチアカウント](/ja/guides/codex-multi-account/))の上に、2 つのランタイム制御があります。単一アカウントの一時停止と、利用可能なアカウントをバーンレートのヘッドルームではなくクォータのリセットが最も早い順に並べ替えることです。どちらも管理ダッシュボードの「Managed pool health」テーブルから、または管理 API を直接呼び出して操作します。どちらもメモリ上のみの状態です — 再起動すると消えるため、`shunt.toml` には一切触れません。

## アカウントの一時停止

一時停止は、設定側の `disabled = true` と同様にアカウントを選択対象から除外しますが、`shunt.toml` を編集したりアカウントをサインアウトさせたりしません。資格情報とクォータ履歴はそのまま維持され、一時停止されたアカウントは再開されるまで選択候補として現れません。

`disabled` との違い:

| | `disabled`(設定) | `paused`(ランタイム) |
| :-- | :-- | :-- |
| 設定方法 | `shunt.toml`、リロードが必要 | 管理ダッシュボードまたは `PATCH /admin/api/pool/{provider}/accounts/{account_ref}` |
| 再起動後も維持 | はい | いいえ |
| ユースケース | デプロイからアカウントを恒久的に除外する | 設定の往復なしに、しばらくの間アカウントを外しておく一時的な運用介入 |

ダッシュボードから: **Manage pool accounts → Managed pool health** を開き、該当アカウントの行の **Pause** をクリックします。状態が `paused` と表示され、**Resume** をクリックすると復帰します。どちらのボタンも write 権限の管理セッションが必要です。

API を直接呼び出す場合(write 権限の資格情報が必要):

`account_ref` は `GET /admin/api/pool` の各 account オブジェクトに含まれる不透明な識別子です。表示名 `name` ではなくこの値を使うため、同名の別アカウントも個別に操作できます。

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool/anthropic/accounts/$ACCOUNT_REF" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"paused": true}'
```

再開するには `"paused": false` を設定します。エンドポイントの完全なリファレンスは [`PATCH /admin/api/pool/{provider}/accounts/{account_ref}`](/ja/reference/endpoints/) を参照してください。

## リセットが最も早い順に並べ替える

`[server.pool] sort_by_reset`(デフォルト `false`)は *available* 階層の並べ替え方法を変更します。予測されるバーンレートのヘッドルームが最大のものではなく、既知のクォータリセットが最も早いアカウントから順に並べます(昇順 — 最も早く回復するアカウントが最初に試され、リセットの兆候がないアカウントは最後に並びます)。狙いは、最も早く補充されるアカウントから消費し、リセットが遅いアカウントは予備として残しておくことです。

`[server.pool]` は — したがってこの設定も — プロバイダー単位ではなくプロセス全体に適用されるため、切り替えるとプールされたすべてのプロバイダーに同時に影響します。

`shunt.toml` で設定する場合:

```toml
[server.pool]
sort_by_reset = true
```

または、プールテーブルの上にあるダッシュボードのチェックボックス(「Rank available accounts by soonest quota reset instead of burn-rate headroom」)でランタイムに切り替えるか、直接呼び出します:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": true}'
```

ランタイムの切り替えは、解除されるかプロセスが再起動されるまで設定ファイルの値を上書きし、その後は再び設定ファイル自身の値が適用されます。再起動せずに明示的にオーバーライドを解除するには `null` を送信します:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": null}'
```

フィールドを完全に省略すると何もしません — 現在のオーバーライド(またはその不在)をそのまま残します。明示的な `null` だけが解除します。

`GET /admin/api/pool` は有効な値(オーバーライドまたは設定値)をトップレベルの `sort_by_reset` ブール値として報告します。`[server.pool]` 自体が存在しない場合、この設定はまったく効果を持ちません — この設定が変更するはずのレガシー選択パス自体が実行されないためです — したがって `[server.pool]` が存在するまでは、ランタイムの切り替えも `GET /admin/api/pool` が報告する値も意味を持ちません。

## 週間リセットが最も早い順に並べ替える

`[server.pool] sort_by_weekly_reset`(デフォルト `false`)は、その並べ替えを週間ウィンドウに絞ります。全ウィンドウの中で最も早いリセットではなく、利用可能なアカウントを管理対象の週間リセット(`7d`、Fable モデルでは `7d_oi`)の昇順に並べます — 週間制限が最も早く補充されるアカウントが最初に試され、週間リセットの兆候がないアカウントは最後に並びます。狙いは、週間補充が最も近いアカウントから消費し、残りは予備として残すことです。両方のフラグが設定された場合、週間リセット順が優先されます。

`shunt.toml` で設定:

```toml
[server.pool]
sort_by_weekly_reset = true
```

または直接ランタイムに切り替え:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_weekly_reset": true}'
```

`sort_by_reset` と同様に、ランタイムの切り替えは解除されるかプロセスが再起動するまで設定ファイルの値を上書きします。明示的に解除するには `null` を送り、フィールドの省略は何もしません。ボディに `"provider": "<name>"` を加えると、上書きはそのプロバイダーだけに適用され、プロセス全体の値より優先されます。存在しないプロバイダー名は `404` を返し、何も変更しません。`GET /admin/api/pool` は有効な値をトップレベルの `sort_by_weekly_reset` ブール値(および各プロバイダーオブジェクトの値)として報告し、`[server.pool]` 自体が存在しない場合、この設定もまったく効果を持ちません。

## 端末でのランキング切り替え

`shunt tui` の Antigravity アカウントには `Quota remaining ↓` と、モデル系統と
報告された期間ごとの字下げした行が表示されます。各行は**残りの割合**と
リセットまでの時間を示します。例: `Gemini Models · weekly: 0% left · resets in 6d`。
TUI は Web ダッシュボードと同じく `GET /admin/api/pool` の `quota_buckets` を読みます。
5h の期間は上流サービスが報告した場合だけ表示します。残りの値がなければ `—`、
リセット時刻がないか無効なら残り時間を表示しません。クォータの行をクリックすると
そのアカウントを選択できます。Claude と Codex の既存の使用量バーとリセット表示は変わりません。

`shunt tui`(`--features tui` でビルド)でプロバイダーのヘッダーまたはアカウントを選び、`m` を押します。アカウントが 2 つ以上ある場合、**custom order → balanced → weekly reset → custom order** の順に切り替わります。

- **Custom order → balanced** は、そのプロバイダーの優先順位を `shunt.toml` から削除し、プール API に `{"provider": "<name>", "sort_by_weekly_reset": false}` を送ります。
- **Balanced → weekly reset** は、設定ファイルを編集せずに `{"provider": "<name>", "sort_by_weekly_reset": true}` を送ります。
- **Weekly reset → custom order** は、現在表示されている順序を、そのプロバイダーの優先順位 `1, 2, 3…` として保存します。そのプロバイダーの週間リセット設定は有効なままです。

週間リセットの切り替えは **選択したプロバイダーだけ** に影響します。カスタム優先順位が優先されるため、そのプロバイダーは引き続き `custom order` と表示されます。Balanced に戻すとそのプロバイダーの週間リセットを無効にし、`sort_by_reset` は変更しません。ランタイムの上書きは解除またはゲートウェイの再起動で終了しますが、カスタム優先順位は再起動後も残ります。TUI は `[server.pool]` を書き換えません。設定の編集が成功して API 呼び出しが失敗した場合は、部分的な変更を報告し、実際の状態を再取得します。

週間リセットには `[server.pool]` と `sort_by_weekly_reset` をサポートするゲートウェイが必要です。有効な値が false のままの場合、TUI は設定が無効であることを報告します。
