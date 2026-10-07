# 패널 개선 · 파일 드래그 앤 드롭 설계

날짜: 2026-10-07 · 브랜치: master (현재 브랜치에서 작업)

## 범위

1. 패널/카드 크기를 모니터 높이에 비례
2. macOS 패널 등장 시 번쩍임 수정
3. 파일 카드를 드래그 앤 드롭으로 복사
4. 이미지 파일 클립 썸네일 미리보기
5. 최대 저장 개수(1000) 제한 제거
6. 트레이 아이콘 왼쪽 클릭 시 메뉴 표시
7. 개별 삭제 확인 설정 토글

범위 밖: 스크린샷(이미지) 클립 드래그, HEIC 썸네일, 기존 파일 클립 썸네일 소급 생성.

## 1. 비례 크기

- `windows.rs`: 패널 높이 = `clamp(work.h_pt × 0.30, 300, 480)` (pt). Windows는 같은 규칙을 pt로 계산한 뒤 `unit`(scale)을 곱해 px로 변환. `PANEL_HEIGHT` 상수를 `PANEL_RATIO`, `PANEL_MIN`, `PANEL_MAX`로 교체.
- `panel.css`: `.card { flex: none; height: 100%; aspect-ratio: 0.95; }` — 너비가 행 높이를 따라감.
- 글자: `.preview` 폰트 `clamp(12px, 100vh / 26, 15px)`, 줄 수 clamp는 고정값 대신 카드 높이에 맞게 `-webkit-line-clamp`를 넉넉히(14) 두고 `overflow:hidden`으로 자름. 패널 창 높이 = 패널 높이이므로 `vh` 기준이 정확함.
- 기준값(30%): MacBook Air 13" 300 · Pro 16" 335 · 27" QHD 432 · 32" 4K 스케일 480.

## 2. 등장 번쩍임 (macOS)

원인: 숨겨진 WKWebView는 페인트하지 않아, `show()` 순간 마지막으로 그려진 "열린 위치" 프레임이 보였다가 `panel://opened` 이후 내려가서 다시 슬라이드함.

- `place_and_show`: macOS에서 `show()` 전에 `NSWindow.setAlphaValue(0)`.
- 프론트 `panel://opened`: (패널은 이미 `open=false`로 대기 중) `requestAnimationFrame` 2회 → `api.revealPanel()` → `setOpen(true)`.
- 새 커맨드 `reveal_panel`: macOS는 alpha 1, 그 외 no-op.

## 3. 파일 드래그 앤 드롭 (복사)

- 의존성: `drag = "2.1"` (Rust만, JS 패키지 없음). `DragMode::Copy` → `NSDragOperationCopy`로 같은 볼륨에서도 항상 복사.
- 대상: `kind == "files"`이고 `!missing`인 카드만.
- 프론트(`Card`/`Panel`): 카드 mousedown 위치 기록 → 5px 이상 이동 시 `api.startDrag(id)` 호출 + `dragging` 상태로 패널을 아래로 슬라이드(`.panel.dragging { transform: translateY(calc(100% + 16px)); transition 200ms }`).
- Rust `start_drag(id)` 커맨드:
  - 파일 존재 확인(없으면 Err, 프론트는 패널 복귀).
  - `AppState.dragging = true` → `Focused(false)` 핸들러는 dragging 중 hide 건너뜀.
  - 패널 `set_ignore_cursor_events(true)`.
  - 메인 스레드에서 `drag::start_drag(panel, DragItem::Files(paths), Image::Raw(stack 첫 썸네일 또는 앱 아이콘 PNG), callback, Options { mode: Copy, .. })`.
  - callback `Dropped`: dragging=false, ignore_cursor_events(false), `touch` + `clips://changed`, 복사음(설정 따름), 토스트(`ToastPayload::copied`), `hide_panel(app, true)`.
  - callback `Cancel`: dragging=false, ignore_cursor_events(false), `panel://drag-cancelled` emit → 프론트가 `dragging=false`로 패널 복귀.
- `copy_clip`의 "복사 후 처리(touch/사운드/토스트)"를 함수로 추출해 드롭과 공유.

## 4. 이미지 파일 썸네일

- 이미지 확장자: png, jpg, jpeg, gif, webp, bmp, tif, tiff (대소문자 무시).
- 표시 규칙:
  - **파일 1개(이미지)**: 썸네일이 본문 전체 + 좌하단 확장자 태그(`JPG`).
  - **파일 여러 개 + 그중 이미지 1개 이상**: 앞 최대 3개 파일을 살짝 기울여 겹친 "사진 더미". 이미지 파일 레이어는 썸네일, 이미지가 아닌 파일 레이어는 문서 타일. 맨 앞(첫 파일)이 위. 태그 없음, 개수는 기존 배지("파일 4개")와 푸터.
  - **이미지가 하나도 없음**: 기존 문서/폴더 아이콘 그대로.
- `watcher.rs`: Files 클립의 앞 3개 경로 중 이미지(50MB 이하)를 각각 `RustImageData::from_path` → `thumbnail(THUMB_SIZE)` → PNG. 실패한 파일은 썸네일 없음(문서 타일).
- `store.rs`:
  - `NewClip::Files(Vec<String>)` → `NewClip::Files { paths, thumbs: Vec<(usize, Vec<u8>)> }` (파일 위치 idx, PNG).
  - 스키마 V2 마이그레이션(`user_version` 2): `CREATE TABLE clip_thumbs (clip_id INTEGER NOT NULL, idx INTEGER NOT NULL, png BLOB NOT NULL, PRIMARY KEY (clip_id, idx))` + `AFTER DELETE ON clips` 트리거로 정리(delete/clear 모두 커버).
  - `ClipDto`에 `stack: Vec<Option<String>>` 추가 — files 클립의 앞 min(3, n)개 레이어, 이미지면 data URL, 아니면 null. 이미지가 하나도 없으면 빈 배열.
  - 드래그 이미지는 `stack`의 첫 썸네일 사용.
- `Card.tsx`: files 클립에서 `stack`이 비어 있지 않으면 — 1개면 `<img class="thumb">` + 태그, 여러 개면 `.stack` 레이어(각각 `<img>` 또는 문서 타일, 회전 −7° / +5° / 0°). 기존 파일 클립(마이그레이션 전)은 `stack`이 비어 기존 아이콘.

## 5. 저장 제한 제거

- `MAX_CLIPS`, `Store::trim()`, `upsert`의 trim 호출 삭제. 관련 테스트 `trims_oldest_beyond_max_and_removes_image_file`는 삭제 시 이미지 파일 제거만 검증하도록 축소(이미 `delete_and_clear_remove_image_files`가 커버하면 삭제).
- 성능: 목록은 50개 페이지 + `last_used_at` 인덱스, 검색은 FTS trigram → 행 수 증가 영향 작음. 디스크는 설정의 "기록 지우기"로 사용자가 관리.

## 6. 트레이

- `tray.rs`: `.show_menu_on_left_click(true)`, `DoubleClick` 핸들러 제거(패널은 "열기" 메뉴/단축키로).

## 7. 개별 삭제 확인 토글

- 설정 키 `confirmDelete` = `"on" | "off"`, 기본 `"off"`(현재 동작 유지). `settings.rs`의 `get` 기본값·`validate`·`SettingsDto`에 추가, `api.ts` `Settings`/`setSetting` 키 타입에 추가.
- 설정 화면: "복사 사운드" 아래 토글 "개별 삭제 시 확인" + 보조 문구 "패널에서 항목을 지울 때 한 번 더 확인합니다". ko/en 문자열 추가.
- 패널: 토글이 켜져 있으면 `delete` 액션 시 즉시 삭제 대신 `confirmingId = 선택 카드 id`. 해당 카드에 오버레이(빨간 테두리, "이 항목을 삭제할까요?", [취소] [삭제], "⏎ 삭제 · esc 취소").
  - 확인 중 키: Enter → 삭제, Escape → 취소(패널은 닫지 않음), 그 외 키·선택 이동·필터 변경·검색 입력 → 취소.
  - 버튼 클릭도 동작. 패널이 닫히면(`panel://closed`) 취소.
  - 네이티브 대화상자는 포커스를 뺏어 패널이 닫히므로 사용하지 않음.
- `keys.ts`: `panelKeyAction`에 `confirming: boolean` 입력 추가 → 확인 중엔 Enter=`confirmDelete`, Escape=`cancelDelete`, 나머지=`cancelDelete` 후 원래 동작 없음(단순화).

## 테스트

- Rust: 패널 높이 clamp(작은/중간/큰 work area, Windows px 변환), 파일 클립 stack 저장·조회(이미지/비이미지 혼합, 삭제 시 clip_thumbs 정리, V1→V2 마이그레이션), 1000개 초과 삽입 후 trim 없음.
- Node: 확장자 태그 함수(`imageTag(path)` → `"PNG"`, 이미지 아니면 null). `keys.test.ts`에 확인 모드 케이스(Enter/Escape/기타 키).
- Rust: `confirmDelete` 기본값 off, 잘못된 값 거부.
- 수동(`yarn tauri dev`, macOS): 등장 번쩍임 없음, 4종 모니터 크기, Finder로 드래그 시 복사(원본 유지) + 토스트, 드래그 취소 시 패널 복귀, 이미지 파일 카드 썸네일, 트레이 왼쪽 클릭 메뉴.
