# CLAUDE.md

@AGENTS.md

## Claude Code 전용 규칙

### Plan Mode

- 큰 변경(파일 3개 이상 / 새 기능 / 새 도메인 / 마이그레이션)은 plan mode로 진행한다.

### Skill Usage

- 가능하면 `superpowers` 계열 스킬을 우선 사용한다.
- 새 기능과 리팩토링은 `superpowers:brainstorming` → `superpowers:writing-plans` → `superpowers:executing-plans` 순서로 진행한다.
- 디버깅: `superpowers:systematic-debugging`
- 완료 직전 검증: `superpowers:verification-before-completion`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `yarn test`, `yarn typecheck`, `yarn lint`를 모두 통과해야 한다.
- 라이브러리 API(Tauri, React Query, daisyUI 등)는 기억에 의존하지 않고 최신 문서를 확인한다.

### UI 작업

- UI를 만들거나 바꾸기 전에 반드시 `docs/design/`과 [design](docs/agents/design.md)을 먼저 읽는다.
- 화면을 바꾸면 `yarn tauri dev`로 띄워서 직접 본다(설치된 Vee가 있으면 별도 identifier로, [build-run](docs/agents/build-run.md) 참고). 라이트/다크 두 테마와 한국어/English 두 언어를 모두 확인한다.

### Language

- 코드 본문(식별자, 주석)과 `docs/agents/`는 영어로 쓴다.
- 사용자에게 보이는 문구는 반드시 `src/shared/i18n/{en,ko}.ts`에 둔다. 컴포넌트에 문자열을 하드코딩하지 않는다.
- 사람용 문서(`docs/development.md`, `docs/release.md`, README, 설계 문서)는 한국어로 써도 된다.

### Convention Enforcement

- 코드를 바꿀 때는 아래 규칙을 반드시 따른다.
  - `docs/agents/conventions/`: rust-modules / tauri-commands / sqlite-migrations / error-handling / module-aliases / naming / typescript / react-sections
  - `docs/agents/libs/`: react-query / zustand-slice / tailwind-daisyui / theme / i18n
- 브랜치·커밋·PR·버전 규칙은 `docs/agents/workflow/`(branching / commit-messages / pr / versioning / release)를 따른다.
- 규칙을 어긴 기존 코드를 발견하면 같은 PR에서 고치지 말고 별도 메모로 남긴다(scope creep 방지).

### Dependency Changes

- `package.json`이나 `src-tauri/Cargo.toml`의 의존성을 추가·제거하거나 버전을 바꿀 때는 반드시 사용자에게 먼저 확인한다.

### Destructive & Outward Operations

- 다음 작업은 사용자가 명시적으로 승인하기 전에는 절대 실행하지 않는다.
  - `git reset --hard`, `git push --force`, 파일 대량 삭제, 브랜치 삭제
  - 릴리스 스크립트(`scripts/release.*`, `scripts/latest-json.mjs`), `gh release` 명령
  - 배포된 마이그레이션 파일 수정
  - 설치된 Vee의 데이터 폴더(`com.bobpark.vee`)를 지우거나 고치는 작업
- `bump-version-up.sh`는 GitHub Actions가 실행한다(`master`로 머지될 때마다). 로컬에서 버전 파일을 직접 고치지 않는다.
