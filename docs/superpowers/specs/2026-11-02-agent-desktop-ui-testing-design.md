# agent-desktop로 UI/UX 직접 테스트 — 설계 (small change)

## 의도

에이전트가 UI 변경을 "타입체크 통과"로 끝내지 않고, 실행 중인 Vee를 `agent-desktop`(https://github.com/vercel-labs/agent-desktop)으로 직접 조작해 확인하게 한다. 규칙을 AGENTS.md에 두고, 실제로 그 방식으로 한 번 테스트해 동작을 검증한다.

## 목표 / 비목표

- 목표: AGENTS.md에 agent-desktop 기반 UI 검증 규칙 추가. 패널 기본 흐름 + 테마/언어 전환을 실제로 agent-desktop으로 테스트.
- 비목표: 자동화된 E2E 테스트 스위트, CI 연동, 앱 코드 변경, 의존성 추가(agent-desktop은 전역 CLI, 이미 설치됨).
- 안티골: 설치된 Vee(`com.bobpark.vee`) 데이터나 단축키를 건드림. 사용자 허락 없이 `--headed`(실제 마우스/키 입력) 사용.

## 접근 방식

1. AGENTS.md 한 줄 + `testing.md` 섹션 — 기존 "AGENTS.md는 지도" 구조에 맞음.
2. 새 topic 문서 `docs/agents/ui-testing.md` + Map 항목.
3. **전부 AGENTS.md에 직접 작성 — 사용자 선택.** 에이전트가 첫 파일에서 바로 보게 됨. AGENTS.md가 길어지는 대가.

## 결정된 내용

- 작업 브랜치: 현재 `feature/agent-desktop`.
- AGENTS.md에 `## UI testing with agent-desktop` 섹션 추가(영어), "Always"에 한 줄 포인터. 내용:
  - 설치/권한 확인: `npm install -g agent-desktop`(mise Node), `agent-desktop permissions`; 처음 쓰기 전 `agent-desktop skills get desktop` / `skills get platform`.
  - dev 실행: `yarn tauri dev --config '{"identifier":"com.bobpark.vee.dev"}'`(설치된 Vee와 분리).
  - 루프: `snapshot -i` → ref로 act → 다시 snapshot/`get`/`is`로 확인. 성공 코드는 효과를 증명하지 않음.
  - 패널 열기: 트레이(`--surface menubar`)로 headless 우선. 단축키 등 실제 입력은 `--headed`이며 사용자 허락 필요.
  - 테스트 데이터: `clipboard-set`은 실제 클립보드 → 실행 중인 모든 Vee에 기록됨.
  - 점검 범위: 패널 기본 흐름, 라이트/다크 × 한국어/English, `screenshot`으로 시각 확인.
- 실제 테스트: 위 절차대로 패널 열기 → 항목 표시/선택, Settings에서 테마·언어 4조합 확인. 결과(통과/막힌 지점)를 보고하고, 막힌 지점에서 배운 것을 문서에 반영.

## 빌더 재량

- 섹션 문구와 명령 예시의 정확한 형태(실제 테스트에서 동작 확인된 명령만 문서에 남김).
- 패널이 headless로 안 열리면 그 시점에 `--headed` 허락을 사용자에게 요청.
