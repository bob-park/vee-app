# 고정 카드 왼쪽 영역 분리 설계

- 날짜: 2026-10-09
- 브랜치: `feature/pinned-sidebar`
- 디자인 기준: `docs/design/kraken-design.md`, 토큰은 `src/theme.css`
- 목업: `.superpowers/brainstorm/36559-1791526028/content/pinned-layout.html` (A안 선택)
- 버전: 0.6.2 → 0.6.3 (patch)

## 목표

자주 쓰는 클립을 검색·스크롤 없이 바로 꺼낼 수 있도록, 고정한 카드를 패널 왼쪽 별도 영역에 항상 보이게 한다.

성공 기준:
1. 고정한 카드는 패널 왼쪽에 일반 카드와 같은 크기로 나오고, 세로 구분선으로 히스토리와 나뉜다.
2. 고정은 최대 3개. 4번째 고정 시도는 막히고 패널 안에 알림이 뜬다.
3. 검색어·종류 필터·`@앱` 태그는 히스토리에만 적용되고 고정 영역은 항상 그대로다.
4. 고정 카드는 히스토리 줄에 중복으로 나오지 않는다.
5. 기존에 4개 이상 고정한 사용자는 업데이트 후 최근 사용한 3개만 고정으로 남는다(나머지는 고정만 풀리고 지워지지 않는다).

## 공통 원칙

- 새 의존성 없음.
- 고정 카드가 자동 정리·앱 제외 정리·전체 삭제에서 살아남는 기존 규칙은 그대로다.
- 한도 3은 백엔드 한 곳(`MAX_PINNED`)에서만 검사한다. 프론트는 거절 응답을 받아 알림만 띄운다.

## 1. 데이터

파일: `src-tauri/src/store.rs`

### 스키마 v5

```sql
BEGIN;
ALTER TABLE clips ADD COLUMN pinned_at INTEGER;
UPDATE clips SET pinned = 0
 WHERE pinned = 1
   AND id NOT IN (SELECT id FROM clips WHERE pinned = 1
                  ORDER BY last_used_at DESC, id DESC LIMIT 3);
UPDATE clips SET pinned_at = last_used_at WHERE pinned = 1;
PRAGMA user_version = 5;
COMMIT;
```

`init()`에 `if version < 5 { conn.execute_batch(SCHEMA_V5)?; }`를 추가한다.

`pinned_at`이 필요한 이유: 고정 영역을 `last_used_at`으로 정렬하면 고정 카드를 복사할 때마다(`touch`) 순서가 바뀐다. 고정 영역은 **고정한 순서**(먼저 고정한 카드가 왼쪽)로 고정한다.

### `set_pinned(id, pinned, now)`

- `pinned = true`이고 이 카드가 아직 고정이 아니며 이미 `MAX_PINNED`(=3)개가 고정돼 있으면 `PinLimit` 에러를 반환하고 아무것도 바꾸지 않는다.
- 이미 고정된 카드를 다시 고정하면 성공(no-op)이며 `pinned_at`은 바뀌지 않는다.
- 고정: `pinned = 1, pinned_at = now`. 해제: `pinned = 0, pinned_at = NULL`.

### `list(...)`

- `pinned_only` 인자를 없애고, 항상 `AND c.pinned = 0`을 붙인다. 정렬·페이지·검색은 그대로.

### `list_pinned()` (신규)

- 검색·종류·앱 조건 없이 `pinned = 1`인 카드를 `ORDER BY pinned_at ASC, id ASC LIMIT MAX_PINNED`로 반환한다. 반환 형식은 `list()`와 같은 `ClipDto`(files의 `stack` 포함). 행 매핑은 `list()`와 공유한다.

## 2. 커맨드

파일: `src-tauri/src/lib.rs`, `src/api.ts`

- `list_clips`: `"pinned"` 분기를 없앤다(`"all"` 또는 종류만 받음).
- `list_pinned` 신규: `store.list_pinned()`를 그대로 반환.
- `set_pinned`: 현재 시각을 넘긴다. `PinLimit`이면 에러 문자열 `"pin_limit"`을 반환한다. 성공 시 기존처럼 `clips://changed`를 보낸다.
- `api.ts`: `Filter`에서 `"pinned"` 제거, `listPinned: () => invoke<Clip[]>("list_pinned")` 추가.

## 3. 패널

파일: `src/panel/Panel.tsx`, `src/panel/Toolbar.tsx`, `src/panel/panel.css`, `src/i18n/{ko,en}.ts`

### 레이아웃

```
[툴바: 검색 · 전체/텍스트/이미지/파일/링크 · ⚙︎                        ]
[고정1][고정2][고정3] │ [히스토리 →→→ 가로 스크롤 ...                   ]
```

- 툴바 아래 본문을 `.body`(가로 flex)로 감싸고 그 안에 `.pinned-row`(flex: none, 스크롤 없음), `.pin-divider`(1px, `var(--border)`), 기존 `.row`(flex: 1, 가로 스크롤)를 둔다.
- 고정 카드는 기존 `Card` 컴포넌트를 그대로 쓴다(크기 동일, `.card.pinned` 테두리·📌 표시 유지).
- 고정 카드가 0개면 `.pinned-row`와 구분선을 렌더하지 않는다.
- 히스토리가 비었는데 고정 카드가 있으면 오른쪽 자리에 기존 `.empty` 문구를 보인다.

### 데이터 흐름

- `pinned: Clip[]` 상태를 추가한다. `reload()`는 `listClips`와 `listPinned`를 함께 부르고 같은 `requestId`로 낡은 응답을 버린다. 검색/필터가 바뀌어도 고정 목록은 다시 받아도 결과가 같으므로 별도 분기는 두지 않는다.
- `loadMore()`는 히스토리만 이어 받는다(오프셋 = `clips.length`, 변경 없음).

### 선택과 키보드

- 화면 순서대로 `items = [...pinned, ...clips]`를 하나의 목록으로 보고 `selected`는 이 목록의 인덱스다. `←/→`는 두 영역을 이어서 이동한다.
- 패널을 열 때·검색/필터가 바뀔 때 기본 선택은 히스토리 첫 카드(`pinned.length`)이고, 히스토리가 비었으면 `0`.
- 선택 카드 스크롤은 `rowRef.children[selected]` 대신 `data-id`로 찾은 카드에 `scrollIntoView`를 한다(고정 영역 카드는 이미 보이므로 영향 없음).
- 무한 스크롤 조건은 히스토리 기준(`selected - pinned.length >= clips.length - 5`)으로 바꾼다.
- `Enter`/`⇧Enter`, `⌘P`, `Delete`, 드래그, 삭제 확인은 `items[selected]`에 그대로 동작한다. `keys.ts`는 바꾸지 않는다.
- `Tab` 필터 순환은 `FILTERS = ["all", "text", "image", "files", "link"]`.

### 한도 알림

- `api.setPinned`가 `"pin_limit"`으로 거절되면 고정 영역 위에 작은 알림 "최대 3개까지 고정할 수 있어요" / "You can pin up to 3 cards"를 2초 띄운다(`.pin-notice`, `var(--card)` 배경, `var(--border)` 테두리, 12px radius, 기존 `.update-pop`와 같은 그림자). 다시 시도하면 타이머를 새로 시작한다.
- 토스트 창은 쓰지 않는다. 토스트는 "복사됨" 전용이고 패널이 내려갈 때 뜨는 구조라, 패널이 열린 상태의 알림은 패널 안이 맞다.
- 0개 고정 상태에서는 한도에 닿을 수 없으므로 알림은 항상 보이는 고정 영역에 붙는다.

### 애니메이션

- 고정/해제 시 카드가 두 영역 사이를 옮겨 간다. `cardLefts`/`playFlip`이 `.row` 하나가 아니라 `.body` 아래 모든 `.card[data-id]`를 측정·재생하도록 바꾼다(`querySelectorAll`). 새 카드 강조 링 동작은 그대로.
- reduced motion에서는 기존과 같이 생략.

### i18n

- `filters.pinned` 삭제.
- `pinLimit` 추가: ko "최대 3개까지 고정할 수 있어요", en "You can pin up to 3 cards".

## 4. 문서·버전

- README "카드 고정" 항목: "고정한 카드는 패널 왼쪽에 따로 모여 항상 보이며 최대 3개까지 고정할 수 있습니다"로 바꾸고 "📌 고정 필터" 언급과 "검색과 필터"의 "고정" 필터를 지운다.
- 버전 0.6.3: `src-tauri/tauri.conf.json`(기준), `package.json`, `src-tauri/Cargo.toml`(빌드로 `Cargo.lock` 갱신). 커밋 메시지 `chore: release 0.6.3`.

## 5. 테스트

`src-tauri/src/store.rs` 테스트 모듈에 추가:
1. `pin_limit_rejects_fourth` — 3개 고정 후 4번째는 `PinLimit`, 이미 고정된 카드 재고정은 성공.
2. `list_excludes_pinned` — 고정 카드는 `list()`에 없고 `list_pinned()`에만 있다.
3. `list_pinned_keeps_pin_order` — 고정 후 `touch`해도 순서는 고정한 순서.
4. `v5_migration_keeps_three_most_recent` — v4 DB에 5개 고정을 만든 뒤 열면 `last_used_at` 최근 3개만 고정, 나머지 5−3개는 남아 있되 고정 해제.

프론트: `yarn typecheck`, `yarn test` 통과. 수동 확인 — 고정 0/1/3개 레이아웃, 4번째 고정 알림, 검색 중 고정 영역 유지, `←/→` 영역 넘나들기, 고정 카드 드래그·복사·삭제, 라이트/다크.

## 범위 밖

- 고정 영역 안에서 순서 바꾸기(드래그 정렬).
- 한도를 설정으로 바꾸는 기능.
