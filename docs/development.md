# 개발 환경 구성과 빌드

## 필요한 도구

| 도구 | 버전 | 비고 |
|---|---|---|
| Rust | stable 1.85 이상 (edition 2024) | [rustup](https://rustup.rs) 으로 설치 |
| Node.js | 26 | `.mise.toml`에 고정 |
| Yarn | 4.18.1 | `.mise.toml`·`package.json#packageManager`에 고정, `nodeLinker: node-modules` |
| [mise](https://mise.jdx.dev) | 최신 | 권장. Node/Yarn 버전을 자동으로 맞춤 |

OS별 추가 준비:

- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Windows**: [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)의 "C++를 사용한 데스크톱 개발" 워크로드, WebView2 런타임(Windows 10/11에는 기본 설치)

## 처음 한 번

### GitHub Packages 토큰 (`GITHUB_NPM_AUTH_TOKEN`)

ESLint/Prettier 설정(`@bob-park/eslint-config-bobpark`, `@bob-park/prettier-config-bobpark`)은 GitHub Packages에서 받습니다. 레포는 공개지만 이 패키지를 받으려면 토큰이 필요해서, 토큰 없이는 `yarn install`이 실패합니다(외부 기여자도 자기 토큰을 만들어야 합니다).

1. GitHub → Settings → Developer settings → Personal access tokens (classic)에서 `read:packages` 권한만 준 토큰을 만듭니다.
2. 환경 변수로 등록합니다. `.yarnrc.yml`이 이 값을 읽습니다. 토큰을 레포에 커밋하지 마세요.
   - macOS: 셸 프로필(`~/.zshrc` 등)에 추가하고 새 터미널을 엽니다.
     ```bash
     export GITHUB_NPM_AUTH_TOKEN=ghp_xxx
     ```
   - Windows: 한 번 실행한 뒤 새 터미널을 엽니다.
     ```powershell
     setx GITHUB_NPM_AUTH_TOKEN ghp_xxx
     ```

### 설치

```bash
git clone git@github.com:bob-park/vee-app.git
cd vee-app
mise install      # Node 26, Yarn 4.18.1 설치 (mise를 쓰는 경우)
yarn install
```

> yarn은 반드시 4.18.1로 실행합니다. mise가 활성화되지 않아 전역 yarn 1.x가 잡히면 `yarn.lock`이 Yarn 1 형식으로 통째로 바뀝니다. `yarn --version`으로 확인하고, 4.x가 아니면 `corepack yarn@4.18.1 <명령>`으로 실행하세요.

## 개발 실행

```bash
yarn tauri dev
```

- Vite 개발 서버(`http://localhost:1420`)와 Rust 앱이 함께 뜹니다. 프론트 코드는 즉시 반영되고, Rust나 `tauri.conf.json`을 바꾸면 앱이 다시 빌드·재시작됩니다.
- 창은 처음에 모두 숨겨져 있습니다. `Cmd/Ctrl+Shift+V`로 패널을, 트레이 아이콘 클릭 → 설정으로 설정 창을 엽니다.
- **설치된 Vee와 함께 띄울 때**는 identifier가 같아서 데이터 폴더와 단축키가 겹칩니다. 개발 빌드는 별도 identifier로 띄우고, 개발 쪽 패널 단축키를 설정에서 다른 키로 바꾸세요.
  ```bash
  yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'
  ```
- 개발 중에도 클립보드 감시는 실제로 동작합니다. 복사하는 내용은 띄워 둔 모든 Vee(개발·설치본)에 기록됩니다.
- React Query 개발 도구 버튼은 개발 빌드의 설정 창 오른쪽 아래에만 보입니다.
- 웹 개발자 도구: 패널이나 설정 창에서 우클릭 → 검사
- **트레이 상주 앱이라 창을 닫아도 프로세스가 남습니다.** `yarn tauri dev`를 다시 실행하기 전에 트레이 아이콘 → **종료**를 하세요. 남아 있으면 Windows에서는 `EBUSY: resource busy or locked`가 나고, 중복 실행 방지 때문에 새 인스턴스는 설정 창만 열고 꺼집니다.
- 개발 빌드는 자동 업데이트 확인을 하지 않습니다(6시간 주기 확인은 release 빌드에서만).

## 테스트와 검사

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib   # Rust 단위 테스트
yarn test                                               # 프론트 순수 로직 (node:test)
yarn typecheck                                          # TypeScript 타입 검사
yarn lint                                               # ESLint (bobpark 설정)
yarn prettier                                           # src/ 포맷 (파일을 직접 고침)
```

- 변경을 끝내기 전에 위 네 가지(cargo test, test, typecheck, lint)가 모두 통과해야 합니다.
- Rust 테스트는 도메인별 저장소·검증(`src-tauri/src/domain/*`), DB 마이그레이션과 이전 버전 DB 업그레이드(`shared/db.rs`), `AppError` 직렬화, 드래그 이미지·클립보드 감시 같은 순수 로직을 다룹니다.
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

## 브랜치와 버전

- 브랜치는 `master` 하나입니다. 작업은 `master`에서 `feature/<주제>`로 분기해 `master`로 PR을 냅니다. `develop`·`hotfix` 브랜치는 없습니다.
- PR이 머지될 때마다 GitHub Actions(`.github/workflows/bump-version.yml`)가 `bump-version-up.sh`로 버전을 올리고 `chore: bump version to X.Y.Z [skip ci]`를 커밋합니다. PR 제목이나 라벨에 `[major]`/`[minor]`가 있으면 그만큼, 없으면 patch를 올립니다.
- 버전 파일(`tauri.conf.json`, `package.json`, `Cargo.toml`, `Cargo.lock`)을 손으로 고치지 마세요. 저장소 설정의 Actions 권한이 **Read and write**여야 합니다.

## 프로젝트 구조

창 라벨(`panel` / `settings` / `toast` / `whats-new` / `convert`)마다 화면이 하나씩 있고, 프론트와 Rust가 같은 이름의 도메인(`clip`, `source_app`, `settings`, `whats_new`, `convert`)으로 나뉩니다. 자세한 규칙은 [`AGENTS.md`](../AGENTS.md)와 [`docs/agents/structure.md`](agents/structure.md)에 있습니다.

```
vee-app/
├─ src/                          # React 프론트엔드
│  ├─ app/
│  │  ├─ main.tsx                # 진입점: 창 라벨로 화면 선택, 첫 화면 전에 설정 로드
│  │  ├─ globals.css             # Tailwind + daisyUI 테마 + vee 토큰 (유일한 CSS, 색은 여기에만)
│  │  ├─ panel/                  # 하단 카드 패널 (PanelApp, _components/)
│  │  ├─ settings/               # 설정 창 (SettingsApp, _components/)
│  │  ├─ toast/                  # "복사됨" 알림
│  │  ├─ whats-new/              # 새 소식 창
│  │  └─ convert/                # 드래그 변환 포맷 도넛
│  ├─ domain/<이름>/             # clip, source_app, settings, whats_new, updater, panel, convert
│  │  ├─ apis/                   # Rust command 호출 함수 + DTO 타입
│  │  ├─ queries/                # React Query 훅
│  │  ├─ components/             # 도메인 컴포넌트
│  │  └─ store/                  # Zustand slice (clip: 패널 검색어·필터·앱 태그)
│  ├─ shared/                    # api(call), components, i18n(en.ts 기준, ko.ts), providers(테마·언어), queries, store
│  └─ utils/                     # 순수 함수 + *.test.ts
├─ src-tauri/
│  ├─ tauri.conf.json            # 창 5개, 번들, 업데이터 공개키
│  ├─ Info.plist                 # macOS: Finder 자동화 권한 문구 (NSAppleEventsUsageDescription)
│  ├─ Entitlements.plist         # macOS: Apple Events 엔타이틀먼트
│  ├─ capabilities/              # 웹뷰 권한 (core:default, dialog:default)
│  ├─ vendor/drag/               # 수정한 drag 크레이트
│  └─ src/
│     ├─ lib.rs                  # 플러그인, 셋업, 창 이벤트, command 등록
│     ├─ app/                    # 창·트레이·업데이터·클립보드 감시·단축키·효과음·최전면 앱·디스크 접근·자동 실행·보관 기간 정리·드래그 변환(convert)·커서 밑 폴더 찾기(drop_target)
│     ├─ domain/                 # clip, source_app, settings, whats_new (mod·model·repository·commands), convert(포맷·작업·이미지 엔진)
│     └─ shared/                 # db.rs, error.rs(AppError), png.rs, migrations/000N_*.sql
├─ scripts/                      # 로컬 릴리스 스크립트, latest.json 생성, 효과음 생성
├─ bump-version-up.sh            # 버전 올리기 (GitHub Actions가 실행)
├─ .github/workflows/            # bump-version.yml: master 머지마다 버전 올림
└─ docs/                         # agents/(에이전트 규칙), design/, 릴리스·개발 문서, superpowers/(설계·계획)
```

## 데이터 초기화

개발 중 히스토리를 비우려면 설정 창의 **히스토리 전체 삭제**를 쓰세요. DB를 처음부터 다시 만들고 싶다면 앱을 종료한 뒤 아래 폴더를 지웁니다(설정과 히스토리가 모두 사라집니다).

- macOS: `~/Library/Application Support/com.bobpark.vee/`
- Windows: `%APPDATA%\com.bobpark.vee\`

DB 스키마는 `PRAGMA user_version`으로 관리합니다. 스키마를 바꿀 때는 `src-tauri/src/shared/migrations/`에 다음 번호의 SQL 파일을 추가하고 `shared/db.rs`의 `MIGRATIONS`에 붙이세요. 이미 배포된 마이그레이션 파일은 절대 고치지 않습니다. 앱보다 새 버전의 DB도 그대로 열립니다(이전 버전으로 되돌려 설치해도 기록이 남음). 자세한 규칙은 [sqlite-migrations](agents/conventions/sqlite-migrations.md)를 보세요.
