# 패널·변환 다듬기 설계

- 날짜: 2026-10-21
- 브랜치: `feature/panel-convert-polish` (master에서 분기)
- 선행 작업: [드래그 파일 변환](2026-10-20-file-convert-design.md), [설정 탭](2026-10-15-settings-tabs-whats-new-design.md)

## 왜

1.0.0으로 드래그 변환을 낸 뒤 쓰면서 걸린 것들을 한 번에 정리한다. 눈에 보이는 깜빡임 두 개, 설정 화면의 권한 항목 정리, 윈도우에서 GPU 인코더를 안 쓰는 문제, 문서 변환 엔진 통일, 그리고 패널을 연 상태에서 마우스로 더블클릭하던 동선을 키보드로 줄이는 고정 카드 단축키.

## 사용자가 정한 것

1. 새 브랜치에서 작업한다.
2. **도넛 잔상**: 다른 종류의 파일을 변환할 때(예: 이미지 변환 뒤 HWP) 이전 종류의 도넛이 잠깐 보이면 안 된다. **macOS와 Windows 모두에서 나타난다.**
3. **패널 깜빡임**: 도넛으로 변환하지 않거나 취소할 때 패널이 잠깐 나왔다 들어가면 안 된다. **macOS에서 봤다.**
4. **복사 디바운스**: 0.5초 → 0.3초.
5. **"권한" 탭 신설**: "전체 디스크 접근"(지금 기록 탭)과 "Finder 창 접근"(지금 변환 탭)을 옮긴다.
6. **"허용됨"은 badge로**: 변환 탭의 "설치됨"과 같은 초록 badge.
7. **변환 키는 지금 그대로**(Option / Ctrl / Shift Segmented). 녹화 방식으로 바꾸지 않는다.
8. **윈도우 GPU 인코더**: NVIDIA GPU가 있는데 MP4 변환 시 작업 관리자에서 CPU만 오른다. 이번에는 **원인을 볼 수 있게** 한다.
   - 하드웨어 시험 인코딩·실제 인코딩이 실패하면 FFmpeg 에러를 로그에 남긴다.
   - 변환 탭 "하드웨어 가속" 줄에 **지금 쓰는 인코더**(NVENC / QSV / AMF / VideoToolbox / CPU)를 badge로 보여준다.
   - 원인 자체의 수정은 로그를 본 뒤 별도 작업.
9. **문서 변환은 LibreOffice로 통일**. HWP·HWPX는 **rhwp 그대로**. 나머지(워드·엑셀·파워포인트 계열, txt, rtf, html)는 모두 `soffice --headless --convert-to`. macOS `textutil` 경로는 없앤다.
10. **고정 카드 단축키**:
    - 패널이 열려 있을 때 **고정된 카드 3개만** 왼쪽부터 1·2·3번.
    - 기본 `Cmd+1/2/3`(Windows `Ctrl+1/2/3`).
    - 동작은 **더블클릭과 똑같다**(복사 → 패널 닫힘(패널 고정 중이면 유지) → 소리·토스트).
    - 설정 "단축키" 탭에서 **세 개를 각각 기존 녹화 방식으로** 바꾼다.

## 목표

- 도넛 창이 뜰 때 첫 화면부터 이번 파일의 조각이 보인다.
- 드래그를 취소하거나 변환 없이 놓았을 때 패널이 한 번만 움직인다(돌아오거나, 닫히거나). 깜빡이지 않는다.
- 복사 후 0.3초 조용하면 기록된다.
- 설정: 권한 탭에 macOS 권한 두 줄, 둘 다 badge. 단축키 탭에 고정 카드 1·2·3.
- 윈도우에서 어떤 인코더를 쓰는지 설정에서 보이고, 실패하면 로그에 FFmpeg 에러 문구가 남는다.
- 윈도우에서도 txt·rtf·html 변환이 나온다(LibreOffice가 있으면).

## 비목표

- 변환 키 녹화 방식.
- HWP를 LibreOffice로 변환.
- 고정 카드가 아닌 기록 카드에 번호 단축키(4~9번).
- 패널이 닫힌 상태에서 쓰는 전역 단축키로 고정 카드 복사.
- GPU 인코더 실패 원인의 수정(로그 확인 후 별도 작업).

## 안 되는 것(실패로 보는 것)

- 번호 단축키가 검색창에 숫자를 입력해 버린다.
- 고정 카드 단축키가 기존 패널 키(`PANEL_KEYS`)나 다른 단축키와 겹치게 저장된다.
- 문서 변환 통일 뒤 macOS에서 LibreOffice 없이 되던 txt/rtf/html 변환이 **조용히** 사라진다. → 사라지지 않고 "LibreOffice 필요"로 흐리게 보여야 한다.

## 현재 코드에서 확인한 것

### 도넛 잔상

- `src-tauri/src/app/convert.rs` `show()`(~207-222)가 `convert://open`을 emit하고 **바로** `overlay.show()`를 부른다. 숨겨진 webview는 마지막으로 그린 프레임(이전 드래그의 도넛)을 먼저 보여준다.
- `convert://close`는 `setHot(null)`만 한다(`ConvertApp.tsx:45-47`, 드롭 이벤트가 hide 뒤에 오므로 slices를 일부러 유지).
- 패널은 같은 문제를 이미 풀었다: `window.rs:276-307`(창 alpha 0으로 show → 프론트가 2 프레임 뒤 `reveal_panel` 호출, 200ms fallback), `PanelApp.tsx:184`.

### 패널 깜빡임

- `window.rs` `finish_drag`(~655-679)가 처음에 `end_drag`로 `dragging=false`를 바로 세운다.
- `Cancel`이면 `panel.set_focus()` + `panel://drag-cancelled` → 프론트 `setDragging(false)`로 패널이 다시 올라온다.
- `lib.rs:81-87`: 패널이 `Focused(false)`를 받고 `dragging`이 false면 `hide_panel`. 드래그 중/직후에 큐에 쌓인 focus-loss가 늦게 오거나, `set_focus`가 자리 잡지 못하면 바로 다시 숨는다 → 깜빡임.
- 기기에서 재현해 어느 쪽인지 확인하지 않았다(빌더가 재현부터).

### 디바운스

- `src-tauri/src/app/watcher.rs:121` `QUIET = 500ms`(trailing debounce, 터미널 copy-on-select용). `POLL = 200ms`.
- 같이 고칠 주석: `state.rs:35`("0.5s debounce"), `state.rs:55`(테스트 주석). `state.rs:37`의 1200ms 자기 쓰기 무시 창은 그대로 충분.

### 권한 줄

- 전체 디스크 접근: `SettingsApp.tsx:74-93` `DiskAccessRow`, 기록 탭 맨 위. "허용됨"이 `text-muted` 글자(`:86`). macOS 외에는 `null`.
- Finder 창 접근: `ConvertSection.tsx:67-77`, `dim={off}`(드래그 변환 꺼짐). "허용됨"은 이미 `BADGE`.
- `BADGE`는 `ConvertSection.tsx:29` 지역 상수. `WhatsNewApp.tsx:98`에도 같은 클래스가 인라인으로 있다.
- 탭 목록: `Tabs.tsx:7` `'general' | 'panel' | 'shortcuts' | 'convert' | 'history'`.

### 하드웨어 인코더

- `src-tauri/src/domain/convert/video.rs` `hw_encoder()`(~81-113): macOS는 항상 VideoToolbox. Windows는 NVENC → QSV → AMF 순서로 1프레임 시험 인코딩(stderr 버림), 앱 실행마다 한 번 `OnceLock`에 캐시.
- 실제 인코딩도 stderr를 버린다(`encode`, ~230). 실패하면 `JobError::Engine` → libx264로 한 번 다시(로그는 `warn` 한 줄뿐).
- 하드웨어 대상은 MP4·MOV·720p(H.264)만. WebM·GIF·MP3는 원래 CPU.
- `-hwaccel auto`로 디코드하고 `-vf scale=...` 필터를 거친다.
- 게이트: `app/convert.rs:281-287`, 설정 `videoHwAccel`(기본 on), 토글은 `ConvertSection.tsx:90-91`.

### 문서 변환

- 표: `src-tauri/src/domain/convert/model.rs:45-53`, 확장자 매핑 `mod.rs:60-78`, 실행 `mod.rs:136-150`(`convert_doc`), LibreOffice 실행 `office.rs`.
- 지금 docx 계열 → txt/rtf/html, txt/html 계열 → 전부는 `textutil`(macOS만). Windows에서는 해당 조각이 아예 빠지고 txt/html 소스는 "지원 안 함" 토스트.
- LibreOffice 필터 문자열: `office.rs:16-27`.

### 고정 카드

- 고정 카드는 최대 3개(`MAX_PINNED`, `clip/model.rs:8`), 고정한 순서, 검색·필터와 무관하게 항상 왼쪽(`PanelApp.tsx:484-499`).
- 더블클릭: `Card.tsx:298` → `PanelApp.tsx` `copy(clip, plain)` → `copy_clip` 명령 → `window::copy_clip`(복사 → `hide_panel` → `confirm_copy`). Shift는 plain text.
- 패널 키 처리: `src/utils/keys.ts` `panelKeyAction`(테스트 `keys.test.ts`), `PanelApp.tsx:304-383`. 패널 전용 단축키 비교는 `acceleratorFromEvent(e, isMac) === settings?.panelPinShortcut` 방식(`PanelApp.tsx:317-318`).
- 패널 전용 단축키 설정 선례: `panelPinShortcut`, `resetSearchShortcut` — 기본값 `domain/settings/mod.rs:20-21`, 검증 `mod.rs:85`, DTO `commands.rs:29-30`, 설정 UI `SettingsApp.tsx:232-247`(`taken`으로 서로·`PANEL_KEYS` 충돌 방지).
- 검색창이 항상 포커스라 수식키 없는 숫자는 검색어로 들어간다.

## 설계

### 1. 도넛 잔상

패널과 같은 "투명하게 띄우고, 그려지면 보이기" 방식을 변환 창에도 쓴다.

- `show()`: alpha 0으로 `overlay.show()` → 프론트가 `convert://open` 처리 후 새 상태가 그려지면(2 rAF) reveal 명령 호출 → alpha 1. 일정 시간 fallback도 패널과 같이.
- 패널 쪽 구현을 그대로 재사용할 수 있으면 재사용한다(헬퍼로 뽑을지는 빌더 판단).
- 패널의 alpha 트릭(`set_panel_alpha`, `window.rs` ~291)은 macOS 전용이다. 잔상은 **Windows에서도 나타나므로 두 플랫폼 모두 고친다.**
  - **계획 단계에서 정한 방식(Claude):** "다시 보일 때 투명하게"가 아니라 **"숨기기 전에 비우기"**. 닫을 때 프론트가 도넛을 지운 빈 프레임을 한 번 그린 뒤 Rust가 창을 숨긴다. 그러면 다음 `show()`에 비치는 마지막 프레임이 빈 화면이라 잔상이 없다. 플랫폼별 API(alpha, layered window)가 필요 없어 macOS·Windows가 같은 코드를 탄다.
  - 빈 프레임을 그리는 동안 창이 잠깐 보이는 상태이므로, 오버레이 위 드롭 판정(`dropped_on_overlay`)은 `is_visible()` 대신 "도넛이 열려 있음" 상태로 한다. 프론트가 응답하지 않으면 Rust가 짧은 fallback 뒤 숨긴다. 닫힘 직후 다시 열리면 늦게 온 숨김 요청은 무시한다.
  - 패널 창의 Windows 잔상은 보고되지 않았으므로 이번 범위가 아니다. 다만 같은 헬퍼로 쉽게 붙는다면 빌더가 메모로 남긴다.

### 2. 패널 깜빡임

- 빌더가 먼저 macOS에서 재현해 원인을 확인한다(`superpowers:systematic-debugging`). 보고된 플랫폼은 macOS. `lib.rs`의 focus-loss 숨김은 두 플랫폼 공통이므로 고친 뒤 Windows 동작이 나빠지지 않게만 한다.
- 유력한 방향: 드래그가 끝난 직후 잠깐 동안 `lib.rs`의 focus-loss 자동 숨김을 막는 유예(예: "드래그 막 끝남" 플래그/시각), 또는 Cancel 시 패널을 되살릴지 숨길지를 한 번만 결정.
- 결과 동작: Cancel이면 패널이 돌아와 그대로 머문다. 변환 없이 놓았으면(`Dropped`) 지금처럼 닫힌다. 어느 쪽이든 한 번만 움직인다.

### 3. 디바운스

- `QUIET`을 300ms로. 주석 두 곳 갱신.

### 4. 권한 탭

- `Tabs.tsx`에 `'permissions'` 추가. i18n `권한` / `Permissions`.
- 내용: 전체 디스크 접근 줄, Finder 창 접근 줄. 기록 탭과 변환 탭에서는 뺀다.
- 두 줄 모두 "허용됨"을 `BADGE`로. `BADGE`는 두 파일에서 쓰게 되므로 공용 위치로 옮긴다(위치는 빌더 판단).
- 허용 안 됨일 때 버튼은 지금 그대로.

### 5. 하드웨어 인코더 진단

- 시험 인코딩과 실제 인코딩의 stderr를 받아, 실패 시 마지막 몇 줄을 `log::warn!`으로 남긴다.
- badge는 **마지막 실제 변환이 쓴 인코더**를 보여준다(시험 결과가 아니라). 시험은 통과했는데 실제 인코딩이 실패해 libx264로 다시 돌렸다면 "NVENC → CPU"처럼 대체됐음을 보인다. 지금 증상(NVIDIA인데 CPU만 오름)이 바로 이 경우일 가능성이 높아서다.
  - 이번 실행에서 아직 H.264 변환이 없으면 시험 결과(`hw_encoder()`)를 보여준다. 시험이 아직 안 돌았으면 설정에서 읽을 때 돌린다(최대 encoder당 5초 → 설정 창이 멈추지 않게 비동기).
  - 실제 결과는 런타임 상태(앱 실행 동안만)로 기억하고, 변환이 끝나면 설정 창이 새 값을 받도록 한다(기존 이벤트/쿼리 무효화 방식을 따른다).
  - FFmpeg가 없으면 badge 없음. `videoHwAccel`이 off면 "CPU".

### 6. 문서 변환 LibreOffice 통일

- `textutil` 경로 삭제(실행 코드, `Caps.textutil`, 관련 테스트).
- txt / rtf / html 대상과 txt·html 소스를 LibreOffice 조각으로. LibreOffice 없으면 `needs: LibreOffice`로 흐리게.
- 필요한 LibreOffice 필터 문자열을 `office.rs`에 추가하고, 실제 `soffice`로 각 조합을 한 번씩 돌려 결과 파일이 생기는지 확인한다.

### 7. 고정 카드 단축키

- 설정 키 3개(이름은 빌더 판단, 예: `pinnedCopy1Shortcut` …). 기본 `CommandOrControl+Digit1/2/3`. 검증은 `panelPinShortcut`과 같은 규칙.
- DTO(Rust `model.rs`/`commands.rs`, TS `settings.dto.ts`)에 추가.
- 패널: `panelKeyAction`에서 세 콤보를 비교해, 해당 자리의 고정 카드가 있으면 `copy(pinned[n], false)`. 없으면 아무것도 안 함. 검색창 입력을 막는다(`preventDefault`).
- 설정 "단축키" 탭에 `ShortcutRecorder` 세 개. `taken`에 `PANEL_KEYS`, 전역 단축키, 패널 고정, 검색 초기화, 다른 고정 카드 단축키 두 개.
- i18n 문구 ko/en.

## Claude가 정한 것 (확인 필요 시 말해 달라)

- 브랜치 이름 `feature/panel-convert-polish`.
- 권한 탭 위치: "기록" 앞(일반 · 패널 · 단축키 · 변환 · **권한** · 기록).
- 권한 탭은 macOS에서만 보인다(두 항목 모두 macOS 전용이라 Windows에서는 빈 탭이 됨).
- Finder 창 접근 줄은 권한 탭으로 옮긴 뒤에도 드래그 변환이 꺼져 있으면 흐리게.
- 번호 단축키에 Shift를 더해 plain text 복사하는 동작은 넣지 않는다(녹화 방식이라 사용자가 Shift 콤보를 직접 지정할 수 있으므로).
- 문서 대상 표: docx 계열과 txt/html 소스 모두 `pdf · docx · odt · txt · rtf · html`에서 자기 형식만 뺀다. 순서는 지금 표를 따른다(pdf 먼저).
- 인코더 badge 문구: `NVENC`, `Quick Sync`, `AMF`, `VideoToolbox`, `CPU`(번역 필요한 것만 i18n).
- 인코더 badge는 시험 결과가 아니라 마지막 실제 변환 기준(대체 시 표시).

## 빌더에게 맡기는 것

- 각 수정의 구체 구현, 헬퍼 추출 여부, 파일 위치.
- 패널 깜빡임의 실제 원인 확인과 그에 맞는 수정.
- 설정 키 이름.

## 검증

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` 통과.
- `yarn tauri dev`로 확인: 라이트/다크, 한국어/English.
  - 이미지 → HWP → 문서 순서로 도넛을 띄워 잔상이 없는지.
  - 변환 키 누른 뒤 놓지 않고 취소 / 변환 키 떼고 놓기 반복으로 패널 깜빡임이 없는지.
  - 권한 탭, 단축키 탭, 인코더 badge.
  - 고정 카드 0·1·3개일 때 `Cmd+1/2/3`.
- Windows 확인(도넛 잔상, 인코더 badge·로그, txt/rtf/html 변환)은 사용자가 윈도우 PC에서.
