# 패널 개선 · 파일 드래그 앤 드롭 설계

날짜: 2026-10-07 · 브랜치: master (현재 브랜치에서 작업)

## 범위

1. 패널/카드 크기를 모니터 높이에 비례
2. macOS 패널 등장 시 번쩍임 수정
3. 파일 카드를 드래그 앤 드롭으로 복사
4. 이미지 파일 클립 썸네일 미리보기
5. 최대 저장 개수(1000) 제한 제거
6. 트레이 아이콘 왼쪽 클릭 시 메뉴 표시

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
  - 메인 스레드에서 `drag::start_drag(panel, DragItem::Files(paths), Image::Raw(thumb 또는 앱 아이콘 PNG), callback, Options { mode: Copy, .. })`.
  - callback `Dropped`: dragging=false, ignore_cursor_events(false), `touch` + `clips://changed`, 복사음(설정 따름), 토스트(`ToastPayload::copied`), `hide_panel(app, true)`.
  - callback `Cancel`: dragging=false, ignore_cursor_events(false), `panel://drag-cancelled` emit → 프론트가 `dragging=false`로 패널 복귀.
- `copy_clip`의 "복사 후 처리(touch/사운드/토스트)"를 함수로 추출해 드롭과 공유.

## 4. 이미지 파일 썸네일

- 이미지 확장자: png, jpg, jpeg, gif, webp, bmp, tif, tiff (대소문자 무시).
- `watcher.rs`: Files 클립에서 첫 이미지 파일(50MB 이하)을 `RustImageData::from_path` → `thumbnail(THUMB_SIZE)` → PNG. 실패 시 썸네일 없이 저장.
- `store.rs`: `NewClip::Files(Vec<String>)` → `NewClip::Files { paths, thumb_png: Option<Vec<u8>> }`, 기존 `thumb_png` 컬럼에 저장(스키마 변경 없음). `ClipDto.thumb`가 파일 클립에도 채워짐.
- `Card.tsx`: files + thumb → `<img class="thumb">` + 좌하단 태그 `EXT` 또는 `EXT · +N`(N = 파일 수 − 1). EXT는 경로 목록에서 첫 이미지 확장자(같은 확장자 목록을 프론트에도 둠). 배지/푸터는 기존 그대로.

## 5. 저장 제한 제거

- `MAX_CLIPS`, `Store::trim()`, `upsert`의 trim 호출 삭제. 관련 테스트 `trims_oldest_beyond_max_and_removes_image_file`는 삭제 시 이미지 파일 제거만 검증하도록 축소(이미 `delete_and_clear_remove_image_files`가 커버하면 삭제).
- 성능: 목록은 50개 페이지 + `last_used_at` 인덱스, 검색은 FTS trigram → 행 수 증가 영향 작음. 디스크는 설정의 "기록 지우기"로 사용자가 관리.

## 6. 트레이

- `tray.rs`: `.show_menu_on_left_click(true)`, `DoubleClick` 핸들러 제거(패널은 "열기" 메뉴/단축키로).

## 테스트

- Rust: 패널 높이 clamp(작은/중간/큰 work area, Windows px 변환), 파일 클립 thumb 저장·조회, 1000개 초과 삽입 후 trim 없음.
- Node: 썸네일 태그 라벨 함수(`imageTag(paths)` → `"PNG"`, `"JPG · +3"`, 이미지 없으면 null).
- 수동(`yarn tauri dev`, macOS): 등장 번쩍임 없음, 4종 모니터 크기, Finder로 드래그 시 복사(원본 유지) + 토스트, 드래그 취소 시 패널 복귀, 이미지 파일 카드 썸네일, 트레이 왼쪽 클릭 메뉴.
