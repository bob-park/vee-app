# 패널 카드 UI · 앱 태그 검색 · 애니메이션 설계

- 날짜: 2026-10-08
- 브랜치: `feature/modify-animations`
- 디자인 기준: `docs/design/kraken-design.md`, 토큰은 `src/theme.css`
- 목업: `.superpowers/brainstorm/96600-1791454627/content/` (card-icon, search-tag, toast-v2, clip-add, drag)

## 목표

패널과 토스트가 더 자연스럽게 보이고 움직이도록 다듬고, 앱 단위로 클립을 찾을 수 있게 한다.

성공 기준:
1. 카드 헤더의 앱 아이콘이 배경처럼 은은하게 보이고 본문 가독성을 해치지 않는다 (라이트/다크).
2. 검색창에 `@` + 앱 이름을 입력하면 자동완성이 뜨고, 선택하면 그 앱의 클립만 보인다.
3. 토스트가 팝 애니메이션으로 등장/퇴장하고, 그림자가 윈도우 경계에서 잘리지 않는다.
4. 패널이 열린 상태에서 다른 앱에서 복사하면 새 카드가 부드럽게 들어온다.
5. 파일/이미지 카드를 끌면 카드가 커서를 따라 움직이다 OS 드래그로 이어진다.

## 공통 원칙

- 새 의존성 없음. CSS 키프레임 + Web Animations API(`element.animate`)만 사용.
- 모든 애니메이션은 `prefers-reduced-motion: reduce`에서 비활성(즉시 최종 상태).
- 이징: 이동은 패널과 같은 `cubic-bezier(0.32, 0.72, 0, 1)`, 스프링/팝은 `cubic-bezier(0.34, 1.56, 0.64, 1)`.

## 1. 카드 헤더 — 아이콘 배경 (목업 D)

파일: `src/panel/Card.tsx`, `src/panel/panel.css`

- 헤더의 24px `.app-icon` / `.app-icon-empty`를 제거한다.
- `clip.appIcon`이 있으면 카드 최상위에 `<img class="card-bg" aria-hidden>`를 렌더링한다.
  - `position: absolute; top: -14px; left: -18px; width: 72px; height: 72px; pointer-events: none; z-index: 0`
  - `opacity: 0.16` (다크 테마 `0.22`)
  - `mask-image: linear-gradient(135deg, #000 30%, transparent 85%)` (`-webkit-` 접두어 포함)
- 헤더 왼쪽 패딩을 `46px`, `min-height: 40px`로 늘려 그 자리에 앱 이름(`.app-name`, 11px, 600, `--muted`, 말줄임)을 표시한다. 이름이 없으면 비운다.
- 헤더/본문/푸터는 `position: relative; z-index: 1`로 배경 위에 둔다.
- 선택/삭제확인 상태 테두리는 기존 그대로.

## 2. `@` 앱 태그 검색 (목업 A)

### 백엔드

파일: `src-tauri/src/store.rs`, `src-tauri/src/lib.rs`

- `Store::apps(query: &str, limit: i64) -> Result<Vec<AppDto>>`
  - `AppDto { id, name, icon: Option<String /* data URL */>, count }`
  - 클립이 1개 이상인 앱만, `name LIKE %query% ESCAPE '\'`(대소문자 무시), `count DESC, name ASC`, 최대 `limit`(8).
- 커맨드 `list_apps(query: String) -> Vec<AppDto>` 등록.
- `Store::list`와 `list_clips`에 `app_id: Option<i64>` 인자를 추가하고, 있으면 `AND c.app_id = ?`.

### 프론트

파일: `src/api.ts`, `src/panel/Panel.tsx`, `src/panel/Toolbar.tsx`, `src/panel/keys.ts`, `src/panel/panel.css`, `src/i18n/{en,ko}.ts`

- `api.listClips(query, kind, appId, offset, limit)`, `api.listApps(query)`, 타입 `App`.
- `Panel` 상태: `app: App | null`, `suggestions: App[]`, `suggestIndex: number`.
- 자동완성 열림 조건: `app === null`이고 `query`가 `@`로 시작. 검색어는 `query.slice(1)`.
  - 열려 있는 동안 `list_clips` 텍스트 검색은 하지 않는다(쿼리 `""`로 취급).
  - 결과가 없으면 "일치하는 앱 없음" 한 줄 표시.
- 선택 시: `app` 설정, `query`를 `""`로, 자동완성 닫힘, 선택 카드 0으로.
- 태그: 입력창 앞에 `<span class="app-tag">`(16px 아이콘 + 이름 + ×). × 클릭 시 해제. `.search`는 `<label>`에서 `<div>`로 바꾼다(label 안의 첫 버튼이 label 대상이 되어 클릭 시 태그가 지워지는 문제 방지).
- 팝오버: 입력창 아래 `position: absolute`, 너비 240px, `.update-pop`과 같은 카드 스타일. 항목은 20px 아이콘, 이름(일치 부분 `--accent` 굵게), 개수. 하단 힌트 "↑↓ 이동 · ↵ 선택 · esc 닫기".
- 키 처리(`panelKeyAction`에 `suggesting: boolean`, `hasTag: boolean` 입력 추가):
  - `suggesting`일 때: `ArrowDown/ArrowUp` → `suggestMove`, `Enter` → `suggestPick`, `Escape` → `suggestClose`(쿼리의 `@…` 제거). 나머지 키는 평소 규칙을 그대로 따른다(Tab 필터 순환 등). IME 조합 중엔 기존처럼 null.
  - `Backspace` + `queryEmpty` + `hasTag` → `clearTag` (카드 삭제보다 우선).
  - `Escape` + `hasTag`(자동완성 닫힘) → 기존대로 패널 숨김.
- `panel://closed`에서 `app`, 자동완성 상태도 초기화.
- 빈 결과 문구는 기존 `t.noResults` 재사용.

## 3. 토스트 — 팝 애니메이션 + 그림자 (목업 B)

### 그림자

- `windows.rs`: `TOAST_HEIGHT` 96 → 132. 너비 420 유지. 기존 위치 테스트(`panel_and_toast_sit_at_the_bottom_of_the_work_area`)의 기대값 갱신.
- `toast.css`: `.toast`의 `left/right: 36px`, `bottom: 36px` (카드 높이 약 68px → 위 여백 약 28px).
- 그림자를 짧은 3겹으로 교체, 테마 토큰 `--toast-shadow` 대신 직접 정의:
  - 라이트: `0 1px 2px rgba(16,24,40,.06), 0 4px 10px rgba(16,24,40,.08), 0 10px 24px rgba(16,24,40,.12)`
  - 다크: `0 1px 2px rgba(0,0,0,.3), 0 4px 10px rgba(0,0,0,.28), 0 10px 24px rgba(0,0,0,.35)`
  - 가장 큰 겹의 번짐(아래 34px, 좌우 24px, 위 14px)이 여백(아래 36, 좌우 36, 위 약 28) 안에 들어온다. 남는 `--toast-shadow` 토큰은 `theme.css`에서 제거.

### 애니메이션

- 등장 `toast-in`(220ms): `scale(.9), opacity 0` → `none, 1`, 이징 `cubic-bezier(.34,1.56,.64,1)`.
- 체크 배지 `toast-check-in`: 80ms 지연 후 `scale(0)` → `1.25` → `1` (약 300ms).
- 퇴장 `.toast.out`(180ms, ease-in): `scale(.96), opacity 0`, `animation-fill-mode: forwards`.
- 새 토스트가 오면 기존처럼 `key`가 바뀌어 처음부터 재생, `out` 해제.

### 숨김 타이밍

- `show_toast`의 타이머 스레드: `TOAST_MS`(1500) 대기 → generation 일치 시 `emit_to(TOAST, "toast://hide")` → `TOAST_EXIT_MS`(180) 대기 → generation 다시 확인 → `hide()`.
- 프론트 `Toast`는 `toast://hide`를 받으면 `out` 상태로 전환.

## 4. 클립 추가 애니메이션 (목업 C)

파일: `src/panel/Panel.tsx`, `src/panel/Card.tsx`(`data-id` 속성), `src/panel/motion.ts`

- `reload`에 `animate` 플래그를 두어, `clips://changed`에서 온 재로딩이고 패널이 `open`일 때만 애니메이션한다. 검색·필터·패널 열기/닫기·더 불러오기에는 적용하지 않는다.
- 커밋 직전 카드 요소들의 `left`를 `Map<id, number>`로 기록하고, `useLayoutEffect`에서:
  - 이전에도 있던 id: 위치 차이 `dx`만큼 `translateX(dx) → none` (360ms, 이동 이징). 맨 앞으로 올라온 기존 카드도 이 규칙으로 이동만 한다(글로우 없음).
  - 처음 보는 id: `scale(.86), opacity 0 → none, 1` (360ms) + 보라 링 글로우 1회(1.4s, `0 0 0 6px` → `0` 퍼짐 후 소멸). 클래스가 아니라 `element.animate`의 box-shadow로 준다(React가 className을 덮어써도 끊기지 않도록).
- 화면 밖(스크롤된 상태)의 카드는 그대로 애니메이션돼도 무방하다.
- 삭제로 인한 재로딩도 같은 경로라 남은 카드는 FLIP으로 메워진다(추가 비용 없음, 의도된 동작).

## 5. 드래그 — 커서를 따라가는 카드 (목업 B)

파일: `src/panel/Card.tsx`, `src/panel/Panel.tsx`, `src/panel/panel.css`

- 대상: 기존과 동일하게 `onDragOut`이 있는 카드(파일/이미지, missing 아님). 텍스트/링크 카드는 변화 없음.
- 마우스다운 후 5px 이상 이동하면 "플로팅" 시작:
  - 원래 자리는 같은 크기의 점선 슬롯(`2px dashed var(--accent-subtle)`, radius 12px)으로 남는다.
  - 카드 복제본(고스트)을 패널 위에 `position: fixed`로 렌더링하고 커서 오프셋을 유지하며 따라간다. `scale(1.05)`, 그림자 `0 14px 34px rgba(16,24,40,.25)`.
  - 기울기: `tilt = clamp(-10, 10, tilt * 0.7 + dx * 0.6)` (dx = 직전 이벤트 대비 x 이동), `rotate(tilt deg)`.
  - 플로팅 중 패널의 `onMouseDown` 포커스 유지 로직과 카드 선택은 기존대로.
- 커서가 패널 윈도우를 벗어나면(`document`의 `mouseleave` 또는 좌표가 `window.innerWidth/innerHeight` 밖) 고스트를 제거하고 기존 `dragOut(clip)` → `api.startDrag` 호출. 이후 흐름(패널 슬라이드 아웃, `panel://drag-cancelled`)은 현재와 동일.
- 패널 안에서 놓으면 고스트가 슬롯 위치로 스프링 이징 320ms 날아간 뒤 제거, 슬롯이 카드로 복귀.
- `prefers-reduced-motion`: 플로팅 없이 기존처럼 임계값에서 바로 `startDrag`.
- **위험:** 커서가 윈도우를 벗어난 뒤 시작하는 네이티브 드래그가 macOS에서 동작하는지(버튼을 누른 채 mousemove가 계속 들어오는지, `drag::start_drag`가 그 시점 이벤트로 세션을 시작할 수 있는지) 미검증. 구현 계획의 첫 작업으로 스파이크해 확인한다. 실패하면 대안: 고스트가 패널 상단 경계 20px 이내로 오면 그 시점(윈도우 안)에서 `startDrag`.
- 패널 윈도우 높이 ≈ 카드 높이이므로 고스트는 주로 좌우로 움직이며 윈도우 경계에서 잘린다. 허용.

## 테스트

- Rust (`cargo test`):
  - `Store::list`의 `app_id` 필터.
  - `Store::apps`: 이름 부분 일치, 대소문자 무시, 클립 없는 앱 제외, 개수순 정렬, limit.
  - 토스트 rect 기대값 갱신.
- 프론트 (`yarn test`):
  - `keys.test.ts`: `suggesting` 중 ↑↓/↵/esc, IME 조합 중 무시, `hasTag` + 빈 쿼리 Backspace → `clearTag`, 태그 없을 때 기존 삭제 동작 유지.
- 수동 확인 (`yarn tauri dev`): 라이트/다크 카드 배경, `@` 자동완성 흐름, 토스트 등장/퇴장·그림자, 패널 열고 외부 복사(신규/기존 클립), 파일·이미지 드래그(패널 안에서 놓기 / 밖으로 끌어 Finder에 드롭), reduced motion.

## 범위 밖

- 텍스트 카드 드래그 아웃.
- 드래그로 가로 스크롤(목업 C 안).
- 다중 앱 태그, 앱 태그와 종류 필터 외 다른 조건 조합 문법.
