# PDF → Markdown 변환 설계

## 의도

PDF 파일 카드를 기존 "드래그 + 변환 키 → 도넛" 흐름으로 `.md` 파일로 바꾼다. 만든 Markdown은 **AI에 넣을 텍스트**로 쓴다. 그래서 구조를 충실히 살리는 것보다 아래 세 가지가 중요하다.

- 페이지 순서대로 문단이 깔끔하게 이어진다.
- 한글이 깨지지 않는다.
- macOS와 Windows에서 **같은 결과**가 나온다. 사용자가 명시한 요구다.

## 목표 / 비목표 / 안티골

- **목표**
  - PDF 카드를 변환 키를 누른 채 폴더로 끌면 도넛에 `MD` 조각이 하나 뜬다.
  - 조각에 놓으면 그 폴더에 `<원본 이름>.md`가 생긴다.
  - 여러 PDF를 한 번에 끌어도 된다. 각 파일이 각자의 `.md`가 된다(기존 다중 파일 job 흐름).
- **비목표**
  - 스캔본 PDF의 OCR
  - 표, 제목, 목록 구조 복원
  - PDF 안 이미지 추출
  - PDF의 다른 형식 변환(PDF → DOCX 등), MD → PDF
  - 엔진 다운로드 UI
- **안티골**
  - 텍스트가 없는 PDF로 빈 `.md`를 조용히 만드는 것. 실패로 알려야 한다.
  - OS마다 다른 엔진을 써서 결과가 달라지는 것
  - 잘못된 PDF 하나 때문에 Vee 전체가 죽는 것

## 제약

- **macOS hardened runtime(library validation).** 다른 팀이 서명한 dylib은 불러올 수 없다. 그래서 PDFium은 다운로드할 수 없고 앱에 같이 넣어 Vee 인증서로 서명해야 한다. `disable-library-validation`은 추가하지 않는다(`docs/agents/workflow/release.md`).
- **플랫폼 동등성.** macOS arm64와 Windows x64에서 같은 동작을 해야 한다(`docs/agents/overview.md`).
- **의존성 추가.** `pdfium-render` 크레이트 추가는 사용자가 승인했다. 그 밖의 의존성 추가는 다시 확인을 받는다.
- **앱 크기.** OS마다 약 7.4–7.6MB(압축 시 약 3.5–3.9MB) 늘어난다. 사용자가 괜찮다고 했다.

## 검토한 방식

| 방식 | 장단점 | 결과 |
|---|---|---|
| A. 순수 Rust PDF 크레이트 | 두 OS에 같은 코드를 쓸 수 있다. 다만 한글 추출 품질이 크레이트마다 들쭉날쭉하다 | 사용자가 C를 선택 |
| B. OS 기본 기능(macOS PDFKit / Windows OCR) | macOS 품질은 좋다. Windows에는 텍스트 API가 없어 두 OS의 결과가 달라진다 | 탈락(동등성 위반) |
| C. 외부 엔진 | 품질이 가장 좋다 | **선택: PDFium** |

C 안에서 다시 고른 결과:

- **poppler `pdftotext`**: macOS용으로 받을 수 있는 빌드가 없다. 탈락.
- **xpdf `pdftotext`**: macOS용 공식 빌드가 Intel 전용이다. 탈락.
- **PDFium**([bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries)): `mac-arm64`와 `win-x64` 빌드가 있다. 두 OS가 같은 엔진을 쓴다. **선택.**
- PDFium을 다운로드 엔진으로 만드는 방안은 위의 library validation 제약 때문에 불가능하다. 그래서 ONNX Runtime처럼 **앱에 같이 넣는다**. 사용자가 이 방식을 승인했다.

## 사용자가 정한 것

- 용도: AI 입력용 텍스트. 구조 복원은 필요 없다.
- 엔진: PDFium. 앱에 같이 넣는다(앱 크기 증가 허용).
- 두 OS에서 같은 결과를 낸다.
- 의존성 `pdfium-render` 추가를 승인했다.
- 작업은 새 브랜치 `feature/pdf-to-md`에서 한다.

## 설계

### 1. 번들 (ONNX Runtime과 같은 경로)

- `bblanchon/pdfium-binaries` 릴리스 `chromium/8125`를 고정한다. V8과 XFA가 없는 기본 빌드를 쓴다.
  - macOS: `pdfium-mac-arm64.tgz` → `lib/libpdfium.dylib`
  - Windows: `pdfium-win-x64.tgz` → `bin/pdfium.dll`
  - 아카이브와 라이브러리 모두 SHA-256을 고정한다. 업스트림이 체크섬을 게시하지 않으므로 고정할 때 직접 잰다.
- 빌드 시 받는 스크립트로 `src-tauri/vendor/pdfium/`에 넣는다. `tauri.conf.json`의 `beforeDevCommand`와 `beforeBuildCommand`에서 실행한다.
  - *내 판단:* `fetch-onnxruntime.mjs`와 같은 형태의 `scripts/fetch-pdfium.mjs`를 새로 만들고 테스트(`*.test.mjs`)도 같이 둔다. 두 스크립트를 하나로 합치는 리팩터는 이번 범위가 아니다.
- macOS는 `tauri.macos.conf.json`의 `bundle.macOS.frameworks`에 추가해 `Contents/Frameworks/`에 넣고 앱과 함께 서명한다. Windows는 `tauri.windows.conf.json`의 `resources`에 추가해 exe 옆에 둔다.
- 런타임 경로는 `ai.rs`의 `bundled_runtime` / `vendor_runtime`과 같은 규칙을 따른다(debug 빌드는 vendor 경로로 대체).
- `vendor/`가 이미 git ignore 대상인지 확인한다(ORT와 같게).
- PDFium 라이선스(BSD-3 및 서드파티 고지)를 ORT를 다루는 방식과 같게 처리한다. 이 부분은 builder가 확인한다.

### 2. 변환 실행: 일회성 자식 프로세스

- 앱 프로세스는 PDFium을 직접 불러오지 않는다. 변환할 파일마다 `vee-app --pdf-worker <src> <out>`을 띄운다.
  - `main.rs`에서 `--ai-worker`와 같은 방식으로 분기한다.
  - 기존 `process::command` / `exec`(타임아웃·취소)를 그대로 쓴다. rhwp를 호출하는 `hwp::convert`와 같은 모양이다.
- 이유
  - 잘못된 PDF에서 PDFium이 crash해도 Vee는 죽지 않는다(안티골).
  - PDFium은 스레드 안전하지 않은데, 프로세스로 분리하면 그 문제가 생기지 않는다.
  - 타임아웃과 취소를 공짜로 얻는다.
- 타임아웃: *내 판단* 120초(`RHWP_TIMEOUT`과 같게).
- 취소: *내 판단* 패널의 취소 버튼은 지금처럼 영상 job에만 보인다(`cancellable` 규칙은 바꾸지 않는다). 취소 플래그는 앱 종료 같은 내부 정리에만 쓴다. PDF 텍스트 추출은 보통 몇 초 안에 끝난다.
- `pdfium-render` 버전과 PDFium 빌드 맞추기: *내 판단* 크레이트 최신 버전을 쓰고, `default-features = false`로 필요한 기능만 켠다. 크레이트의 `pdfium_XXXX` 기능이 지원하는 PDFium API 버전에 맞춰 고정 바이너리 릴리스를 고른다. 바이너리를 바꾸면 SHA-256을 다시 잰다. 크레이트 기능을 켜고 끄면서 하위 의존성이 바뀌는 것은 이미 승인한 `pdfium-render` 추가에 포함된다. 다른 직접 의존성이 새로 필요해지면 그때 다시 확인을 받는다.
- 범위 밖: 파일이 아니라 PDF 데이터 자체가 들어 있는 클립(미리보기에서 복사한 `com.adobe.pdf` 등)
- 자식의 종료 코드
  - 0: `out`을 썼다.
  - 그 밖: 실패 → `JobError::Engine`

### 3. 변환 규칙 (워커 안)

- `Pdfium::bind_to_library(<번들 경로>)`로 불러온 뒤 `load_pdf_from_file(src, None)`으로 연다.
- 암호가 걸린 PDF와 열 수 없는 PDF는 실패로 처리한다.
- 출력은 순수 함수 `to_markdown(title, pages: &[String]) -> Option<String>`이 만든다. 단위 테스트 대상이다.
  ```
  # <원본 파일 이름(확장자 제외)>

  ## Page 1

  <1쪽 텍스트>

  ## Page 2
  ...
  ```
  - 페이지 텍스트는 `page.text().all()` 결과를 쓴다.
  - 줄바꿈은 `\n`으로 통일하고, 각 줄의 오른쪽 공백을 지운다. 빈 줄이 3줄 이상 이어지면 1줄로 줄인다.
  - 텍스트가 빈 페이지는 머리말만 남긴다. 페이지 번호가 원본과 맞아야 하기 때문이다.
  - **모든 페이지가 비면 `None`이 되고, 결과는 실패다.** 스캔본 PDF가 여기에 해당한다. 오류 종류는 *내 판단*으로 기존 `JobError::Engine`을 재사용한다. 새 오류 종류나 i18n 문구는 만들지 않는다.
- UTF-8로 쓰며 BOM은 넣지 않는다.

### 4. Convert 도메인

- `Format::Md`를 추가한다. 확장자와 serde 이름은 모두 `"md"`다.
- `doc_source`에 `"pdf" => (PDF_TARGETS, None)`을 추가한다. PDF는 Md가 아니므로 own은 `None`이다.
- **구현 시 변경:** `Engine::Pdfium`과 `PDF_TARGETS`는 만들지 않았다. PDF는 영상처럼 `formats_for`/`run`에서 별도 분기(`is_pdf`)로 처리하고 `MD` 조각은 `needs: None`이다. 엔진 이름이 프론트엔드에 새지 않아 더 작은 변경이다.
- (원안) `PDF_TARGETS = &[(Format::Md, <엔진>)]`
  - 엔진: `Engine::Pdfium`을 추가하되, `Caps::has`가 항상 `true`를 돌려준다. 번들돼 있어 설치할 것이 없으므로 조각이 흐리게 표시되지 않는다. *내 판단*이며, `DocTargets` 타입을 그대로 쓰기 위한 최소 변경이다. builder가 더 작은 방법을 찾으면 그쪽을 써도 된다. 단, 프론트엔드에 설치 UI가 생기면 안 된다.
  - `Engine`은 serde로 프론트엔드에 노출된다. 프론트엔드 `Engine` 유니언과 엔진 이름을 쓰는 곳(dim 툴팁 등)에 영향이 없는지 builder가 확인한다.
- `convert_doc`에서 `Engine::Pdfium` 분기는 위 2번의 워커를 호출한다.
- PDF와 다른 문서 계열을 섞어 끌면 기존 규칙대로 도넛이 비어 있다.
- `.pdf`는 지금 `Format::from_ext`(이미지 원본)에서 `None`이다. 이 동작은 그대로 둔다.

### 5. 프론트엔드

- `src/domain/convert/apis/convert.dto.ts`의 `Format` 유니언에 `'md'`를 추가한다.
- 라벨은 `formatLabel`이 자동으로 `MD`로 만든다. 테스트를 한 줄 추가한다.
- 도넛 조각에 형식별 아이콘이나 색이 있다면 기존 문서 형식(`txt` 등)의 규칙을 따른다.
- 새 화면이나 설정은 없다. 그래서 AGENTS.md의 "목업 3안" 절차는 적용하지 않는다. 기존 도넛에 조각 하나와 라벨 하나가 더해질 뿐이다.

### 6. 테스트

- Rust
  - `formats_for`: PDF 하나 → `[Md]`, PDF 여러 개 → `[Md]`, PDF + docx → 빈 결과
  - `to_markdown`: 페이지 머리말, 공백 정리, 빈 페이지 유지, 전부 빈 페이지면 `None`
  - 워커 통합 테스트: vendor의 PDFium으로 작은 한글 텍스트 PDF를 변환한다. ORT 테스트처럼 vendor 라이브러리가 없으면 건너뛴다.
    - 테스트 PDF는 저장소에 작은 fixture로 넣는다. 한글 폰트가 임베드된 PDF를 만드는 방법은 builder가 정한다.
    - 정답 파일: fixture와 함께 기대 출력 `.md`도 저장소에 넣고, 테스트는 결과가 바이트 단위로 같은지 비교한다. 같은 테스트를 Windows에서 `cargo test`로 돌리면 두 OS의 결과가 같은지 확인된다(*내 판단*. Windows 실행은 사용자가 할 수 있을 때 한다).
- JS: `fetch-pdfium.test.mjs`(엔트리 고르기), `formatLabel('md') === 'MD'`
- 마무리 검사 네 가지: `cargo test --lib`, `yarn test`, `yarn typecheck`, `yarn lint`
- 실제 앱에서 PDF 카드 → `MD` 조각 → `.md` 생성을 확인한다. 라이트/다크, 한국어/English 모두 본다.

### 7. 문서

- 다음 문서를 갱신한다.
  - `docs/agents/overview.md`: 변환 대상, 번들 라이브러리
  - `docs/agents/configuration.md`: frameworks/resources 표
  - `docs/agents/workflow/release.md`: 번들 라이브러리 목록
  - `docs/agents/architecture.md`: `--pdf-worker`
- What's New 릴리스 노트 문구가 필요한지는 기존 기능을 추가할 때의 관례를 따른다.

## builder에게 맡기는 것

- fetch 스크립트를 `fetch-onnxruntime.mjs`와 얼마나 공유할지(복제 vs 작은 공통 함수)
- `Engine::Pdfium`을 둘지, 더 작은 대안을 쓸지(4번 조건 안에서)
- 한글 fixture PDF를 만드는 방법
- 라이선스 고지 위치
