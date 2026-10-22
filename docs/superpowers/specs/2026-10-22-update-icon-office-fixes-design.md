# 업데이트·메뉴 버튼·문서 변환 고침, PNG → 아이콘 설계

- 날짜: 2026-10-22
- 브랜치: `feature/update-icon-office-fixes` (master에서 분기)

## 왜

1.2.0을 쓰면서 걸린 세 가지를 고치고, 작은 기능 하나를 더한다.

- 업데이트를 받아 둔 사이 더 새 버전이 나오면, 옛 버전으로 재시작한 뒤 **또** 업데이트해야 한다.
- 패널 우측 상단 ⚙︎ 버튼의 아이콘이 가운데가 아니고, 호버 회전이 흔들린다.
- LibreOffice로 pptx·docx·xlsx → pdf 등을 변환하면 한글 문서가 원본과 너무 다르다.
- PNG를 아이콘(`.ico`, `.icns`)으로 바꾸고 싶다.

## 사용자가 정한 것

1. 새 브랜치에서 작업한다.
2. **업데이트는 두 방법을 모두 쓴다**:
   - 받아 둔 업데이트가 있어도 주기 확인(6시간)을 계속하고, 더 새 버전이 있으면 받아 갈아끼운다.
   - "재시작"을 누르면 설치 직전에 한 번 더 확인하고, 더 새 버전이 있으면 받아서 그걸 설치한다.
   - 이때 **기다림이 생겨도 괜찮다**.
3. **메뉴 버튼**: 아이콘을 버튼 정중앙에 두고, 흔들리며 도는 호버 애니메이션을 고친다.
4. **문서 변환**: 줄·페이지 밀림, 글꼴 다름, 글자 깨짐(□)이 모두 나타난다. 원본에 가깝게 만든다.
5. **PNG → 아이콘**: `.ico`와 `.icns` **둘 다**.
6. **이미지 변환 대상에서 BMP·TIFF를 빼고** 그 자리에 ICO·ICNS를 넣는다.

## 현재 상태(원인)

- **업데이트**: `src-tauri/src/app/updater.rs`의 `check()`는 상태가 `Ready`이면 바로 반환한다. 한 번 받으면 주기 확인도 수동 확인도 더 이상 GitHub에 묻지 않는다. `install_and_restart()`는 받아 둔 것을 그대로 설치한다. 트레이 "update" 메뉴(`app/tray.rs`)도 같은 함수를 동기로 부른다.
- **⚙︎ 버튼**: `src/app/panel/_components/Toolbar.tsx` 마지막 버튼이 SVG가 아니라 글자 `⚙︎`다. 글꼴 메트릭 탓에 상자 중심과 글리프 중심이 어긋나서, 중앙이 아니고 `rotate-45`가 그 어긋난 중심을 축으로 돌아 흔들려 보인다. 바로 옆 핀 버튼은 SVG(`size-4.5`, `strokeWidth=2`)다.
- **문서 변환**: `src-tauri/src/domain/convert/office.rs`의 `soffice_args`는 `-env:UserInstallation=<app data>/lo-profile`로 Vee 전용 **빈 프로필**을 쓴다. 한국어 로캘·아시아 언어 설정·글꼴 대체표가 없다. 맥에는 맑은 고딕·굴림·바탕 등 MS 글꼴이 없어(이 개발 맥에서 확인) 다른 글꼴로 대체되며 폭이 달라져 줄·페이지가 밀리고, 한글 없는 글꼴로 떨어지면 □가 된다(추정, 빌더가 재현으로 확인).
- **이미지 변환**: `domain/convert/image.rs`. macOS는 `sips`, Windows는 `image` 크레이트. 아이콘 형식은 없다. `formats_for`는 이미지 원본이면 `IMAGE_TARGETS`를 그대로 준다.

## 목표

- 받아 둔 버전보다 새 버전이 나오면, 다음 주기 확인 뒤 패널의 "새 버전 x.y.z"가 새 번호로 바뀐다.
- 재시작을 누른 시점에 GitHub의 최신 버전이 설치된다. 기다리는 동안 확인 창이 "확인 중" 상태로 보인다.
- 트레이에서 "업데이트 후 재시작"을 눌러도 같다.
- ⚙︎ 아이콘이 버튼 정중앙에 있고, 호버 시 제자리에서 45° 돈다.
- 맑은 고딕·굴림·돋움·바탕·궁서 등으로 된 한글 docx/pptx/xlsx → pdf 결과에 □가 없고, 줄·페이지 배치가 원본(Word/PowerPoint/Excel에서 본 것)에 최대한 가깝다.
- 이미지를 변환 도넛에 놓으면 BMP·TIFF 조각은 없고 `ICO`, `ICNS` 조각이 보이며, 맥·윈도우 모두에서 두 형식을 만든다.

## 비목표

- 원본 글꼴이 PC에 없을 때 **완전히 똑같은** 결과. 대체 글꼴로 근접하게만 맞춘다.
- 글꼴을 내려받거나 앱에 동봉하기.
- 아이콘 → PNG 역변환(ico/icns를 원본으로 받기).
- BMP·TIFF **원본** 읽기는 그대로 둔다. 빠지는 건 대상뿐이다.
- 업데이트 진행률 표시.

## 안티 목표(이러면 실패)

- 재시작을 눌렀는데 오프라인이라 아무것도 설치되지 않는다. → 재확인이 실패하면 받아 둔 버전으로 설치한다.
- 재확인 중 확인 창이 사라지거나 "새 버전" 칩이 깜빡인다. → 설치 경로에서는 상태를 `Checking`으로 바꾸지 않는다.
- 주기 확인 다운로드와 설치가 겹쳐 반쯤 받은 파일을 설치한다.
- 의존성 추가 또는 `Cargo.toml` 변경(필요해지면 먼저 사용자에게 묻는다. CLAUDE.md 규칙).

## 설계

### 1. 업데이트 (`app/updater.rs`, `app/tray.rs`, 패널)

- `check()`: `Ready`일 때도 확인한다. 막는 건 `Checking`(진행 중)뿐이다.
  - 받아 둔 게 있는 상태에서 확인할 때는 상태를 `Checking`으로 바꾸지 **않는다**(칩이 사라지지 않게). 조용히 확인한다.
  - 더 새 버전이면 받은 뒤 `pending`을 교체하고 `Ready { 새 version }`을 보낸다. whats_new도 새 버전으로 저장한다.
  - 새 게 없거나 실패하면 `Ready`를 유지한다(`UpToDate`/`Failed`로 덮지 않는다).
  - `updater.check()`는 현재 실행 중인 버전과 비교하므로, 받아 둔 버전과 같거나 **낮은**(릴리스 철회 등) 버전이 돌아오면 다시 받지 않는다. semver로 엄격히 더 클 때만 교체한다.
- `install_and_restart()`는 async가 된다. 위 조용한 확인을 한 번 하고(실패하면 무시), 그때의 `pending`을 설치하고 재시작한다.
- 동시성: 확인·다운로드·설치를 하나의 async 잠금(예: `tokio::sync::Mutex<()>`, tauri가 이미 쓰는 런타임)으로 직렬화한다. 주기 확인이 받는 중에 설치를 누르면 그것이 끝나기를 기다린다.
- command `install_update_and_restart`는 async. 트레이는 `async_runtime::spawn`으로 부른다.
- 설치 경로의 재확인에서 더 새 버전을 찾으면 `Checking`을 거치지 않고 `Ready { 새 version }`을 보낸다(칩 번호가 바뀜).
- 설정 창의 "재시작하여 업데이트" 버튼(`src/app/settings/SettingsApp.tsx`)도 패널과 똑같이: 누르면 비활성 + 같은 i18n 키의 "확인 중…", 실패하면 오류 문구.
- 패널: 재시작을 누르면 `installing` 상태(PanelApp 로컬)로 재시작 버튼을 비활성 + "확인 중…" 문구. 실패하면 지금처럼 `updateFailed`. 문구는 `i18n/{en,ko}.ts`에 둔다.

### 2. ⚙︎ 버튼 (`Toolbar.tsx`)

- 글자 `⚙︎`를 인라인 SVG 톱니로 바꾼다. 핀 SVG와 같은 `block size-4.5`, `viewBox 0 0 24 24`, `stroke currentColor`, `strokeWidth 2`.
- 회전은 SVG 자신에 건다(`origin-center` 확인). 툴팁이 같이 돌지 않게 지금처럼 분리한다. `motion-safe`는 유지한다.
- 경로는 Lucide `settings` 같은 대칭 톱니(빌더 선택). 중심이 viewBox 중심(12,12)과 일치해야 한다.

### 3. 문서 변환 (`domain/convert/office.rs`, `app/convert.rs`)

- soffice를 부르기 전에 프로필 안에 `user/registrymodifications.xcu`를 Vee가 쓴다(없거나 Vee가 쓴 판이 낡았으면). 담을 것:
  - 로캘: UI·문서 기본 아시아 언어 `ko-KR`, 아시아 언어 지원 켬.
  - 글꼴 대체표(Tools ▸ Options ▸ Fonts의 Replacement Table, "항상 바꾸기"):
    - macOS: 맑은 고딕/Malgun Gothic, 굴림/Gulim, 돋움/Dotum, 새굴림 → Apple SD Gothic Neo. 바탕/Batang, 궁서/Gungsuh, 신명조 → AppleMyungjo. 영문 Calibri/Cambria는 LibreOffice 동봉 Carlito/Caladea가 이미 메트릭 호환이라 둔다.
    - Windows: 대부분 원본 글꼴이 있으므로 대체표는 비거나 최소.
  - 정확한 키 이름과 형식은 LibreOffice 문서/실제 프로필로 확인한다(기억에 의존 금지).
- `.xcu` 생성은 순수 함수(플랫폼 → 문자열)로 두고 단위 테스트한다. 파일 쓰기는 `convert_lo` 직전.
- 이미 만들어진 사용자 프로필(1.2.0까지 생성된 것)에도 적용돼야 한다. LibreOffice가 종료하며 `registrymodifications.xcu`를 다시 쓰므로 주석 표식은 남지 않는다. **매 실행 전에 Vee 판으로 다시 쓴다**(가장 단순, 사용자가 이 프로필을 손댈 일은 없음).
- **검증**: 맑은 고딕·바탕을 쓴 한글 docx, pptx, xlsx 표본을 만들어 이 맥에서 변환 전후를 비교한다. 기준 렌더링은 이 맥에 설치된 Word/PowerPoint/Excel의 PDF 내보내기(페이지 수, □ 유무, 눈으로 배치). 대체 글꼴 후보(Apple SD Gothic Neo, 나눔고딕 등)는 이 비교로 고른다. 표본 파일은 레포에 넣지 않는다.

### 4. PNG → 아이콘 (`domain/convert/`)

- `Format`에 `Ico`, `Icns` 추가(확장자 `ico`, `icns`). `IMAGE_TARGETS`에서 `Tiff`, `Bmp`를 빼고 그 자리에 `Ico`, `Icns`를 넣는다(순서: PNG, JPG, WEBP, HEIC, ICO, ICNS, GIF, PDF, 빌더가 도넛에서 보고 조정 가능). 모든 이미지 원본에 대해 나온다.
- `Format::Tiff`/`Bmp`는 원본 인식(`from_ext`)과 엔진 경로에서 계속 쓰이면 남기고, 대상으로만 빠진다.
- 원본이 PNG가 아니면 먼저 PNG로 만든다: macOS는 기존 `sips` → 임시 PNG(WEBP 경로와 같은 방식, macOS `image` 빌드엔 jpeg·tiff 디코더가 없음), Windows는 `image`로 바로 읽는다.
- 프런트 DTO, 조각 라벨, i18n도 따라 맞춘다(`ICO`, `ICNS`).
- 플랫폼 공통 코드로 직접 쓴다(새 의존성 없음). `image` 크레이트(png 기능 있음)로 PNG를 읽고 RGBA로 정사각형 캔버스에 가운데 정렬(투명 여백), `imageops::resize`(Lanczos3)로 각 크기를 만들고 PNG로 인코딩한다.
  - `.ico`: ICONDIR + ICONDIRENTRY + PNG 데이터. 크기 16, 24, 32, 48, 64, 128, 256 (256은 폭/높이 바이트 0).
  - `.icns`: `icns` 헤더 + 블록. `icp4`(16), `icp5`(32), `ic07`(128), `ic08`(256), `ic09`(512), `ic10`(1024), `ic11`(16@2x=32), `ic12`(32@2x=64), `ic13`(128@2x=256), `ic14`(256@2x=512). 모두 PNG 페이로드.
- 원본이 작아도 늘려서 채운다(흐려질 수 있음, 그대로 둔다).
- 테스트: 작은 PNG로 만든 ico/icns의 헤더·엔트리 수·크기를 검증하고, ico는 `image`로 다시 읽어 본다(ico 기능이 없으면 헤더 파싱만).

## 제약

- CLAUDE.md / AGENTS.md 규칙: 코드·주석 영어, 사용자 문구는 `i18n/{en,ko}.ts`, 의존성 변경 전 사용자 확인, 배포된 마이그레이션 손대지 않음.
- 완료 전 `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` 통과.
- UI 변경(⚙︎, 확인 중 문구, 아이콘 조각)은 `yarn tauri dev`로 라이트/다크, 한국어/English 확인.

## 빌더에게 맡기는 것

- 톱니 SVG 경로, 회전 축 지정 방식.
- `.xcu` 키 구조, Vee 판 표식 방식, 최종 대체 글꼴 선택(표본 비교 근거로).
- 업데이트 잠금의 구체 타입과 위치.
- "확인 중" 문구 내용(en/ko).

## 내 판단(사용자 확인 전 결정)

- 받아 둔 업데이트가 있을 때의 조용한 확인, 재확인 실패 시 받아 둔 버전 설치.
- 설치 대기 중 확인 창 버튼을 "확인 중…"으로. 설정 창의 업데이트 버튼도 똑같이.
- 받아 둔 버전보다 semver로 클 때만 교체.
- LibreOffice 프로필 설정 파일은 매 변환 전에 다시 씀.
- ⚙︎를 핀과 같은 크기·선 굵기의 SVG로.
- 아이콘: 새 의존성 없이 직접 작성, 표준 크기 묶음, 정사각형 아닌 원본은 투명 여백, 작은 원본도 늘림.
- ICO·ICNS를 PNG뿐 아니라 **모든 이미지 원본**에 연다(BMP·TIFF 자리를 대신한다는 요청을 그렇게 읽음).
- 글꼴 대체표 대상 목록과 macOS 대체 글꼴 1차 후보.
