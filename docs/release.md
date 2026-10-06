# 릴리스 절차

CI는 없다. 모든 빌드·서명·업로드는 개인 장비에서 한다.

## 준비 (한 번만)
- macOS: `gh auth login`, `jq`, `rustup target add x86_64-apple-darwin`, 로그인 키체인의 `Developer ID Application` 인증서
- `~/.config/vee/sign.env`: `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `TAURI_SIGNING_PRIVATE_KEY`(절대 경로 또는 키 내용), `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- Windows PC: Rust, Node 24/yarn 4, `gh auth login`, PowerShell 7.5+, `%USERPROFILE%\.config\vee\sign.env`(Tauri 키 두 줄)와 개인키 파일

## 절차
1. `src-tauri/tauri.conf.json`의 `version`을 올리고 커밋·푸시
2. macOS: `scripts/release.sh`
3. Windows: `pwsh scripts/release.ps1`
4. 두 스크립트는 순서 무관. **동시에 실행하지 않는다** (`latest.json` 병합이 겹침)
5. GitHub 초안에서 `latest.json`에 `darwin-aarch64`, `darwin-x86_64`, `windows-x86_64`가 모두 있는지 확인
6. `gh release edit v<version> -R bob-park/vee-app --draft=false`로 게시 → 설치된 앱이 6시간 안에(또는 "업데이트 확인"으로) 감지
