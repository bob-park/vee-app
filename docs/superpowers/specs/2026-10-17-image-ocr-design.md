# 이미지 OCR 검색 설계

- 날짜: 2026-10-17
- 브랜치: `feature/image-ocr` (master에서 분기)

## 왜

스크린샷과 이미지가 히스토리에 쌓이면 썸네일만으로는 구분이 안 되어 찾기 어렵다. 이미지 속 글자(한글·영어)를 OCR로 뽑아 기존 검색창에서 그대로 검색되게 한다.

## 사용자가 정한 것

1. 새 브랜치에서 작업한다.
2. 대상: **이미지 클립**(클립보드에 복사한 스크린샷/이미지)과 **파일 클립 속 이미지 파일**(Finder/탐색기에서 복사한 이미지 파일).
3. OCR은 **OS 내장**을 쓴다. macOS는 Vision, Windows는 `Windows.Media.Ocr`.
4. 한글과 영어 모두 검색되어야 한다. **가장 중요한 기준은 한글 인식 정확도**, 그다음이 속도다.
5. Windows도 지원한다. 단 Windows는 "일단 기능이 있는 것"이 목표이고 macOS 수준 품질을 요구하지 않는다.
6. **기존 히스토리의 이미지도** 백그라운드에서 OCR한다.
7. **OCR 켜기/끄기 설정**을 둔다.
8. 파일 클립은 **이미지 파일 수 제한 없이** 모두 OCR한다.
9. 개발 후 스크린샷 20~30장으로 직접 테스트한다. 성능이 부족하면 그때 다른 방법을 검토한다(이번 범위 아님).

## 목표

- 이미지 클립과 파일 클립의 이미지 파일에서 텍스트를 뽑아 저장하고, 검색(FTS trigram 3글자 이상, `LIKE` 1~2글자)이 이 텍스트도 대상으로 한다.
- OCR은 클립 저장을 막지 않는다. 저장 후 백그라운드에서 한 장씩 처리한다.
- 앱 업데이트 후 기존 이미지도 백그라운드에서 처리되어 검색된다.
- 측정 도구: 스크린샷 폴더와 기대 단어 목록을 주면 macOS/Windows OCR의 단어 적중률과 장당 시간을 보고한다.

## 하지 않는 것

- OCR 텍스트를 카드에 보여주거나 하이라이트하는 UI.
- 서드파티 OCR 엔진(PaddleOCR, Tesseract 등) 포함. 측정 결과가 나쁠 때 따로 검토한다.
- 이미지가 아닌 파일(PDF 등)의 텍스트 추출.

## 실패로 보는 것 (기술적으로 동작해도)

- 복사 직후 클립이 늦게 나타나거나 패널이 버벅임 (OCR이 watcher 스레드나 DB 락을 오래 잡는 경우).
- 기존 이미지 수천 장 백필이 CPU를 계속 점유해 시스템이 느려짐.
- Windows에서 한국어 언어팩이 없다는 이유로 에러 토스트·크래시가 남.
- 기존 텍스트 검색 결과나 속도가 나빠짐.

## 설계

### 데이터 (마이그레이션 `0006`)

- `clips.ocr_text TEXT` 추가. 의미:
  - `NULL` = 아직 OCR 안 함 (처리 대상)
  - `''` = OCR 했으나 글자 없음 / 실패 / 대상 아님 (다시 시도하지 않음)
  - 그 외 = 인식된 텍스트 (줄은 `\n`으로 연결)
- 텍스트·링크 클립과 이미지가 없는 파일 클립은 마이그레이션 시 `''`로 채워 백필 대상에서 뺀다. 새로 들어오는 이런 클립도 저장 시 `''`.
- `clips_fts`를 `(text, ocr_text)` 두 컬럼으로 다시 만들고(외부 콘텐츠 `content='clips'`, `tokenize='trigram'` 유지), 세 트리거를 `UPDATE OF text, ocr_text`까지 반영하도록 다시 만든 뒤 `rebuild`한다.
- [sqlite-migrations](../../agents/conventions/sqlite-migrations.md) 규칙을 따른다. 배포된 0001~0005는 건드리지 않는다.

### 검색

- `repository::list`의 3글자 이상 FTS 검색은 테이블 전체(두 컬럼)에 MATCH. 1~2글자 `LIKE`는 `c.text LIKE ? OR c.ocr_text LIKE ?`.
- 결과 정렬·필터(kind, app)는 그대로.

### OCR 모듈

- `src-tauri/src/app/ocr.rs` (OS 연동이므로 `app/`). 공개 함수 하나: `recognize(path: &Path) -> Result<String, Unavailable>`. `Ok("")`는 글자 없음/읽을 수 없는 파일, `Err(Unavailable)`은 이 OS에서 OCR 엔진을 쓸 수 없음. 플랫폼별 `#[cfg]` 구현.
- **macOS**: Vision `VNRecognizeTextRequest`, `recognitionLevel = accurate`, `recognitionLanguages = ["ko-KR", "en-US"]`. 한국어를 지원하지 않는 macOS(12 이하)에서는 지원되는 언어만으로 동작하고 에러를 내지 않는다.
- **Windows**: `Windows.Media.Ocr.OcrEngine`. 사용자 프로필 언어로 엔진을 만들고, 실패하면 설치된 OCR 언어 중 `ko`를 우선 선택하고 없으면 첫 번째 언어를 쓴다. 엔진을 아예 못 만들면 `Err(Unavailable)` (로그만 남김).
- 입력은 디스크의 이미지 파일 경로(이미지 클립은 `image_path`의 PNG, 파일 클립은 원본 파일 경로).

### 백그라운드 워커

- `lib.rs` setup에서 스레드 하나를 띄운다. 루프:
  1. DB에서 `ocr_text IS NULL`이고 kind가 `image`/`files`인 클립을 **`last_used_at` 최신순으로 하나** 가져온다 (락은 조회 동안만. 구버전 앱으로 내려갔다 온 경우 텍스트 클립도 `NULL`일 수 있어 kind로 제한한다).
  2. 락 없이 OCR 실행.
  3. 결과를 `UPDATE clips SET ocr_text = ? WHERE id = ?` (그 사이 삭제됐으면 0행, 무시).
     `Err(Unavailable)`이면 아무것도 쓰지 않고 **이번 실행 동안 워커를 멈춘다**. `NULL`이 남으므로 다음 앱 실행에서 엔진이 생겼다면(언어팩 설치 등) 다시 처리된다.
  4. 대상이 없으면 watcher의 신호(새 클립 저장)를 기다린다.
- 새 클립은 watcher가 `capture` 후 채널로 워커를 깨운다. 백필도 같은 루프라 따로 코드가 없다: 최신 클립부터 처리되어 최근 이미지가 먼저 검색된다.
- 백필 중 시스템 부하를 피하기 위해 장 사이에 짧게 쉰다(값은 빌더 재량).
- 파일 클립: `text`의 경로 중 이미지 확장자(`is_image_path`)이고 존재하며 50MB 이하인 파일을 **모두** OCR해 결과를 이어붙인다(장 사이 쉬는 간격 동일). 해당 파일이 하나도 없으면 `''`.
- 이미지 클립의 PNG가 없어졌으면 `''`.
- 처리 결과로 이벤트를 보내 열린 패널이 검색 중이면 목록을 갱신할지는 빌더 재량 (조용히 다음 검색부터 반영돼도 됨).

### 설정

- settings 키 `ocr`, 값 `on`/`off`, 기본 `on` (기존 `sound` 같은 문자열 on/off 패턴). Settings 창에서 `Row` + `Switch`로 노출하며, 위치는 히스토리 관련 섹션(빌더 재량). 문구는 `en.ts`/`ko.ts`.
- `off`: 워커가 새 OCR을 하지 않는다(새 클립은 `NULL`로 남음). 이미 추출된 텍스트는 계속 검색된다.
- `off` → `on`: 워커를 깨워 밀린 `NULL`을 최신순으로 처리한다.

### 측정 도구

- `#[ignore]` 테스트 하나(예: `ocr::bench`): 환경 변수 `VEE_OCR_BENCH_DIR` 폴더의 이미지들과 `expected.tsv`(`파일명<TAB>단어1,단어2,…`)를 읽어 `recognize`를 돌린다.
- 출력: 장별 시간, 놓친 단어, 전체 한글/영어 단어 적중률, 평균·최대 시간. 적중 판정은 검색과 같은 기준(대소문자 무시 부분 문자열).
- 실행: `cargo test --manifest-path src-tauri/Cargo.toml --lib ocr::bench -- --ignored --nocapture`.
- 사용자가 스크린샷 20~30장과 `expected.tsv`를 만들고 macOS·Windows에서 각각 돌린다.

## 의존성 (사용자 승인함, 2026-10-17)

- macOS: `objc2-vision` 추가 (기존 `objc2` 0.6 / `objc2-foundation` 0.3 계열에 맞는 버전).
- Windows: 기존 `windows` 크레이트에 기능 추가 — `Media_Ocr`, `Graphics_Imaging`, `Storage`, `Storage_Streams`, `Foundation`, `Foundation_Collections`, `Globalization` (실제 필요한 것만).

## 내 판단으로 정한 것 (확인 부탁)

- OCR 텍스트는 UI에 보이지 않고 검색에만 쓴다.
- OCR을 꺼도 이미 추출된 텍스트는 검색에 남긴다. 켜면 꺼져 있던 동안의 이미지도 처리한다.
- Windows에서 한국어 OCR이 없으면 설치된 다른 언어로 인식해 저장한다(나중에 한국어팩을 깔아도 이미 처리된 이미지는 다시 하지 않음). OCR 엔진 자체가 없으면 처리하지 않고 `NULL`로 남겨 다음 실행에 재시도.
- 합격 기준 제안: macOS에서 한글 기대 단어 적중률 90% 이상, 장당 1초 이하. 미달 시 대안 검토.

## 빌더 재량

- Vision/WinRT 호출 세부, 워커의 쉬는 간격, 이벤트 갱신 여부, 테스트 구성.
- 검증: `cargo test --lib`, `yarn test`, `yarn typecheck`, `yarn lint` 통과. UI 변경은 없지만 dev 빌드로 스크린샷 복사 → 검색되는지 직접 확인.
