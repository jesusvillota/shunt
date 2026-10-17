---
title: 账户池控制
description: 暂停单个账户池账户,并按配额最快重置的顺序对可用账户排序 —— 两者都可在管理仪表盘中于运行时完成,无需改配置或重启。
---

在账户池选择([Anthropic 多账户](/zh-cn/guides/anthropic-multi-account/)、[Codex 多账户](/zh-cn/guides/codex-multi-account/))之上,有两个运行时控制:暂停单个账户,以及让可用账户按配额最快重置的顺序排序,而不是按燃烧速率余量排序。两者都通过管理仪表盘的 "Managed pool health" 表格操作,或直接调用管理 API。两者都只存在于内存中 —— 重启即清空,因此都不会改动 `shunt.toml`。

## 暂停一个账户

暂停会像配置侧的 `disabled = true` 一样把账户排除在选择之外,但不会编辑 `shunt.toml`,也不会把账户登出。凭证和配额历史都原样保留;被暂停的账户在恢复之前不会出现在候选列表中。

它与 `disabled` 不同:

| | `disabled`(配置) | `paused`(运行时) |
| :-- | :-- | :-- |
| 设置方式 | `shunt.toml`,需要重载 | 管理仪表盘或 `PATCH /admin/api/pool/{provider}/accounts/{account_ref}` |
| 重启后是否保留 | 是 | 否 |
| 使用场景 | 从部署中永久移除一个账户 | 无需往返配置文件,临时把某个账户挪开一会儿 |

在仪表盘中:打开 **Manage pool accounts → Managed pool health**,点击该账户行的 **Pause**。其状态会显示为 `paused`;点击 **Resume** 即可恢复。这两个按钮都需要 write 级别的管理会话。

直接调用 API(需要 write 级别凭证):

`account_ref` 是 `GET /admin/api/pool` 每个 account 对象返回的不透明标识符。使用它而不是显示名称 `name`，可以分别控制同名但不同的账户。

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool/anthropic/accounts/$ACCOUNT_REF" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"paused": true}'
```

设置 `"paused": false` 即可恢复。完整端点参考见 [`PATCH /admin/api/pool/{provider}/accounts/{account_ref}`](/zh-cn/reference/endpoints/)。

## 按最快重置排序

`[server.pool] sort_by_reset`(默认 `false`)会改变 *available* 层的排序方式:不再按预计的燃烧速率余量最大排序,而是按已知的最早配额重置时间升序排序(最先恢复的账户被最先尝试;没有重置信号的账户排在最后)。这样做的思路是先耗尽最快恢复的账户,把重置更晚的账户留作缓冲。

`[server.pool]` —— 以及这个设置 —— 是进程级的,不是按 provider 划分的,因此切换它会同时影响池中的所有 provider。

在 `shunt.toml` 中设置:

```toml
[server.pool]
sort_by_reset = true
```

或者通过账户池表格上方的仪表盘复选框("Rank available accounts by soonest quota reset instead of burn-rate headroom")在运行时切换,或直接调用:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": true}'
```

运行时切换会覆盖配置文件中的值,直到被清除或进程重启,此后又会恢复应用配置文件自身的值。要在不重启的情况下显式清除覆盖值,发送 `null`:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": null}'
```

完全省略该字段不会有任何效果 —— 会保留当前的覆盖值(或其缺失状态)不变;只有显式的 `null` 才会清除它。

`GET /admin/api/pool` 会以顶层的 `sort_by_reset` 布尔值报告当前生效的值(覆盖值或配置值)。若 `[server.pool]` 本身不存在,这个设置完全不起作用 —— 它原本要改变的旧版选择路径根本不会运行 —— 因此在 `[server.pool]` 存在之前,运行时切换和 `GET /admin/api/pool` 报告的值都没有意义。

## 按周重置最早排序

`[server.pool] sort_by_weekly_reset`(默认 `false`)把上述排序收窄到按周窗口:不再按所有窗口中最早的重置时间,而是按管理性周重置(`7d`,Fable 模型为 `7d_oi`)升序对可用账户排序 —— 周配额最先恢复的账户被最先尝试;没有周重置信号的账户排在最后。这样做的思路是先耗尽离周配额回满最近的账户,把其余账户留作缓冲。两个标志同时设置时,按周重置的顺序优先。

在 `shunt.toml` 中设置:

```toml
[server.pool]
sort_by_weekly_reset = true
```

或直接在运行时切换:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_weekly_reset": true}'
```

与 `sort_by_reset` 一样,运行时切换会覆盖配置文件的值,直到被清除或进程重启;显式清除请发送 `null`,省略该字段则为空操作。在请求体中加入 `"provider": "<name>"` 可让覆盖值只作用于该提供商,且优先于进程级的值。未知的 provider 名称会返回 `404`,且不做任何更改。`GET /admin/api/pool` 会以顶层的 `sort_by_weekly_reset` 布尔值(以及每个提供商对象上的值)报告生效值;若 `[server.pool]` 本身不存在,该设置同样完全不起作用。

## 终端排序模式切换

`shunt tui` 中的 Antigravity 账户每个占一行。配额列为 `Gemini · weekly`
和 `Claude + GPT · weekly`。宽度充足时，这两列与其他提供商的 5h 和 7d
列位于相同位置。使用量条和百分比显示**已使用**的配额
（`used = 1 - remaining`）。使用率达到 70% 时变为黄色，达到 90% 时变为红色。
某个系列耗尽时，显示红色满条和 `100%`。所有已报告系列的时间窗口都耗尽时，
State 列显示红色 `exhausted`。暂停、禁用和其他阻止使用的状态保留原有文字。

TUI 读取 `GET /admin/api/pool` 的 `quota_buckets`。缺失或未知的读数显示暗色
`—`，不会显示成 `0%`。重置时间缺失或无效时不显示倒计时。只报告每周配额时，
只显示每周窗口。上游额外报告的窗口（5h 或第三个系列），仅在带名称的单元格能完整显示时
才对齐出现在 Plan 之后，否则省略。重置倒计时不会被截断，而是缩短为最大单位
（`2d`）或消失。宽度变小时，标题缩短为 `7d`；更窄时只显示一个配额列，
且任一已报告系列耗尽时 State 列显示 `exhausted`。放不下的标题尾部列名会省略。非常小的终端也会缩短
账户名。行不会换行。点击账户行可选择账户，点击列标题不会选择账户。
Claude 和 Codex 的现有使用量条和重置时间显示保持不变。

在 `shunt tui`（使用 `--features tui` 构建）中，选择提供商标题或账户，然后按 `m`。提供商至少有两个账户时，按 **custom order → balanced → weekly reset → custom order** 顺序切换。

- **Custom order → balanced** 从 `shunt.toml` 删除该提供商的优先级，并向账户池 API 发送 `{"provider": "<name>", "sort_by_weekly_reset": false}`。
- **Balanced → weekly reset** 发送 `{"provider": "<name>", "sort_by_weekly_reset": true}`，不编辑配置文件。
- **Weekly reset → custom order** 将当前显示顺序保存为该提供商的优先级 `1, 2, 3…`。该提供商的按周重置设置保持开启。

按周重置开关只影响 **所选的提供商**。自定义优先级优先，因此该提供商仍显示 `custom order`。返回 balanced 会关闭该提供商的按周重置，但不会修改 `sort_by_reset`。运行时覆盖在清除或网关重启时结束；自定义优先级在重启后保留。TUI 不会写入 `[server.pool]`。如果配置编辑成功而 API 调用失败，TUI 会报告部分修改并重新获取实际状态。

按周重置需要 `[server.pool]` 和支持 `sort_by_weekly_reset` 的网关。如果实际生效值仍为 false，TUI 会提示该设置未生效。

## 选择 `goal` 和 `/delegate-cli` 使用的模型

在 `shunt tui` 中按 `M` 打开 Models 对话框。它为 `codex`、`opencode` 和 `agy` 各设一个标签页，列出这些 CLI 当前提供的模型。`Space` 开关选中的模型，`Tab` 切换平台，`f` 开关过滤器。

过滤器开启时，`goal` 工具和 `/delegate-cli` 只能看到已开启的模型，`goal` 会拒绝指定了已关闭模型的路由。默认情况下，每个模型系列的最新版本为开启：`gpt-6.1-sol` 开启，`gpt-6-sol` 关闭。你的选择保存在 `~/.shunt/models.json`，不会改变网关路由、`/v1/models` 或 Claude Code 的 `/model` 选择器。在 shell 中可用 `shunt models` 输出同一份列表。在 `opencode` 选项卡按 `r` 会用无头 agent 重新排列免费模型，验证后的顺序和 `ranked_at` 时间保存在同一文件中，且 `shunt models opencode` 会输出 `rank` 编号。

在 Models 对话框按 `s`，依次选择排名 agent 的平台、模型和推理强度。用方向键选择，Enter 进入下一步，最后一步按 Enter 保存，Esc 取消。`opencode` 选项卡显示 `platform · model · effort`（`default` 表示 CLI 默认值）。模型来自现有目录；Codex 显示各模型支持的推理强度。Claude 提供最新的 `sonnet`、`opus` 和 `haiku` 别名。选择以 `"ranker": {"platform": "codex", "model": "gpt-6-sol", "effort": "high"}` 保存到 `~/.shunt/models.json`。model 和 effort 均可省略。旧的 `"ranker": "claude"` 继续使用原有命令；更改排名 agent 前，字符串格式保持不变。Claude 和 AGY 传入 `--model`、`--effort`；Codex 传入 `-m` 和 `-c 'model_reasoning_effort="high"'`。OpenCode 传入 `-m`，但已安装的 CLI 没有强度参数，因此忽略 effort。AGY 未指定模型时仍通过 `claude -p` 使用原有 Antigravity 默认模型。`chatgptweb` 仍被拒绝，因为 shunt 无法启动其浏览器会话。
