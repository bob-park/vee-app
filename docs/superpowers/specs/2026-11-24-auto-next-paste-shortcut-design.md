# 자동 다음 붙여넣기 단축키 토글 — 설계

## 의도

자동 다음 붙여넣기를 설정 화면을 열지 않고 전역 단축키로 켜고 끈다. 켜지거나 꺼질 때 토스트로 알려준다. 일반 탭의 스위치는 없앤다.

## 목표 / 비목표 / 안티 목표

- 목표
  - 전역 단축키 하나로 `autoNextPaste`를 on ↔ off로 뒤집는다.
  - 기본 단축키는 `Cmd+Option+Shift+N`(Windows `Ctrl+Alt+Shift+N`), 설정의 "단축키" 탭에서 바꿀 수 있다.
  - 켜짐 / 꺼짐 / 권한 없음을 토스트로 알린다.
  - 단축키 탭의 해당 행에서 현재 켜짐/꺼짐을 볼 수 있다.
- 비목표
  - 트레이 메뉴 토글, 패널 안 상태 표시, 토글 소리.
  - 자동 넘김 동작 자체(`advance`, 키 감시)의 변경.
- 안티 목표
  - 권한이 없어 동작하지 않는데 "켜짐"이라고 알리는 것.
  - 켜고 끄는 방법이 단축키 등록 실패로 완전히 사라지는 것(행의 경고 + 다시 녹화로 복구 가능해야 한다).

## 제약

- 전역 단축키는 `src-tauri/src/app/shortcut.rs`의 `Global`로 관리한다. 등록, 중복 검사(`set_shortcut`), 실패 목록(`failures`), 단축키 탭의 `ShortcutRecorder`가 모두 `Global::ALL`과 설정 키를 따른다.
- 설정은 key-value이고 기본값은 `domain/settings/mod.rs`의 `default_for`에 있다. 마이그레이션은 필요 없다.
- macOS에서 키 감시는 입력 모니터링 권한이 필요하다(`auto_paste::platform::access()`가 `Some(false)`면 없음). Windows는 `None`.
- 토스트는 `window::show_toast` + `ToastPayload` 하나로 그린다.
- 사용자에게 보이는 문구는 `src/shared/i18n/{en,ko}.ts`에만 둔다.
- 의존성 변경 없음.

## 접근

1. **`Global`에 네 번째 단축키 추가 (선택)** — 기존 등록/중복 검사/실패 표시/레코더를 그대로 쓴다. 새 코드는 토글 핸들러와 토스트 형태뿐이다.
2. 키 감시(`auto_paste`의 tap)에서 토글 키도 함께 감지 — 꺼져 있을 때는 감시가 없어서 켤 수 없고, "꺼져 있으면 키를 보지 않는다"는 기존 안티 목표와 충돌한다. 제외.
3. 패널 전용 단축키 — 패널이 열려 있을 때만 동작하므로 "패널을 열지 않고 연달아 붙여넣기"라는 기능 목적과 맞지 않는다. 제외.

## 설계

### Rust

- `settings/model.rs`: `DEFAULT_AUTO_NEXT_PASTE_SHORTCUT = "CommandOrControl+Alt+Shift+KeyN"`, `SettingsDto.auto_next_paste_shortcut`.
- `settings/mod.rs`: `default_for("autoNextPasteShortcut")`. 값은 다른 전역 단축키처럼 `set_shortcut`으로만 저장한다(`validate`는 거부).
- `shortcut.rs`: `Global::AutoNextPaste`(키 `autoNextPasteShortcut`)를 `ALL`에 추가, `register`의 핸들러가 `auto_paste::toggle(app)`을 부른다.
- `auto_paste.rs`: `pub fn toggle(app)`
  - 현재 off이고 `platform::access() == Some(false)`: 값은 바꾸지 않고 `platform::request_access()`(첫 번째만 시스템 프롬프트) 후 "권한 필요" 토스트.
  - 그 외: 값을 뒤집어 저장 → `sync(app)` → `settings::commands::changed(app)` → 켜짐/꺼짐 토스트.
  - 판단부는 순수 함수(`next_state(on, access) -> 결과`)로 빼서 테스트한다.
- `window.rs`: `ToastPayload`에 `auto_next_paste: Option<AutoPasteNote>`(`on` / `off` / `needsAccess`, camelCase 직렬화)와 생성자 하나. `ok`는 `on`·`off`에서 true, `needsAccess`에서 false.

### 프론트

- `settings.dto.ts`: `GlobalShortcutKey`와 `Settings`에 `autoNextPasteShortcut` 추가. `clip.dto.ts`의 `ToastPayload`에 `autoNextPaste` 추가.
- `SettingsApp.tsx`
  - 일반 탭의 자동 다음 붙여넣기 `Row` + `Switch` 제거.
  - `SHORTCUT_KEYS`, `isGlobal`, `shortcutLabel`에 새 키 추가. 힌트는 "현재 켜짐 / 현재 꺼짐"을 포함한다(켜져 있는데 권한이 없으면 기존 `autoNextPasteAccessHint`).
- `PermissionsSection.tsx`: 변경 없음(계속 `settings.autoNextPaste`를 따른다).
- `ToastApp.tsx`: `p.autoNextPaste`가 있으면 제목 "자동 다음 붙여넣기", 요약 "켜짐" / "꺼짐" / "입력 모니터링 권한이 필요해요". 타일은 기존 것을 쓴다: 켜짐은 성공색 + 체크 배지, 꺼짐은 배지 없이 흐린 색, 권한 없음은 기존 실패 모양(`!`).
- i18n: 위 문구를 `en.ts` / `ko.ts`에 추가, 쓰지 않게 되는 문구는 제거.

### 문서

- `docs/agents/architecture.md`, `docs/agents/configuration.md`의 자동 다음 붙여넣기 설명을 단축키 토글 기준으로 고친다.

### 테스트

- Rust: `Global` 키 매핑 테스트에 새 키, `next_state` 단위 테스트, 설정 기본값 테스트.
- 완료 전 네 가지 검사(`cargo test --lib`, `yarn test`, `yarn typecheck`, `yarn lint`)와 `yarn tauri dev`로 라이트/다크 · ko/en 확인.

## 내가 정한 것 (확인 필요)

- 기본 단축키 `Cmd+Option+Shift+N`이 다른 앱/OS와 겹치지 않는지는 실행해서 확인한다.
- 켜짐/꺼짐 상태는 재시작 후에도 유지된다(지금과 같음).
- 새 화면이나 새 컴포넌트가 없어(기존 레코더 행, 기존 토스트 카드 재사용) 목업 3안 단계는 건너뛴다.
- 꺼짐 토스트의 타일 모양(배지 없는 흐린 아이콘).

## 구현자에게 남기는 것

- `AutoPasteNote`를 enum으로 둘지 문자열로 둘지, 토스트 타일의 아이콘 선택.
- `changed(app)`를 `app` 계층에서 부르는 위치(기존 `rust-modules` 규칙의 의존 방향을 따른다).
