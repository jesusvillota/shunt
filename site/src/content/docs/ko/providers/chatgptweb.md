---
title: "ChatGPT Web (텍스트 전용)"
description: "ChatGPT Web (텍스트 전용)"
---

Codex Responses 엔드포인트에서 `kind = "chatgptweb_browser"`를 사용하세요. 로그인한 브라우저 프로필 하나와 별도의 Node 도우미가 필요합니다. 도구와 이미지는 오류를 반환합니다. 이 단계에서는 계정을 순환하지 않습니다.

## 설정

설정한 브라우저 경로는 Codex 모델 선택기에 표시됩니다. `upstream_model`은 선택 사항입니다. 설정하면 `Medium` 또는 `High` 같은 화면의 추론 노력 레이블과 정확히 일치해야 합니다. 생략하면 노력을 확인하지 않습니다. 도우미는 선택된 노력을 변경하지 않습니다. `auth = "none"`과 `profile_dir`이 있는 계정 하나를 사용하세요. `SHUNT_CHATGPTWEB_HELPER`에 도우미의 절대 `cli.mjs` 경로를 지정하세요. 기존 `base_url` 필드는 필수지만 사용하지 않습니다. 매 요청에 전체 텍스트 기록을 보내세요. 전송 여부가 불확실한 요청은 재시도하지 않습니다.

[Configuration example and transport details (English)](https://github.com/jesusvillota/shunt/blob/main/docs/chatgptweb-provider.md)

도우미를 직접 실행할 때 `--chrome`은 `http://127.0.0.1:9333`에서 실행 중인 Chrome에 연결합니다. `--cdp-url`은 인증 정보가 없는 HTTP 루프백 주소만 허용합니다. 별도 프로필 `~/.local/share/shunt/chatgptweb-browser/chrome/cemfi`와 디버깅 포트를 사용하여 Chrome을 수동으로 시작하세요. 도우미는 브라우저 플래그를 추가하지 않으며 종료 시 Chrome을 닫지 않습니다. `--headless`는 도우미가 시작하는 Chromium에만 적용되며 `--chrome`과 함께 사용할 수 없습니다. Shunt는 이 플래그들을 전달하지 않습니다.
