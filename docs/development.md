# 개발 환경 구성과 빌드

## 필요한 도구

| 도구 | 버전 | 비고 |
|---|---|---|
| Rust | stable 1.85 이상 (edition 2024) | [rustup](https://rustup.rs) 으로 설치 |
| Node.js | 24 | `.mise.toml`에 고정 |
| Yarn | 4.18.0 | `.mise.toml`에 고정, `nodeLinker: node-modules` |
| [mise](https://mise.jdx.dev) | 최신 | 권장. Node/Yarn 버전을 자동으로 맞춤 |

OS별 추가 준비:

- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Windows**: [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)의 "C++를 사용한 데스크톱 개발" 워크로드, WebView2 런타임(Windows 10/11에는 기본 설치)

## 처음 한 번

```bash
git clone git@github.com:bob-park/vee-app.git
cd vee-app
mise install      # Node 24, Yarn 4.18 설치 (mise를 쓰는 경우)
yarn install
```

> 이 레포의 yarn은 공개된 지 하루가 안 된 패키지를 설치하지 않습니다(`npmMinimalAgeGate`). 막 나온 버전을 추가할 때 "quarantined" 오류가 나면 한 단계 낮은 버전을 지정하세요.

## 개발 실행

```bash
yarn tauri dev
```

- Vite 개발 서버(`http://localhost:1420`)와 Rust 앱이 함께 뜹니다. 프론트 코드는 즉시 반영되고, Rust나 `tauri.conf.json`을 바꾸면 앱이 다시 빌드·재시작됩니다.
- 창은 처음에 모두 숨겨져 있습니다. `Cmd/Ctrl+Shift+V`로 패널을, 트레이 아이콘 우클릭 → 설정으로 설정 창을 엽니다.
- 웹 개발자 도구: 패널이나 설정 창에서 우클릭 → 검사
- **트레이 상주 앱이라 창을 닫아도 프로세스가 남습니다.** `yarn tauri dev`를 다시 실행하기 전에 트레이 아이콘 → **종료**를 하세요. 남아 있으면 Windows에서는 `EBUSY: resource busy or locked`가 나고, 중복 실행 방지 때문에 새 인스턴스는 설정 창만 열고 꺼집니다.
- 개발 빌드는 자동 업데이트 확인을 하지 않습니다(6시간 주기 확인은 release 빌드에서만).

## 테스트와 검사

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib   # Rust 단위 테스트
yarn test                                               # 프론트 순수 로직 (node:test)
yarn typecheck                                          # TypeScript 타입 검사
```

- Rust 테스트는 SQLite 저장소(`store.rs`), 설정 검증, 창 배치 계산 등을 다룹니다.
- 실제 데스크톱이 필요한 스모크 테스트(최전면 앱·아이콘 읽기)는 기본으로 건너뜁니다. 직접 돌리려면:
  ```bash
  cargo test --manifest-path src-tauri/Cargo.toml -- --ignored reads_frontmost_app_and_icon
  ```
- 프론트와 스크립트 테스트는 별도 라이브러리 없이 Node 내장 `node:test`로 `src/**/*.test.ts`, `scripts/*.test.mjs`를 실행합니다.

## 빌드

```bash
yarn tauri build
```

- 결과물: `src-tauri/target/release/bundle/` (macOS `.app`/`.dmg`, Windows NSIS `-setup.exe`)
- `createUpdaterArtifacts`가 켜져 있어서 업데이트용 서명 키가 필요합니다. 서명 없이 확인만 할 때는 번들 없이 빌드하세요.
  ```bash
  yarn tauri build --no-bundle
  ```
- 서명·공증·GitHub 릴리스 업로드는 [릴리스 절차](release.md)를 따릅니다(`scripts/release.sh`, `scripts/release.ps1`, `scripts/latest-json.mjs`).

## 프로젝트 구조

```
vee-app/
├─ src/                    # React 프론트엔드 (창 라벨 panel / settings / toast로 화면 분기)
│  ├─ main.tsx             # 진입점
│  ├─ api.ts               # Rust command 호출 래퍼와 타입
│  ├─ prefs.tsx            # 설정 로드, 테마 적용, 언어 사전 제공
│  ├─ theme.css            # 디자인 토큰 (docs/design/kraken-design.md 기반)
│  ├─ i18n/                # en.ts(기준), ko.ts(typeof en → 키 누락 시 컴파일 에러)
│  ├─ panel/               # 하단 카드 패널, 키보드 처리(keys.ts)
│  ├─ settings/            # 설정 창, 단축키 녹화(accelerator.ts)
│  └─ toast/               # "복사됨" 알림
├─ src-tauri/
│  ├─ tauri.conf.json      # 창 3개, 번들, 업데이터 공개키
│  ├─ capabilities/        # 웹뷰 권한 (core:default, dialog:default)
│  └─ src/
│     ├─ lib.rs            # 앱 셋업, 공용 상태, 클립 command
│     ├─ store.rs          # SQLite 저장·검색 (Tauri 비의존)
│     ├─ watcher.rs        # 클립보드 감시
│     ├─ source_app.rs     # 최전면 앱·아이콘·효과음 (macOS/Windows)
│     ├─ windows.rs        # 패널·toast·설정 창 배치와 복사
│     ├─ settings.rs       # 설정 command, 전역 단축키
│     ├─ tray.rs           # 트레이 아이콘·메뉴
│     └─ updater.rs        # 업데이트 확인·설치
├─ scripts/                # 로컬 릴리스 스크립트, latest.json 생성(latest-json.mjs)
└─ docs/                   # 설계·계획·릴리스·개발 문서
```

## 데이터 초기화

개발 중 히스토리를 비우려면 설정 창의 **히스토리 전체 삭제**를 쓰세요. DB를 처음부터 다시 만들고 싶다면 앱을 종료한 뒤 아래 폴더를 지웁니다(설정과 히스토리가 모두 사라집니다).

- macOS: `~/Library/Application Support/com.bobpark.vee/`
- Windows: `%APPDATA%\com.bobpark.vee\`

DB 스키마는 `PRAGMA user_version`으로 관리합니다. 스키마를 바꿀 때는 `store.rs`에 다음 버전 마이그레이션을 추가하세요.
