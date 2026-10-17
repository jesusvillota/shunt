---
title: 풀 계정 제어
description: 개별 풀 계정을 일시 정지하고, 사용 가능한 계정을 가장 빨리 리셋되는 순서로 정렬하기 — 둘 다 관리자 대시보드에서 런타임에, 설정 변경이나 재시작 없이.
---

계정 풀 선택([Anthropic 멀티 계정](/ko/guides/anthropic-multi-account/), [Codex 멀티 계정](/ko/guides/codex-multi-account/)) 위에는 두 가지 런타임 제어가 있습니다: 단일 계정 일시 정지, 그리고 사용 가능한 계정을 번-레이트 헤드룸 대신 가장 빨리 쿼터가 리셋되는 순서로 정렬하는 것입니다. 두 기능 모두 관리자 대시보드의 "Managed pool health" 표에서, 또는 관리자 API를 직접 호출해서 조작합니다. 둘 다 메모리 전용입니다 — 재시작하면 초기화되므로 `shunt.toml`은 전혀 건드리지 않습니다.

## 계정 일시 정지

일시 정지는 설정 쪽의 `disabled = true`와 마찬가지로 계정을 선택 대상에서 제외하지만, `shunt.toml`을 편집하거나 계정을 로그아웃시키지 않습니다. 자격 증명과 쿼터 이력은 그대로 유지되며, 일시 정지된 계정은 재개될 때까지 단순히 선택 후보로 나타나지 않습니다.

`disabled`와는 다릅니다:

| | `disabled` (설정) | `paused` (런타임) |
| :-- | :-- | :-- |
| 설정 방법 | `shunt.toml`, 리로드 필요 | 관리자 대시보드 또는 `PATCH /admin/api/pool/{provider}/accounts/{account_ref}` |
| 재시작 후 유지 | 예 | 아니오 |
| 사용 사례 | 배포에서 계정을 영구적으로 제외 | 설정 왕복 없이 잠시 계정을 빼두는 임시 운영 개입 |

대시보드에서: **Manage pool accounts → Managed pool health**를 열고 해당 계정 행의 **Pause**를 클릭합니다. 상태가 `paused`로 표시되며, **Resume**을 클릭하면 다시 복귀합니다. 두 버튼 모두 write 등급 관리자 세션이 필요합니다.

API를 직접 호출하는 경우 (write 등급 자격 증명 필요):

`account_ref`는 `GET /admin/api/pool`의 각 account 객체가 반환하는 불투명 식별자입니다. 표시 이름 `name` 대신 이 값을 사용하므로 같은 이름의 서로 다른 계정도 개별적으로 제어할 수 있습니다.

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool/anthropic/accounts/$ACCOUNT_REF" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"paused": true}'
```

재개하려면 `"paused": false`로 설정하세요. 전체 엔드포인트 참조는 [`PATCH /admin/api/pool/{provider}/accounts/{account_ref}`](/ko/reference/endpoints/)를 참고하세요.

## 가장 빨리 리셋되는 순서로 정렬하기

`[server.pool] sort_by_reset`(기본값 `false`)은 *available* 계층의 정렬 방식을 바꿉니다: 가장 큰 예상 번-레이트 헤드룸 대신, 계정을 알려진 쿼터 리셋 시각이 가장 이른 순서(오름차순 — 가장 먼저 회복되는 계정을 먼저 시도하고, 리셋 신호가 없는 계정은 맨 뒤로 정렬)로 정렬합니다. 아이디어는 가장 먼저 보충될 계정을 먼저 소진시켜, 리셋이 늦은 계정은 예비로 남겨두는 것입니다.

`[server.pool]`은 — 따라서 이 설정도 — 프로바이더별이 아니라 프로세스 전체에 적용되므로, 이를 토글하면 풀링된 모든 프로바이더에 동시에 영향을 줍니다.

`shunt.toml`에서 설정:

```toml
[server.pool]
sort_by_reset = true
```

또는 풀 표 위의 대시보드 체크박스("Rank available accounts by soonest quota reset instead of burn-rate headroom")로 런타임에 토글하거나, 직접 호출:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": true}'
```

런타임 토글은 해제되거나 프로세스가 재시작될 때까지 설정 파일의 값을 덮어쓰며, 그 이후에는 다시 설정 파일의 값이 적용됩니다. 재시작 없이 명시적으로 오버라이드를 해제하려면 `null`을 보내세요:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_reset": null}'
```

필드를 아예 생략하면 아무 효과가 없습니다 — 현재 오버라이드(또는 그 부재)를 그대로 둡니다; 명시적인 `null`만이 해제합니다.

`GET /admin/api/pool`은 유효한 값(오버라이드 또는 설정값)을 최상위 `sort_by_reset` 불리언으로 보고합니다. `[server.pool]` 자체가 없으면 이 설정은 전혀 효과가 없습니다 — 이 설정이 바꾸려는 레거시 선택 경로 자체가 실행되지 않기 때문입니다 — 따라서 `[server.pool]`이 존재하기 전까지는 런타임 토글과 `GET /admin/api/pool`이 보고하는 값 모두 아무 의미가 없습니다.

## 주간 리셋이 가장 빠른 순서로 정렬하기

`[server.pool] sort_by_weekly_reset`(기본값 `false`)은 그 정렬을 주간 윈도우로 좁힙니다: 모든 윈도우 중 가장 이른 리셋이 아니라, 사용 가능한 계정을 관리 주간 리셋(`7d`, Fable 모델은 `7d_oi`) 기준 오름차순으로 정렬합니다 — 주간 한도가 가장 먼저 보충되는 계정을 먼저 시도하고, 주간 리셋 신호가 없는 계정은 맨 뒤로 정렬합니다. 주간 리필이 가장 임박한 계정부터 소진시키고 나머지는 남겨두려는 것입니다. 두 플래그가 모두 설정되면 주간 리셋 순서가 우선합니다.

`shunt.toml`에서 설정:

```toml
[server.pool]
sort_by_weekly_reset = true
```

또는 직접 런타임에 토글:

```bash
curl -X PATCH "$SHUNT_URL/admin/api/pool" \
  -H "x-shunt-admin-token: $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"sort_by_weekly_reset": true}'
```

`sort_by_reset`과 마찬가지로, 런타임 토글은 해제되거나 프로세스가 재시작될 때까지 설정 파일의 값을 덮어쓰며, 명시적으로 해제하려면 `null`을 보내고 필드를 생략하면 아무 효과가 없습니다. 바디에 `"provider": "<name>"`을 추가하면 해당 공급자에게만 덮어쓰기가 적용되며, 이는 프로세스 전체 값보다 우선합니다. 알 수 없는 공급자 이름은 `404`를 반환하며 아무것도 변경하지 않습니다. `GET /admin/api/pool`은 유효한 값을 최상위 `sort_by_weekly_reset` 불리언(및 각 공급자 객체의 값)으로 보고하며, `[server.pool]` 자체가 없으면 이 설정 역시 전혀 효과가 없습니다.

## 터미널 순위 모드 전환

`shunt tui`의 Antigravity 계정은 계정당 한 줄로 표시됩니다. 할당량 열은
`Gemini · weekly`와 `Claude + GPT · weekly`입니다. 충분한 너비에서는 다른
공급자의 5h 및 7d 열과 같은 위치에 표시됩니다. 막대와 백분율은 **사용한**
할당량(`used = 1 - remaining`)을 나타냅니다. 사용률 70%부터 노란색,
90%부터 빨간색입니다. 소진된 계열은 빨간색 전체 막대와 `100%`로 표시됩니다.
보고된 모든 계열의 기간이 소진되면 State 열에 빨간색 `exhausted`가 표시됩니다.
일시 중지, 비활성화 및 기타 차단 상태는 기존 표시를 유지합니다.

TUI는 `GET /admin/api/pool`의 `quota_buckets`를 읽습니다. 값이 없거나 알 수
없으면 흐린 `—`를 표시하며 `0%`로 표시하지 않습니다. 초기화 시간이 없거나
잘못되면 남은 시간을 표시하지 않습니다. 주간 할당량만 보고되면 주간 열만
표시합니다. 추가로 보고된 기간(5h 또는 세 번째 패밀리)은 이름이 포함된 셀 전체가
들어갈 때만 Plan 뒤에 정렬되어 표시되고, 아니면 생략됩니다. 초기화 남은 시간은
중간에 잘리지 않고 가장 큰 단위(`2d`)로 줄거나 사라집니다. 너비가 줄면 제목을
`7d`로 줄이고, 더 좁아지면 할당량 열 하나만 표시하며 보고된 패밀리 중 하나라도
소진되면 State 열이 `exhausted`가 됩니다. 들어가지 않는 제목의 뒤쪽 열 이름은
생략됩니다. 매우 작은 터미널에서는 계정 이름도 줄입니다. 행은 줄바꿈되지
않습니다. 계정 행을 클릭하면 계정을 선택하며, 열 제목은 계정을 선택하지
않습니다. Claude와 Codex의 기존 사용량 막대와 초기화 표시는 유지됩니다.

`shunt tui`(`--features tui`로 빌드)에서 공급자 헤더나 계정을 선택하고 `m`을 누릅니다. 계정이 두 개 이상이면 **custom order → balanced → weekly reset → custom order** 순서로 전환됩니다.

- **Custom order → balanced**는 해당 공급자의 우선순위를 `shunt.toml`에서 제거하고 풀 API에 `{"provider": "<name>", "sort_by_weekly_reset": false}`를 보냅니다.
- **Balanced → weekly reset**는 설정 파일을 편집하지 않고 `{"provider": "<name>", "sort_by_weekly_reset": true}`를 보냅니다.
- **Weekly reset → custom order**는 현재 표시된 순서를 해당 공급자의 우선순위 `1, 2, 3…`으로 저장합니다. 해당 공급자의 주간 리셋 설정은 계속 켜져 있습니다.

주간 리셋 스위치는 **선택한 공급자에게만** 영향을 줍니다. 사용자 지정 우선순위가 먼저 적용되므로 해당 공급자는 계속 `custom order`로 표시됩니다. Balanced로 돌아가면 해당 공급자의 주간 리셋을 끄며 `sort_by_reset`은 변경하지 않습니다. 런타임 덮어쓰기는 해제하거나 게이트웨이를 재시작할 때 끝나지만 사용자 지정 우선순위는 유지됩니다. TUI는 `[server.pool]`을 쓰지 않습니다. 설정 편집이 성공하고 API 호출이 실패하면 부분 변경을 알리고 실제 상태를 새로 조회합니다.

주간 리셋에는 `[server.pool]`과 `sort_by_weekly_reset`을 지원하는 게이트웨이가 필요합니다. 유효한 값이 계속 false이면 TUI는 설정이 비활성 상태라고 알립니다.

## `goal`과 `/delegate-cli`가 사용할 모델 고르기

`shunt tui`에서 `M`을 누르면 Models 대화상자가 열립니다. `codex`, `opencode`, `agy` 탭이 각각 있고, 해당 CLI가 지금 제공하는 모델을 나열합니다. `Space`는 선택한 모델을 켜거나 끄고, `Tab`은 플랫폼을 바꾸며, `f`는 필터를 켜거나 끕니다.

필터가 켜져 있는 동안 `goal` 도구와 `/delegate-cli`는 켜진 모델만 보며, `goal`은 꺼진 모델을 지정한 경로를 거부합니다. 기본값으로 각 모델 계열의 최신 버전이 켜집니다. 예를 들어 `gpt-6.1-sol`은 켜지고 `gpt-6-sol`은 꺼집니다. 선택은 `~/.shunt/models.json`에 저장되며 게이트웨이 라우팅, `/v1/models`, Claude Code의 `/model` 선택기에는 영향을 주지 않습니다. 셸에서는 `shunt models`로 같은 목록을 볼 수 있습니다. `opencode` 탭에서 `r`을 누르면 헤드리스 에이전트로 무료 모델 순위를 다시 매기며, 검증된 순서와 `ranked_at` 시간이 같은 파일에 저장되고 `shunt models opencode`가 `rank` 번호를 출력합니다.
