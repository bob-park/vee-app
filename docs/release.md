# 릴리스 절차

빌드 CI는 없다. 모든 빌드·서명·업로드는 개인 장비에서 `master` 기준으로 한다. GitHub Actions는 버전만 올린다. macOS는 Apple Silicon만 지원한다.

## 준비 (한 번만)
- macOS: `gh auth login`, 로그인 키체인의 `Developer ID Application` 인증서
- 공증은 App Store Connect **API 키** 방식이다(Apple ID/앱 암호 방식은 쓰지 않는다).
  1. App Store Connect → 사용자 및 액세스 → 통합 → App Store Connect API에서 키를 만든다(액세스: Developer).
  2. 내려받은 `AuthKey_<키 ID>.p8`을 `~/.config/apple/`에 둔다. 한 번만 내려받을 수 있으니 백업한다.
  3. 키 ID와 발급자 ID(Issuer ID)를 로그인 키체인에 넣는다.
     ```bash
     security add-generic-password -a "$USER" -s tauri-apple-api-key -w '<키 ID>'
     security add-generic-password -a "$USER" -s tauri-apple-api-issuer -w '<Issuer ID>'
     ```
- `~/.config/apple/.env`(앱 공통): `APPLE_SIGNING_IDENTITY="Developer ID Application: ..."`
- 업데이터 서명 키(macOS): 개인키 내용과 암호를 로그인 키체인에 넣는다. `release-macos.sh`가 `scripts/load-updater-key-macos.sh`로 읽어 온다.
  ```bash
  security add-generic-password -a "$USER" -s tauri-updater-vee -w '<개인키 내용>'
  security add-generic-password -a "$USER" -s tauri-updater-vee-pw -w '<키 암호>'
  ```
  - 직접 `yarn tauri build`를 할 때는 먼저 `source scripts/load-updater-key-macos.sh`를 실행한다.
  - 공개키는 저장소의 `src-tauri/updater.key.pub`에 있고, `tauri.conf.json`의 `plugins.updater.pubkey`와 내용이 같다. 키를 바꾸면 둘 다 고친다.
  - 예전 `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID`가 남아 있으면 지운다. `APPLE_ID`가 있으면 Tauri가 API 키 대신 그쪽으로 공증한다.
- 서명·공증된 앱만 만들어 보려면 `scripts/release-macos.sh --build-only`를 실행한다(업로드 없음). Tauri는 `.app`만 공증하므로, 스크립트가 빌드 뒤 `.dmg`도 `notarytool`로 공증하고 스테이플한다.
- Windows PC: Rust, Node 26/yarn 4, `gh auth login`, PowerShell 7
- 업데이터 서명 키(Windows): 저장소 루트에서 아래를 한 번 실행하고, 물어보는 두 값(개인키 내용, 키 암호)을 붙여 넣는다. 자격 증명 관리자에 `tauri-updater-vee`, `tauri-updater-vee-pw`로 저장되고, `release-windows.ps1`이 같은 스크립트로 읽어 온다.
  ```powershell
  . scripts/load-updater-key-windows.ps1 -Save
  ```
  - `cmdkey /pass`는 길이 제한 때문에 개인키를 다 담지 못하므로 쓰지 않는다.
  - 직접 `yarn tauri build`를 할 때는 먼저 `. scripts/load-updater-key-windows.ps1`을 실행한다.
  - 예전 `%USERPROFILE%\.config\vee\sign.env`와 개인키 파일은 저장한 뒤 지워도 된다.
- `GITHUB_NPM_AUTH_TOKEN`: `read:packages` 권한의 GitHub 토큰을 환경 변수로 등록한다(macOS는 셸 프로필에 `export`, Windows는 `setx`). 없으면 `yarn install`이 실패한다. 자세한 방법은 [개발 문서](development.md#github-packages-토큰-github_npm_auth_token)
- yarn은 반드시 4.18.1 (`../mise.toml`)로 실행한다. mise가 활성화되지 않아 전역 yarn 1.x가 잡히면 `yarn.lock`이 Yarn 1 형식으로 통째로 바뀐다. `yarn --version`으로 확인하고, 4.x가 아니면 `corepack yarn@4.18.1 <명령>`으로 실행한다.

## 업데이터 서명 키 만들기·바꾸기
키는 처음 한 번만 만든다. **키를 바꾸면 이미 설치된 앱은 새 키로 서명된 업데이트를 받지 못하고, 사용자가 직접 다시 설치해야 한다.** 개인키를 잃어버렸을 때만 바꾼다.

1. `yarn tauri signer generate -w <경로>` → `<경로>`(개인키)와 `<경로>.pub`(공개키)가 생긴다. 암호를 물어본다(`-p`로 지정, 같은 경로에 이미 있으면 `-f`).
2. 개인키 **내용**과 암호를 위 "준비"대로 키체인(macOS)과 자격 증명 관리자(Windows)에 넣는다.
3. `<경로>.pub`을 `src-tauri/updater.key.pub`으로 복사하고, 같은 내용을 `tauri.conf.json`의 `plugins.updater.pubkey`에 넣어 커밋한다.
4. 개인키 파일은 오프라인 백업만 남기고 작업 기계에서 지운다.

파일 하나를 손으로 다시 서명할 때: 키를 불러온 뒤(`source scripts/load-updater-key-macos.sh` / `. scripts/load-updater-key-windows.ps1`) `yarn tauri signer sign --app-version <버전> <파일>`. `yarn tauri build`가 업데이트 파일은 알아서 서명하므로 보통은 필요 없다.

## 절차
1. `master`로 머지 → GitHub Actions가 버전을 올려 `chore: bump version to X.Y.Z [skip ci]`를 커밋한다. 릴리스할 각 기계에서 `master`를 pull하고 `src-tauri/tauri.conf.json`의 `version`이 올라갔는지 확인한다. 버전 파일은 손으로 고치지 않는다.
2. macOS: `scripts/release-macos.sh` → 초안 릴리스에 dmg, `Vee_<버전>_aarch64.app.tar.gz`와 `.sig` 업로드
3. Windows: `pwsh scripts/release-windows.ps1` → 같은 릴리스에 `Vee_<버전>_x64-setup.exe`와 `.sig` 업로드
4. 2·3은 순서 무관. 둘 다 끝나면 아무 기계에서 `node scripts/latest-json.mjs v<버전>` → 릴리스의 `.sig`를 모아 `latest.json`을 만들어 올린다(올라온 플랫폼만 들어감)
   - `latest-json.mjs` 전에 릴리스 본문을 완성한다. 본문이 그대로 앱의 새 소식 창에 나온다.
5. 출력된 플랫폼 목록에 `darwin-aarch64`, `windows-x86_64`가 모두 있는지 확인
6. `gh release edit v<버전> -R bob-park/vee-app --draft=false`로 게시 → 설치된 앱이 6시간 안에(또는 "업데이트 확인"으로) 감지

## macOS만 먼저 게시하기
Windows 빌드를 바로 할 수 없을 때는 macOS만 먼저 내보내고, Windows는 나중에 같은 릴리스에 더한다.

1. 위 절차 1·2를 한다.
2. `node scripts/latest-json.mjs v<버전>` → 출력이 `darwin-aarch64`뿐인지 확인
3. `gh release edit v<버전> -R bob-park/vee-app --draft=false`로 게시
4. 확인: `curl -sL https://github.com/bob-park/vee-app/releases/latest/download/latest.json`에 새 버전과 `darwin-aarch64`가 보여야 한다.

이 상태의 영향:
- macOS 사용자는 바로 업데이트를 받는다.
- Windows 사용자는 `latest.json`에 `windows-x86_64`가 없으므로 업데이트를 받지 않고 이전 버전에 머문다(오류는 없음). 이 릴리스가 "Latest"가 되므로 Releases 페이지에서 새로 내려받으려는 Windows 사용자는 설치 파일을 찾을 수 없다 — 가능한 빨리 Windows를 더한다.

나중에 Windows를 더할 때:
1. Windows PC에서 `pwsh scripts/release-windows.ps1` → 이미 게시된 같은 릴리스에 업로드된다(새 초안을 만들지 않음).
2. 아무 기계에서 `node scripts/latest-json.mjs v<버전>`을 다시 실행 → `latest.json`을 덮어쓴다. 출력에 두 플랫폼이 모두 있는지 확인한다. 이미 게시된 상태이므로 `--draft=false`는 다시 할 필요 없다.

## 이미 게시된 버전
- `release-macos.sh`/`release-windows.ps1`은 `v<버전>`이 이미 **게시**(draft 아님)되어 있고 그 플랫폼 파일이 올라가 있으면 멈춘다. 게시된 버전은 덮어쓰지 않는다. 고칠 것이 있으면 PR을 머지해 버전을 올린 뒤 새로 릴리스한다.
- 게시된 릴리스에 아직 없는 플랫폼을 더하는 것은 된다(아래 "macOS만 먼저 게시하기").

## 문제 해결
- `failed to decode secret key: ... Invalid symbol` — 키체인/자격 증명 관리자의 `tauri-updater-vee`에 개인키 **내용**이 아닌 다른 값(파일 경로, 잘린 값)이 들어 있다. 다시 저장한다. macOS에서는 이 오류가 공증까지 끝난 뒤 마지막에 나며, 그때는 릴리스에 아무것도 올라가지 않는다.
- `Credential Manager has no tauri-updater-vee` — Windows에서 위 "준비"의 `-Save`를 하지 않았다.
- `missing .../AuthKey_<ID>.p8` 또는 `security: SecKeychainSearchCopyNext` — 키체인 항목(`tauri-apple-api-key`, `tauri-apple-api-issuer`, `tauri-updater-vee`, `tauri-updater-vee-pw`)이나 `~/.config/apple/`의 `.p8`이 없다. 위 "준비"를 다시 본다.
