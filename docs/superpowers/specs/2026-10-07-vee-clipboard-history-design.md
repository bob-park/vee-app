# Vee — 클립보드 히스토리 앱 설계

- 작성일: 2026-10-07
- 브랜치: `feature/paste`
- 참고: macOS 앱 [Paste](https://pasteapp.io), `docs/design/kraken-design.md`
- 브레인스토밍 시안: `.superpowers/brainstorm/` (B 시안, 테마/i18n, 복사 표시 C)

## 1. 목표와 범위

사용자가 복사한 텍스트·이미지·파일을 SQLite에 기록하고, 단축키로 화면 하단에 카드형 패널을 띄워 검색·선택해 다시 클립보드로 복사하는 데스크톱 앱. macOS와 Windows를 지원한다.

**성공 기준**
- 단축키 → 1초 안에 하단 패널 표시
- 카드 선택(더블클릭/Enter) → 클립보드에 복사되고 "복사됨" 토스트 표시
- 재부팅 후에도 히스토리 유지, 검색 가능

**포함**
- 클립보드 히스토리(텍스트/링크/이미지/파일), 출처 앱 아이콘, 검색·타입 필터
- 시스템 트레이 상주, 로그인 시 자동 실행, GitHub Release 기반 자동 업데이트
- 라이트/다크 테마, 한국어/English, 단축키 변경

**제외 (YAGNI)**
- 자동 붙여넣기(선택 시 직전 앱에 `Cmd/Ctrl+V` 입력) — 복사만 한다. 추후 `copy_clip` 흐름 끝에 키 입력 단계를 추가하는 방식으로 확장 가능
- 핀보드("유용한 링크" 등 사용자 정의 보드)
- Linux 지원, Windows 코드 서명
- 동기화/클라우드

## 2. 기술 스택

- Tauri v2 (Rust) + React + TypeScript + Vite, 패키지 매니저 yarn 4 (`.mise.toml`의 node 24)
- Rust 크레이트: `tauri` 2, `tauri-plugin-global-shortcut`, `tauri-plugin-autostart`, `tauri-plugin-updater`, `tauri-plugin-single-instance`, `tauri-plugin-log`, `clipboard-rs`, `rusqlite`(feature `bundled`), `sha2`, `image`(썸네일), macOS `objc2-app-kit`, Windows `windows`
- 프론트엔드 의존성: `react`, `react-dom`, `@tauri-apps/api`, 사용 플러그인의 JS 바인딩. i18n·상태관리 라이브러리 없음

## 3. 레포 구조

루트의 기존 `Cargo.toml`, `src/main.rs`(Hello world)는 제거하고 Tauri 표준 레이아웃으로 전환한다. 크레이트 이름 `vee-app` 유지.

```
vee-app/
├─ package.json, vite.config.ts, tsconfig.json, index.html
├─ src/                          # 프론트엔드
│  ├─ main.tsx                   # 창 라벨(panel / settings / toast)로 화면 분기
│  ├─ api.ts                     # invoke/listen 래퍼와 타입
│  ├─ panel/   Panel, Toolbar(검색·필터칩), CardRow, Card
│  ├─ settings/ Settings, ShortcutRecorder
│  ├─ toast/   Toast
│  ├─ i18n/    en.ts, ko.ts(typeof en), useT()
│  └─ theme.css                  # Kraken 토큰 → CSS 변수
└─ src-tauri/
   ├─ Cargo.toml, tauri.conf.json, capabilities/
   └─ src/
      ├─ main.rs / lib.rs        # 셋업, 플러그인·command 등록
      ├─ watcher.rs              # 클립보드 감시·정규화
      ├─ source_app.rs           # 최전면 앱 정보 (#[cfg] macOS / Windows)
      ├─ store.rs                # SQLite
      ├─ windows.rs              # panel/toast 위치·표시·숨김, settings 닫기→숨김
      ├─ settings.rs             # 설정 읽기/쓰기, 단축키 재등록
      ├─ tray.rs                 # 트레이 아이콘·메뉴
      └─ updater.rs              # 주기적 확인·백그라운드 다운로드
```

각 Rust 모듈은 하나의 책임만 가지며, `store`는 Tauri에 의존하지 않아 단독 테스트가 가능해야 한다.

## 4. 창

| 라벨 | 속성 | 역할 |
|---|---|---|
| `panel` | 프레임 없음, 투명, 항상 위, 작업표시줄/Dock 비표시 | 커서가 있는 모니터 하단(작업 영역 기준)에 전체 폭 × 약 300px로 표시 |
| `toast` | 프레임 없음, 투명, 항상 위, `focus: false`, 커서 이벤트 무시 | 패널과 같은 모니터 하단 중앙에 1.5초 표시 |
| `settings` | 일반 창 | 닫기 버튼 → 숨김(앱 종료 아님) |

앱 종료는 트레이 메뉴의 "종료"로만 한다. macOS에서는 Dock 아이콘을 숨기는 accessory 모드로 실행한다.

## 5. 데이터 모델

DB 경로: `<app_data_dir>/vee.db`, WAL 모드. 이미지 원본은 `<app_data_dir>/images/<hash>.png`.

```sql
CREATE TABLE apps (
  id        INTEGER PRIMARY KEY,
  bundle_id TEXT UNIQUE NOT NULL,   -- macOS bundle id / Windows exe 경로
  name      TEXT NOT NULL,
  icon_png  BLOB
);
CREATE TABLE clips (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('text','link','image','files')),
  hash         TEXT UNIQUE NOT NULL,   -- sha256(kind + 내용)
  text         TEXT,                   -- text/link: 원문, files: 줄바꿈 구분 경로 목록
  image_path   TEXT,                   -- image: 원본 PNG 경로
  thumb_png    BLOB,                   -- image: 긴 변 320px 썸네일
  meta         TEXT,                   -- 카드 하단 정보 ("1280×720", 글자 수 등)
  app_id       INTEGER REFERENCES apps(id),
  created_at   INTEGER NOT NULL,       -- unix ms
  last_used_at INTEGER NOT NULL        -- 정렬 기준 (DESC)
);
CREATE INDEX clips_last_used ON clips(last_used_at DESC);
CREATE VIRTUAL TABLE clips_fts USING fts5(
  text, content='clips', content_rowid='id', tokenize='trigram'
);
-- clips_fts 동기화용 INSERT/UPDATE/DELETE 트리거
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

스키마 버전은 `PRAGMA user_version`으로 관리한다.

**규칙**
- `kind = link`: 앞뒤 공백을 제거한 텍스트가 공백 없는 단일 URL(`http(s)://`, `git@`, `ssh://`)일 때
- 중복: 같은 `hash`가 이미 있으면 새 행 없이 `last_used_at`과 `app_id`만 갱신
- 보관 한도: 1,000개. 초과 시 `last_used_at`이 가장 오래된 행부터 삭제하고 해당 이미지 파일도 삭제
- 이미지 50MB 초과는 저장하지 않음
- 검색: 검색어 3글자 이상 → FTS5 trigram `MATCH`, 1~2글자 → `text LIKE '%q%'`. 파일은 경로 텍스트로 검색되며 이미지는 검색 대상이 아님

**settings 키**: `theme` (system|light|dark, 기본 system), `locale` (system|ko|en, 기본 system), `shortcut` (기본 macOS `Cmd+Shift+V`, Windows `Ctrl+Shift+V`). 자동 실행 여부는 `tauri-plugin-autostart` 상태를 그대로 사용한다(별도 저장 없음).

## 6. 흐름

**복사 감지 (watcher)**
1. `clipboard-rs` 감시 스레드가 변경을 감지
2. 무시 조건: macOS `org.nspasteboard.ConcealedType`/`TransientType` 포함, 또는 앱이 `copy_clip`으로 직접 쓴 내용(쓰기 직후 기록한 hash와 일치)
3. 우선순위대로 정규화: 파일 목록 → 이미지 → 텍스트
4. `source_app::frontmost()`로 출처 앱 조회(실패 시 `None`)
5. `store.upsert()` → 한도 정리
6. `clips://changed` 이벤트 emit

**패널 열기**: 전역 단축키 → 패널이 숨겨져 있으면 위치 계산 후 표시·포커스, 열려 있으면 숨김. 열 때마다 프론트엔드가 검색어·필터를 초기화하고 첫 카드를 선택한다.

**선택 → 복사**: 더블클릭/Enter → `copy_clip(id)`
1. 해당 내용을 클립보드에 쓰고(text / PNG / 파일 목록) watcher 무시용 hash 기록
2. `last_used_at` 갱신
3. 패널 숨김 → OS가 직전 앱으로 포커스 복귀
4. toast 창에 "✓ 복사됨 · {요약}" 표시 후 1.5초 뒤 숨김. 요약은 텍스트 앞부분(최대 40자) / "이미지" / "파일 N개"
5. 파일 원본이 없으면 복사하지 않고 toast에 실패 메시지

**Command API (프론트엔드 → Rust)**
- `list_clips(query: string, kind: 'all'|'text'|'link'|'image'|'files', offset, limit) -> ClipDto[]`
- `copy_clip(id)`, `delete_clip(id)`, `clear_history()`
- `get_settings()`, `set_setting(key, value)`, `set_shortcut(accel) -> Result`
- `check_update()`, `install_update_and_restart()`

`ClipDto`는 `id, kind, text_preview(최대 500자), thumb(base64), meta, app{name, icon(base64)}?, last_used_at, missing(bool)`.

## 7. UI

디자인 토큰은 `docs/design/kraken-design.md`를 따른다(라이트: 흰 배경, `#7132f5`, 12px radius, whisper 그림자). 다크는 표면 `#16171c`/카드 `#1f2027`/테두리 `#2e2f39`, 강조색 `#9b7bff`. 테마는 `<html data-theme>`로 전환한다.

**패널 (B 시안)**
- 툴바: 검색창, 필터 칩(전체/텍스트/이미지/파일/링크), 설정 아이콘
- 카드(약 150×170): 왼쪽 상단 출처 앱 아이콘(없으면 기본 아이콘), 타입 배지(파일·폴더는 초록), 상대 시간 / 본문 미리보기 / 하단 정보
  - 텍스트: 앞부분 몇 줄, 링크: 보라색 URL, 이미지: 썸네일, 파일: 파일 아이콘 + "첫 파일명 외 N", 폴더면 폴더 아이콘, 원본 없음: "파일 없음"
- 선택 카드: 보라 테두리 + 16% 보라 링
- 목록: 첫 50개, 오른쪽 끝 근접 시 50개씩 추가 로드(가상 스크롤 없음). `clips://changed` 수신 시 새로고침

**키보드**
| 키 | 동작 |
|---|---|
| `←` `→` | 카드 선택 이동 |
| `Enter` / 더블클릭 | 복사 후 닫기 |
| 문자 입력 | 검색창에 바로 입력 |
| `Tab` | 필터 칩 순환 |
| `Delete`, 빈 검색어에서 `Backspace` | 선택 카드 삭제 |
| `Esc`, 같은 단축키, 포커스 잃음 | 패널 숨김 |
| 휠 | 가로 스크롤 |

**설정 창**: 테마(시스템/라이트/다크), 언어(시스템/한국어/English), 로그인 시 자동 실행 토글, 단축키(클릭 → 키 조합 녹화 → 즉시 재등록, 실패 시 이전 값 유지 + "다른 앱에서 사용 중"), 업데이트(현재 버전, 마지막 확인 결과, "업데이트 확인"/"업데이트 후 재시작"), 히스토리 전체 삭제(확인 다이얼로그).

**i18n**: `en.ts`가 기준 사전, `ko.ts`는 `typeof en` 타입으로 선언해 키 누락 시 컴파일 에러. `locale=system`이면 `navigator.language`가 `ko`로 시작할 때 한국어, 그 외 English. 상대 시간은 `Intl.RelativeTimeFormat`. 트레이 메뉴 문자열(4개)은 Rust에서 같은 규칙으로 선택한다.

## 8. 트레이

- 더블클릭: 패널 열기
- 메뉴: 열기 / 설정 / 업데이트 확인(업데이트 준비 시 "업데이트 후 재시작") / 종료
- macOS 메뉴바에서 더블클릭 이벤트가 안정적으로 오지 않으면 macOS는 클릭 시 메뉴 표시로 대체한다(구현 시 검증)

## 9. 자동 업데이트와 릴리스

- `tauri-plugin-updater` 엔드포인트: `https://github.com/bob-park/vee-app/releases/latest/download/latest.json`
- 확인 시점: 앱 시작 시, 이후 6시간마다, 사용자가 "업데이트 확인" 클릭 시
- 새 버전 발견 → 백그라운드 다운로드·설치 준비 → 트레이 메뉴와 설정 창에 "업데이트 후 재시작" 표시. 재시작은 사용자 클릭 시에만
- 서명 공개키: `~/.config/vee/bee.key.pub` (Tauri CLI 설치 후 사용자가 `yarn tauri signer generate -w ~/.config/vee/bee.key`로 생성) → `tauri.conf.json`의 `plugins.updater.pubkey`에 커밋
- CI: `.github/workflows/release.yml`, `v*` 태그 push 시 `tauri-apps/tauri-action`으로 macOS `aarch64-apple-darwin`, `x86_64-apple-darwin`, Windows `x86_64-pc-windows-msvc` 빌드 → 초안 GitHub Release에 번들과 `latest.json` 업로드
- 시크릿: `~/.config/vee/sign.env`의 `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`를 GitHub 레포 Secrets로 등록(구현 시 사용자 확인 후 `gh secret set`). macOS 서명 인증서(.p12)를 CI에서 쓰려면 `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`가 추가로 필요하며, 없으면 macOS 빌드는 로컬에서 `sign.env`를 source해 수행한다
- 값은 절대 커밋하지 않는다
- Windows는 코드 서명 없음 → 첫 설치 시 SmartScreen 경고 허용

## 10. 오류 처리

| 상황 | 동작 |
|---|---|
| DB 열기/마이그레이션 실패 | 오류 다이얼로그 후 종료. 자동 초기화 금지 |
| 클립보드 읽기 실패·미지원 형식 | 로그 후 계속 감시 |
| 이미지 50MB 초과 | 저장 생략 |
| 단축키 등록 실패 | 이전 단축키 유지, 설정 창에 메시지 |
| 파일 원본 없음 | 카드에 "파일 없음", 복사 시 toast 실패 메시지 |
| 출처 앱 조회 실패 | `app_id = NULL`, 기본 아이콘 |
| 업데이트 확인 실패 | 조용히 다음 주기 재시도, 설정 창에 마지막 결과 표시 |

로그: `tauri-plugin-log`로 앱 로그 디렉토리에 파일 기록. 클립보드 **내용**은 로그에 남기지 않는다.

## 11. 테스트

**자동 (Rust, `cargo test`)** — `store`를 인메모리 SQLite로 검증
- 같은 hash upsert 시 행 수 유지 + `last_used_at` 갱신
- 1,000개 초과 시 가장 오래된 항목과 이미지 파일 삭제
- 한국어 3글자 이상 trigram 검색, 1~2글자 LIKE 대체 검색
- 텍스트/링크 분류

**컴파일 타임**: i18n 키 누락(`ko: typeof en`), TypeScript 타입 체크(`tsc --noEmit`)

**수동 체크리스트 (macOS, Windows 각각)**
- [ ] 텍스트·이미지·파일·폴더 복사 시 카드 생성, 출처 앱 아이콘 표시
- [ ] 비밀번호 관리자 복사는 저장되지 않음 (macOS)
- [ ] 단축키 → 커서가 있는 모니터 하단에 패널, 다시 누르면 숨김
- [ ] 검색(한/영, 1~2글자 포함), 필터, 키보드 조작
- [ ] 더블클릭/Enter → 복사, 패널 닫힘, toast 표시, 다른 앱에서 붙여넣기 확인
- [ ] 단축키 변경 및 충돌 시 메시지
- [ ] 설정 창 닫기 → 트레이 상주, 트레이 더블클릭 → 패널
- [ ] 로그인 시 자동 실행 on/off
- [ ] 라이트/다크/시스템, 한국어/English/시스템 전환
- [ ] 구버전 설치 후 새 릴리스 발행 → 업데이트 감지, "업데이트 후 재시작"으로 갱신
