# 템플릿 구조 적용 설계

- 날짜: 2026-10-16
- 브랜치: `feature/apply-template` (master에서 분기)
- 템플릿: `/Users/hwpark/Documents/rust-workspace/template-rust-app`
- 디자인 기준: `docs/design/kraken-design.md` (두 프로젝트에 같은 파일이 있음)

## 왜

vee-app을 템플릿 프로젝트와 같은 모양으로 맞춘다. 앞으로 에이전트(Claude, 기타)가 같은 규칙 문서(`AGENTS.md`, `docs/agents/**`)를 보고 두 프로젝트를 똑같이 다룰 수 있게 하는 것이 목적이다. 그리고 버전 올리기를 손으로 하지 않고 GitHub Actions가 하게 한다.

## 사용자가 정한 것

1. 새 브랜치에서 작업한다.
2. 버전 자동 올림 워크플로의 기준 브랜치는 `develop`이 아니라 `master`다.
3. 템플릿의 `docs`, `AGENTS.md`, `CLAUDE.md`를 반영한다.
4. 템플릿이 쓰는 라이브러리와 디렉터리 구조를 적용한다. 단, vee-app이 쓰지 않는 라이브러리는 넣지 않는다.
5. **프론트엔드 CSS를 Tailwind로 전부 다시 쓴다.** 템플릿처럼 바꾼다.
6. **화면은 지금과 똑같이 보여야 한다.** daisyUI 기본 모양으로 바뀌면 안 된다.
7. **Rust도 템플릿처럼** `app/ · domain/ · shared/` 구조로 바꾼다.

## 목표

- 프론트엔드: `src/{app,domain/<name>/{apis,queries,components,store},shared,utils}` 구조. Tailwind 4 + daisyUI 5, React Query, Zustand(+immer), classnames, ESLint/Prettier.
- Rust: `src-tauri/src/{lib.rs,app/,domain/<name>/{mod,model,repository,commands}.rs,shared/}` 구조. `thiserror` 기반 `AppError`, 커맨드 에러는 `{ code, message }`.
- 문서: `AGENTS.md`, `CLAUDE.md`, `docs/agents/**`를 vee-app 실제 모습에 맞게 고쳐서 넣는다.
- CI: `.github/workflows/bump-version.yml` + `bump-version-up.sh`. `master` 머지 때 버전이 오른다.

## 하지 않는 것

- 기능 추가, 동작 변경, 버그 수정. 리팩터링 중 발견한 버그나 규칙 위반은 고치지 말고 메모로 남긴다(템플릿 `CLAUDE.md` 규칙).
- `develop` 브랜치 만들기, git-flow 도입.
- 템플릿에만 있고 vee-app이 쓰지 않는 기능 이식: 예를 들어 템플릿의 샘플 `Greeting`, 앱 메뉴(`menu.rs`) 등.
- vee-app 고유 의존성 제거(global-shortcut, dialog, drag(벤더링), clipboard-rs, sha2, base64, objc2, windows, image 등은 그대로).
- 실패로 보는 것: 화면이 조금이라도 달라 보이는 것, 기존 동작(단축키, 드래그, 애니메이션, 창 위치, 업데이트, 새 소식)이 바뀌는 것, 기존 사용자의 DB/설정이 깨지는 것.

## 제약

- 기존 사용자 데이터 호환: SQLite 스키마와 설정 키는 그대로 둔다. 템플릿의 `shared/migrations` 방식을 쓰더라도 지금 DB를 그대로 열 수 있어야 한다(새 마이그레이션으로 스키마를 바꾸지 않는다).
- Tauri 커맨드 이름, 이벤트 이름(`settings://changed` 등), 창 label(`panel`, `settings`, `toast`, `whats-new`)은 바꾸지 않는다. 바꾸면 capabilities와 업데이트 경로가 꼬일 위험이 있다.
- macOS·Windows 모두 동작해야 한다.
- 완료 조건: `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint` 전부 통과. 그리고 `yarn tauri dev`로 띄워 라이트/다크 × 한국어/English 네 조합에서 패널·설정·토스트·새 소식 창을 지금(master) 화면과 나란히 비교해 같은지 확인한다.

## 부분별 설계

### 1. 버전 자동 올림 (CI)

- 템플릿의 `.github/workflows/bump-version.yml`, `bump-version-up.sh`, `scripts/bump-version-up.test.mjs`를 가져온다.
- 트리거는 `push: branches: [master]` 하나. 템플릿의 `develop`/`main`/`hotfix` 분기 규칙은 없앤다. master에 머지될 때마다 PR 제목/라벨(`[major]`, `[minor]`, 없으면 patch)로 올린다.
- `bump-version-up.sh`의 `CRATE_NAME`은 `src-tauri/Cargo.toml`의 `[package] name`과 같게 한다. 이 값은 실제 파일에서 확인한다.
- 이 브랜치 자체를 master에 머지하면 첫 자동 bump가 일어난다(0.8.1 → 0.8.2 또는 PR 제목에 따라). 이 동작은 의도된 것이다.
- `docs/release.md`의 1단계("버전을 올리고 커밋·푸시")는 "Actions가 올린 버전 확인"으로 바꾼다.
- (제 판단) `scripts/release.sh`/`release.ps1`에 템플릿이 가진 "이미 게시된 릴리스 덮어쓰기 방지" 검사를 옮긴다. `--target main`은 넣지 않는다(기본 브랜치가 master).

### 2. 문서

- `AGENTS.md`, `CLAUDE.md`, `docs/agents/**` 전부를 가져와 vee-app에 맞게 고친다.
  - 앱 이름·설명(`overview.md`): 클립보드 기록 앱 Vee. 창 4개(panel, settings, toast, whats-new). 트레이 상주.
  - `structure.md`, `architecture.md`: vee-app의 실제 도메인 이름과 트리.
  - `tech-stack.md`: vee-app 고유 의존성 추가, TypeScript 버전(아래 참고).
  - `workflow/branching.md`, `versioning.md`, `release.md`, `pr.md`: `main`/`develop` → `master` 하나. `feature/<topic>`은 master에서 분기해 master로 PR. hotfix 규칙은 없앤다.
  - `CLAUDE.md` 마지막 줄의 bump 설명을 master 기준으로 바꾼다.
  - 템플릿에만 해당하는 내용(placeholder 이름 바꾸기 안내 등)은 뺀다.
- `docs/development.md`, `docs/release.md`는 기존 vee-app 내용을 유지하면서 템플릿의 구성과 새 명령(`yarn lint`, `yarn prettier`)을 반영한다.
- `docs/design/kraken-design.md`는 이미 같다. 그대로 둔다.
- `docs/superpowers/**`(기존 설계/계획)와 `docs/images/`는 그대로 둔다.
- `README.md`는 디렉터리 경로가 바뀐 곳만 고친다.

### 3. 라이브러리

추가(템플릿과 같은 버전 범위):

- dependencies: `@tanstack/react-query`, `classnames`, `immer`, `zustand`
- devDependencies: `@tailwindcss/vite`, `tailwindcss`, `daisyui`, `@tanstack/react-query-devtools`, `eslint`, `prettier`, `@bob-park/eslint-config-bobpark`, `@bob-park/prettier-config-bobpark`
- Cargo: `thiserror = "2"`
- 설정 파일: `eslint.config.mjs`, `package.json`의 `"prettier"` 키, `lint`/`prettier` 스크립트, `.yarnrc.yml`의 GitHub Packages 설정(bobpark 패키지용), `vite.config.ts`에 Tailwind 플러그인.

TypeScript: vee-app은 `^7.0.2`, 템플릿은 `^6`이다. 템플릿 문서에 따르면 bobpark ESLint 설정이 쓰는 `typescript-eslint`가 TS 7을 아직 지원하지 않는다. **(제 판단) 템플릿을 따라 `^6`으로 내린다.** 설치 시점에 `typescript-eslint`가 TS 7을 지원하면 7을 유지한다.

bobpark 패키지(사용자 결정): 템플릿 그대로 GitHub Packages에서 받는다. vee-app은 공개 저장소라 `GITHUB_NPM_AUTH_TOKEN`(read:packages 권한) 없이는 `yarn install`이 실패한다. 이 점과 토큰 설정 방법(macOS·Windows 모두)을 `docs/development.md`와 `docs/release.md` 준비 항목에 적는다. `.yarnrc.yml`은 템플릿의 `npmRegistries`/`npmScopes`를 가져오되, 그 밖의 키(`enableScripts`, `npmMinimalAgeGate`, `approvedGitRepositories`)도 템플릿과 맞춘다.

넣지 않는 것: 템플릿에 있지만 vee-app에 해당 기능이 없는 것. 현재 비교 결과 위 목록 외에는 차이가 없다. 모든 의존성 변경은 이 문서 승인으로 승인된 것으로 본다. 이 목록 밖의 변경은 따로 묻는다.

### 4. 프론트엔드 구조

현재(`src/`): `api.ts`, `prefs.tsx`, `theme.css`, `i18n/`, `panel/`, `settings/`, `toast/`, `whatsnew/`.

목표 매핑(초안, 정확한 파일 단위는 계획 단계에서 정한다):

| 지금 | 옮길 곳 |
| ---- | ------- |
| `main.tsx` | `app/main.tsx` (window label로 화면 선택, provider 장착) |
| `panel/Panel.tsx` + 하위 컴포넌트 | `app/panel/PanelApp.tsx` + `app/panel/_components/` 또는 `domain/clip/components/` |
| `settings/Settings.tsx` 등 | `app/settings/SettingsApp.tsx` + `_components/` |
| `toast/`, `whatsnew/` | `app/toast/`, `app/whats-new/` |
| `api.ts` | 도메인별 `domain/<name>/apis/<name>.ts` + `.dto.ts`, 공통 `shared/api/` |
| `prefs.tsx` | React Query 설정 쿼리 + `shared/providers/{theme,i18n}` |
| `i18n/` | `shared/i18n/` |
| 순수 함수(`keys.ts`, `motion.ts`, `selection.ts`, `fileThumb.ts`, `accelerator.ts`, `dropdownKeys.ts`, `notes.ts` 등)와 테스트 | `utils/` (React/DOM/IPC에 닿는 것은 제외) |
| `theme.css`, `*.css` | `app/globals.css` 하나 + Tailwind 클래스 |

도메인 후보(Rust와 같은 이름): `clip`(기록, 고정, 드래그, 복사), `app_source`/`source_app`(앱 목록·제외), `settings`, `updater`, `whats_new`. 이름은 계획 단계에서 Rust와 맞춰 확정한다.

상태:

- 서버 상태(설정, 클립 목록, 앱 목록, 통계, 업데이트 상태)는 React Query. 창이 여러 개라 Rust 이벤트(`settings://changed` 등)를 받으면 해당 쿼리를 무효화한다(템플릿 `docs/agents/libs/react-query.md` 패턴).
- 클라이언트 UI 상태(패널 검색어·필터 유지, 선택 등)는 Zustand slice. 단, 지금 동작이 바뀌면 안 된다. 예를 들어 "검색·필터를 열 때마다 유지"는 그대로다.

### 5. 스타일 (Tailwind 다시 쓰기)

- 지금 CSS 약 1,500줄(`panel.css` 706, `settings.css` 407, `theme.css` 172, `toast.css` 136, `whatsnew.css` 80)을 Tailwind 클래스로 옮긴다.
- 색·간격·반경·그림자 등 지금 `theme.css`에 있는 토큰은 `app/globals.css`의 daisyUI 테마(light/dark) 변수와 `@theme` 토큰으로 옮긴다. **값은 지금과 똑같이** 한다. 색을 정의하는 곳은 `globals.css` 하나뿐이다.
- daisyUI 컴포넌트 클래스는 겉모습이 지금과 같게 만들 수 있을 때만 쓴다. 같게 안 되면 Tailwind 유틸리티로 직접 짠다.
- Tailwind로 표현하기 어려운 것(키프레임 애니메이션, `::-webkit-scrollbar`, 창별 `data-window` 스타일 등)은 `globals.css`의 `@layer`/`@keyframes`에 남겨도 된다. 목표는 "컴포넌트별 CSS 파일 0개"다.
- 템플릿의 `shared/components/settings/*`(SettingRow, Switch, Segmented 등)는 vee-app 설정 화면의 모양과 맞을 때만 재사용하고, 다르면 vee-app 모양을 기준으로 만든다.

### 6. Rust 구조

현재(`src-tauri/src/`): `lib.rs`(커맨드 다수 + AppState), `store.rs`(1,166줄, SQLite 전부), `settings.rs`, `windows.rs`, `tray.rs`, `updater.rs`, `watcher.rs`, `sound.rs`, `source_app.rs`, `disk_access.rs`, `whats_new.rs`.

목표:

- `lib.rs`: 플러그인, setup, `generate_handler!`만.
- `app/`: 도메인이 아닌 셸. `state.rs`(AppState), `window.rs`(지금 `windows.rs`), `tray.rs`, `updater.rs`, `autostart.rs`, `i18n.rs`(트레이 라벨/locale), `shortcut.rs`(전역 단축키), `disk_access.rs`, `sound.rs`, `watcher.rs`(클립보드 감시) 등.
- `domain/<name>/{mod,model,repository,commands}.rs`: `clip`, `source_app`, `settings`, `whats_new` 등. `store.rs`의 SQL은 도메인별 `repository.rs`로 나눈다.
- `shared/`: `db.rs`(연결 열기, 마이그레이션), `error.rs`(`AppError` + `serde` 직렬화로 `{ code, message }`).
- 커맨드 반환 타입 `Result<_, String>` → `Result<_, AppError>`. 프론트엔드 `call()`이 `{ code, message }`를 받도록 같이 바꾼다. 사용자에게 보이는 에러 문구는 지금과 같게 유지한다.
- 기존 Rust 테스트는 모두 옮겨서 통과시킨다.
- DB: `store.rs`의 인라인 V1~V5 SQL은 `shared/migrations/000N_*.sql` 파일로 옮긴다(SQL 내용은 그대로, `user_version` 번호도 그대로). 템플릿의 `SchemaTooNew`(더 새 DB 거부)는 **가져오지 않는다**. 지금처럼 더 새 DB도 열어야 다운그레이드 재설치가 깨지지 않는다(동작 변경 금지 원칙).

## 빌더에게 맡기는 것

- 정확한 파일 분할과 도메인 이름(위 표는 초안). Rust와 프론트엔드 도메인 이름은 같아야 한다.
- 커밋 순서와 단위. 단, 각 커밋 시점에 빌드와 테스트가 통과해야 한다.
- 화면 비교 방법(스크린샷 나란히 비교 등).

## 제가 정한 것 (확인 필요)

- 브랜치 이름 `feature/apply-template`.
- master 머지마다 bump. hotfix 규칙 없음.
- TypeScript `^6`으로 내림(ESLint 호환).
- 릴리스 스크립트에 "게시된 릴리스 덮어쓰기 방지" 검사 이식.
- 이 작업 전체를 PR 하나로 낸다.
- "똑같이 보임"의 기준은 master와 이 브랜치를 같은 창 크기로 스크린샷 찍어 나란히 놓고 눈으로 비교해 차이가 없는 것. 픽셀 diff 도구는 만들지 않는다. 단 Tailwind preflight·daisyUI 기본값(폰트, focus ring, `color-scheme`, 스크롤바)이 지금 모양을 바꾸는 곳은 모두 되돌린다.
- DB `SchemaTooNew` 미도입(위 6절).
