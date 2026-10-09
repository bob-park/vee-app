# 클립보드 보관 기간 · 카드 고정 · 앱 제외 · 서식 없이 복사 설계

- 날짜: 2026-10-09
- 브랜치: `feature/clipboard-retention`
- 디자인 기준: `docs/design/kraken-design.md`, 토큰은 `src/theme.css`
- 목업: `.superpowers/brainstorm/70551-1791512817/content/` (retention-layout, features, pin-layout)

## 목표

히스토리가 끝없이 쌓이지 않게 보관 기간을 정할 수 있게 하고, 그와 맞물려 "남길 것"과 "아예 남기지 않을 것"을 사용자가 고를 수 있게 한다.

성공 기준:
1. 설정에서 보관 기간을 제한 없음(기본) / 최근 7일 / 최근 30일 / 최근 90일 중 고를 수 있고, 기간이 지난 항목은 자동으로 지워진다.
2. 기간을 줄여 기존 항목이 지워질 때는 지워질 개수를 보여주고 확인을 받는다.
3. 카드를 고정하면 자동 정리·앱 제외 정리·전체 삭제 어디서도 지워지지 않는다.
4. 제외 목록에 넣은 앱에서 복사한 내용은 저장되지 않는다.
5. `⇧Enter`로 서식을 뺀 텍스트만 복사할 수 있다.

## 공통 원칙

- 새 의존성 없음.
- **고정 카드는 자동으로 지우지 않는다.** 보관 기간 정리, 앱 제외 정리, 전체 삭제 모두 같은 규칙을 따른다. 고정 카드는 사용자가 직접 삭제(`Delete`)할 때만 지워진다.
- 모든 처리는 로컬에서만 한다. 로그에는 정리된 개수만 남기고 내용은 남기지 않는다.

## 1. 데이터

파일: `src-tauri/src/store.rs`

### 스키마 v4

```sql
BEGIN;
ALTER TABLE clips ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE apps  ADD COLUMN excluded INTEGER NOT NULL DEFAULT 0;
PRAGMA user_version = 4;
COMMIT;
```

`init()`에 `if version < 4 { conn.execute_batch(SCHEMA_V4)?; }`를 추가한다. 정리 쿼리는 기존 `clips_last_used` 색인(`last_used_at DESC`)을 그대로 쓴다.

### 설정값

- 키 `retention`, 값 `off` | `7` | `30` | `90`, 기본 `off`.
- `settings::get` 기본값과 `settings::validate` 화이트리스트에 추가하고, `SettingsDto`에 `retention`을 넣는다.
- 기준 시각은 `last_used_at`(마지막으로 복사하거나 다시 쓴 시각)이다. 오래전에 만들었어도 최근에 다시 쓴 항목은 남는다.
- `settings::retention_cutoff(store, now) -> Option<i64>`: `off`면 `None`, 아니면 `now - days * 86_400_000`.

### Store 함수

| 함수 | 동작 |
|---|---|
| `prunable_count(cutoff) -> i64` | `last_used_at < cutoff AND pinned = 0`인 행 수 |
| `prune(cutoff) -> usize` | 위 조건의 행을 지우고 그 `image_path` 파일을 지운다. 지운 개수를 돌려준다 |
| `set_pinned(id, pinned)` | `pinned` 갱신 |
| `app_clip_count(app_id) -> i64` | 그 앱의 `pinned = 0` 행 수 (앱 제외 확인용) |
| `set_excluded(app_id, excluded) -> usize` | `excluded` 갱신. `true`면 그 앱의 `pinned = 0` 행과 이미지를 지우고 개수를 돌려준다 |
| `excluded_apps() -> Vec<AppDto>` | `excluded = 1`인 앱 (설정 태그 표시용) |
| `is_excluded(bundle_id) -> bool` | watcher가 저장 전에 확인 |
| `stats() -> StatsDto` | `{ count, imageBytes }`. `imageBytes`는 `images_dir` 파일 크기 합 |
| `clear()` (변경) | `pinned = 0` 행만 지우고, 그 행들의 `image_path` 파일만 지운다 (폴더 전체를 비우지 않는다) |

- `prune`, `set_excluded`, `clear`는 한 트랜잭션에서 지울 행의 `image_path`를 먼저 모은 뒤 행을 지우고, 커밋 후 파일을 지운다. 파일 삭제 실패는 기존 `delete()`처럼 무시한다.
- `list()`에 `pinned: bool` 필터 인자를 추가하고(`AND c.pinned = 1`), `ClipDto`에 `pinned`를 넣는다. 정렬은 기존 최근순 그대로다.
- `apps()`(`@` 자동완성)는 제외된 앱도 그대로 보여준다. 기존 기록을 찾는 용도라서다.

## 2. 백엔드 흐름

파일: `src-tauri/src/lib.rs`, `settings.rs`, `watcher.rs`, `windows.rs`

### 자동 정리

- `retention::prune_now(app)`: `retention_cutoff`가 있으면 `store.prune(cutoff)`. 1개 이상 지웠으면 `log::info!("pruned {n} clips")`와 `clips://changed`를 보낸다. 실패하면 `log::warn!`만 남긴다.
- 실행 시점: 앱 시작(`setup`), 1시간마다(`updater::spawn_periodic`과 같은 스레드 + `sleep` 방식, 디버그 빌드에서도 동작), `retention` 설정 변경 직후.
- 파일은 `settings.rs` 안에 함수로 둔다. 별도 모듈은 만들지 않는다.

### 명령

| 명령 | 비고 |
|---|---|
| `count_prunable(value) -> i64` | 아직 저장하지 않은 보관 기간 값으로 지워질 개수를 센다 (확인 박스용) |
| `set_setting("retention", v)` | 기존 명령. 저장 후 `key == "retention"`이면 정리하고, 정리 실패는 `Err`로 돌려준다 |
| `set_pinned(id, pinned)` | 후 `clips://changed` |
| `count_app_clips(app_id) -> i64` | 앱 제외 확인 박스용 |
| `set_app_excluded(app_id, excluded)` | 후 `clips://changed`, `settings://changed` |
| `list_excluded_apps() -> Vec<AppDto>` | |
| `get_stats() -> StatsDto` | |
| `copy_clip(id, plain)` | `plain` 인자 추가 |
| `list_clips(..., pinned)` | `pinned` 인자 추가 |

- 전체 삭제 확인 문구에 고정 개수가 필요하므로 `get_stats`가 `pinned`(고정 개수)도 돌려준다: `{ count, pinned, imageBytes }`.

### watcher

`Handler::capture`에서 `source_app::frontmost()`를 얻은 직후, `store.is_excluded(&front.bundle_id)?`가 참이면 저장·효과음·이벤트 없이 `Ok(())`로 끝낸다. 제외된 앱은 이미 `apps` 테이블에 있으므로 아이콘 조회도 하지 않는다.

### 서식 없이 복사

`windows::copy_clip(app, id, plain)`: `plain`이면 `store.formats(id)`를 읽지 않고 빈 목록을 `write_clipboard`에 넘긴다. 결과적으로 텍스트·링크는 `set_text`만 쓰고, 이미지·파일 카드는 일반 복사와 같다. Windows는 원래 `snapshot`이 빈 목록이라 결과가 같다.

## 3. 패널 UI

파일: `src/panel/Toolbar.tsx`, `Card.tsx`, `Panel.tsx`, `keys.ts`, `panel.css`, `src/api.ts`, `src/i18n/{ko,en}.ts`

### 필터 칩

- `FILTERS = ["all", "pinned", "text", "image", "files", "link"]`. 라벨은 `📌 고정` / `📌 Pinned`. `Tab` 순환에 포함한다.
- `pinned` 필터는 `kind` 없이 `pinned: true`로 `list_clips`를 부른다. 검색어·`@앱` 태그와 함께 쓸 수 있다.

### 고정 카드 표시

- 순서는 최근순 그대로.
- `.card.pinned`: 테두리 `color-mix(in srgb, var(--accent) 45%, transparent)`. 선택·삭제 확인 테두리가 우선한다.
- 오른쪽 위에 `.pin` 버튼(📌). 고정 카드는 항상 보이고, 아닌 카드는 hover 때만 흐리게(opacity 0.5) 보인다. 누르면 고정/해제하고, 클릭이 카드 선택·더블클릭 복사로 번지지 않게 `stopPropagation`한다. `aria-label`은 "고정" / "고정 해제".
- 고정 필터에서 고정을 풀면 다음 목록 갱신 때 빠진다.

### 키

`KeyInput`에 `metaOrCtrl: boolean`(macOS `metaKey`, Windows `ctrlKey`)을 추가한다.

| 키 | 동작 |
|---|---|
| `⌘P` / `Ctrl+P` | `{ type: "togglePin" }` — 선택한 카드 고정/해제. `key`는 대소문자 무시 |
| `⇧Enter` | `{ type: "copy", plain: true }` |
| `Enter` | `{ type: "copy", plain: false }` |
| `⇧` + 더블클릭 | 서식 없이 복사 |

- IME 조합 중, 삭제 확인 중에는 기존 규칙대로 무시된다. `@` 자동완성이 열려 있을 때 `⇧Enter`는 `suggestPick`과 같다.
- `⌘P`는 `preventDefault`해서 웹뷰 인쇄를 막는다.

### 토스트

`plain` 복사일 때 문구를 "서식 없이 복사됨" / "Copied as plain text"로 바꾼다. 나머지(효과음, 패널 닫힘)는 같다.

## 4. 설정 UI

파일: `src/settings/Settings.tsx`, `settings.css`, `src/i18n/{ko,en}.ts`

맨 아래 섹션을 "클립보드 기록" 섹션으로 바꾸고, `UpdateRow`는 위 섹션 끝으로 옮긴다. 섹션 위에 작은 제목(`11px`, 600, `--muted`)을 둔다.

```
클립보드 기록
┌──────────────────────────────────────────────┐
│ 보관 기간                                     │
│ 마지막으로 쓴 지 기간이 지난 항목은 자동으로 정리돼요 │
│ [제한 없음|최근 7일|최근 30일|최근 90일]        │
│  └ 확인 박스 (지울 항목이 있을 때만)             │
│────────────────────────────────────────────── │
│ 기록하지 않을 앱                               │
│ [1Password ×] [터미널 ×]          [앱 추가 ▾] │
│  └ 확인 박스 (그 앱의 기록이 있을 때만)          │
│────────────────────────────────────────────── │
│ 저장된 항목 3,912개 · 이미지 412MB   [전체 삭제] │
└──────────────────────────────────────────────┘
```

### 보관 기간

- 전체 너비 `Segmented`(`.seg.wide`, 버튼이 칸을 나눠 가짐).
- 선택하면 `count_prunable(value)`를 부른다. 0이면 바로 `set_setting`. 1 이상이면 그 칸에 빨간 점선(`.pending`)을 두고 아래에 확인 박스를 펼친다: "'최근 30일'로 바꾸면 오래된 항목 N개가 지금 바로 삭제돼요. 되돌릴 수 없어요." [취소] [삭제하고 적용].
- 취소하면 원래 값으로 돌아간다.

### 기록하지 않을 앱

- 제외된 앱을 태그(앱 아이콘 + 이름 + ×)로 보여준다. ×를 누르면 `set_app_excluded(id, false)`.
- "앱 추가" `select`: `list_apps("")` 결과 중 제외되지 않은 앱. 고르면 `count_app_clips(id)`가 0이면 바로 제외, 1 이상이면 확인 박스: "'{앱}'의 기록 N개도 삭제돼요. 고정한 카드는 남아요." [취소] [삭제하고 제외].
- 행 설명: "이 앱에서 복사한 내용은 저장하지 않아요".

### 확인 박스

- 보관 기간과 앱 제외가 같은 `ConfirmBox` 컴포넌트(메시지, 확인 라벨, onConfirm, onCancel)를 쓴다. 배경 `rgba(209,52,75,.06)`, 테두리 `rgba(209,52,75,.2)`, 버튼은 기존 `.btn` / `.btn.danger`.
- 한 번에 하나만 열린다. 다른 확인 박스를 열거나 다른 값을 고르면 이전 것은 취소된다.
- 개수는 "확인 박스를 연 순간" 기준이다. 그사이 복사가 일어나 실제 삭제 개수가 조금 달라도 맞추지 않는다. 삭제 기준은 늘 적용하는 순간이라 데이터가 잘못 지워지지 않는다.

### 저장 현황 + 전체 삭제

- `get_stats()`로 "저장된 항목 N개 · 이미지 412MB"를 보여준다. 설정 창이 열릴 때와 `clips://changed`를 받을 때 다시 부른다. 용량은 KB/MB/GB로 간단히 표시한다.
- 전체 삭제는 기존처럼 OS 확인창(`ask`). 고정 카드가 있으면 문구가 "고정한 카드 N개를 제외하고 모두 삭제할까요? 되돌릴 수 없어요."로 바뀐다.

## 5. 오류 처리

- 자동 정리(시작·주기) 실패: `log::warn!`만 남기고 다음 주기에 다시 시도한다.
- 설정 화면에서 일으킨 정리·제외 실패: 기존 `run()`이 화면 하단 `error` 줄에 보여준다. 설정값은 이미 저장돼 있어 다음 주기에 다시 정리된다.
- 잘못된 `retention` 값: `validate()`가 거부한다.
- 이미지 파일 삭제 실패: 무시한다. DB 행은 지워져 다시 나타나지 않는다.

## 6. 테스트

### Rust

- `prune`: 기준 시각 직전·직후 항목 중 직전 것만 지우고, 고정 카드는 남기고, 지운 이미지 파일이 사라진다.
- `prunable_count`가 `prune`이 돌려주는 개수와 같다.
- `clear`: 고정 카드와 그 이미지 파일이 남는다.
- `set_excluded(true)`: 그 앱의 고정 안 된 행만 지우고 다른 앱 행은 남긴다. `is_excluded`가 참이 된다. `false`로 되돌리면 거짓.
- `list(pinned = true)`가 고정 카드만 돌려준다.
- v3 스키마 DB를 열면 v4가 되고 기존 행이 `pinned = 0`으로 남는다.
- `retention` 기본값 `off`, `validate`가 `off/7/30/90`만 허용, `retention_cutoff` 계산.
- 기존 `history_is_not_trimmed`는 그대로 통과한다.

### TypeScript (`keys.test.ts`)

- `⌘P`/`Ctrl+P` → `togglePin`, 그냥 `p` → `null`(검색 입력).
- `Enter` → `copy` `plain: false`, `⇧Enter` → `copy` `plain: true`.
- IME 조합 중·삭제 확인 중에는 위 키가 기존 규칙대로 처리된다.

### 수동 확인 (실제 앱)

- 보관 기간을 줄였을 때 확인 박스 개수와 실제 삭제 결과, 패널 즉시 갱신.
- 앱 제외 후 그 앱에서 복사해도 카드가 생기지 않음. 제외 해제 후 다시 기록됨.
- Notion/Word 서식 텍스트를 `⇧Enter`로 복사해 붙였을 때 서식이 빠짐.
- 고정 칩, 배지, hover 📌, `⌘P`, 전체 삭제 후 고정 카드 유지.

## 범위 밖

- 기록 일시정지, 크게 보기(Space), 바로 붙여넣기 — 다음 버전에서 따로 다룬다.
- 고정 카드 순서 직접 바꾸기, 최대 개수 제한.
