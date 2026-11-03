# Windows 창 녹화를 WGC로 바꾸기 (설계)

- 날짜: 2026-11-03
- 브랜치: `feature/capture-cursor-icons` (사용자 지정. 포인터 아이콘 PR #31과 같은 브랜치)
- 관련: `docs/superpowers/specs/2026-11-03-screen-capture-recording-design.md` (녹화 기능 원래 설계)

## 의도

Windows에서 창을 골라 녹화하면, 지금은 녹화를 시작할 때 그 창이 있던 **화면 사각형**을 FFmpeg `gdigrab`으로 찍는다(`src-tauri/src/app/recorder/win.rs`의 `Selection::Window` 분기). 그래서 아래 문제가 생긴다.

- 앞에 겹친 다른 앱이 함께 녹화된다.
- 녹화 중에 창을 옮기면 따라가지 않는다.
- 최소화하면 뒤에 있던 화면이 찍힌다.

macOS는 ScreenCaptureKit의 `desktopIndependentWindow` 필터로 **그 창만** 녹화한다. Windows도 macOS처럼 그 창만 담기게 Windows.Graphics.Capture(WGC)로 바꾼다.

## 목표

- 창 녹화 영상에 고른 창의 내용만 담긴다. 앞을 가린 다른 창은 담기지 않는다.
- 녹화 중에 창을 옮기거나 다른 모니터로 보내도 계속 그 창을 담는다.
- 결과 파일, 저장, 변환, 토스트 흐름은 지금과 같다.

## 비목표

- **소리는 그대로.** 시스템 전체 WASAPI 루프백을 유지한다. 앱 소리만 담기(Windows 11 프로세스 루프백)는 하지 않는다. (사용자 결정)
- **영역 녹화는 그대로.** `Selection::Rect`는 지금처럼 `gdigrab`을 쓴다.
- 스크린샷(`capture/shot.rs`)은 바꾸지 않는다.
- 녹화 중 창이 닫혔을 때 자동 정지는 하지 않는다. 녹화 바 UI까지 바뀌기 때문이다.
- macOS 코드는 바꾸지 않는다.

## 실패로 보는 경우

- 녹화 대상 앱이 버벅이거나 깜빡인다(`PrintWindow` 방식을 버린 이유).
- 영상 길이가 실제 녹화 시간과 어긋나 소리와 싱크가 틀어진다. 창 내용이 안 바뀌면 WGC가 프레임을 주지 않기 때문이다.
- 녹화 중 창 크기를 바꾸면 파일이 깨지거나 재생이 안 된다.
- 정지나 취소 뒤에 캡처 세션, D3D 장치, 스레드, FFmpeg가 남는다. 메모리가 돌아오지 않는 것도 실패다. 사용자는 메모리 반환을 최우선으로 본다.

## 동작 (합의한 것)

| 상황 | 동작 |
|---|---|
| 다른 창이 앞을 가림 | 고른 창만 찍힌다 |
| 창 이동 / 다른 모니터로 이동 | 따라간다 |
| 창 크기 변경 | 영상 크기는 시작할 때 크기로 고정. 커진 창은 비율을 유지해 줄여서 맞추고, 작아진 창은 확대하지 않고 원래 크기로 가운데 둔다(선명함 우선). 남는 곳은 검은 여백 *(작아진 창 처리: 설계자 판단)* |
| 최소화 | 마지막 화면이 멈춘 채 이어진다 |
| 창 닫힘 | 마지막 화면이 멈춘 채 이어지고, 사용자가 정지하면 저장 |
| 마우스 포인터 | 영상에 포함 (`IsCursorCaptureEnabled = true`, 지금 `-draw_mouse 1`과 같음) |
| 노란 캡처 테두리 | Windows 11: 끈다(`IsBorderRequired = false`). Windows 10: OS가 강제로 그려서 남는다. 사용자가 괜찮다고 했다 |
| 소리 | 지금처럼 시스템 전체 루프백 |

영상 크기의 기준은 시작할 때 창의 DWM 확장 프레임(지금 `window_rect`) 크기가 아니라 **WGC가 보고하는 창 크기**(`GraphicsCaptureItem::Size`)다. **긴 쪽이 1920px를 넘으면 비율을 유지해 긴 쪽 1920px로 줄인다**(사용자 결정: 4K 창도 어떤 PC에서나 끊김 없이 녹화되게). 그다음 짝수로 내림한다(`plan::even`). 이 계산(창 크기 → 고정 출력 W×H)은 `plan.rs`의 순수 함수로 두고 단위 테스트한다. 시작할 때 창 크기가 출력 크기보다 크면 매 프레임 줄여야 하므로, 축소 경로가 30fps를 유지하는지가 성능의 핵심이다.

## 접근 방법

| | 방법 | 장점 | 단점 |
|---|---|---|---|
| **1 (채택)** | `windows` 크레이트(이미 0.62.2)에 WGC/D3D11 기능을 추가하고 WGC를 직접 쓴다 | 새 크레이트가 없다. 이미 WinRT(OCR, Calendar)를 이 크레이트로 쓰고 있어 방식이 같다. 녹화 품질이 안정적이다 | Direct3D 준비 코드를 직접 써야 해서 `win.rs`가 200~300줄 는다 |
| 2 | `windows-capture` 크레이트 추가 | WGC/D3D 준비 코드가 줄어든다 | 새 의존성이 생기고, 다른 `windows` 버전이 중복으로 빌드될 수 있다. 일정 간격 송출과 크기 맞추기는 어차피 직접 해야 한다 |
| 3 | 프레임마다 `PrintWindow` | 의존성 변경이 없고 Windows 10에도 테두리가 없다 | 초당 30번 호출하면 큰 창의 대상 앱이 버벅이거나 깜빡일 수 있다 |

사용자가 1을 골랐다.

## 의존성 변경 (사용자 승인됨)

`src-tauri/Cargo.toml`의 `[target.'cfg(windows)'.dependencies] windows` 기능에 추가한다.

- `Graphics_Capture`
- `Graphics_DirectX`
- `Graphics_DirectX_Direct3D11`
- `Win32_Graphics_Direct3D`
- `Win32_Graphics_Direct3D11`
- `Win32_Graphics_Dxgi`
- `Win32_Graphics_Dxgi_Common`
- `Win32_System_WinRT`
- `Win32_System_WinRT_Direct3D11`
- `Win32_System_WinRT_Graphics_Capture`

컴파일에 필요한 기능이 이 목록과 다를 수 있다. `windows` 크레이트 안에서 기능 이름만 더하거나 빼는 정도는 이 승인 범위로 본다. 새 크레이트 추가나 버전 변경은 다시 승인받는다.

## 구조 (빌더 몫이 아닌 결정)

창 녹화 경로만 바뀐다. `Running`의 공개 API(`start` / `stop` / `cancel`)와 `mod.rs`는 그대로 둔다.

```
WGC 캡처 스레드 ──(최신 프레임 BGRA, Arc<Mutex<..>>)──▶ 송출 스레드 (30 fps 타이머)
                                                          │ 고정 W×H BGRA
                                                          ▼
                                     FFmpeg stdin: -f rawvideo -pix_fmt bgra -s WxH -r 30 -i -
                                                          ▼
                                     video.mkv (libx264 ultrafast crf 18 yuv420p, 지금과 같음)
WASAPI 루프백 스레드 ──▶ audio.wav (그대로)
정지: 송출 종료 → stdin 닫기 → FFmpeg 정상 종료 → mux_args (그대로)
```

- **캡처**: `IGraphicsCaptureItemInterop::CreateForWindow(HWND(id))`로 아이템을 만든다. `Direct3D11CaptureFramePool::CreateFreeThreaded`(B8G8R8A8, 버퍼 2개)와 `CreateCaptureSession`을 쓴다. 프레임마다 staging 텍스처로 복사해 CPU에서 읽는다. 행 pitch를 처리해서 촘촘한 BGRA로 만든다.
- **프레임 크기 변경**: 도착한 프레임의 `ContentSize`가 풀 크기와 다르면 `Recreate`로 풀을 다시 만든다.
- **일정 간격 송출**: 송출 스레드가 33.3ms마다 "마지막으로 받은 프레임"을 FFmpeg에 쓴다. 새 프레임이 없으면 직전 것을 다시 쓴다. 정적인 창, 최소화, 닫힌 창 모두 이것으로 해결되고 영상 길이가 벽시계 시간과 같아진다. 첫 프레임이 오기 전에는 검은 화면을 쓴다.
- **크기 맞추기**: 프레임 크기가 고정 W×H와 같으면 그대로 쓴다. 다르면 비율을 유지해 W×H 안에 맞추고(`image` 크레이트 resize, 이미 의존성) 가운데 두며 나머지는 검게 채운다. 이 "맞추기 사각형" 계산은 순수 함수로 `plan.rs`에 두고 단위 테스트한다.
- **FFmpeg 인자**: `plan.rs`에 `pipe_args(w, h, out) -> Vec<OsString>`를 새로 만든다. 출력 쪽 인자는 `grab_args`와 같게 맞춘다. 단위 테스트한다.
- **WGC를 쓸 수 없을 때**: `GraphicsCaptureSession::IsSupported()`가 false이거나 D3D11 장치(하드웨어, WARP 모두)를 못 만들면, 창 녹화는 **지금의 `gdigrab` 화면 사각형 방식으로 대체**한다. RDP, VM, GPU 드라이버가 없는 환경에서 오늘 되던 창 녹화가 깨지지 않게 하기 위해서다. 대체할 때는 로그를 남긴다. 창이 이미 사라졌으면(`IsWindow`가 false) 대체하지 않고 `Err`를 낸다. 창은 있는데 아이템 생성이 실패하면(관리자 권한·보호된 창 등) `gdigrab`으로 대체한다. 아이템은 만들어졌지만 시작 후 약 500 ms 안에 첫 프레임이 오지 않는 창도 검은 화면을 녹화하지 않도록 `gdigrab`으로 대체한다. *(설계자 판단)*
- **시작 실패 판정**: 지금처럼 `EARLY_EXIT` 뒤에 FFmpeg가 살아 있는지 본다. 그 전에 창이 사라졌으면(`IsWindow`가 false) `start`가 `Err`를 낸다. 캡처할 수 없는 창은 `Err`가 아니라 위의 `gdigrab` 대체로 간다.
- **정리**: `halt` / `Drop`에서 송출과 캡처를 멈추고 스레드를 join한다. 세션과 프레임 풀은 `Close`한다. FFmpeg는 지금처럼 `Killed`로 처리한다. 정지 후 프레임 버퍼가 남지 않아야 한다.
- **테두리**: Windows 11이면 `GraphicsCaptureSession::IsBorderRequired(false)`를 시도하고, 실패하면 무시한다(로그만 남김). 필요하면 `GraphicsCaptureAccess::RequestAccessAsync(Borderless)`도 시도한다. Windows 10에서는 시도하지 않거나, 실패를 무시한다.
- **최소 지원 버전**: 녹화 하한은 이미 Windows 10 2004(19041)다. WGC 창 캡처(1903+)와 `IsCursorCaptureEnabled`(2004+)가 모두 이 안에 들어서 하한이 바뀌지 않는다.

## 제약

- Rust 모듈 규칙(`docs/agents/conventions/rust-modules.md`)을 따른다. Windows 전용 코드는 `#[cfg(windows)]`인 `recorder/win.rs` 안에 둔다. 파일이 너무 커지면 `recorder/wgc.rs`로 나눠도 된다(빌더 판단).
- 오류는 기존 `other(step, e)` → `AppError::Other` 형식을 쓴다.
- 사용자에게 보이는 문구가 새로 생기지 않는다(i18n 변경 없음).
- 이 Mac에서 Windows 코드를 실행할 수 없다. 개발 중에는 `cargo check --target x86_64-pc-windows-msvc`(타깃 설치됨)로 컴파일을 확인한다. `tauri-build`의 Windows 리소스 검사 때문에 실패하면 그 사실을 기록한다. 실제 동작은 사용자가 Windows 기기에서 확인한다.
- 네 가지 체크(`cargo test --lib` / `yarn test` / `yarn typecheck` / `yarn lint`)를 통과해야 한다.

## 빌더에게 남긴 것

- 프레임 수신 방식: `FrameArrived` 이벤트와 별도 스레드 `TryGetNextFrame` 폴링 중 무엇을 쓸지.
- D3D11 장치 생성 플래그(`BGRA_SUPPORT`). 하드웨어 생성이 실패하면 WARP로 다시 시도할지.
- staging 텍스처 재사용 방식.
- 리사이즈 필터 종류(속도 우선 `Triangle` 정도).
- `win.rs` 분할 여부.

## Windows 기기 확인 항목 (사용자)

1. 다른 창으로 대상 창을 가린 채 녹화해서, 영상에 대상 창만 나오는지 확인
2. 녹화 중 창 이동, 다른 모니터로 이동
3. 녹화 중 창 크기 변경(키우기, 줄이기)했을 때 검은 여백으로 맞춰지고 파일이 정상 재생되는지
4. 최소화했다가 복원
5. 내용이 거의 안 바뀌는 창을 1분 녹화했을 때 영상 길이와 소리 싱크
6. Windows 11에서 노란 테두리가 없는지, Windows 10에서 노란 테두리는 있어도 녹화는 정상인지
7. 정지·취소 후 작업 관리자에서 Vee 메모리가 녹화 전 수준으로 돌아오는지
8. 영역 녹화가 예전처럼 동작하는지
