---
title: "ChatGPT Web (텍스트 전용)"
description: "ChatGPT Web (텍스트 전용)"
---

Codex Responses 엔드포인트에서 `kind = "chatgptweb_browser"`를 사용하세요. 로그인한 브라우저 프로필 하나와 별도의 Node 도우미가 필요합니다. 도구와 이미지는 오류를 반환합니다. 이 단계에서는 계정을 순환하지 않습니다.

## 설정

설정한 브라우저 경로는 Codex 모델 선택기에 표시됩니다. `upstream_model`은 브라우저에서 선택한 모델 이름과 같아야 합니다. `auth = "none"`과 `profile_dir`이 있는 계정 하나를 사용하세요. `SHUNT_CHATGPTWEB_HELPER`에 도우미의 절대 `cli.mjs` 경로를 지정하세요. 기존 `base_url` 필드는 필수지만 사용하지 않습니다. 매 요청에 전체 텍스트 기록을 보내세요. 전송 여부가 불확실한 요청은 재시도하지 않습니다.

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)

도우미를 직접 실행할 때 `--chrome`으로 설치된 Google Chrome을 선택합니다. Playwright 기본 플래그를 유지합니다. 별도 프로필 `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi`를 사용하세요. 기본 실행은 창을 열며 `--headless`는 창을 숨깁니다. Shunt는 이 플래그들을 전달하지 않습니다.
