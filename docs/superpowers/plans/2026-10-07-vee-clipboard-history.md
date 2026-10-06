# Vee 클립보드 히스토리 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 복사한 텍스트·링크·이미지·파일을 SQLite에 기록하고, 단축키로 화면 하단 카드 패널을 띄워 검색·선택해 다시 클립보드로 복사하는 macOS/Windows 트레이 앱 "Vee"를 만든다.

**Architecture:** Rust(Tauri 2)가 클립보드 감시·출처 앱 감지·저장/검색·창 제어·트레이·업데이트를 전부 담당하고, React(TS+Vite)는 `invoke`/이벤트로 받은 데이터를 그리기만 한다. `store.rs`는 Tauri에 의존하지 않아 `cargo test`로 단독 검증한다. 창은 `panel`/`toast`/`settings` 3개를 `tauri.conf.json`에 숨김 상태로 선언하고, 하나의 프론트엔드 번들이 창 라벨로 화면을 분기한다.

**Tech Stack:** Tauri 2.12 · Rust edition 2024 · rusqlite 0.40(bundled, FTS5 trigram) · clipboard-rs 0.3.5 · React 19 · TypeScript 7 · Vite 8 · yarn 4.18 · Node 24

**Spec:** `docs/superpowers/specs/2026-10-07-vee-clipboard-history-design.md`

## Global Constraints

- 작업 브랜치: `feature/paste` (현재 브랜치, worktree 사용 안 함)
- Tauri는 **2.x 안정판**만 사용 (crates.io의 3.0 alpha 금지). 버전: `tauri 2.12.1`, `tauri-build 2.7.1`, `tauri-plugin-global-shortcut 2.4.0`, `tauri-plugin-autostart 2.7.0`, `tauri-plugin-updater 2.13.1`, `tauri-plugin-single-instance 2.5.2`, `tauri-plugin-log 2.10.0`, `tauri-plugin-dialog 2.8.1`, `clipboard-rs 0.3.5`, `rusqlite 0.40.2`(feature `bundled`), `sha2 0.11.0`, `base64 0.22`, `sys-locale 0.3.2`, `image 0.25`, macOS `objc2-app-kit 0.3.2`/`objc2-foundation 0.3.2`, Windows `windows 0.62.2`/`windows-icons 0.4.0`
- npm: `@tauri-apps/api ^2.12.1`, `@tauri-apps/cli ^2.12.1`, `@tauri-apps/plugin-dialog ^2.8.1`, `react`/`react-dom ^19.3.0`, `vite ^8.3.3`, `@vitejs/plugin-react ^6.1.2`, `typescript ^7.0.2`, `@types/node ^24`. **i18n·상태관리·테스트 라이브러리 추가 금지** (프론트 테스트는 Node 내장 `node:test`)
- `productName: "Vee"`, `identifier: "com.bobpark.vee"`, 버전은 `src-tauri/tauri.conf.json`의 `version`
- 선택 시 **복사만** 한다. 자동 붙여넣기(키 입력 시뮬레이션) 금지
- 수치: 보관 1,000개 · 이미지 50MB 초과 미저장 · 미리보기 500자 · 페이지 50개 · 패널 높이 300 logical px · toast 1.5초 · 업데이트 확인 6시간
- 기본 단축키 `CommandOrControl+Shift+V`
- 모든 UI 문자열은 `src/i18n/en.ts`·`ko.ts` 사전을 거친다. 트레이 메뉴만 Rust `tray.rs`에서 ko/en 선택
- 디자인 토큰은 `docs/design/kraken-design.md`(라이트 `#7132f5`, radius 12px) + 다크 `#16171c`/`#1f2027`/`#2e2f39`/`#9b7bff`
- 클립보드 **내용**은 절대 로그에 남기지 않는다
- `~/.config/vee/sign.env`의 값, 서명 개인키는 절대 커밋하지 않는다. 공개키(`bee.key.pub`)만 커밋
- 모든 cargo 명령은 레포 루트에서 `--manifest-path src-tauri/Cargo.toml`로 실행한다

## 스펙과 달라진 점 (구현 근거)

1. **자기 쓰기 무시**: 스펙의 "hash 일치로 무시" 대신 `copy_clip`이 클립보드에 쓴 직후 **500ms 동안 watcher를 무시**한다. 이미지는 클립보드를 거치면 PNG 바이트가 달라져 hash로는 중복 이미지가 생기기 때문이다.
2. **macOS 포커스 복귀**: Dock 없는(accessory) 앱은 창을 숨겨도 직전 앱으로 포커스가 자동 복귀하지 않는다. 패널을 열 때 최전면 앱 pid를 기억하고, Enter/Esc로 닫을 때 그 앱을 다시 활성화한다(붙여넣기 키 입력은 하지 않음).
3. `meta`는 언어 중립 값만 저장한다: 이미지 `"1280×720"`, 파일 `"3"`(개수). 글자 수는 `length(text)`로 `charCount`에 담는다.
4. `ClipDto`에 `charCount`, `isDir` 추가. toast 페이로드는 `{ ok, text, files, image }`.
5. command 추가: `hide_panel`, `open_settings`, `set_autostart`, `get_update_status`.
6. 썸네일·아이콘 축소는 `clipboard-rs`가 재노출하는 `RustImage` API를 쓰고, `image` 크레이트는 Windows 아이콘 PNG 인코딩에만 쓴다.
7. 의존성 추가: `base64`(data URL), `sys-locale`(트레이 언어), `tauri-plugin-dialog`(DB 오류·전체 삭제 확인), `windows-icons`(Windows 앱 아이콘), dev `tempfile`.
8. Rust 모듈 `windows.rs`는 Windows 크레이트 `windows`와 이름이 겹치므로, `source_app.rs`에서는 항상 `::windows::...` 절대 경로로 크레이트를 참조한다.

## Review Focus

1. **한글 입력 중 Enter/방향키/Backspace** — IME 조합 중인 키 입력은 카드 복사·이동·삭제로 해석되면 안 된다 → Task 6 `keys.test.ts`
2. **검색어에 `%`, `_`, `"`, `*`, `AND` 같은 특수 문자** — 오류 없이 글자 그대로 검색돼야 한다 → Task 2 `search_treats_special_characters_literally`
3. **공백·줄바꿈만 복사** — 빈 카드가 생기면 안 된다 → Task 3 `whitespace_only_text_is_ignored`
4. **패널이 열린 사이 항목이 삭제/정리된 뒤 선택** — 패닉 없이 실패 toast만 떠야 한다 → Task 2 `content_of_missing_id_is_none` + Task 4 `copy_clip`의 `None` 처리
5. **수 MB짜리 로그 텍스트 복사** — 전체는 저장하되 패널로는 500자만 보내 UI가 멈추지 않아야 한다 → Task 2 `large_text_preview_is_capped_but_counted`

## 수동 검증이 필요한 위험 지점

- macOS 메뉴바 아이콘 더블클릭 이벤트 (Task 7) — 안 오면 스펙 8장의 대체안(클릭 시 메뉴) 적용
- macOS는 다른 앱이 쓰는 단축키도 등록이 성공할 수 있어 충돌 감지가 Windows보다 약함 (Task 8)
- Windows에서 toast 표시가 직전 앱 포커스를 빼앗는지 (Task 4/11)

## 파일 구조

```
vee-app/
├─ package.json, tsconfig.json, vite.config.ts, index.html, app-icon.svg
├─ scripts/release.sh, scripts/release.ps1
├─ docs/release.md
├─ src/
│  ├─ main.tsx              창 라벨로 Panel/Settings/Toast 선택
│  ├─ api.ts                invoke 래퍼 + 타입
│  ├─ prefs.tsx             설정 로드, 테마 적용, 언어 사전 제공 (usePrefs)
│  ├─ theme.css             토큰 + 공통 스타일
│  ├─ i18n/en.ts, ko.ts, index.ts
│  ├─ panel/Panel.tsx, Toolbar.tsx, Card.tsx, keys.ts, keys.test.ts, panel.css
│  ├─ settings/Settings.tsx, ShortcutRecorder.tsx, accelerator.ts, accelerator.test.ts, settings.css
│  └─ toast/Toast.tsx, toast.css
└─ src-tauri/
   ├─ Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, icons/
   └─ src/
      ├─ main.rs           vee_app_lib::run()
      ├─ lib.rs            AppState, 셋업, 클립 command
      ├─ store.rs          SQLite (Tauri 비의존)
      ├─ source_app.rs     최전면 앱·아이콘·재활성화 (macOS/Windows)
      ├─ watcher.rs        클립보드 감시
      ├─ windows.rs        panel/toast/settings 창, copy_clip
      ├─ settings.rs       설정 command, 단축키 등록
      ├─ tray.rs           트레이 아이콘·메뉴
      └─ updater.rs        업데이트 확인·다운로드·설치
```

---

### Task 1: Tauri + React 프로젝트 골격

**Files:**
- Delete: `Cargo.toml`, `Cargo.lock`, `src/main.rs` (루트 Hello world)
- Create: `package.json`, `tsconfig.json`, `vite.config.ts`, `index.html`, `app-icon.svg`, `src/main.tsx`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/icons/*`(생성)

**Interfaces:**
- Produces: 라이브러리 크레이트 `vee_app_lib`의 `pub fn run()`; 창 라벨 `panel`, `toast`, `settings`; yarn 스크립트 `dev`, `build`, `typecheck`, `test`, `tauri`

- [ ] **Step 1: 루트 Rust 바이너리 제거**

```bash
git rm -q Cargo.toml Cargo.lock src/main.rs
```

- [ ] **Step 2: `package.json` 작성**

```json
{
  "name": "vee-app",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "typecheck": "tsc --noEmit",
    "test": "node --test \"src/**/*.test.ts\"",
    "tauri": "tauri"
  }
}
```

- [ ] **Step 3: 의존성 설치**

```bash
yarn add react@^19.3.0 react-dom@^19.3.0 @tauri-apps/api@^2.12.1
yarn add -D @tauri-apps/cli@^2.12.1 @types/react@^19.3.0 @types/react-dom@^19.3.0 @vitejs/plugin-react@^6.1.2 typescript@^7.0.2 vite@^8.3.3
```

Expected: `node_modules/` 생성, `yarn.lock` 생성 (`.yarnrc.yml`의 `nodeLinker: node-modules` 사용)

- [ ] **Step 4: `tsconfig.json`, `vite.config.ts`, `index.html` 작성**

`tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true,
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "types": ["vite/client", "node"]
  },
  "include": ["src", "vite.config.ts"]
}
```

`"types"`에 `node`가 있으므로 `@types/node`도 설치한다: `yarn add -D @types/node@^24` (`node:test` 타입용).

`vite.config.ts`:
```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "es2022", outDir: "dist" },
});
```

`index.html`:
```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Vee</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: 임시 `src/main.tsx` 작성** (Task 5에서 교체)

```tsx
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";

createRoot(document.getElementById("root")!).render(<p>{getCurrentWindow().label}</p>);
```

- [ ] **Step 6: Rust 크레이트 작성**

`src-tauri/Cargo.toml`:
```toml
[package]
name = "vee-app"
version = "0.1.0"
edition = "2024"

[lib]
name = "vee_app_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.7.1", features = [] }

[dependencies]
tauri = { version = "2.12.1", features = ["tray-icon", "image-png", "macos-private-api"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
log = "0.4"
```

`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vee_app_lib::run()
}
```

`src-tauri/src/lib.rs`:
```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: `src-tauri/tauri.conf.json` 작성**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Vee",
  "version": "0.1.0",
  "identifier": "com.bobpark.vee",
  "build": {
    "beforeDevCommand": "yarn dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "yarn build",
    "frontendDist": "../dist"
  },
  "app": {
    "macOSPrivateApi": true,
    "windows": [
      {
        "label": "panel",
        "title": "Vee",
        "visible": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "shadow": false,
        "visibleOnAllWorkspaces": true,
        "width": 1200,
        "height": 300
      },
      {
        "label": "toast",
        "title": "Vee",
        "visible": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "shadow": false,
        "focus": false,
        "focusable": false,
        "visibleOnAllWorkspaces": true,
        "width": 360,
        "height": 52
      },
      {
        "label": "settings",
        "title": "Vee",
        "visible": false,
        "width": 520,
        "height": 600,
        "resizable": false,
        "center": true
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg", "nsis"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["panel", "toast", "settings"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 8: 앱 아이콘 생성**

`app-icon.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <rect x="64" y="64" width="896" height="896" rx="200" fill="#7132f5"/>
  <path d="M300 300 L512 740 L724 300" fill="none" stroke="#ffffff" stroke-width="110" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
```

Run: `yarn tauri icon app-icon.svg`
Expected: `src-tauri/icons/`에 `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico` 등 생성

- [ ] **Step 9: 빌드 확인**

Run: `yarn build && cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 둘 다 성공. (`generate_context!`는 `dist/`와 아이콘이 있어야 컴파일된다)

Run: `yarn tauri dev` → 프로세스가 오류 없이 떠 있는지 확인 후 Ctrl+C (창은 모두 숨김이라 보이지 않는 게 정상)

- [ ] **Step 10: Commit**

```bash
git add -A package.json yarn.lock tsconfig.json vite.config.ts index.html app-icon.svg src src-tauri
git commit -m "chore: scaffold Tauri 2 + React app"
```

---

### Task 2: SQLite 저장소 (`store.rs`)

**Files:**
- Create: `src-tauri/src/store.rs`
- Modify: `src-tauri/Cargo.toml` (의존성), `src-tauri/src/lib.rs` (`mod store;`)

**Interfaces:**
- Produces (모두 `crate::store`):
  - `pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>`
  - `pub const MAX_CLIPS: i64 = 1000`
  - `pub enum Kind { Text, Link, Image, Files }` — serde 소문자, `as_str()`, `parse(&str) -> Option<Kind>`
  - `pub enum NewClip { Text(String), Image { png: Vec<u8>, thumb_png: Vec<u8>, width: u32, height: u32 }, Files(Vec<String>) }`
  - `pub enum ClipContent { Text(String), Image(PathBuf), Files(Vec<String>) }`
  - `pub struct ClipDto { id, kind, text_preview: Option<String>, char_count: i64, thumb: Option<String>, meta: Option<String>, app_name: Option<String>, app_icon: Option<String>, last_used_at: i64, missing: bool, is_dir: bool }` — serde camelCase, 이미지는 `data:image/png;base64,...`
  - `pub fn classify_text(&str) -> Kind`
  - `Store::open(db_path, images_dir)`, `Store::open_in_memory(images_dir)`, `app_known(&str) -> Result<bool>`, `upsert_app(bundle_id, name, icon_png: Option<&[u8]>) -> Result<i64>`, `upsert(NewClip, app_id: Option<i64>, now: i64) -> Result<()>`, `list(query, kind: Option<Kind>, offset, limit) -> Result<Vec<ClipDto>>`, `content(id) -> Result<Option<ClipContent>>`, `touch(id, now)`, `delete(id)`, `clear()`, `get_setting(key) -> Result<Option<String>>`, `set_setting(key, value)`

- [ ] **Step 1: 의존성 추가** — `src-tauri/Cargo.toml`의 `[dependencies]`에 추가하고 `[dev-dependencies]` 섹션을 만든다

```toml
rusqlite = { version = "0.40.2", features = ["bundled"] }
sha2 = "0.11.0"
base64 = "0.22"

[dev-dependencies]
tempfile = "3"
```

`src-tauri/src/lib.rs` 맨 위에 `mod store;` 추가.

- [ ] **Step 2: 실패하는 테스트 작성** — `src-tauri/src/store.rs`를 아래 테스트 모듈만 가진 파일로 만든다 (구현은 Step 4)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(&dir.path().join("images")).unwrap();
        (store, dir)
    }

    fn text(s: &str) -> NewClip {
        NewClip::Text(s.to_string())
    }

    fn texts(clips: &[ClipDto]) -> Vec<String> {
        clips.iter().map(|c| c.text_preview.clone().unwrap_or_default()).collect()
    }

    fn image(byte: u8) -> NewClip {
        NewClip::Image { png: vec![byte; 8], thumb_png: vec![9], width: 10, height: 20 }
    }

    fn image_path(store: &Store, id: i64) -> std::path::PathBuf {
        match store.content(id).unwrap() {
            Some(ClipContent::Image(p)) => p,
            other => panic!("expected image, got {other:?}"),
        }
    }

    #[test]
    fn same_text_bumps_instead_of_duplicating() {
        let (s, _d) = store();
        s.upsert(text("hello"), None, 1).unwrap();
        s.upsert(text("world"), None, 2).unwrap();
        s.upsert(text("hello"), None, 3).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!(texts(&all), vec!["hello", "world"]);
        assert_eq!(all[0].last_used_at, 3);
    }

    #[test]
    fn classifies_links() {
        assert_eq!(classify_text("https://example.com/a?b=1"), Kind::Link);
        assert_eq!(classify_text("  git@github.com:bob-park/vee-app.git\n"), Kind::Link);
        assert_eq!(classify_text("ssh://host/repo"), Kind::Link);
        assert_eq!(classify_text("see https://example.com"), Kind::Text);
        assert_eq!(classify_text("https://a.com\nhttps://b.com"), Kind::Text);
        assert_eq!(classify_text("hello"), Kind::Text);
    }

    #[test]
    fn trims_oldest_beyond_max_and_removes_image_file() {
        let (s, _d) = store();
        s.upsert(image(1), None, 0).unwrap();
        let first = s.list("", None, 0, 1).unwrap()[0].id;
        let path = image_path(&s, first);
        assert!(path.exists());
        for i in 1..=MAX_CLIPS {
            s.upsert(text(&format!("clip {i}")), None, i).unwrap();
        }
        let all = s.list("", None, 0, 2000).unwrap();
        assert_eq!(all.len() as i64, MAX_CLIPS);
        assert!(all.iter().all(|c| c.kind != Kind::Image));
        assert!(!path.exists());
    }

    #[test]
    fn searches_korean_with_trigram_and_short_queries_with_like() {
        let (s, _d) = store();
        s.upsert(text("클립보드 히스토리 앱"), None, 1).unwrap();
        s.upsert(text("cargo tauri dev"), None, 2).unwrap();
        s.upsert(text("Vee"), None, 3).unwrap();
        assert_eq!(texts(&s.list("히스토", None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&s.list("보드", None, 0, 50).unwrap()), vec!["클립보드 히스토리 앱"]);
        assert_eq!(texts(&s.list("TAURI", None, 0, 50).unwrap()), vec!["cargo tauri dev"]);
        assert_eq!(texts(&s.list("ve", None, 0, 50).unwrap()), vec!["Vee"]);
    }

    #[test]
    fn search_treats_special_characters_literally() {
        let (s, _d) = store();
        s.upsert(text("100% done"), None, 1).unwrap();
        s.upsert(text("a_b"), None, 2).unwrap();
        s.upsert(text("say \"hi\" AND bye*"), None, 3).unwrap();
        s.upsert(text("plain"), None, 4).unwrap();
        assert_eq!(texts(&s.list("%", None, 0, 50).unwrap()), vec!["100% done"]);
        assert_eq!(texts(&s.list("_", None, 0, 50).unwrap()), vec!["a_b"]);
        assert_eq!(texts(&s.list("\"hi\" AND", None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert_eq!(texts(&s.list("bye*", None, 0, 50).unwrap()), vec!["say \"hi\" AND bye*"]);
        assert!(s.list("NOT", None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn filters_by_kind() {
        let (s, _d) = store();
        s.upsert(text("hello"), None, 1).unwrap();
        s.upsert(text("https://example.com"), None, 2).unwrap();
        s.upsert(NewClip::Files(vec!["/tmp/x".into()]), None, 3).unwrap();
        let links = s.list("", Some(Kind::Link), 0, 50).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, Kind::Link);
        assert_eq!(s.list("", Some(Kind::Files), 0, 50).unwrap().len(), 1);
        assert_eq!(s.list("", Some(Kind::Text), 0, 50).unwrap().len(), 1);
    }

    #[test]
    fn files_report_count_missing_and_folder() {
        let (s, d) = store();
        let file = d.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let path = |p: &Path| p.to_string_lossy().into_owned();
        s.upsert(NewClip::Files(vec![path(&file)]), None, 1).unwrap();
        s.upsert(NewClip::Files(vec![path(d.path())]), None, 2).unwrap();
        s.upsert(NewClip::Files(vec!["/definitely/not/here.txt".into(), path(&file)]), None, 3).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!((all[0].missing, all[0].is_dir, all[0].meta.as_deref()), (true, false, Some("2")));
        assert_eq!((all[1].missing, all[1].is_dir, all[1].meta.as_deref()), (false, true, Some("1")));
        assert_eq!((all[2].missing, all[2].is_dir, all[2].meta.as_deref()), (false, false, Some("1")));
    }

    #[test]
    fn large_text_preview_is_capped_but_counted() {
        let (s, _d) = store();
        s.upsert(text(&"가".repeat(10_000)), None, 1).unwrap();
        let clip = &s.list("", None, 0, 50).unwrap()[0];
        assert_eq!(clip.text_preview.as_ref().unwrap().chars().count(), 500);
        assert_eq!(clip.char_count, 10_000);
    }

    #[test]
    fn content_of_missing_id_is_none() {
        let (s, _d) = store();
        assert_eq!(s.content(42).unwrap(), None);
    }

    #[test]
    fn content_round_trips_text_and_files() {
        let (s, _d) = store();
        s.upsert(text("hi"), None, 1).unwrap();
        s.upsert(NewClip::Files(vec!["/a".into(), "/b".into()]), None, 2).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        assert_eq!(s.content(all[0].id).unwrap(), Some(ClipContent::Files(vec!["/a".into(), "/b".into()])));
        assert_eq!(s.content(all[1].id).unwrap(), Some(ClipContent::Text("hi".into())));
    }

    #[test]
    fn image_has_thumb_and_dimensions() {
        let (s, _d) = store();
        s.upsert(image(1), None, 1).unwrap();
        let clip = &s.list("", None, 0, 50).unwrap()[0];
        assert_eq!(clip.kind, Kind::Image);
        assert_eq!(clip.meta.as_deref(), Some("10×20"));
        assert!(clip.thumb.as_ref().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn delete_and_clear_remove_image_files() {
        let (s, _d) = store();
        s.upsert(image(1), None, 1).unwrap();
        s.upsert(image(2), None, 2).unwrap();
        let all = s.list("", None, 0, 50).unwrap();
        let (a, b) = (image_path(&s, all[0].id), image_path(&s, all[1].id));
        s.delete(all[0].id).unwrap();
        assert!(!a.exists() && b.exists());
        s.clear().unwrap();
        assert!(!b.exists());
        assert!(s.list("", None, 0, 50).unwrap().is_empty());
    }

    #[test]
    fn touch_moves_clip_to_front() {
        let (s, _d) = store();
        s.upsert(text("old"), None, 1).unwrap();
        s.upsert(text("new"), None, 2).unwrap();
        let old = s.list("", None, 0, 50).unwrap()[1].id;
        s.touch(old, 3).unwrap();
        assert_eq!(texts(&s.list("", None, 0, 50).unwrap()), vec!["old", "new"]);
    }

    #[test]
    fn upsert_app_keeps_existing_icon() {
        let (s, _d) = store();
        assert!(!s.app_known("com.a").unwrap());
        let first = s.upsert_app("com.a", "A", Some(&[1, 2])).unwrap();
        let second = s.upsert_app("com.a", "A2", None).unwrap();
        assert_eq!(first, second);
        assert!(s.app_known("com.a").unwrap());
        s.upsert(text("x"), Some(first), 1).unwrap();
        let clip = &s.list("", None, 0, 50).unwrap()[0];
        assert_eq!(clip.app_name.as_deref(), Some("A2"));
        assert!(clip.app_icon.is_some());
    }

    #[test]
    fn settings_round_trip() {
        let (s, _d) = store();
        assert_eq!(s.get_setting("theme").unwrap(), None);
        s.set_setting("theme", "dark").unwrap();
        s.set_setting("theme", "light").unwrap();
        assert_eq!(s.get_setting("theme").unwrap().as_deref(), Some("light"));
    }

    #[test]
    fn history_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("vee.db");
        let images = dir.path().join("images");
        Store::open(&db, &images).unwrap().upsert(text("persist me"), None, 1).unwrap();
        let reopened = Store::open(&db, &images).unwrap();
        assert_eq!(texts(&reopened.list("", None, 0, 50).unwrap()), vec!["persist me"]);
    }
}
```

- [ ] **Step 3: 테스트가 실패하는지 확인**

Run: `cargo test --manifest-path src-tauri/Cargo.toml store::`
Expected: 컴파일 실패 (`Store`, `NewClip` 등이 정의되지 않음)

- [ ] **Step 4: 구현 작성** — `src-tauri/src/store.rs`의 테스트 모듈 **위에** 추가

```rust
//! SQLite-backed clip history. Deliberately Tauri-free so it can be unit tested.

use base64::{Engine, engine::general_purpose::STANDARD};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter, types::Value};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub const MAX_CLIPS: i64 = 1000;
const PREVIEW_CHARS: i64 = 500;

const SCHEMA_V1: &str = "
BEGIN;
CREATE TABLE apps (
  id        INTEGER PRIMARY KEY,
  bundle_id TEXT UNIQUE NOT NULL,
  name      TEXT NOT NULL,
  icon_png  BLOB
);
CREATE TABLE clips (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('text','link','image','files')),
  hash         TEXT UNIQUE NOT NULL,
  text         TEXT,
  image_path   TEXT,
  thumb_png    BLOB,
  meta         TEXT,
  app_id       INTEGER REFERENCES apps(id),
  created_at   INTEGER NOT NULL,
  last_used_at INTEGER NOT NULL
);
CREATE INDEX clips_last_used ON clips(last_used_at DESC);
CREATE VIRTUAL TABLE clips_fts USING fts5(text, content='clips', content_rowid='id', tokenize='trigram');
CREATE TRIGGER clips_ai AFTER INSERT ON clips BEGIN
  INSERT INTO clips_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER clips_ad AFTER DELETE ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER clips_au AFTER UPDATE OF text ON clips BEGIN
  INSERT INTO clips_fts(clips_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO clips_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
PRAGMA user_version = 1;
COMMIT;
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Link,
    Image,
    Files,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Link => "link",
            Kind::Image => "image",
            Kind::Files => "files",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "text" => Some(Kind::Text),
            "link" => Some(Kind::Link),
            "image" => Some(Kind::Image),
            "files" => Some(Kind::Files),
            _ => None,
        }
    }
}

pub enum NewClip {
    Text(String),
    Image { png: Vec<u8>, thumb_png: Vec<u8>, width: u32, height: u32 },
    Files(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub enum ClipContent {
    Text(String),
    Image(PathBuf),
    Files(Vec<String>),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDto {
    pub id: i64,
    pub kind: Kind,
    pub text_preview: Option<String>,
    pub char_count: i64,
    pub thumb: Option<String>,
    pub meta: Option<String>,
    pub app_name: Option<String>,
    pub app_icon: Option<String>,
    pub last_used_at: i64,
    pub missing: bool,
    pub is_dir: bool,
}

/// A single URL (no whitespace inside) is a link; anything else is text.
pub fn classify_text(text: &str) -> Kind {
    let t = text.trim();
    let is_url = ["http://", "https://", "ssh://", "git@"].iter().any(|p| t.starts_with(p));
    if is_url && !t.chars().any(char::is_whitespace) { Kind::Link } else { Kind::Text }
}

fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(png))
}

fn escape_like(q: &str) -> String {
    q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Quote the whole query as one FTS5 phrase so operators like AND/NOT/* are literal.
fn fts_phrase(q: &str) -> String {
    format!("\"{}\"", q.replace('"', "\"\""))
}

pub struct Store {
    conn: Connection,
    images_dir: PathBuf,
}

impl Store {
    pub fn open(db_path: &Path, images_dir: &Path) -> Result<Store> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::init(Connection::open(db_path)?, images_dir)
    }

    pub fn open_in_memory(images_dir: &Path) -> Result<Store> {
        Self::init(Connection::open_in_memory()?, images_dir)
    }

    fn init(conn: Connection, images_dir: &Path) -> Result<Store> {
        std::fs::create_dir_all(images_dir)?;
        conn.query_row("PRAGMA journal_mode=WAL", [], |_| Ok(()))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            conn.execute_batch(SCHEMA_V1)?;
        }
        Ok(Store { conn, images_dir: images_dir.to_path_buf() })
    }

    pub fn app_known(&self, bundle_id: &str) -> Result<bool> {
        let found = self
            .conn
            .query_row("SELECT 1 FROM apps WHERE bundle_id = ?1", [bundle_id], |_| Ok(()))
            .optional()?;
        Ok(found.is_some())
    }

    pub fn upsert_app(&self, bundle_id: &str, name: &str, icon_png: Option<&[u8]>) -> Result<i64> {
        Ok(self.conn.query_row(
            "INSERT INTO apps (bundle_id, name, icon_png) VALUES (?1, ?2, ?3)
             ON CONFLICT(bundle_id) DO UPDATE SET
               name = excluded.name,
               icon_png = COALESCE(excluded.icon_png, apps.icon_png)
             RETURNING id",
            params![bundle_id, name, icon_png],
            |r| r.get(0),
        )?)
    }

    /// Inserts a new clip, or moves an identical one to the front.
    pub fn upsert(&self, clip: NewClip, app_id: Option<i64>, now: i64) -> Result<()> {
        let hash = match &clip {
            NewClip::Text(t) => sha256_hex(&[b"text\0", t.as_bytes()]),
            NewClip::Image { png, .. } => sha256_hex(&[b"image\0", png]),
            NewClip::Files(f) => sha256_hex(&[b"files\0", f.join("\n").as_bytes()]),
        };
        let bumped = self.conn.execute(
            "UPDATE clips SET last_used_at = ?1, app_id = ?2 WHERE hash = ?3",
            params![now, app_id, hash],
        )?;
        if bumped > 0 {
            return Ok(());
        }
        let (kind, text, image_path, thumb, meta) = match clip {
            NewClip::Text(t) => (classify_text(&t), Some(t), None, None, None),
            NewClip::Image { png, thumb_png, width, height } => {
                let path = self.images_dir.join(format!("{hash}.png"));
                std::fs::write(&path, png)?;
                let path = path.to_string_lossy().into_owned();
                (Kind::Image, None, Some(path), Some(thumb_png), Some(format!("{width}×{height}")))
            }
            NewClip::Files(f) => {
                let count = f.len().to_string();
                (Kind::Files, Some(f.join("\n")), None, None, Some(count))
            }
        };
        self.conn.execute(
            "INSERT INTO clips (kind, hash, text, image_path, thumb_png, meta, app_id, created_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![kind.as_str(), hash, text, image_path, thumb, meta, app_id, now],
        )?;
        self.trim()
    }

    fn trim(&self) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM clips ORDER BY last_used_at DESC, id DESC LIMIT -1 OFFSET ?1")?;
        let ids: Vec<i64> = stmt.query_map([MAX_CLIPS], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for id in ids {
            self.delete(id)?;
        }
        Ok(())
    }

    pub fn list(&self, query: &str, kind: Option<Kind>, offset: i64, limit: i64) -> Result<Vec<ClipDto>> {
        let mut sql = String::from(
            "SELECT c.id, c.kind, substr(c.text, 1, ?), COALESCE(length(c.text), 0), c.thumb_png, c.meta,
                    a.name, a.icon_png, c.last_used_at,
                    CASE c.kind WHEN 'files' THEN c.text WHEN 'image' THEN c.image_path END
             FROM clips c LEFT JOIN apps a ON a.id = c.app_id
             WHERE 1 = 1",
        );
        let mut args = vec![Value::Integer(PREVIEW_CHARS)];
        if let Some(k) = kind {
            sql.push_str(" AND c.kind = ?");
            args.push(Value::Text(k.as_str().into()));
        }
        let q = query.trim();
        match q.chars().count() {
            0 => {}
            1 | 2 => {
                sql.push_str(" AND c.text LIKE ? ESCAPE '\\'");
                args.push(Value::Text(format!("%{}%", escape_like(q))));
            }
            _ => {
                sql.push_str(" AND c.id IN (SELECT rowid FROM clips_fts WHERE clips_fts MATCH ?)");
                args.push(Value::Text(fts_phrase(q)));
            }
        }
        sql.push_str(" ORDER BY c.last_used_at DESC, c.id DESC LIMIT ? OFFSET ?");
        args.push(Value::Integer(limit));
        args.push(Value::Integer(offset));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), |r| {
            let kind = Kind::parse(&r.get::<_, String>(1)?).unwrap_or(Kind::Text);
            let paths: Option<String> = r.get(9)?;
            let paths: Vec<&Path> = paths.as_deref().map(|p| p.lines().map(Path::new).collect()).unwrap_or_default();
            Ok(ClipDto {
                id: r.get(0)?,
                kind,
                text_preview: r.get(2)?,
                char_count: r.get(3)?,
                thumb: r.get::<_, Option<Vec<u8>>>(4)?.map(|b| data_url(&b)),
                meta: r.get(5)?,
                app_name: r.get(6)?,
                app_icon: r.get::<_, Option<Vec<u8>>>(7)?.map(|b| data_url(&b)),
                last_used_at: r.get(8)?,
                missing: paths.iter().any(|p| !p.exists()),
                is_dir: kind == Kind::Files && paths.len() == 1 && paths[0].is_dir(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn content(&self, id: i64) -> Result<Option<ClipContent>> {
        let row = self
            .conn
            .query_row("SELECT kind, text, image_path FROM clips WHERE id = ?1", [id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?))
            })
            .optional()?;
        Ok(row.map(|(kind, text, image_path)| match Kind::parse(&kind) {
            Some(Kind::Image) => ClipContent::Image(PathBuf::from(image_path.unwrap_or_default())),
            Some(Kind::Files) => ClipContent::Files(text.unwrap_or_default().lines().map(String::from).collect()),
            _ => ClipContent::Text(text.unwrap_or_default()),
        }))
    }

    pub fn touch(&self, id: i64, now: i64) -> Result<()> {
        self.conn.execute("UPDATE clips SET last_used_at = ?1 WHERE id = ?2", params![now, id])?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let image_path: Option<Option<String>> = self
            .conn
            .query_row("SELECT image_path FROM clips WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        self.conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
        if let Some(Some(path)) = image_path {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM clips", [])?;
        for entry in std::fs::read_dir(&self.images_dir)? {
            let _ = std::fs::remove_file(entry?.path());
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `cargo test --manifest-path src-tauri/Cargo.toml store::`
Expected: 16 passed. (`trims_oldest...`는 1,000건 삽입이라 수 초 걸릴 수 있다)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/store.rs src-tauri/src/lib.rs
git commit -m "feat: add SQLite clip store with trigram search"
```

---

### Task 3: 출처 앱 감지 + 클립보드 감시 + 클립 command

**Files:**
- Create: `src-tauri/src/source_app.rs`, `src-tauri/src/watcher.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs` (전체 교체)

**Interfaces:**
- Consumes: `store::{Store, NewClip, Kind, ClipDto}` (Task 2)
- Produces:
  - `source_app::FrontApp { pub bundle_id: String, pub name: String, pub path: String, pub pid: i32 }`
  - `source_app::frontmost() -> Option<FrontApp>`, `source_app::icon_png(&FrontApp) -> Option<Vec<u8>>`, `source_app::activate(pid: i32)` (macOS 전용 동작, Windows는 no-op)
  - `watcher::spawn(AppHandle)`, `watcher::text_clip(String) -> Option<NewClip>`
  - `crate::AppState { pub store: Mutex<Store>, pub prev_app_pid: Mutex<Option<i32>>, .. }` + `suppress_watcher()`, `watcher_suppressed()`
  - `crate::now_ms() -> i64`
  - 이벤트 `clips://changed` (payload 없음)
  - command `list_clips(query: String, kind: String, offset: i64, limit: i64) -> Result<Vec<ClipDto>, String>`, `delete_clip(id)`, `clear_history()`

- [ ] **Step 1: 의존성 추가** — `src-tauri/Cargo.toml`

```toml
# [dependencies]에 추가
clipboard-rs = "0.3.5"

[target.'cfg(target_os = "macos")'.dependencies]
objc2-app-kit = "0.3.2"
objc2-foundation = "0.3.2"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.62.2", features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging", "Win32_System_Threading"] }
windows-icons = "0.4.0"
image = { version = "0.25", default-features = false, features = ["png"] }
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src-tauri/src/watcher.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_only_text_is_ignored() {
        assert!(text_clip("   \n\t ".into()).is_none());
        assert!(text_clip(String::new()).is_none());
    }

    #[test]
    fn text_is_kept_verbatim() {
        match text_clip("  hi \n".into()) {
            Some(NewClip::Text(t)) => assert_eq!(t, "  hi \n"),
            _ => panic!("expected text clip"),
        }
    }
}
```

`src-tauri/src/lib.rs` 맨 위에 `mod watcher;` 추가 후

Run: `cargo test --manifest-path src-tauri/Cargo.toml watcher::`
Expected: 컴파일 실패 (`text_clip` 없음)

- [ ] **Step 3: `source_app.rs` 작성**

```rust
//! Which app is in front, its icon, and (macOS) re-activating it.

pub struct FrontApp {
    /// macOS bundle identifier, or the full exe path on Windows.
    pub bundle_id: String,
    pub name: String,
    /// .app bundle path (macOS) or exe path (Windows); used to load the icon.
    pub path: String,
    pub pid: i32,
}

#[cfg(target_os = "macos")]
pub fn frontmost() -> Option<FrontApp> {
    use objc2_app_kit::NSWorkspace;
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let bundle_id = app.bundleIdentifier()?.to_string();
    let name = app.localizedName().map(|n| n.to_string()).unwrap_or_else(|| bundle_id.clone());
    let path = app.bundleURL().and_then(|u| u.path()).map(|p| p.to_string()).unwrap_or_default();
    Some(FrontApp { bundle_id, name, path, pid: app.processIdentifier() })
}

#[cfg(target_os = "macos")]
pub fn icon_png(app: &FrontApp) -> Option<Vec<u8>> {
    use clipboard_rs::{RustImageData, common::RustImage};
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    if app.path.is_empty() {
        return None;
    }
    let tiff = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&app.path)).TIFFRepresentation()?;
    let image = RustImageData::from_bytes(&tiff.to_vec()).ok()?;
    Some(image.thumbnail(64, 64).ok()?.to_png().ok()?.get_bytes().to_vec())
}

#[cfg(target_os = "macos")]
pub fn activate(pid: i32) {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        app.activateWithOptions(NSApplicationActivationOptions::empty());
    }
}

#[cfg(windows)]
pub fn frontmost() -> Option<FrontApp> {
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use ::windows::core::PWSTR;
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let queried = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        queried.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let name = std::path::Path::new(&path).file_stem()?.to_string_lossy().into_owned();
        Some(FrontApp { bundle_id: path.clone(), name, path, pid: pid as i32 })
    }
}

#[cfg(windows)]
pub fn icon_png(app: &FrontApp) -> Option<Vec<u8>> {
    let icon = windows_icons::get_icon_by_path(&app.path).ok()?;
    let icon = image::imageops::thumbnail(&icon, 64, 64);
    let mut out = std::io::Cursor::new(Vec::new());
    icon.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Windows hands focus back to the previous window when ours hides, so nothing to do.
#[cfg(windows)]
pub fn activate(_pid: i32) {}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "needs a desktop session"]
    fn reads_frontmost_app_and_icon() {
        let app = super::frontmost().expect("frontmost app");
        assert!(!app.name.is_empty());
        let png = super::icon_png(&app).expect("icon");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
```

- [ ] **Step 4: `watcher.rs` 구현 작성** (테스트 모듈 위에)

```rust
//! Watches the system clipboard and records every change in the store.

use crate::{AppState, now_ms, source_app, store::NewClip};
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat,
    common::RustImage,
};
use tauri::{AppHandle, Emitter, Manager};

/// Pasteboard markers set by password managers and other privacy-aware apps.
const CONCEALED_FORMATS: [&str; 3] = [
    "org.nspasteboard.ConcealedType",
    "org.nspasteboard.TransientType",
    "ExcludeClipboardContentFromMonitorProcessing",
];
const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024;
const THUMB_SIZE: u32 = 320;

pub fn text_clip(text: String) -> Option<NewClip> {
    (!text.trim().is_empty()).then_some(NewClip::Text(text))
}

/// Files win over images, images over text: copying a file in Finder also
/// puts its icon and name on the pasteboard.
fn read(ctx: &ClipboardContext) -> clipboard_rs::Result<Option<NewClip>> {
    if CONCEALED_FORMATS.iter().any(|f| ctx.has(ContentFormat::Other(f.to_string()))) {
        return Ok(None);
    }
    if ctx.has(ContentFormat::Files) {
        let files = ctx.get_files()?;
        if !files.is_empty() {
            return Ok(Some(NewClip::Files(files)));
        }
    }
    if ctx.has(ContentFormat::Image) {
        let image = ctx.get_image()?;
        let png = image.to_png()?.get_bytes().to_vec();
        if png.len() > MAX_IMAGE_BYTES {
            log::info!("skipping image larger than 50MB");
            return Ok(None);
        }
        let (width, height) = image.get_size();
        let thumb_png = image.thumbnail(THUMB_SIZE, THUMB_SIZE)?.to_png()?.get_bytes().to_vec();
        return Ok(Some(NewClip::Image { png, thumb_png, width, height }));
    }
    if ctx.has(ContentFormat::Text) {
        return Ok(text_clip(ctx.get_text()?));
    }
    Ok(None)
}

struct Handler {
    app: AppHandle,
    ctx: ClipboardContext,
}

impl Handler {
    fn capture(&self) -> crate::store::Result<()> {
        let state = self.app.state::<AppState>();
        if state.watcher_suppressed() {
            return Ok(());
        }
        let Some(clip) = read(&self.ctx)? else { return Ok(()) };
        let front = source_app::frontmost();
        let store = state.store.lock().unwrap();
        let app_id = match front {
            Some(front) => {
                let icon = if store.app_known(&front.bundle_id)? { None } else { source_app::icon_png(&front) };
                Some(store.upsert_app(&front.bundle_id, &front.name, icon.as_deref())?)
            }
            None => None,
        };
        store.upsert(clip, app_id, now_ms())?;
        drop(store);
        self.app.emit("clips://changed", ())?;
        Ok(())
    }
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) {
        if let Err(e) = self.capture() {
            log::warn!("clipboard capture failed: {e}");
        }
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let (ctx, mut watcher) = match (ClipboardContext::new(), ClipboardWatcherContext::new()) {
            (Ok(ctx), Ok(watcher)) => (ctx, watcher),
            (Err(e), _) | (_, Err(e)) => return log::error!("clipboard watcher unavailable: {e}"),
        };
        watcher.add_handler(Handler { app, ctx });
        watcher.start_watch();
    });
}
```

- [ ] **Step 5: `lib.rs` 전체 교체**

```rust
mod source_app;
mod store;
mod watcher;

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use store::{ClipDto, Kind, Store};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub store: Mutex<Store>,
    /// Frontmost app when the panel opened, re-activated when it closes (macOS).
    pub prev_app_pid: Mutex<Option<i32>>,
    suppress_until: Mutex<Instant>,
}

impl AppState {
    fn new(store: Store) -> Self {
        Self { store: Mutex::new(store), prev_app_pid: Mutex::new(None), suppress_until: Mutex::new(Instant::now()) }
    }

    /// Ignore clipboard changes briefly after we write the clipboard ourselves.
    pub fn suppress_watcher(&self) {
        *self.suppress_until.lock().unwrap() = Instant::now() + Duration::from_millis(500);
    }

    pub fn watcher_suppressed(&self) -> bool {
        Instant::now() < *self.suppress_until.lock().unwrap()
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[tauri::command]
fn list_clips(state: State<AppState>, query: String, kind: String, offset: i64, limit: i64) -> Result<Vec<ClipDto>, String> {
    let kind = match kind.as_str() {
        "all" => None,
        k => Some(Kind::parse(k).ok_or_else(|| format!("unknown kind: {k}"))?),
    };
    state.store.lock().unwrap().list(&query, kind, offset.max(0), limit.clamp(1, 200)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_clip(app: AppHandle, state: State<AppState>, id: i64) -> Result<(), String> {
    state.store.lock().unwrap().delete(id).map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    state.store.lock().unwrap().clear().map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = Store::open(&data_dir.join("vee.db"), &data_dir.join("images"))?;
            app.manage(AppState::new(store));
            watcher::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_clips, delete_clip, clear_history])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: 테스트 통과 확인**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: watcher 2개 + store 16개 통과, `reads_frontmost_app_and_icon`은 ignored

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored reads_frontmost_app_and_icon`
Expected: PASS (터미널/IDE가 최전면 앱으로 잡힘)

- [ ] **Step 7: 실제 감시 수동 확인 (macOS)**

1. `yarn tauri dev` 실행
2. Chrome에서 텍스트 복사, Finder에서 파일 복사, 스크린샷을 클립보드로 복사(`Cmd+Ctrl+Shift+4`)
3. 다른 터미널에서:
```bash
sqlite3 ~/Library/Application\ Support/com.bobpark.vee/vee.db \
  "SELECT c.kind, substr(c.text,1,30), c.meta, a.name, length(a.icon_png) FROM clips c LEFT JOIN apps a ON a.id=c.app_id ORDER BY c.id DESC LIMIT 5;"
```
Expected: `files`/`image`/`text` 행과 각 출처 앱 이름·아이콘 길이(>0)

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src
git commit -m "feat: watch clipboard and record source app"
```

---

### Task 4: 창 제어 (panel/toast/settings) + 기본 단축키 + 복사

**Files:**
- Create: `src-tauri/src/windows.rs`, `src-tauri/src/settings.rs` (단축키 등록만, Task 5에서 확장)
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs` (전체 교체)

**Interfaces:**
- Consumes: `AppState`, `store::{ClipContent}`, `source_app::{frontmost, activate}`
- Produces:
  - 상수 `windows::{PANEL, TOAST, SETTINGS}`
  - `windows::toggle_panel(&AppHandle)`, `show_panel(&AppHandle)`, `hide_panel(&AppHandle, restore_focus: bool)`, `show_settings(&AppHandle)`, `copy_clip(&AppHandle, id: i64) -> Result<(), String>`
  - `windows::ToastPayload { ok: bool, text: Option<String>, files: usize, image: bool }` (serde camelCase)
  - 이벤트 `panel://opened` (panel 창 대상), `toast://show` (toast 창 대상, `ToastPayload`)
  - `settings::DEFAULT_SHORTCUT`, `settings::register_shortcut(&AppHandle, &str) -> Result<(), String>`
  - command `copy_clip(id)`, `hide_panel()`, `open_settings()`

- [ ] **Step 1: 의존성 추가** — `src-tauri/Cargo.toml` `[dependencies]`

```toml
tauri-plugin-global-shortcut = "2.4.0"
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src-tauri/src/windows.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_preview_collapses_whitespace_and_caps_length() {
        assert_eq!(toast_preview("a\n   b\tc"), "a b c");
        let long = "x".repeat(41);
        assert_eq!(toast_preview(&long), format!("{}…", "x".repeat(40)));
        assert_eq!(toast_preview(&"가".repeat(40)), "가".repeat(40));
    }
}
```

`lib.rs`에 `mod windows;` 추가 후

Run: `cargo test --manifest-path src-tauri/Cargo.toml windows::`
Expected: 컴파일 실패 (`toast_preview` 없음)

- [ ] **Step 3: `windows.rs` 구현** (테스트 모듈 위에)

```rust
//! Panel, toast and settings window behaviour, plus copying a clip back.

use crate::{AppState, now_ms, source_app, store::ClipContent};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub const PANEL: &str = "panel";
pub const TOAST: &str = "toast";
pub const SETTINGS: &str = "settings";

const PANEL_HEIGHT: f64 = 300.0;
const PANEL_MARGIN: f64 = 8.0;
const TOAST_WIDTH: f64 = 360.0;
const TOAST_HEIGHT: f64 = 52.0;
const TOAST_BOTTOM: f64 = 32.0;
const TOAST_MS: u64 = 1500;
const TOAST_PREVIEW_CHARS: usize = 40;

static TOAST_GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToastPayload {
    pub ok: bool,
    pub text: Option<String>,
    pub files: usize,
    pub image: bool,
}

impl ToastPayload {
    fn copied(content: &ClipContent) -> Self {
        match content {
            ClipContent::Text(t) => Self { ok: true, text: Some(toast_preview(t)), files: 0, image: false },
            ClipContent::Image(_) => Self { ok: true, text: None, files: 0, image: true },
            ClipContent::Files(f) => Self { ok: true, text: None, files: f.len(), image: false },
        }
    }

    fn failed() -> Self {
        Self { ok: false, text: None, files: 0, image: false }
    }
}

fn toast_preview(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= TOAST_PREVIEW_CHARS {
        return flat;
    }
    format!("{}…", flat.chars().take(TOAST_PREVIEW_CHARS).collect::<String>())
}

fn cursor_monitor(app: &AppHandle) -> Option<Monitor> {
    let pos = app.cursor_position().ok()?;
    app.monitor_from_point(pos.x, pos.y).ok().flatten().or_else(|| app.primary_monitor().ok().flatten())
}

pub fn toggle_panel(app: &AppHandle) {
    let visible = app.get_webview_window(PANEL).and_then(|p| p.is_visible().ok()).unwrap_or(false);
    if visible { hide_panel(app, true) } else { show_panel(app) }
}

pub fn show_panel(app: &AppHandle) {
    let Some(panel) = app.get_webview_window(PANEL) else { return };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.set_focus();
        return;
    }
    *app.state::<AppState>().prev_app_pid.lock().unwrap() = source_app::frontmost().map(|a| a.pid);
    if let Err(e) = place_and_show(app, &panel) {
        log::error!("failed to show panel: {e}");
    }
}

fn place_and_show(app: &AppHandle, panel: &WebviewWindow) -> tauri::Result<()> {
    if let Some(monitor) = cursor_monitor(app) {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let margin = (PANEL_MARGIN * scale) as i32;
        let height = (PANEL_HEIGHT * scale) as u32;
        let width = area.size.width.saturating_sub(2 * margin as u32);
        panel.set_size(PhysicalSize::new(width, height))?;
        panel.set_position(PhysicalPosition::new(
            area.position.x + margin,
            area.position.y + area.size.height as i32 - height as i32 - margin,
        ))?;
    }
    panel.show()?;
    panel.set_focus()?;
    app.emit_to(PANEL, "panel://opened", ())?;
    Ok(())
}

/// `restore_focus` re-activates the app that was in front before the panel opened.
/// Pass `false` when the user already clicked into another app.
pub fn hide_panel(app: &AppHandle, restore_focus: bool) {
    if let Some(panel) = app.get_webview_window(PANEL) {
        let _ = panel.hide();
    }
    let previous = app.state::<AppState>().prev_app_pid.lock().unwrap().take();
    if let (true, Some(pid)) = (restore_focus, previous) {
        source_app::activate(pid);
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn show_toast(app: &AppHandle, payload: ToastPayload) {
    let Some(toast) = app.get_webview_window(TOAST) else { return };
    if let Some(monitor) = cursor_monitor(app) {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let (w, h) = ((TOAST_WIDTH * scale) as i32, (TOAST_HEIGHT * scale) as i32);
        let _ = toast.set_size(PhysicalSize::new(w as u32, h as u32));
        let _ = toast.set_position(PhysicalPosition::new(
            area.position.x + (area.size.width as i32 - w) / 2,
            area.position.y + area.size.height as i32 - h - (TOAST_BOTTOM * scale) as i32,
        ));
    }
    let _ = app.emit_to(TOAST, "toast://show", payload);
    let _ = toast.set_ignore_cursor_events(true);
    let _ = toast.show();

    let generation = TOAST_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(TOAST_MS));
        // A newer toast restarted the timer; let it hide the window.
        if TOAST_GENERATION.load(Ordering::SeqCst) == generation {
            if let Some(toast) = app.get_webview_window(TOAST) {
                let _ = toast.hide();
            }
        }
    });
}

fn write_clipboard(state: &AppState, content: &ClipContent) -> Result<(), String> {
    use clipboard_rs::{Clipboard, ClipboardContext, RustImageData, common::RustImage};
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    state.suppress_watcher();
    match content {
        ClipContent::Text(text) => ctx.set_text(text.clone()),
        ClipContent::Image(path) => {
            let image = RustImageData::from_path(&path.to_string_lossy()).map_err(|e| e.to_string())?;
            ctx.set_image(image)
        }
        ClipContent::Files(files) => {
            if files.iter().any(|f| !std::path::Path::new(f).exists()) {
                return Err("a copied file no longer exists".into());
            }
            ctx.set_files(files.clone())
        }
    }
    .map_err(|e| e.to_string())
}

/// Copies a clip back to the clipboard, closes the panel and confirms with a toast.
pub fn copy_clip(app: &AppHandle, id: i64) -> Result<(), String> {
    let state = app.state::<AppState>();
    let content = state.store.lock().unwrap().content(id).map_err(|e| e.to_string())?;
    let result = match content {
        Some(content) => write_clipboard(&state, &content).map(|()| content),
        None => Err("clip no longer exists".into()),
    };
    hide_panel(app, true);
    match result {
        Ok(content) => {
            let _ = state.store.lock().unwrap().touch(id, now_ms());
            let _ = app.emit("clips://changed", ());
            show_toast(app, ToastPayload::copied(&content));
            Ok(())
        }
        Err(e) => {
            log::warn!("copy failed: {e}");
            show_toast(app, ToastPayload::failed());
            Err(e)
        }
    }
}
```

- [ ] **Step 4: `settings.rs` 작성 (단축키 등록만)**

```rust
//! User settings and the global shortcut.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

pub fn register_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(accel, |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                crate::windows::toggle_panel(app);
            }
        })
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 5: `lib.rs` 전체 교체**

```rust
mod settings;
mod source_app;
mod store;
mod watcher;
mod windows;

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use store::{ClipDto, Kind, Store};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

pub struct AppState {
    pub store: Mutex<Store>,
    /// Frontmost app when the panel opened, re-activated when it closes (macOS).
    pub prev_app_pid: Mutex<Option<i32>>,
    suppress_until: Mutex<Instant>,
}

impl AppState {
    fn new(store: Store) -> Self {
        Self { store: Mutex::new(store), prev_app_pid: Mutex::new(None), suppress_until: Mutex::new(Instant::now()) }
    }

    /// Ignore clipboard changes briefly after we write the clipboard ourselves.
    pub fn suppress_watcher(&self) {
        *self.suppress_until.lock().unwrap() = Instant::now() + Duration::from_millis(500);
    }

    pub fn watcher_suppressed(&self) -> bool {
        Instant::now() < *self.suppress_until.lock().unwrap()
    }
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[tauri::command]
fn list_clips(state: State<AppState>, query: String, kind: String, offset: i64, limit: i64) -> Result<Vec<ClipDto>, String> {
    let kind = match kind.as_str() {
        "all" => None,
        k => Some(Kind::parse(k).ok_or_else(|| format!("unknown kind: {k}"))?),
    };
    state.store.lock().unwrap().list(&query, kind, offset.max(0), limit.clamp(1, 200)).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_clip(app: AppHandle, state: State<AppState>, id: i64) -> Result<(), String> {
    state.store.lock().unwrap().delete(id).map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
fn clear_history(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    state.store.lock().unwrap().clear().map_err(|e| e.to_string())?;
    let _ = app.emit("clips://changed", ());
    Ok(())
}

#[tauri::command]
fn copy_clip(app: AppHandle, id: i64) -> Result<(), String> {
    windows::copy_clip(&app, id)
}

#[tauri::command]
fn hide_panel(app: AppHandle) {
    windows::hide_panel(&app, true);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    windows::hide_panel(&app, false);
    windows::show_settings(&app);
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = Store::open(&data_dir.join("vee.db"), &data_dir.join("images"))?;
            app.manage(AppState::new(store));
            watcher::spawn(app.handle().clone());
            if let Err(e) = settings::register_shortcut(app.handle(), settings::DEFAULT_SHORTCUT) {
                log::error!("failed to register shortcut: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (windows::SETTINGS, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            (windows::PANEL, WindowEvent::Focused(false)) => windows::hide_panel(window.app_handle(), false),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            list_clips,
            delete_clip,
            clear_history,
            copy_clip,
            hide_panel,
            open_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: 테스트 + 빌드 확인**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 모두 통과 (`windows::tests::toast_preview...` 포함)

- [ ] **Step 7: 수동 확인 (macOS)**

1. `yarn tauri dev`
2. `Cmd+Shift+V` → 커서가 있는 모니터 하단에 "panel" 글자가 있는 창이 뜬다. 다시 누르면 사라지고 직전 앱이 다시 활성화된다
3. 패널을 띄운 뒤 다른 앱 클릭 → 패널이 숨겨진다
4. 패널을 띄우고 우클릭 → 검사(devtools) 콘솔에서 `await window.__TAURI_INTERNALS__.invoke("copy_clip", { id: 1 })` → 패널이 닫히고 하단에 toast 창이 1.5초 떴다 사라지며, 다른 앱에서 `Cmd+V`로 1번 항목이 붙여넣어진다
5. 존재하지 않는 id(`{ id: 999999 }`)로 같은 호출 → 패닉 없이 Promise가 reject되고 toast가 뜬다

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src
git commit -m "feat: panel/toast windows, global shortcut and copy back"
```

---

### Task 5: 설정 백엔드 + 프론트엔드 기반 (테마·i18n·toast 화면)

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/settings.rs` (전체 교체), `src-tauri/src/lib.rs` (부분 수정)
- Create: `src/api.ts`, `src/prefs.tsx`, `src/theme.css`, `src/i18n/en.ts`, `src/i18n/ko.ts`, `src/i18n/index.ts`, `src/toast/Toast.tsx`, `src/toast/toast.css`, `src/panel/Panel.tsx`(임시), `src/settings/Settings.tsx`(임시)
- Modify: `src/main.tsx` (전체 교체)

**Interfaces:**
- Consumes: `AppState.store`, `store::Store::{get_setting, set_setting}`, `settings::register_shortcut`
- Produces:
  - Rust: `settings::get(&Store, key) -> String`(기본값 포함), `settings::register_stored_shortcut(&AppHandle)`, command `get_settings() -> SettingsDto`, `set_setting(key, value)`, `set_autostart(enabled: bool)`, `set_shortcut(accel)`; 이벤트 `settings://changed`
  - TS: `api` 객체(아래 `api.ts`), 타입 `Kind`, `Filter`, `Clip`, `Settings`, `UpdateStatus`, `ToastPayload`; `usePrefs(): { settings, locale, t, reload }`; `dicts`, `resolveLocale`, `relativeTime`; `Dict` 타입

- [ ] **Step 1: 의존성 추가** — `src-tauri/Cargo.toml` `[dependencies]`

```toml
tauri-plugin-autostart = "2.7.0"
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src-tauri/src/settings.rs` 끝에 추가

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_known_settings_only() {
        assert!(validate("theme", "dark").is_ok());
        assert!(validate("locale", "ko").is_ok());
        assert!(validate("theme", "purple").is_err());
        assert!(validate("shortcut", "Alt+X").is_err()); // shortcuts go through set_shortcut
        assert!(validate("unknown", "x").is_err());
    }

    #[test]
    fn missing_settings_fall_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        assert_eq!(get(&store, "theme"), "system");
        assert_eq!(get(&store, "shortcut"), DEFAULT_SHORTCUT);
        store.set_setting("theme", "dark").unwrap();
        assert_eq!(get(&store, "theme"), "dark");
    }
}
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml settings::`
Expected: 컴파일 실패 (`validate`, `get` 없음)

- [ ] **Step 3: `settings.rs` 전체 교체** (테스트 모듈은 그대로 끝에 유지)

```rust
//! User settings and the global shortcut.

use crate::{AppState, store::Store};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+V";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    theme: String,
    locale: String,
    shortcut: String,
    autostart: bool,
    version: String,
}

/// Stored value, or the default for a known key.
pub fn get(store: &Store, key: &str) -> String {
    let default = match key {
        "theme" | "locale" => "system",
        "shortcut" => DEFAULT_SHORTCUT,
        _ => "",
    };
    store.get_setting(key).ok().flatten().unwrap_or_else(|| default.to_string())
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    let ok = match key {
        "theme" => matches!(value, "system" | "light" | "dark"),
        "locale" => matches!(value, "system" | "ko" | "en"),
        _ => false,
    };
    if ok { Ok(()) } else { Err(format!("invalid setting {key}={value}")) }
}

pub fn register_shortcut(app: &AppHandle, accel: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(accel, |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                crate::windows::toggle_panel(app);
            }
        })
        .map_err(|e| e.to_string())
}

/// Registers the saved shortcut at startup, falling back to the default.
pub fn register_stored_shortcut(app: &AppHandle) {
    let stored = get(&app.state::<AppState>().store.lock().unwrap(), "shortcut");
    if let Err(e) = register_shortcut(app, &stored) {
        log::error!("failed to register shortcut {stored}: {e}");
        if stored != DEFAULT_SHORTCUT {
            if let Err(e) = register_shortcut(app, DEFAULT_SHORTCUT) {
                log::error!("failed to register default shortcut: {e}");
            }
        }
    }
}

fn changed(app: &AppHandle) {
    let _ = app.emit("settings://changed", ());
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> SettingsDto {
    let store = state.store.lock().unwrap();
    SettingsDto {
        theme: get(&store, "theme"),
        locale: get(&store, "locale"),
        shortcut: get(&store, "shortcut"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn set_setting(app: AppHandle, state: State<AppState>, key: String, value: String) -> Result<(), String> {
    validate(&key, &value)?;
    state.store.lock().unwrap().set_setting(&key, &value).map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }.map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

/// Swaps the global shortcut; on failure the previous one is restored.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<AppState>, accel: String) -> Result<(), String> {
    let old = get(&state.store.lock().unwrap(), "shortcut");
    if accel == old {
        return Ok(());
    }
    let _ = app.global_shortcut().unregister(old.as_str());
    if let Err(e) = register_shortcut(&app, &accel) {
        let _ = register_shortcut(&app, &old);
        return Err(e);
    }
    state.store.lock().unwrap().set_setting("shortcut", &accel).map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}
```

- [ ] **Step 4: `lib.rs` 수정**

1. `.plugin(tauri_plugin_global_shortcut::Builder::new().build())` 다음 줄에 추가:
```rust
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
```
2. setup 안의 아래 블록을
```rust
            if let Err(e) = settings::register_shortcut(app.handle(), settings::DEFAULT_SHORTCUT) {
                log::error!("failed to register shortcut: {e}");
            }
```
다음 한 줄로 교체:
```rust
            settings::register_stored_shortcut(app.handle());
```
3. `generate_handler!` 목록 끝에 추가: `settings::get_settings, settings::set_setting, settings::set_autostart, settings::set_shortcut`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 모두 통과 (settings 2개 포함)

- [ ] **Step 5: `src/api.ts` 작성**

```ts
import { invoke } from "@tauri-apps/api/core";

export type Kind = "text" | "link" | "image" | "files";
export type Filter = Kind | "all";

export interface Clip {
  id: number;
  kind: Kind;
  textPreview: string | null;
  charCount: number;
  thumb: string | null;
  meta: string | null;
  appName: string | null;
  appIcon: string | null;
  lastUsedAt: number;
  missing: boolean;
  isDir: boolean;
}

export interface Settings {
  theme: "system" | "light" | "dark";
  locale: "system" | "ko" | "en";
  shortcut: string;
  autostart: boolean;
  version: string;
}

export type UpdateStatus =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "upToDate"; checkedAt: number }
  | { status: "ready"; version: string }
  | { status: "failed"; checkedAt: number };

export interface ToastPayload {
  ok: boolean;
  text: string | null;
  files: number;
  image: boolean;
}

export const api = {
  listClips: (query: string, kind: Filter, offset: number, limit: number) =>
    invoke<Clip[]>("list_clips", { query, kind, offset, limit }),
  copyClip: (id: number) => invoke<void>("copy_clip", { id }),
  deleteClip: (id: number) => invoke<void>("delete_clip", { id }),
  clearHistory: () => invoke<void>("clear_history"),
  hidePanel: () => invoke<void>("hide_panel"),
  openSettings: () => invoke<void>("open_settings"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSetting: (key: "theme" | "locale", value: string) => invoke<void>("set_setting", { key, value }),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  setShortcut: (accel: string) => invoke<void>("set_shortcut", { accel }),
  checkUpdate: () => invoke<UpdateStatus>("check_update"),
  getUpdateStatus: () => invoke<UpdateStatus>("get_update_status"),
  installUpdate: () => invoke<void>("install_update_and_restart"),
};
```

- [ ] **Step 6: i18n 사전 작성**

`src/i18n/en.ts`:
```ts
export const en = {
  search: "Search",
  filters: { all: "All", text: "Text", image: "Images", files: "Files", link: "Links" },
  kinds: { text: "Text", link: "Link", image: "Image", folder: "Folder" },
  filesCount: (n: number) => (n === 1 ? "1 file" : `${n} files`),
  moreFiles: (name: string, more: number) => (more > 0 ? `${name} +${more}` : name),
  chars: (n: number) => `${n.toLocaleString("en")} chars`,
  missing: "File not found",
  empty: "Nothing copied yet",
  noResults: "No results",
  copyHint: "⏎ Copy",
  copied: "Copied",
  copyFailed: "Couldn't copy — the original is gone",
  settings: {
    title: "Settings",
    theme: "Theme",
    language: "Language",
    system: "System",
    light: "Light",
    dark: "Dark",
    launchAtLogin: "Launch at login",
    shortcut: "Shortcut",
    shortcutHint: "Open the history panel",
    recording: "Press keys…",
    shortcutFailed: "Couldn't register this shortcut. It may be used by another app.",
    updates: "Updates",
    version: (v: string) => `v${v}`,
    notChecked: "Not checked yet",
    checking: "Checking…",
    upToDate: "Up to date",
    checkFailed: "Couldn't check for updates",
    updateReady: (v: string) => `v${v} is ready`,
    checkNow: "Check now",
    restartToUpdate: "Restart to update",
    history: "History",
    clearHistory: "Clear history",
    clearConfirm: "Delete all clipboard history? This can't be undone.",
  },
};

export type Dict = typeof en;
```

`src/i18n/ko.ts`:
```ts
import type { Dict } from "./en.ts";

export const ko: Dict = {
  search: "검색",
  filters: { all: "전체", text: "텍스트", image: "이미지", files: "파일", link: "링크" },
  kinds: { text: "텍스트", link: "링크", image: "이미지", folder: "폴더" },
  filesCount: (n: number) => `파일 ${n}개`,
  moreFiles: (name: string, more: number) => (more > 0 ? `${name} 외 ${more}개` : name),
  chars: (n: number) => `${n.toLocaleString("ko")}자`,
  missing: "파일 없음",
  empty: "아직 복사한 항목이 없어요",
  noResults: "검색 결과가 없어요",
  copyHint: "⏎ 복사",
  copied: "복사됨",
  copyFailed: "원본이 없어 복사하지 못했어요",
  settings: {
    title: "설정",
    theme: "테마",
    language: "언어",
    system: "시스템",
    light: "라이트",
    dark: "다크",
    launchAtLogin: "로그인 시 자동 실행",
    shortcut: "단축키",
    shortcutHint: "히스토리 패널 열기",
    recording: "키를 누르세요…",
    shortcutFailed: "이 단축키를 등록하지 못했어요. 다른 앱에서 사용 중일 수 있어요.",
    updates: "업데이트",
    version: (v: string) => `v${v}`,
    notChecked: "아직 확인하지 않음",
    checking: "확인 중…",
    upToDate: "최신 버전",
    checkFailed: "업데이트를 확인하지 못했어요",
    updateReady: (v: string) => `v${v} 준비됨`,
    checkNow: "업데이트 확인",
    restartToUpdate: "업데이트 후 재시작",
    history: "히스토리",
    clearHistory: "히스토리 전체 삭제",
    clearConfirm: "클립보드 히스토리를 모두 삭제할까요? 되돌릴 수 없어요.",
  },
};
```

`src/i18n/index.ts`:
```ts
import { en, type Dict } from "./en.ts";
import { ko } from "./ko.ts";

export type Locale = "ko" | "en";

export const dicts: Record<Locale, Dict> = { ko, en };

export function resolveLocale(setting: string): Locale {
  if (setting === "ko" || setting === "en") return setting;
  return navigator.language.toLowerCase().startsWith("ko") ? "ko" : "en";
}

export function relativeTime(ms: number, locale: Locale, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const sec = Math.round((ms - now) / 1000);
  const abs = Math.abs(sec);
  if (abs < 60) return rtf.format(sec, "second");
  if (abs < 3600) return rtf.format(Math.round(sec / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(sec / 3600), "hour");
  return rtf.format(Math.round(sec / 86400), "day");
}
```

- [ ] **Step 7: `src/prefs.tsx` 작성**

```tsx
import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Settings } from "./api.ts";
import { dicts, resolveLocale, type Locale } from "./i18n/index.ts";
import type { Dict } from "./i18n/en.ts";

interface Prefs {
  settings: Settings;
  locale: Locale;
  t: Dict;
  reload: () => Promise<void>;
}

const PrefsContext = createContext<Prefs | null>(null);
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

export function PrefsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [systemDark, setSystemDark] = useState(darkQuery.matches);

  const reload = useCallback(async () => {
    setSettings(await api.getSettings());
  }, []);

  useEffect(() => {
    void reload();
    const unlisten = listen("settings://changed", () => void reload());
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    darkQuery.addEventListener("change", onChange);
    return () => {
      void unlisten.then((off) => off());
      darkQuery.removeEventListener("change", onChange);
    };
  }, [reload]);

  const theme = settings?.theme === "light" || settings?.theme === "dark" ? settings.theme : systemDark ? "dark" : "light";
  const locale = resolveLocale(settings?.locale ?? "system");

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale;
  }, [theme, locale]);

  if (!settings) return null;
  return <PrefsContext.Provider value={{ settings, locale, t: dicts[locale], reload }}>{children}</PrefsContext.Provider>;
}

export function usePrefs(): Prefs {
  const prefs = useContext(PrefsContext);
  if (!prefs) throw new Error("usePrefs must be used inside PrefsProvider");
  return prefs;
}
```

- [ ] **Step 8: `src/theme.css` 작성**

```css
:root {
  --font: "Helvetica Neue", Helvetica, Arial, sans-serif;
  --mono: Menlo, Consolas, monospace;
  --radius: 12px;
}

:root[data-theme="light"] {
  --surface: #ffffff;
  --card: #ffffff;
  --border: #dedee5;
  --divider: #f0f0f4;
  --subtle: rgba(148, 151, 169, 0.08);
  --text: #101114;
  --muted: #9497a9;
  --accent: #7132f5;
  --accent-subtle: rgba(133, 91, 251, 0.16);
  --badge-bg: rgba(104, 107, 130, 0.12);
  --badge-fg: #484b5e;
  --ok-bg: rgba(20, 158, 97, 0.16);
  --ok-fg: #026b3f;
  --danger: #d1344b;
  --toast-bg: #101114;
  --toast-fg: #ffffff;
}

:root[data-theme="dark"] {
  --surface: #16171c;
  --card: #1f2027;
  --border: #2e2f39;
  --divider: #2a2b33;
  --subtle: rgba(148, 151, 169, 0.12);
  --text: #ececf1;
  --muted: #8a8da0;
  --accent: #9b7bff;
  --accent-subtle: rgba(133, 91, 251, 0.24);
  --badge-bg: rgba(148, 151, 169, 0.16);
  --badge-fg: #c9cad6;
  --ok-bg: rgba(20, 158, 97, 0.22);
  --ok-fg: #5fd39c;
  --danger: #ff6b81;
  --toast-bg: #ececf1;
  --toast-fg: #101114;
}

* {
  box-sizing: border-box;
  margin: 0;
}

html,
body,
#root {
  height: 100%;
}

body {
  font-family: var(--font);
  font-size: 13px;
  color: var(--text);
  background: var(--surface);
  -webkit-font-smoothing: antialiased;
  user-select: none;
  overflow: hidden;
}

html[data-window="panel"] body,
html[data-window="toast"] body {
  background: transparent;
}

button {
  font: inherit;
  color: inherit;
  border: none;
  background: none;
  cursor: pointer;
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
```

- [ ] **Step 9: toast 화면 작성**

`src/toast/Toast.tsx`:
```tsx
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ToastPayload } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import "./toast.css";

export function Toast() {
  const { t } = usePrefs();
  const [toast, setToast] = useState<{ payload: ToastPayload; key: number } | null>(null);

  useEffect(() => {
    const unlisten = listen<ToastPayload>("toast://show", (e) => setToast({ payload: e.payload, key: Date.now() }));
    return () => void unlisten.then((off) => off());
  }, []);

  if (!toast) return null;
  const p = toast.payload;
  if (!p.ok) {
    return (
      <div key={toast.key} className="toast toast-error" role="status">
        ⚠ {t.copyFailed}
      </div>
    );
  }
  const summary = p.image ? t.kinds.image : p.files > 0 ? t.filesCount(p.files) : p.text;
  return (
    <div key={toast.key} className="toast" role="status">
      <span className="toast-check">✓</span>
      <b>{t.copied}</b>
      {summary && <span className="toast-summary">· {summary}</span>}
    </div>
  );
}
```

`src/toast/toast.css`:
```css
.toast {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 0 16px;
  border-radius: var(--radius);
  background: var(--toast-bg);
  color: var(--toast-fg);
  white-space: nowrap;
  animation: toast-in 160ms ease-out;
}

.toast-check {
  color: #5fd39c;
  font-weight: 700;
}

.toast-summary {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  opacity: 0.8;
}

.toast-error {
  background: var(--danger);
  color: #ffffff;
}

@keyframes toast-in {
  from {
    transform: translateY(6px);
    opacity: 0;
  }
  to {
    transform: none;
    opacity: 1;
  }
}
```

- [ ] **Step 10: 임시 화면과 `main.tsx` 교체**

`src/panel/Panel.tsx` (Task 6에서 교체):
```tsx
export function Panel() {
  return null;
}
```

`src/settings/Settings.tsx` (Task 8에서 교체):
```tsx
export function Settings() {
  return null;
}
```

`src/main.tsx`:
```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PrefsProvider } from "./prefs.tsx";
import { Panel } from "./panel/Panel.tsx";
import { Settings } from "./settings/Settings.tsx";
import { Toast } from "./toast/Toast.tsx";
import "./theme.css";

const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
const View = label === "panel" ? Panel : label === "toast" ? Toast : Settings;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PrefsProvider>
      <View />
    </PrefsProvider>
  </StrictMode>,
);
```

- [ ] **Step 11: 확인**

Run: `yarn typecheck`
Expected: 오류 없음 (ko 사전에서 키를 하나 지우면 오류가 나는지 한 번 확인 후 되돌린다)

Run: `yarn tauri dev` → 패널을 띄우고 devtools에서 `copy_clip` 호출(Task 4 Step 7-4와 동일) → 하단 toast에 "✓ 복사됨 · {텍스트 앞부분}"이 보이고, 시스템을 다크 모드로 바꾸면 toast 색이 반전된다

- [ ] **Step 12: Commit**

```bash
git add src src-tauri
git commit -m "feat: settings commands, theme, i18n and toast view"
```

---

### Task 6: 패널 UI (B 시안)

**Files:**
- Create: `src/panel/keys.ts`, `src/panel/keys.test.ts`, `src/panel/Toolbar.tsx`, `src/panel/Card.tsx`, `src/panel/panel.css`
- Modify: `src/panel/Panel.tsx` (전체 교체)

**Interfaces:**
- Consumes: `api.{listClips, copyClip, deleteClip, hidePanel, openSettings}`, `usePrefs`, `relativeTime`, 이벤트 `clips://changed`, `panel://opened`
- Produces: `panelKeyAction(KeyInput): KeyAction`, `FILTERS: Filter[]`

- [ ] **Step 1: 실패하는 테스트 작성** — `src/panel/keys.test.ts`

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { panelKeyAction, type KeyInput } from "./keys.ts";

const press = (key: string, extra: Partial<KeyInput> = {}) =>
  panelKeyAction({ key, shiftKey: false, isComposing: false, queryEmpty: true, ...extra });

test("ignores every key while an IME composition is active", () => {
  for (const key of ["Enter", "ArrowLeft", "ArrowRight", "Backspace", "Tab", "Escape"]) {
    assert.equal(press(key, { isComposing: true }), null, key);
  }
});

test("enter copies, escape hides, arrows move", () => {
  assert.deepEqual(press("Enter"), { type: "copy" });
  assert.deepEqual(press("Escape"), { type: "hide" });
  assert.deepEqual(press("ArrowRight"), { type: "move", delta: 1 });
  assert.deepEqual(press("ArrowLeft"), { type: "move", delta: -1 });
});

test("backspace deletes only when the search box is empty", () => {
  assert.deepEqual(press("Backspace"), { type: "delete" });
  assert.equal(press("Backspace", { queryEmpty: false }), null);
  assert.deepEqual(press("Delete", { queryEmpty: false }), { type: "delete" });
});

test("tab cycles filters forward, shift+tab backward", () => {
  assert.deepEqual(press("Tab"), { type: "cycleFilter", delta: 1 });
  assert.deepEqual(press("Tab", { shiftKey: true }), { type: "cycleFilter", delta: -1 });
});

test("other keys are left to the search box", () => {
  assert.equal(press("a"), null);
});
```

Run: `yarn test`
Expected: FAIL (`./keys.ts` 없음)

- [ ] **Step 2: `src/panel/keys.ts` 구현**

```ts
export type KeyAction =
  | { type: "move"; delta: 1 | -1 }
  | { type: "copy" }
  | { type: "hide" }
  | { type: "cycleFilter"; delta: 1 | -1 }
  | { type: "delete" }
  | null;

export interface KeyInput {
  key: string;
  shiftKey: boolean;
  /** True while Hangul/IME composition owns the keyboard. */
  isComposing: boolean;
  queryEmpty: boolean;
}

export function panelKeyAction(e: KeyInput): KeyAction {
  if (e.isComposing) return null;
  switch (e.key) {
    case "ArrowRight":
      return { type: "move", delta: 1 };
    case "ArrowLeft":
      return { type: "move", delta: -1 };
    case "Enter":
      return { type: "copy" };
    case "Escape":
      return { type: "hide" };
    case "Tab":
      return { type: "cycleFilter", delta: e.shiftKey ? -1 : 1 };
    case "Delete":
      return { type: "delete" };
    case "Backspace":
      return e.queryEmpty ? { type: "delete" } : null;
    default:
      return null;
  }
}
```

Run: `yarn test`
Expected: 5 tests pass

- [ ] **Step 3: `src/panel/Toolbar.tsx` 작성**

```tsx
import type { RefObject } from "react";
import type { Filter } from "../api.ts";
import { usePrefs } from "../prefs.tsx";

export const FILTERS: Filter[] = ["all", "text", "image", "files", "link"];

interface Props {
  query: string;
  onQuery: (query: string) => void;
  filter: Filter;
  onFilter: (filter: Filter) => void;
  inputRef: RefObject<HTMLInputElement | null>;
  onSettings: () => void;
}

export function Toolbar({ query, onQuery, filter, onFilter, inputRef, onSettings }: Props) {
  const { t } = usePrefs();
  return (
    <div className="toolbar">
      <label className="search">
        <span aria-hidden>🔍</span>
        <input
          ref={inputRef}
          value={query}
          placeholder={t.search}
          aria-label={t.search}
          onChange={(e) => onQuery(e.target.value)}
          autoFocus
          spellCheck={false}
        />
      </label>
      <div className="chips" role="tablist">
        {FILTERS.map((f) => (
          <button
            key={f}
            role="tab"
            aria-selected={f === filter}
            className={f === filter ? "chip on" : "chip"}
            onClick={() => onFilter(f)}
            tabIndex={-1}
          >
            {t.filters[f]}
          </button>
        ))}
      </div>
      <span className="spacer" />
      <button className="icon-btn" onClick={onSettings} aria-label={t.settings.title} tabIndex={-1}>
        ⚙︎
      </button>
    </div>
  );
}
```

- [ ] **Step 4: `src/panel/Card.tsx` 작성**

```tsx
import type { Clip } from "../api.ts";
import type { Dict } from "../i18n/en.ts";
import { relativeTime } from "../i18n/index.ts";
import { usePrefs } from "../prefs.tsx";

interface Props {
  clip: Clip;
  selected: boolean;
  onSelect: () => void;
  onCopy: () => void;
}

function fileCount(clip: Clip): number {
  return Number(clip.meta ?? "1");
}

function badgeLabel(clip: Clip, t: Dict): string {
  switch (clip.kind) {
    case "files":
      return clip.isDir ? t.kinds.folder : t.filesCount(fileCount(clip));
    default:
      return t.kinds[clip.kind];
  }
}

function footLabel(clip: Clip, t: Dict): string {
  if (clip.missing) return t.missing;
  switch (clip.kind) {
    case "image":
      return clip.meta ?? "";
    case "files": {
      const first = (clip.textPreview ?? "").split("\n")[0];
      const name = first.split(/[\\/]/).filter(Boolean).pop() ?? first;
      return t.moreFiles(name, fileCount(clip) - 1);
    }
    default:
      return t.chars(clip.charCount);
  }
}

function Body({ clip }: { clip: Clip }) {
  if (clip.kind === "image") return clip.thumb ? <img className="thumb" src={clip.thumb} alt="" /> : null;
  if (clip.kind === "files") return <div className={clip.isDir ? "doc folder" : "doc"} aria-hidden />;
  return <p className={clip.kind === "link" ? "preview link" : "preview"}>{clip.textPreview}</p>;
}

export function Card({ clip, selected, onSelect, onCopy }: Props) {
  const { t, locale } = usePrefs();
  return (
    <div
      className={selected ? "card selected" : "card"}
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      onDoubleClick={onCopy}
    >
      <div className="card-head">
        {clip.appIcon ? (
          <img className="app-icon" src={clip.appIcon} alt={clip.appName ?? ""} title={clip.appName ?? ""} />
        ) : (
          <span className="app-icon app-icon-empty" aria-hidden />
        )}
        <span className={clip.kind === "files" ? "badge badge-file" : "badge"}>{badgeLabel(clip, t)}</span>
        <span className="time">{relativeTime(clip.lastUsedAt, locale)}</span>
      </div>
      <div className="card-body">
        <Body clip={clip} />
      </div>
      <div className={clip.missing ? "card-foot missing" : "card-foot"}>
        <span>{footLabel(clip, t)}</span>
        {selected && <span>{t.copyHint}</span>}
      </div>
    </div>
  );
}
```

- [ ] **Step 5: `src/panel/Panel.tsx` 전체 교체**

```tsx
import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type UIEvent, type WheelEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Clip, type Filter } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { Card } from "./Card.tsx";
import { panelKeyAction } from "./keys.ts";
import { FILTERS, Toolbar } from "./Toolbar.tsx";
import "./panel.css";

const PAGE = 50;

export function Panel() {
  const { t } = usePrefs();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [clips, setClips] = useState<Clip[]>([]);
  const [selected, setSelected] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const requestId = useRef(0);
  const loadingMore = useRef(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const rowRef = useRef<HTMLDivElement>(null);

  /** Reloads the first page. Responses from superseded searches are dropped. */
  const reload = useCallback(
    async (keepSelection: boolean) => {
      const id = ++requestId.current;
      const page = await api.listClips(query, filter, 0, PAGE);
      if (id !== requestId.current) return;
      setClips(page);
      setHasMore(page.length === PAGE);
      setSelected((s) => (keepSelection ? Math.max(0, Math.min(s, page.length - 1)) : 0));
      if (!keepSelection) rowRef.current?.scrollTo({ left: 0 });
    },
    [query, filter],
  );

  const loadMore = useCallback(async () => {
    if (!hasMore || loadingMore.current) return;
    loadingMore.current = true;
    const id = requestId.current;
    try {
      const page = await api.listClips(query, filter, clips.length, PAGE);
      if (id !== requestId.current) return;
      setClips((prev) => [...prev, ...page]);
      setHasMore(page.length === PAGE);
    } finally {
      loadingMore.current = false;
    }
  }, [query, filter, clips.length, hasMore]);

  useEffect(() => {
    void reload(false);
  }, [reload]);

  // Subscribe once; always call the latest reload.
  const reloadRef = useRef(reload);
  reloadRef.current = reload;
  useEffect(() => {
    const offChanged = listen("clips://changed", () => void reloadRef.current(true));
    const offOpened = listen("panel://opened", () => {
      setQuery("");
      setFilter("all");
      void reloadRef.current(false);
      inputRef.current?.focus();
    });
    return () => {
      void offChanged.then((off) => off());
      void offOpened.then((off) => off());
    };
  }, []);

  useEffect(() => {
    rowRef.current?.children[selected]?.scrollIntoView({ block: "nearest", inline: "nearest" });
    if (selected >= clips.length - 5) void loadMore();
  }, [selected, clips.length, loadMore]);

  const copy = (clip: Clip | undefined) => {
    if (clip) void api.copyClip(clip.id).catch(() => {});
  };

  const remove = (clip: Clip | undefined) => {
    if (clip) void api.deleteClip(clip.id);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const action = panelKeyAction({
      key: e.key,
      shiftKey: e.shiftKey,
      // WebKit reports keyCode 229 for the Enter that commits a Hangul syllable.
      isComposing: e.nativeEvent.isComposing || e.keyCode === 229,
      queryEmpty: query === "",
    });
    if (!action) return;
    e.preventDefault();
    switch (action.type) {
      case "move":
        setSelected((s) => Math.max(0, Math.min(s + action.delta, clips.length - 1)));
        break;
      case "copy":
        copy(clips[selected]);
        break;
      case "hide":
        void api.hidePanel();
        break;
      case "cycleFilter": {
        const n = FILTERS.length;
        setFilter(FILTERS[(FILTERS.indexOf(filter) + action.delta + n) % n]);
        break;
      }
      case "delete":
        remove(clips[selected]);
        break;
    }
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) e.currentTarget.scrollLeft += e.deltaY;
  };

  const onScroll = (e: UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    if (el.scrollLeft + el.clientWidth > el.scrollWidth - 400) void loadMore();
  };

  return (
    <div
      className="panel"
      onKeyDown={onKeyDown}
      // Keep keyboard focus in the search box no matter what is clicked.
      onMouseDown={(e) => {
        if (e.target !== inputRef.current) e.preventDefault();
      }}
    >
      <Toolbar
        query={query}
        onQuery={setQuery}
        filter={filter}
        onFilter={setFilter}
        inputRef={inputRef}
        onSettings={() => void api.openSettings()}
      />
      {clips.length === 0 ? (
        <div className="empty">{query || filter !== "all" ? t.noResults : t.empty}</div>
      ) : (
        <div className="row" ref={rowRef} role="listbox" onWheel={onWheel} onScroll={onScroll}>
          {clips.map((clip, i) => (
            <Card
              key={clip.id}
              clip={clip}
              selected={i === selected}
              onSelect={() => setSelected(i)}
              onCopy={() => copy(clip)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 6: `src/panel/panel.css` 작성**

```css
.panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 10px 12px 12px;
  border-radius: 16px;
  border: 1px solid var(--border);
  background: var(--surface);
  overflow: hidden;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 10px;
}

.search {
  flex: 0 0 220px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: var(--radius);
  background: var(--subtle);
  color: var(--muted);
}

.search input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font: inherit;
}

.chips {
  display: flex;
  gap: 6px;
}

.chip {
  padding: 4px 10px;
  border-radius: var(--radius);
  background: var(--subtle);
  font-size: 12px;
}

.chip.on {
  background: var(--accent-subtle);
  color: var(--accent);
  font-weight: 600;
}

.spacer {
  flex: 1;
}

.icon-btn {
  padding: 4px 6px;
  border-radius: 8px;
  color: var(--muted);
  font-size: 15px;
}

.icon-btn:hover {
  background: var(--subtle);
}

.row {
  flex: 1;
  display: flex;
  gap: 10px;
  padding: 4px;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}

.row::-webkit-scrollbar {
  display: none;
}

.empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--muted);
}

.card {
  flex: 0 0 180px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--card);
  box-shadow: 0 1px 4px rgba(16, 24, 40, 0.04);
  cursor: default;
}

.card.selected {
  border: 2px solid var(--accent);
  box-shadow: 0 0 0 4px var(--accent-subtle);
}

.card-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px;
}

.app-icon {
  width: 24px;
  height: 24px;
  flex: none;
  border-radius: 6px;
}

.app-icon-empty {
  background: var(--subtle);
  border: 1px solid var(--border);
}

.badge {
  overflow: hidden;
  padding: 2px 6px;
  border-radius: 6px;
  background: var(--badge-bg);
  color: var(--badge-fg);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.badge-file {
  background: var(--ok-bg);
  color: var(--ok-fg);
}

.time {
  margin-left: auto;
  color: var(--muted);
  font-size: 11px;
  white-space: nowrap;
}

.card-body {
  flex: 1;
  min-height: 0;
  padding: 0 8px;
  overflow: hidden;
}

.preview {
  display: -webkit-box;
  overflow: hidden;
  font-size: 12px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-all;
  -webkit-line-clamp: 8;
  -webkit-box-orient: vertical;
}

.preview.link {
  color: var(--accent);
}

.thumb {
  width: 100%;
  height: 100%;
  object-fit: contain;
  border-radius: 6px;
}

.doc {
  width: 48px;
  height: 58px;
  margin: 12px auto 0;
  border-radius: 4px;
  border: 1px solid var(--border);
  background: var(--subtle);
}

.doc.folder {
  width: 64px;
  height: 46px;
  margin-top: 22px;
  border: none;
  border-radius: 4px 6px 6px 6px;
  background: #5aa9f0;
}

.card-foot {
  display: flex;
  justify-content: space-between;
  gap: 6px;
  padding: 6px 8px;
  border-top: 1px solid var(--divider);
  color: var(--muted);
  font-size: 11px;
}

.card-foot span {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.card-foot.missing {
  color: var(--danger);
}
```

- [ ] **Step 7: 확인**

Run: `yarn test && yarn typecheck`
Expected: 테스트 5개 통과, 타입 오류 없음

수동 (`yarn tauri dev`, macOS):
1. 텍스트·링크·이미지·파일·폴더를 각각 복사 → `Cmd+Shift+V` → 카드가 B 시안처럼 보이고 왼쪽 상단에 출처 앱 아이콘, 파일/폴더는 초록 배지
2. 한글로 "클립" 입력 중 Enter → 카드가 복사되지 않고 글자만 확정된다
3. `→`/`←` 이동, `Tab` 필터 순환, `Enter` → 패널이 닫히고 "✓ 복사됨" toast, 직전 앱에서 `Cmd+V`로 붙여넣기 확인
4. 빈 검색창에서 `Backspace` → 선택 카드 삭제, 선택이 옆 카드로 유지
5. 휠로 가로 스크롤, 끝에 가까워지면 다음 50개 로드 (60개 이상 복사해 확인)
6. 톱니 버튼 → 패널이 닫히고 (빈) 설정 창이 열린다

- [ ] **Step 8: Commit**

```bash
git add src/panel
git commit -m "feat: clipboard history panel UI"
```

---

### Task 7: 트레이 · 단일 인스턴스 · 로그 · Dock 숨김 · DB 오류 처리

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs` (`run()` 교체), `src-tauri/src/settings.rs` (`set_setting`에 한 줄)

**Interfaces:**
- Consumes: `windows::{show_panel, show_settings}`, `settings::get`, `AppState`
- Produces: `tray::create(&AppHandle) -> tauri::Result<()>`, `tray::refresh(&AppHandle)`, `tray::resolve_locale(&str) -> &'static str`

- [ ] **Step 1: 의존성 추가** — `src-tauri/Cargo.toml` `[dependencies]`

```toml
tauri-plugin-single-instance = "2.5.2"
tauri-plugin-log = "2.10.0"
tauri-plugin-dialog = "2.8.1"
sys-locale = "0.3.2"
```

- [ ] **Step 2: 실패하는 테스트 작성** — `src-tauri/src/tray.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_locale_wins_over_system() {
        assert_eq!(resolve_locale("ko"), "ko");
        assert_eq!(resolve_locale("en"), "en");
        assert!(["ko", "en"].contains(&resolve_locale("system")));
        assert_eq!(labels("ko").quit, "종료");
        assert_eq!(labels("en").quit, "Quit");
    }
}
```

`lib.rs`에 `mod tray;` 추가 후 Run: `cargo test --manifest-path src-tauri/Cargo.toml tray::`
Expected: 컴파일 실패

- [ ] **Step 3: `tray.rs` 구현** (테스트 모듈 위에)

```rust
//! Menu-bar / system-tray icon.

use crate::{AppState, settings, windows};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

pub const TRAY_ID: &str = "main";

pub struct Labels {
    pub open: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
}

pub fn labels(locale: &str) -> Labels {
    if locale == "ko" {
        Labels { open: "열기", settings: "설정", quit: "종료" }
    } else {
        Labels { open: "Open", settings: "Settings", quit: "Quit" }
    }
}

/// Same rule as the frontend: "system" follows the OS language.
pub fn resolve_locale(setting: &str) -> &'static str {
    match setting {
        "ko" => "ko",
        "en" => "en",
        _ if sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("ko")) => "ko",
        _ => "en",
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let setting = settings::get(&app.state::<AppState>().store.lock().unwrap(), "locale");
    let l = labels(resolve_locale(&setting));
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", l.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", l.settings, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("bundle has an icon"))
        .tooltip("Vee")
        .menu(&menu)
        // Left click is reserved for double-click → panel; right click opens the menu.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => windows::show_panel(app),
            "settings" => windows::show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                windows::show_panel(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu after the language changes.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    match build_menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(e) => log::warn!("tray menu rebuild failed: {e}"),
    }
}
```

- [ ] **Step 4: `settings.rs`의 `set_setting` 수정** — `changed(&app);` 바로 위에 추가

```rust
    if key == "locale" {
        crate::tray::refresh(&app);
    }
```

- [ ] **Step 5: `lib.rs`의 `run()` 전체 교체**

```rust
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just opens the settings window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| windows::show_settings(app)))
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            let store = match Store::open(&data_dir.join("vee.db"), &data_dir.join("images")) {
                Ok(store) => store,
                Err(e) => {
                    // Never wipe the history automatically; tell the user and quit.
                    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                    log::error!("failed to open database: {e}");
                    app.dialog()
                        .message(format!("Vee couldn't open its history database.\n\n{e}"))
                        .title("Vee")
                        .kind(MessageDialogKind::Error)
                        .show(|_| std::process::exit(1));
                    return Ok(());
                }
            };
            app.manage(AppState::new(store));
            tray::create(app.handle())?;
            watcher::spawn(app.handle().clone());
            settings::register_stored_shortcut(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (windows::SETTINGS, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            (windows::PANEL, WindowEvent::Focused(false)) => windows::hide_panel(window.app_handle(), false),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            list_clips,
            delete_clip,
            clear_history,
            copy_clip,
            hide_panel,
            open_settings,
            settings::get_settings,
            settings::set_setting,
            settings::set_autostart,
            settings::set_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: 확인**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 모두 통과

수동 (`yarn tauri dev`, macOS):
1. Dock에 아이콘이 없고 메뉴바에 보라색 V 아이콘이 있다
2. 메뉴바 아이콘 더블클릭 → 패널이 열린다. **안 열리면** 스펙 8장 대체안대로 `show_menu_on_left_click(true)`로 바꾸고 메뉴의 "열기"로 연다(결정 내용을 커밋 메시지에 남긴다)
3. 우클릭 메뉴: 열기 / 설정 / 종료가 동작한다. 설정 창의 닫기 버튼 → 앱이 종료되지 않고 트레이에 남는다
4. 앱이 떠 있는 상태에서 `yarn tauri dev`를 한 번 더 실행하거나 빌드된 앱을 다시 열기 → 새 인스턴스 대신 설정 창이 열린다
5. 로그 파일 `~/Library/Logs/com.bobpark.vee/`가 생기고, 복사한 **내용**이 로그에 없다
6. DB 오류 처리: 앱 종료 → `chmod 000 ~/Library/Application\ Support/com.bobpark.vee/vee.db` → 실행 → 오류 다이얼로그 후 종료 → `chmod 644`로 복구하고 히스토리가 그대로인지 확인

- [ ] **Step 7: Commit**

```bash
git add src-tauri
git commit -m "feat: tray icon, single instance, logging and db error dialog"
```

---

### Task 8: 설정 창 UI (테마·언어·자동 실행·단축키·전체 삭제)

**Files:**
- Create: `src/settings/accelerator.ts`, `src/settings/accelerator.test.ts`, `src/settings/ShortcutRecorder.tsx`, `src/settings/settings.css`
- Modify: `src/settings/Settings.tsx` (전체 교체), `src-tauri/capabilities/default.json`, `package.json`

**Interfaces:**
- Consumes: `api.{setSetting, setAutostart, setShortcut, clearHistory}`, `usePrefs`
- Produces: `acceleratorFromEvent(KeyLike, mac: boolean): string | null`, `formatAccelerator(accel, mac): string`, Settings 안의 `Row` 컴포넌트(Task 9에서 사용)

- [ ] **Step 1: dialog JS 바인딩 설치 + 권한 추가**

Run: `yarn add @tauri-apps/plugin-dialog@^2.8.1`

`src-tauri/capabilities/default.json`의 `permissions`를 `["core:default", "dialog:default"]`로 변경

- [ ] **Step 2: 실패하는 테스트 작성** — `src/settings/accelerator.test.ts`

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { acceleratorFromEvent, formatAccelerator, type KeyLike } from "./accelerator.ts";

const ev = (over: Partial<KeyLike>): KeyLike => ({
  key: "v",
  code: "KeyV",
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...over,
});

test("Cmd on mac and Ctrl on windows both map to CommandOrControl", () => {
  assert.equal(acceleratorFromEvent(ev({ metaKey: true, shiftKey: true }), true), "CommandOrControl+Shift+KeyV");
  assert.equal(acceleratorFromEvent(ev({ ctrlKey: true, shiftKey: true }), false), "CommandOrControl+Shift+KeyV");
  assert.equal(acceleratorFromEvent(ev({ ctrlKey: true, altKey: true, code: "Digit1", key: "1" }), true), "Control+Alt+Digit1");
});

test("waits while only modifier keys are held", () => {
  assert.equal(acceleratorFromEvent(ev({ key: "Meta", code: "MetaLeft", metaKey: true }), true), null);
  assert.equal(acceleratorFromEvent(ev({ key: "Shift", code: "ShiftLeft", shiftKey: true }), false), null);
});

test("rejects shortcuts without a real modifier", () => {
  assert.equal(acceleratorFromEvent(ev({}), true), null);
  assert.equal(acceleratorFromEvent(ev({ shiftKey: true }), true), null);
});

test("formats for display", () => {
  assert.equal(formatAccelerator("CommandOrControl+Shift+V", true), "⌘ ⇧ V");
  assert.equal(formatAccelerator("CommandOrControl+Shift+KeyV", false), "Ctrl + Shift + V");
  assert.equal(formatAccelerator("Control+Alt+Digit1", true), "⌃ ⌥ 1");
});
```

Run: `yarn test`
Expected: FAIL (`./accelerator.ts` 없음)

- [ ] **Step 3: `src/settings/accelerator.ts` 구현**

```ts
const MODIFIER_KEYS = new Set(["Meta", "Control", "Alt", "Shift", "OS"]);

export interface KeyLike {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** Builds a Tauri global-shortcut accelerator, or null while the combo is incomplete/invalid. */
export function acceleratorFromEvent(e: KeyLike, mac: boolean): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const mods: string[] = [];
  if (mac ? e.metaKey : e.ctrlKey) mods.push("CommandOrControl");
  if (mac && e.ctrlKey) mods.push("Control");
  if (!mac && e.metaKey) mods.push("Super");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  const realModifiers = mods.filter((m) => m !== "Shift");
  if (realModifiers.length === 0) return null;
  return [...mods, e.code].join("+");
}

export function formatAccelerator(accel: string, mac: boolean): string {
  const names: Record<string, string> = mac
    ? { CommandOrControl: "⌘", Command: "⌘", Super: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧" }
    : { CommandOrControl: "Ctrl", Command: "Win", Super: "Win", Control: "Ctrl", Alt: "Alt", Shift: "Shift" };
  return accel
    .split("+")
    .map((part) => names[part] ?? part.replace(/^(Key|Digit)/, ""))
    .join(mac ? " " : " + ");
}
```

Run: `yarn test`
Expected: keys 5개 + accelerator 4개 통과

- [ ] **Step 4: `src/settings/ShortcutRecorder.tsx` 작성**

```tsx
import { useState, type KeyboardEvent } from "react";
import { api } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { acceleratorFromEvent, formatAccelerator } from "./accelerator.ts";

const isMac = navigator.userAgent.includes("Mac");

export function ShortcutRecorder({ value }: { value: string }) {
  const { t } = usePrefs();
  const [recording, setRecording] = useState(false);
  const [failed, setFailed] = useState(false);

  const onKeyDown = async (e: KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      setRecording(false);
      return;
    }
    const accel = acceleratorFromEvent(e, isMac);
    if (!accel) return;
    setRecording(false);
    try {
      await api.setShortcut(accel);
      setFailed(false);
    } catch {
      setFailed(true);
    }
  };

  return (
    <div className="recorder">
      <button
        className={recording ? "kbd recording" : "kbd"}
        onClick={() => {
          setRecording(true);
          setFailed(false);
        }}
        onKeyDown={(e) => void onKeyDown(e)}
        onBlur={() => setRecording(false)}
      >
        {recording ? t.settings.recording : formatAccelerator(value, isMac)}
      </button>
      {failed && (
        <span className="error" role="alert">
          {t.settings.shortcutFailed}
        </span>
      )}
    </div>
  );
}
```

- [ ] **Step 5: `src/settings/Settings.tsx` 전체 교체**

```tsx
import { useState, type ReactNode } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { ShortcutRecorder } from "./ShortcutRecorder.tsx";
import "./settings.css";

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div>
        {label}
        {hint && <small>{hint}</small>}
      </div>
      {children}
    </div>
  );
}

function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="seg" role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={o.value === value ? "on" : ""}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Settings() {
  const { settings, t } = usePrefs();
  const s = t.settings;
  const [error, setError] = useState<string | null>(null);
  const run = (p: Promise<unknown>) => void p.then(() => setError(null)).catch((e) => setError(String(e)));

  const clear = async () => {
    if (await ask(s.clearConfirm, { title: "Vee", kind: "warning" })) run(api.clearHistory());
  };

  return (
    <main className="settings">
      <h1>{s.title}</h1>
      <section>
        <Row label={s.theme}>
          <Segmented
            value={settings.theme}
            onChange={(v) => run(api.setSetting("theme", v))}
            options={[
              { value: "system", label: s.system },
              { value: "light", label: s.light },
              { value: "dark", label: s.dark },
            ]}
          />
        </Row>
        <Row label={s.language}>
          <Segmented
            value={settings.locale}
            onChange={(v) => run(api.setSetting("locale", v))}
            options={[
              { value: "system", label: s.system },
              { value: "ko", label: "한국어" },
              { value: "en", label: "English" },
            ]}
          />
        </Row>
        <Row label={s.launchAtLogin}>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label={s.launchAtLogin}
            checked={settings.autostart}
            onChange={(e) => run(api.setAutostart(e.target.checked))}
          />
        </Row>
        <Row label={s.shortcut} hint={s.shortcutHint}>
          <ShortcutRecorder value={settings.shortcut} />
        </Row>
      </section>
      <section>
        <Row label={s.history}>
          <button className="btn danger" onClick={() => void clear()}>
            {s.clearHistory}
          </button>
        </Row>
      </section>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </main>
  );
}
```

- [ ] **Step 6: `src/settings/settings.css` 작성**

```css
.settings {
  height: 100%;
  padding: 24px 28px;
  overflow-y: auto;
}

.settings h1 {
  margin-bottom: 16px;
  font-size: 22px;
  font-weight: 600;
}

.settings section {
  margin-bottom: 16px;
  padding: 0 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--card);
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 0;
  border-bottom: 1px solid var(--divider);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-row small {
  display: block;
  margin-top: 2px;
  color: var(--muted);
  font-size: 11px;
}

.seg {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border-radius: 10px;
  background: var(--subtle);
}

.seg button {
  padding: 4px 10px;
  border-radius: 8px;
  color: var(--muted);
  font-size: 12px;
}

.seg button.on {
  background: var(--card);
  color: var(--text);
  font-weight: 600;
  box-shadow: 0 1px 4px rgba(16, 24, 40, 0.08);
}

.switch {
  position: relative;
  width: 36px;
  height: 20px;
  appearance: none;
  border-radius: 10px;
  background: var(--border);
  cursor: pointer;
  transition: background 120ms;
}

.switch::after {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #ffffff;
  transition: transform 120ms;
}

.switch:checked {
  background: var(--accent);
}

.switch:checked::after {
  transform: translateX(16px);
}

.recorder {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  max-width: 260px;
  text-align: right;
}

.kbd {
  min-width: 96px;
  padding: 6px 12px;
  border-radius: 10px;
  background: var(--badge-bg);
  color: var(--badge-fg);
  font-weight: 600;
}

.kbd.recording {
  background: var(--accent-subtle);
  color: var(--accent);
}

.btn {
  padding: 8px 14px;
  border-radius: var(--radius);
  background: var(--accent-subtle);
  color: var(--accent);
  font-weight: 600;
  white-space: nowrap;
}

.btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.btn.danger {
  background: rgba(209, 52, 75, 0.12);
  color: var(--danger);
}

.error {
  color: var(--danger);
  font-size: 12px;
}
```

- [ ] **Step 7: 확인**

Run: `yarn test && yarn typecheck`
Expected: 통과

수동 (`yarn tauri dev`, macOS):
1. 트레이 → 설정 → 테마 라이트/다크/시스템 전환 시 설정 창·패널·toast가 즉시 바뀐다
2. 언어 한국어/English 전환 → 설정 창·패널 문구와 **트레이 메뉴**가 바뀐다
3. 로그인 시 자동 실행 on → `~/Library/LaunchAgents/`에 Vee plist 생성, off → 삭제
4. 단축키 클릭 → `Cmd+Option+V` 입력 → 표시가 `⌘ ⌥ V`로 바뀌고 새 단축키로 패널이 열리며 이전 단축키는 동작하지 않는다. `Esc`로 녹화 취소 확인. 앱 재시작 후에도 새 단축키 유지
5. 히스토리 전체 삭제 → 확인 다이얼로그 → 예 → 패널이 비어 있다

- [ ] **Step 8: Commit**

```bash
git add package.json yarn.lock src/settings src-tauri/capabilities
git commit -m "feat: settings window with theme, language, autostart and shortcut recorder"
```

---

### Task 9: 자동 업데이트

**Files:**
- Create: `src-tauri/src/updater.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `src-tauri/src/tray.rs`, `src/settings/Settings.tsx`
- 사용자 장비: `~/.config/vee/bee.key`, `~/.config/vee/bee.key.pub`, `~/.config/vee/sign.env`

**Interfaces:**
- Consumes: `tray::refresh`, `now_ms`, `Row`(Settings.tsx)
- Produces: `updater::UpdateStatus`(serde `{status: "idle"|"checking"|"upToDate"|"ready"|"failed", checkedAt?, version?}`), `updater::UpdateState::ready_version() -> Option<String>`, `updater::check(&AppHandle)`, `updater::install_and_restart(&AppHandle) -> Result<(), String>`, `updater::spawn_periodic(AppHandle)`; command `check_update`, `get_update_status`, `install_update_and_restart`; 이벤트 `update://status`

- [ ] **Step 1: (사용자) 서명 키 생성** — 비밀번호를 묻는 대화형 명령이므로 사용자가 직접 실행한다

```
! yarn tauri signer generate -w ~/.config/vee/bee.key
```

그다음 사용자가 `~/.config/vee/sign.env`에서 `TAURI_SIGNING_PRIVATE_KEY`를 **절대 경로**(`"/Users/hwpark/.config/vee/bee.key"`) 또는 키 내용으로, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`를 방금 정한 비밀번호로 맞춘다. (`~`는 따옴표 안에서 확장되지 않는다)

확인: `test -f ~/.config/vee/bee.key.pub && echo ok` → `ok`

- [ ] **Step 2: 의존성과 설정 추가**

`src-tauri/Cargo.toml` `[dependencies]`:
```toml
tauri-plugin-updater = "2.13.1"
```

`src-tauri/tauri.conf.json` — `bundle`에 `"createUpdaterArtifacts": true` 추가, 최상위에 `plugins` 추가 (`pubkey`는 `cat ~/.config/vee/bee.key.pub`의 한 줄 전체):
```json
  "plugins": {
    "updater": {
      "pubkey": "<~/.config/vee/bee.key.pub 내용>",
      "endpoints": ["https://github.com/bob-park/vee-app/releases/latest/download/latest.json"],
      "windows": { "installMode": "passive" }
    }
  }
```

- [ ] **Step 3: 실패하는 테스트 작성** — `src-tauri/src/updater.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_serializes_like_the_typescript_union() {
        let json = |s: UpdateStatus| serde_json::to_value(s).unwrap();
        assert_eq!(json(UpdateStatus::Idle), serde_json::json!({ "status": "idle" }));
        assert_eq!(
            json(UpdateStatus::UpToDate { checked_at: 5 }),
            serde_json::json!({ "status": "upToDate", "checkedAt": 5 })
        );
        assert_eq!(
            json(UpdateStatus::Ready { version: "0.2.0".into() }),
            serde_json::json!({ "status": "ready", "version": "0.2.0" })
        );
    }
}
```

`lib.rs`에 `mod updater;` 추가 후 Run: `cargo test --manifest-path src-tauri/Cargo.toml updater::`
Expected: 컴파일 실패

- [ ] **Step 4: `updater.rs` 구현** (테스트 모듈 위에)

```rust
//! Checks GitHub Releases, downloads in the background, installs only when asked.

use crate::{now_ms, tray};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate { checked_at: i64 },
    Ready { version: String },
    Failed { checked_at: i64 },
}

#[derive(Default)]
pub struct UpdateState {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<(Update, Vec<u8>)>>,
}

impl UpdateState {
    pub fn ready_version(&self) -> Option<String> {
        match &*self.status.lock().unwrap() {
            UpdateStatus::Ready { version } => Some(version.clone()),
            _ => None,
        }
    }
}

fn set_status(app: &AppHandle, status: UpdateStatus) {
    *app.state::<UpdateState>().status.lock().unwrap() = status.clone();
    let _ = app.emit("update://status", status);
    tray::refresh(app);
}

async fn fetch(app: &AppHandle) -> tauri_plugin_updater::Result<Option<(Update, Vec<u8>)>> {
    let Some(update) = app.updater()?.check().await? else { return Ok(None) };
    let bytes = update.download(|_, _| {}, || {}).await?;
    Ok(Some((update, bytes)))
}

pub async fn check(app: &AppHandle) -> UpdateStatus {
    let current = app.state::<UpdateState>().status.lock().unwrap().clone();
    if matches!(current, UpdateStatus::Ready { .. } | UpdateStatus::Checking) {
        return current;
    }
    set_status(app, UpdateStatus::Checking);
    let status = match fetch(app).await {
        Ok(Some((update, bytes))) => {
            let version = update.version.clone();
            *app.state::<UpdateState>().pending.lock().unwrap() = Some((update, bytes));
            UpdateStatus::Ready { version }
        }
        Ok(None) => UpdateStatus::UpToDate { checked_at: now_ms() },
        Err(e) => {
            log::warn!("update check failed: {e}");
            UpdateStatus::Failed { checked_at: now_ms() }
        }
    };
    set_status(app, status.clone());
    status
}

pub fn install_and_restart(app: &AppHandle) -> Result<(), String> {
    let pending = app.state::<UpdateState>().pending.lock().unwrap().take();
    let Some((update, bytes)) = pending else { return Err("no update has been downloaded".into()) };
    update.install(&bytes).map_err(|e| e.to_string())?;
    app.restart();
}

/// Checks at startup and every 6 hours. Skipped in debug builds.
pub fn spawn_periodic(app: AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(move || loop {
        tauri::async_runtime::block_on(check(&app));
        std::thread::sleep(CHECK_EVERY);
    });
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> UpdateStatus {
    check(&app).await
}

#[tauri::command]
pub fn get_update_status(state: State<UpdateState>) -> UpdateStatus {
    state.status.lock().unwrap().clone()
}

#[tauri::command]
pub fn install_update_and_restart(app: AppHandle) -> Result<(), String> {
    install_and_restart(&app)
}
```

- [ ] **Step 5: `lib.rs` 수정**

1. `.plugin(tauri_plugin_autostart::init(...))` 다음에 추가:
```rust
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateState::default())
```
2. setup의 `settings::register_stored_shortcut(app.handle());` 다음에 추가:
```rust
            updater::spawn_periodic(app.handle().clone());
```
3. `generate_handler!` 목록 끝에 추가: `updater::check_update, updater::get_update_status, updater::install_update_and_restart`

- [ ] **Step 6: 트레이에 업데이트 항목 추가** — `tray.rs`

1. `Labels`에 필드 2개 추가, `labels()`에 값 추가:
```rust
pub struct Labels {
    pub open: &'static str,
    pub settings: &'static str,
    pub check_update: &'static str,
    pub restart_update: &'static str,
    pub quit: &'static str,
}

pub fn labels(locale: &str) -> Labels {
    if locale == "ko" {
        Labels { open: "열기", settings: "설정", check_update: "업데이트 확인", restart_update: "업데이트 후 재시작", quit: "종료" }
    } else {
        Labels { open: "Open", settings: "Settings", check_update: "Check for Updates", restart_update: "Restart to Update", quit: "Quit" }
    }
}
```
2. `build_menu`를 교체:
```rust
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let setting = settings::get(&app.state::<AppState>().store.lock().unwrap(), "locale");
    let l = labels(resolve_locale(&setting));
    let ready = app.state::<UpdateState>().ready_version().is_some();
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", l.open, true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", l.settings, true, None::<&str>)?,
            &MenuItem::with_id(app, "update", if ready { l.restart_update } else { l.check_update }, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", l.quit, true, None::<&str>)?,
        ],
    )
}
```
3. `on_menu_event`의 match에 `"quit"` 위로 추가:
```rust
            "update" => {
                if app.state::<UpdateState>().ready_version().is_some() {
                    if let Err(e) = updater::install_and_restart(app) {
                        log::error!("update install failed: {e}");
                    }
                } else {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        updater::check(&app).await;
                    });
                }
            }
```
4. 맨 위 `use`를 `use crate::{AppState, settings, updater::{self, UpdateState}, windows};`로 변경

- [ ] **Step 7: 설정 창에 업데이트 행 추가** — `src/settings/Settings.tsx`

1. import 교체:
```tsx
import { useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type UpdateStatus } from "../api.ts";
import { relativeTime } from "../i18n/index.ts";
```
2. `Settings` 함수 위에 추가:
```tsx
function UpdateRow() {
  const { settings, t, locale } = usePrefs();
  const s = t.settings;
  const [status, setStatus] = useState<UpdateStatus>({ status: "idle" });

  useEffect(() => {
    void api.getUpdateStatus().then(setStatus);
    const unlisten = listen<UpdateStatus>("update://status", (e) => setStatus(e.payload));
    return () => void unlisten.then((off) => off());
  }, []);

  const detail = (() => {
    switch (status.status) {
      case "idle":
        return s.notChecked;
      case "checking":
        return s.checking;
      case "upToDate":
        return `${s.upToDate} · ${relativeTime(status.checkedAt, locale)}`;
      case "failed":
        return `${s.checkFailed} · ${relativeTime(status.checkedAt, locale)}`;
      case "ready":
        return s.updateReady(status.version);
    }
  })();

  return (
    <Row label={s.updates} hint={`${s.version(settings.version)} · ${detail}`}>
      {status.status === "ready" ? (
        <button className="btn" onClick={() => void api.installUpdate()}>
          {s.restartToUpdate}
        </button>
      ) : (
        <button className="btn" disabled={status.status === "checking"} onClick={() => void api.checkUpdate()}>
          {s.checkNow}
        </button>
      )}
    </Row>
  );
}
```
3. `Settings` JSX의 두 번째 `<section>` 안, 히스토리 `Row` 위에 `<UpdateRow />` 추가

- [ ] **Step 8: 확인**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && yarn test && yarn typecheck`
Expected: 모두 통과

수동 (`yarn tauri dev`): 설정 → "업데이트 확인" → 아직 릴리스가 없으므로 "업데이트를 확인하지 못했어요 · 지금" 표시, 앱은 정상. 트레이 메뉴에 "업데이트 확인" 항목 존재. (실제 업데이트는 Task 11에서 검증)

- [ ] **Step 9: Commit** — 공개키만 커밋되는지 확인

```bash
git add src-tauri src/settings
git diff --cached | grep -iE "PRIVATE|PASSWORD" && echo "STOP: secret in diff" || git commit -m "feat: background updates from GitHub Releases"
```

---

### Task 10: 로컬 릴리스 스크립트

**Files:**
- Create: `scripts/release.sh`, `scripts/release.ps1`, `docs/release.md`

**Interfaces:**
- Consumes: `src-tauri/tauri.conf.json`의 `version`, `~/.config/vee/sign.env`, 번들 산출물(`Vee.app.tar.gz(.sig)`, `*.dmg`, `*-setup.exe(.sig)`)
- Produces: GitHub 초안 릴리스 `v<version>`과 병합된 `latest.json`

- [ ] **Step 1: `scripts/release.sh` 작성**

```bash
#!/usr/bin/env bash
# Builds, signs and notarizes the macOS app for both architectures, then uploads
# the bundles and merges darwin entries into latest.json on a draft release.
set -euo pipefail
cd "$(dirname "$0")/.."

set -a
# shellcheck disable=SC1091
source "$HOME/.config/vee/sign.env"
set +a

REPO="bob-park/vee-app"
VERSION=$(jq -r .version src-tauri/tauri.conf.json)
TAG="v$VERSION"
BASE_URL="https://github.com/$REPO/releases/download/$TAG"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  yarn tauri build --target "$target"
done

gh release view "$TAG" -R "$REPO" >/dev/null 2>&1 \
  || gh release create "$TAG" -R "$REPO" --draft --title "Vee $TAG" --notes "Vee $TAG"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
latest="$work/latest.json"
if ! gh release download "$TAG" -R "$REPO" -p latest.json -D "$work" 2>/dev/null; then
  jq -n --arg v "$VERSION" --arg d "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{version: $v, notes: ("Vee v" + $v), pub_date: $d, platforms: {}}' > "$latest"
fi

for entry in "aarch64-apple-darwin:darwin-aarch64:aarch64" "x86_64-apple-darwin:darwin-x86_64:x64"; do
  IFS=: read -r target platform arch <<< "$entry"
  bundle="src-tauri/target/$target/release/bundle"
  # Both architectures produce Vee.app.tar.gz; rename so they can live side by side.
  tarball="$work/Vee_${VERSION}_${arch}.app.tar.gz"
  cp "$bundle/macos/Vee.app.tar.gz" "$tarball"
  dmg=$(ls "$bundle"/dmg/*.dmg)
  gh release upload "$TAG" -R "$REPO" --clobber "$tarball" "$dmg"
  jq --arg p "$platform" --arg sig "$(cat "$bundle/macos/Vee.app.tar.gz.sig")" \
     --arg url "$BASE_URL/$(basename "$tarball")" \
     '.platforms[$p] = {signature: $sig, url: $url}' "$latest" > "$latest.tmp"
  mv "$latest.tmp" "$latest"
done

gh release upload "$TAG" -R "$REPO" --clobber "$latest"
echo "Draft $TAG updated with macOS builds."
echo "After the Windows upload, publish with: gh release edit $TAG -R $REPO --draft=false"
```

Run: `chmod +x scripts/release.sh && bash -n scripts/release.sh`
Expected: 출력 없음(문법 OK). `shellcheck`가 설치돼 있으면 `shellcheck scripts/release.sh`도 경고 없음

- [ ] **Step 2: `scripts/release.ps1` 작성** (PowerShell 7.5+)

```powershell
# Builds the Windows installer, uploads it and merges the windows entry into latest.json
# on the draft release. Run from a Windows PC after copying the signing key there.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Get-Content (Join-Path $env:USERPROFILE ".config\vee\sign.env") |
  Where-Object { $_ -match '^\s*([A-Z_]+)=(.*)$' } |
  ForEach-Object { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2].Trim().Trim('"', "'"), "Process") }

$repo = "bob-park/vee-app"
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "v$version"

yarn tauri build
if ($LASTEXITCODE) { throw "tauri build failed" }

$setup = Get-ChildItem "src-tauri/target/release/bundle/nsis/*-setup.exe" | Select-Object -First 1
$signature = (Get-Content "$($setup.FullName).sig" -Raw).Trim()

gh release view $tag -R $repo *> $null
if ($LASTEXITCODE) {
  gh release create $tag -R $repo --draft --title "Vee $tag" --notes "Vee $tag"
  if ($LASTEXITCODE) { throw "could not create release $tag" }
}
gh release upload $tag -R $repo --clobber $setup.FullName
if ($LASTEXITCODE) { throw "upload failed" }

$work = New-Item -ItemType Directory (Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid()))
$latestPath = Join-Path $work "latest.json"
gh release download $tag -R $repo -p latest.json -D $work 2>$null
if (Test-Path $latestPath) {
  # -DateKind String keeps pub_date exactly as written (needs PowerShell 7.5+).
  $latest = Get-Content $latestPath -Raw | ConvertFrom-Json -DateKind String
} else {
  $latest = [pscustomobject]@{
    version   = $version
    notes     = "Vee $tag"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [pscustomobject]@{}
  }
}
$entry = [pscustomobject]@{
  signature = $signature
  url       = "https://github.com/$repo/releases/download/$tag/$($setup.Name)"
}
$latest.platforms | Add-Member -NotePropertyName "windows-x86_64" -NotePropertyValue $entry -Force
$latest | ConvertTo-Json -Depth 5 | Set-Content $latestPath -Encoding utf8NoBOM
gh release upload $tag -R $repo --clobber $latestPath
if ($LASTEXITCODE) { throw "latest.json upload failed" }

Remove-Item $work -Recurse -Force
Write-Host "Draft $tag updated with the Windows build."
Write-Host "Once macOS is uploaded too: gh release edit $tag -R $repo --draft=false"
```

- [ ] **Step 3: `docs/release.md` 작성**

```markdown
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
```

- [ ] **Step 4: Commit**

```bash
git add scripts docs/release.md
git commit -m "chore: local release scripts for macOS and Windows"
```

---

### Task 11: 통합 검증과 첫 릴리스

**Files:** 없음 (검증만; 발견된 버그는 해당 Task 파일에서 수정 후 별도 커밋)

- [ ] **Step 1: 자동 검사 전체**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && yarn test && yarn typecheck`
Expected: 전부 통과

- [ ] **Step 2: 첫 릴리스 v0.1.0 (사용자 확인 후 실행)** — GitHub에 공개되는 작업이므로 실행 전 사용자에게 확인받는다

1. macOS: `scripts/release.sh` → 서명·공증 성공, 초안에 dmg 2개·tar.gz 2개·`latest.json`
2. Windows PC: `pwsh scripts/release.ps1` → setup.exe 업로드, `latest.json`에 `windows-x86_64` 병합
3. 게시: `gh release edit v0.1.0 -R bob-park/vee-app --draft=false`
4. 각 OS에 v0.1.0 설치 (macOS는 dmg, Windows는 setup.exe — SmartScreen "추가 정보 → 실행")

- [ ] **Step 3: 스펙 11장 수동 체크리스트 (macOS, Windows 각각)**

- [ ] 텍스트·이미지·파일·폴더 복사 시 카드 생성, 출처 앱 아이콘 표시
- [ ] 비밀번호 관리자 복사는 저장되지 않음 (macOS, 1Password 등)
- [ ] 단축키 → 커서가 있는 모니터 하단에 패널, 다시 누르면 숨김 (모니터 2대면 각각)
- [ ] 검색(한/영, 1~2글자 포함, `%`·`"` 포함), 필터, 키보드 조작, 한글 조합 중 Enter
- [ ] 더블클릭/Enter → 복사, 패널 닫힘, toast 표시, 직전 앱에서 붙여넣기 확인
- [ ] Windows: toast가 떠도 직전 앱의 포커스가 유지되는지 (빼앗기면 이슈로 기록)
- [ ] 단축키 변경 및 충돌 시 메시지 (Windows는 다른 앱이 쓰는 조합으로 확인)
- [ ] 설정 창 닫기 → 트레이 상주, 트레이 더블클릭 → 패널
- [ ] 로그인 시 자동 실행 on → 재로그인 후 자동 실행, off → 실행 안 됨
- [ ] 라이트/다크/시스템, 한국어/English/시스템 전환 (트레이 메뉴 포함)
- [ ] 재부팅 후 히스토리 유지

- [ ] **Step 4: 업데이트 end-to-end (사용자 확인 후 실행)**

1. `version`을 `0.1.1`로 올려 커밋 → Task 10 절차로 v0.1.1 릴리스·게시
2. v0.1.0이 설치된 각 OS에서 설정 → "업데이트 확인" → "v0.1.1 준비됨", 트레이 메뉴가 "업데이트 후 재시작"으로 바뀜
3. "업데이트 후 재시작" → 앱이 재시작되고 설정의 버전이 v0.1.1, 히스토리 유지

- [ ] **Step 5: 마무리**

체크리스트 결과(실패 항목 포함)를 사용자에게 보고하고 `superpowers:finishing-a-development-branch`로 넘어간다.
