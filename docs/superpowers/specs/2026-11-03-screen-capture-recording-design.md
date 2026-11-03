# 스크린 캡처 · 스크린 녹화 설계

- 날짜: 2026-11-03
- 브랜치: `feature/screen-capture-recording`
- 상태: 초안 (사용자 승인 전)
- 지원: 캡처 = Vee 지원 OS 전체, 녹화 = macOS 15+ / Windows 10 2004(19041)+

## 1. 의도

Vee는 클립보드 기록 앱이다. 스크린샷과 화면 녹화는 결국 "붙여넣을 무언가"를 만드는 일이라, Vee 안에서 바로 찍고 바로 클립보드(와 Vee 기록)에 들어가게 한다. OS 기본 도구는 저장 위치·포맷·클립보드 동작을 원하는 대로 못 바꾸고, Windows 캡처 도구는 macOS처럼 스페이스바로 앱 창 전체를 고를 수 없다. 그래서 **두 OS에서 똑같이 동작하는 Vee 자체 선택 화면**을 만든다.

## 2. 목표

- **스크린 캡처** (기본 `⌘⌥⇧4`, Windows `Ctrl+Alt+Shift+4`)
  - 화면이 어두워지고 드래그로 영역 선택. **스페이스바**를 누르면 창 선택 모드로 바뀌고, 커서 아래 앱 창이 강조되며 클릭하면 그 창 전체를 캡처. 스페이스바를 다시 누르면 영역 모드로 돌아감. `Esc` = 취소.
  - 결과는 **무조건 클립보드**에 들어가고 Vee 기록에도 이미지 클립으로 남는다. 파일로 저장하지 않는다.
  - 셔터 소리: macOS는 시스템의 스크린샷 소리(`/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Screen Capture.aif`)를 실행 중에 재생한다(앱에 넣지 않음, Apple 소유). Windows는 퍼블릭 도메인 Holga 셔터 녹음(Wikimedia Commons `File:Holga_shuttersound.ogg`, 0.28s부터 0.62s 구간, loudnorm)을 WAV로 앱에 넣는다.
- **스크린 녹화** (기본 `⌘⌥⇧5`, Windows `Ctrl+Alt+Shift+5`)
  - 캡처와 같은 선택 화면(영역 드래그 / 스페이스바 = 창).
  - 선택 후 화면 하단 가운데 **플로팅 알약(목업 A안)**: 포맷 선택(MP4 · MOV · WEBM · GIF, 기본값은 설정값) + 녹화 버튼 + 취소. `Enter`로도 시작.
  - 녹화 중 알약: 빨간 점 + 경과 시간 · (GIF일 때) 남은 시간 막대 · 정지 · 취소. 녹화 단축키를 다시 누르면 정지.
  - **시스템 소리(지금 재생 중인 앱 소리)만** 녹음한다. 마이크는 녹음하지 않는다. GIF는 소리가 없다.
  - 정지하면 고른 포맷으로 저장(기본 **Downloads**)하고 **파일 클립으로 클립보드**에 넣고 Vee 기록에도 남긴다. 완료 토스트에 저장 위치를 보여준다.
  - 시작·종료 소리: 목업의 **Two-Tone** (시작 660Hz → 990Hz, 종료 990Hz → 660Hz, 사인파, 각 0.14s / 0.2s, 0.13s 간격). `scripts/gen-sounds.mjs`로 WAV를 만들어 앱에 넣는다(두 OS 공통).
  - GIF 제한: **최대 30초**. 알약에 남은 시간을 보여주고 30초가 되면 자동 정지. GIF는 15fps, 긴 변 최대 960px로 변환한다(30초 기준 대략 수~수십 MB).
- **설정**
  - 일반 탭: 녹화 저장 폴더(기본 Downloads, 폴더 고르기 버튼 — 이미 있는 `tauri-plugin-dialog` 사용), 기본 녹화 포맷(`Segmented` MP4/MOV/WEBM/GIF).
  - 단축키 탭: 캡처 단축키, 녹화 단축키(기존 `ShortcutRecorder`, 다른 단축키와 겹치면 거절하는 기존 규칙 그대로). 등록이 실패하면(다른 앱·OS가 이미 사용 중) 단축키 아래에 안내 문구를 띄우고, macOS에서는 **시스템 설정 › 키보드 › 키보드 단축키**를 여는 버튼을 둔다. OS 단축키를 Vee가 직접 끄지는 않는다.
  - 권한 탭(macOS 전용, 이미 있음): **화면 기록** 행 추가. 허용됨 → 배지, 아직 안 물어봄 → "허용"(시스템 요청), 거부됨 → "시스템 설정 열기"(개인정보 보호 › 화면 및 시스템 오디오 기록). 시스템 소리 녹음도 같은 권한으로 처리된다. Windows는 필요한 권한이 없어서 지금처럼 이 탭이 숨겨져 있다.

## 3. 하지 않는 것 (non-goals)

- 마이크 녹음, 웹캠, 클릭 표시, 녹화 일시정지, 녹화 중 영역 이동/크기 변경.
- 캡처 파일 저장, 캡처 후 편집기 자동 열기(편집은 기존 드래그→편집 경로로).
- 모니터 여러 대에 걸친 영역. 선택 화면은 **단축키를 누를 때 커서가 있는 모니터 하나**에만 뜬다.
- macOS 15 미만, Windows 10 2004(빌드 19041) 미만에서 녹화. 단축키를 누르면 "macOS 15 / Windows 10 2004 이상에서 쓸 수 있어요" 토스트. **캡처는 모든 지원 버전에서 동작**한다(찍기 전에 선택 화면을 숨기므로 캡처 제외 API가 필요 없다).
- 트레이 메뉴 항목 추가.
- OS 기본 스크린샷 단축키를 자동으로 끄기(`defaults write com.apple.symbolichotkeys`): 몰래 OS 설정을 바꾸는 일이라 하지 않는다.

## 4. 실패로 보는 것 (anti-goals)

- 제어 알약이나 선택 화면의 어두운 막이 **녹화 영상이나 캡처 이미지에 찍히는 것**.
- Vee 자신의 시작 소리가 녹화에 들어가는 것.
- 녹화 중 FFmpeg/녹화 프로세스가 앱 종료·충돌 뒤에도 남는 것, 임시 파일이 남는 것.
- 녹화 파일이 클립보드에는 들어갔는데 디스크에 없는 것(또는 그 반대).
- 녹화가 끝난 뒤에도 큰 버퍼가 메모리에 남는 것(사용자가 가장 중요하게 보는 항목: 작업이 끝나면 메모리가 돌아와야 한다).

## 5. 제약

- 컨벤션: `docs/agents/conventions/*`, `docs/agents/libs/*`, `docs/design/aurora-glass.md`. 문자열은 `src/shared/i18n/{en,ko}.ts`, 색은 `globals.css`만.
- **새 의존성은 사용자 승인 후 추가.** 예상되는 것: macOS ScreenCaptureKit 바인딩(`objc2-screen-capture-kit` 등), Windows `windows` 크레이트 기능 추가(`Win32_Graphics_Gdi`, `Win32_Graphics_Dwm` 등). 플랜에서 목록을 만들어 한 번에 확인받는다.
- FFmpeg는 앱에 포함되지 않고 설정 › 변환에서 내려받는 엔진(`app/engines.rs`)이다.
  - macOS: MP4/MOV는 FFmpeg 없이 저장된다. WEBM/GIF는 FFmpeg가 필요하다. 없으면 알약의 해당 포맷이 흐려지고 안내가 뜬다.
  - Windows: 녹화 자체에 FFmpeg가 필요하다. 없으면 녹화 단축키 → "설정 › 변환에서 FFmpeg를 받아 주세요" 토스트 + 설정 변환 탭 열기.
- macOS 화면 기록 권한(TCC)이 없으면 캡처·녹화 모두 시작하지 않고 설정 › 권한 탭을 연다.
- 설정은 key-value라 마이그레이션은 필요 없다(새 키의 기본값은 `settings::default`).

## 6. 방식 (Approaches)

| | 방식 | 장점 | 단점 |
|-|------|------|------|
| **1 (채택)** | Vee가 투명 전체 화면 창으로 선택 화면을 직접 그린다. 캡처는 고른 영역/창만 찍는다. 녹화는 macOS ScreenCaptureKit, Windows FFmpeg + WASAPI 루프백. | 두 OS에서 선택 화면과 동작이 완전히 같다. 알약도 같은 창에 자연스럽게 붙는다. | 직접 만들 게 많다(창 감지, 모니터 처리). |
| 2 | macOS 캡처만 `screencapture -i -c`(OS 화면), 나머지는 1번 | macOS 캡처 코드가 줄고 스페이스바도 OS가 처리 | 앱 안에 선택 화면이 두 종류가 되고 Windows와도 다름 → "Windows도 똑같이" 요구와 어긋남 |
| (뺌) | 선택 화면을 Rust 네이티브로 그림 | — | 웹뷰보다 코드가 훨씬 많고 얻는 게 없음 |

사용자가 **1번**을 골랐다.

## 7. 정해진 설계

### 7.1 창

- `capture` (신규): 선택 화면. 투명·테두리 없음·항상 위·작업 표시줄에 안 보임. 커서 아래 모니터 전체 크기. 포커스를 받아 키보드(Space/Enter/Esc)를 받는다. 다른 창처럼 `create: false`로 만들고 setup에서 빌드. 캡처·녹화 공용(`capture://open` 이벤트에 `{ mode: "capture" | "record" }`).
- `recorder` (신규): 녹화 중 알약. 작은 투명 창, 모니터 하단 가운데, 항상 위, 포커스를 뺏지 않는다. 녹화 시작 시 `capture` 창은 숨기고 이 창을 띄운다(녹화 중엔 화면을 어둡게 하지 않는다).
- 두 창 모두 **캡처에서 빠진다**: macOS는 `SCContentFilter`에서 Vee 프로세스의 창을 제외, Windows는 `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`. 캡처는 `capture` 창을 숨긴 뒤 찍는다.
- 디자인: Aurora Glass. 알약은 `.glass`, 녹화 버튼은 `from-primary to-sky` 그라데이션, 정지는 빨강(새 토큰 `--vee-rec`, 라이트/다크 값 모두), 점 깜빡임은 reduced-motion에서 끈다. 목업: `.superpowers/brainstorm/67055-1793633854/content/record-hud.html`의 A안.

### 7.2 흐름

1. 단축키 → (macOS 권한 확인 · macOS 15 확인(녹화) · Windows FFmpeg 확인(녹화)) → 모니터 정보와 창 목록을 담아 `capture` 창 표시.
2. 영역 드래그 또는 스페이스바 → 창 선택. 창 목록은 열 때 한 번 받는다: macOS `CGWindowListCopyWindowInfo`(이미 의존성에 있음), Windows `EnumWindows` + DWM 확장 프레임 크기. Vee 자신의 창, 데스크톱, 메뉴바/작업표시줄은 뺀다.
3. **캡처**: 마우스를 떼거나 창을 클릭하면 바로 캡처(확인 단계 없음) → 셔터 소리 → 이미지 클립 저장 + 클립보드 → 기존 "복사됨" 토스트.
4. **녹화**: 선택 후 알약에서 포맷 확인 → 녹화(Enter) → 시작 소리 → `recorder` 창 → 정지(버튼/단축키/GIF 30초) → 종료 소리 → (필요하면) FFmpeg 변환 → 저장 폴더로 이동 → 파일 클립 + 클립보드 → "저장됨" 토스트(경로). 취소는 임시 파일을 지우고 아무것도 남기지 않는다.

### 7.3 캡처 구현

- macOS: 영역은 `screencapture -x -R x,y,w,h <tmp>.png`, 창은 `screencapture -x -o -l <windowId> <tmp>.png`(그림자 없음). Retina 원래 해상도 그대로.
- Windows: 영역은 GDI `BitBlt`, 창은 `PrintWindow(PW_RENDERFULLCONTENT)` 또는 화면에서 창 사각형을 잘라내기(빌더 선택). DPI 원래 해상도.
- 결과 PNG → 이미지 클립(`clip::upsert`) → `window::write_clipboard`(watcher 억제 포함) 경로 재사용. 출처 앱은 창 모드면 그 창의 앱, 영역 모드면 Vee.

### 7.4 녹화 구현

- macOS 15+: `SCStream` + `SCRecordingOutput`으로 화면 + 시스템 소리를 한 파일(MP4 또는 MOV, 고른 포맷에 맞춤. WEBM/GIF는 MP4로 녹화 후 변환)에 바로 쓴다. `capturesAudio = true`, `excludesCurrentProcessAudio = true`, 커서 표시. 영역은 `sourceRect`, 창은 `desktopIndependentWindow` 필터.
- Windows: FFmpeg로 화면(`gdigrab` 또는 `ddagrab`, 영역 오프셋/크기, 창 모드는 시작 시점의 창 사각형)을 받고, 같은 시간에 WASAPI 루프백(`windows` 크레이트의 `Win32_Media_Audio`, 이미 켜져 있음)으로 시스템 소리를 WAV로 받는다. 정지 시 FFmpeg에 `q`를 보내 정상 종료 → 두 파일을 합친다.
- Vee 자신의 소리가 녹음되지 않게(Windows 루프백은 모든 소리를 받는다; 프로세스 제외 루프백은 사실상 Windows 11 전용이라 쓰지 않음): 시작 소리가 끝난 뒤 녹음을 시작하고, 종료 소리는 녹음을 멈춘 뒤 낸다. 녹화 중에는 Vee의 복사 소리를 내지 않는다(macOS도 같은 규칙, 거기선 `excludesCurrentProcessAudio`가 한 번 더 막는다).
- 변환: 기존 `domain/convert/video.rs`의 FFmpeg 인자 생성·진행률·취소·`process.rs`의 kill-on-drop 가드를 재사용한다. GIF는 palettegen/paletteuse, 15fps, 긴 변 960px.
- 임시 파일은 `<temp>/vee-record-<identifier>/`. 성공·실패·취소 모두 지운다. 녹화 중 앱 종료 시 녹화를 멈추고 임시 파일을 지운다.
- 파일 이름: `Vee Recording 2026-11-03 at 15.12.08.mp4`(로케일 무관 고정 형식). 같은 이름이 있으면 ` (2)`를 붙인다.

### 7.5 설정 키

| 키 | 기본값 | 값 |
|----|--------|----|
| `captureShortcut` | `CommandOrControl+Alt+Shift+4` | 단축키 |
| `recordShortcut` | `CommandOrControl+Alt+Shift+5` | 단축키 |
| `recordFolder` | `""` (= OS Downloads 폴더) | 절대 경로 |
| `recordFormat` | `mp4` | `mp4` · `mov` · `webm` · `gif` |

`settings::is_valid`와 기존 단축키 충돌 검사에 두 단축키를 넣는다. 저장 폴더가 없어졌으면 Downloads로 저장하고 토스트로 알린다.

### 7.6 오류

- 권한 없음 → 설정 › 권한 탭을 연다(`settings://tab` 기존 경로).
- 녹화 실패/변환 실패 → 실패 토스트. 녹화 원본(MP4)이 있으면 그것만이라도 저장 폴더에 남긴다.
- 디스크 공간 부족, 폴더 쓰기 불가 → 실패 토스트에 이유.
- 새 `AppError` 코드가 필요하면 `docs/agents/conventions/error-handling.md`를 따른다.

## 8. 내가 정한 것 (사용자가 따로 말하지 않음)

- Windows 기본 단축키는 `Ctrl+Alt+Shift+4/5` (`CommandOrControl` 매핑).
- Windows 셔터 소리는 Holga(퍼블릭 도메인).
- GIF는 15fps, 긴 변 960px, 30초에서 자동 정지.
- 녹화 포맷 기본값은 MP4. 포맷 목록은 MP4 · MOV · WEBM · GIF.
- 캡처·녹화 소리는 기존 `sound` 설정(on/off)을 따른다. 끄면 셔터/시작/종료 소리도 나지 않는다.
- 캡처는 확인 없이 바로 찍힌다(macOS 기본 동작과 같음).
- 녹화 중 영역 테두리 표시는 넣지 않는다(목업 A안의 빨간 테두리는 v1에서 뺌). 필요하면 클릭이 통과하는 창으로 나중에 추가.
- 녹화 중엔 커서를 녹화한다.
- FFmpeg가 없을 때(macOS) 저장된 `recordFormat`이 WEBM/GIF면 알약은 그 녹화에서만 MP4를 고른 상태로 열고(설정값은 바꾸지 않음), WEBM/GIF는 흐리게 + 안내. 설정의 `Segmented`에서는 FFmpeg가 없어도 WEBM/GIF를 고를 수 있고, 힌트에 "FFmpeg 필요"를 보여준다.
- 선택 화면은 커서가 있는 모니터 하나에만 뜬다.

## 9. 빌더에게 맡기는 것

- Windows 화면 녹화에 `gdigrab`과 `ddagrab` 중 무엇을 쓸지(성능 vs 호환성), 창 캡처를 `PrintWindow`로 할지 화면 잘라내기로 할지.
- 새 Rust 모듈 배치(`app/capture.rs`, `app/recorder.rs` 같은 앱 셸 vs `domain/` 분리)는 `docs/agents/conventions/rust-modules.md`를 따라 정한다.
- 선택 화면의 세부 시각(크기 라벨, 창 강조 모양, 십자선).
- 테스트 범위: 순수 로직(FFmpeg 인자, 파일 이름, 설정 검증, 창 목록 필터, 사각형 좌표 변환)은 `cargo test`/`node:test`로, 실제 캡처는 dev 앱에서 직접 확인.
