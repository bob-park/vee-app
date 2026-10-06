# 릴리스 절차

CI는 없다. 모든 빌드·서명·업로드는 개인 장비에서 한다. macOS는 Apple Silicon만 지원한다.

## 준비 (한 번만)
- macOS: `gh auth login`, 로그인 키체인의 `Developer ID Application` 인증서
- `~/.config/vee/sign.env`: `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `TAURI_SIGNING_PRIVATE_KEY`(절대 경로 또는 키 내용), `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- Windows PC: Rust, Node 24/yarn 4, `gh auth login`, PowerShell 7, `%USERPROFILE%\.config\vee\sign.env`(Tauri 키 두 줄, Windows 경로)와 개인키 파일

## 절차
1. `src-tauri/tauri.conf.json`의 `version`을 올리고 커밋·푸시
2. macOS: `scripts/release.sh` → 초안 릴리스에 dmg, `Vee_<버전>_aarch64.app.tar.gz`와 `.sig` 업로드
3. Windows: `pwsh scripts/release.ps1` → 같은 초안에 `Vee_<버전>_x64-setup.exe`와 `.sig` 업로드
4. 2·3은 순서 무관. 둘 다 끝나면 아무 기계에서 `node scripts/latest-json.mjs v<버전>` → 릴리스의 `.sig`를 모아 `latest.json`을 만들어 올린다(올라온 플랫폼만 들어감)
5. 출력된 플랫폼 목록에 `darwin-aarch64`, `windows-x86_64`가 모두 있는지 확인
6. `gh release edit v<버전> -R bob-park/vee-app --draft=false`로 게시 → 설치된 앱이 6시간 안에(또는 "업데이트 확인"으로) 감지
