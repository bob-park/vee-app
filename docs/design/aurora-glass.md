# Aurora Glass

2026-10 리디자인부터 Vee의 시각 언어. kiri-app의 Aurora Glass(오로라 블롭 + 유리 카드 + 하나의 스프링 곡선)를 Vee의 다섯 창에 옮긴 것이다. 다만 Vee는 다른 앱 위에 뜨는 창이 많아서 **"은은한 오로라"**만 쓴다: 창 바탕은 단색이고, 오로라는 상단(툴바/탭 바) 뒤에서만 빛처럼 번진다.

브랜드 색은 [kraken-design.md](./kraken-design.md)를 따른다. 두 문서가 다를 때(글꼴, radius, 표면, 모션)는 **이 문서가 이긴다**.

아래 값은 모두 `src/app/globals.css`에 있다. 컴포넌트는 토큰 클래스만 쓰고 색 값을 직접 쓰지 않는다.

## 1. 팔레트

### Core (daisyUI `light` / `dark` 테마)

| 역할 | Light | Dark | 클래스 |
| ---- | ----- | ---- | ------ |
| 창 바탕 | `#f6f5fb` | `#121318` | `bg-base-100` |
| 한 단 올라온 면 | `#f7f7fa` | `#1d1e25` | `bg-base-200` |
| 테두리 | `#dedee5` | `#2c2e38` | `border-base-300` |
| 글자 | `#101114` | `#ececf2` | `text-base-content` |
| 브랜드 | `#7132f5` | `#8b5cff` | `bg-primary`, `text-primary` |

Radius: box 14px(`--radius-box: 0.875rem`), field 12px, selector 10px. daisyUI depth/noise는 쓰지 않는다. 나머지 Vee 토큰(`--vee-card`, `--vee-muted`, `--vee-toast-*`, …)은 그대로다.

### Aurora Glass 토큰 (`[data-theme]`의 `--vee-*` → `@theme inline`)

| 토큰 | Light | Dark | 용도 |
| ---- | ----- | ---- | ---- |
| `--color-panel` | `#eeedf6` | `#1a1b22` | 패널 창 바탕 (`bg-panel`). 흰/검은 페이지 위에서도 테두리가 보이도록 `base-100`과 따로 둔다 |
| `--color-glass` | `rgba(255,255,255,.58)` | `rgba(30,31,40,.55)` | 유리 채움 |
| `--color-glass-edge` | `rgba(255,255,255,.85)` | `rgba(255,255,255,.08)` | 유리 1px 테두리 |
| `--color-aurora-1` | `#a98bff` | `#5b3bd6` | 오로라 블롭 (보라) |
| `--color-aurora-2` | `#7fd4ff` | `#1f6f9e` | 오로라 블롭 (하늘) |
| `--color-aurora-3` | `#ffb3d9` | `#7a2d63` | 오로라 블롭 (분홍) |
| `--color-sky` | `#5fc3ff` | 같음 | CTA·링크 그라데이션의 두 번째 색 (`from-primary to-sky`) |

새 토큰은 두 테마 값을 모두 갖는다(같은 값이면 `@theme inline`에 바로 두고 이유를 주석으로 남긴다).

## 2. 글꼴

- Pretendard Variable(npm `pretendard`, `pretendardvariable.css`를 `globals.css`에서 import). 한글 렌더링 때문에 고른 것.
- 대체: `Pretendard`, `-apple-system`, `Helvetica Neue`, `Arial`. 고정폭은 그대로 `Menlo, Consolas`.
- 본문 크기와 line-height(`normal`)는 그대로 13px.

## 3. 유리

- `.glass` = `--color-glass` 채움 + 1px `--color-glass-edge` 테두리 + `backdrop-filter: blur(18px) saturate(1.4)`. daisyUI의 `glass`는 `exclude`에 있어서 우리 것이 이긴다.
- 유리 레시피는 이것 하나다. 두 번째 레시피를 만들지 않는다.
- **유리를 쓰는 곳:** 패널의 검색창·필터·아이콘 버튼·카드, 설정의 탭 바와 섹션, 토스트의 아이콘 타일, 새 기능 안내의 항목 카드 — 각 작업이 정한 곳만.
- **단색 `bg-card`를 그대로 쓰는 곳:** 드롭다운, 모달, 그 밖에 내용 위에 떠서 읽혀야 하는 면. `--vee-card`는 단색으로 남는다.
- 유리는 CSS(`backdrop-filter`)만 쓴다. macOS 네이티브 vibrancy는 쓰지 않는다. 그래서 유리 뒤에 비치는 건 앱 안의 오로라와 바탕뿐이다.

## 4. 은은한 오로라

```tsx
<div className="relative">
  <Aurora className="h-28" />
  <header className="relative">…</header>
</div>
```

- `<Aurora />`(`src/shared/components/Aurora.tsx`)는 `.aurora` 띠와 `.aurora-blob` 세 개를 그린다. `aria-hidden`.
- `.aurora`는 positioned 부모의 **위쪽에 붙는 띠**다(`inset: 0 0 auto`). 높이는 쓰는 쪽이 정한다. 창 전체를 채우지 않는다 — 창 바탕은 단색 `bg-base-100`(패널은 `bg-panel`).
- 띠는 `z-index: 0`이므로 뒤에 오는 내용은 `relative`를 줘야 위에 그려진다.
- 블롭은 aurora-1/2/3 색, `blur(46px)`, `opacity .4`, `animate-drift`(12s, alternate)를 -4s/-8s로 엇갈려 돈다. **세 개를 넘지 않는다**(`:nth-child(n+4)`는 숨김).

## 5. 모션

하나의 스프링 `--ease-spring: cubic-bezier(0.2, 0.9, 0.3, 1.2)`(`ease-spring` 유틸리티)를 등장에 쓴다.

| 종류 | 시간 | 이징 | 비고 |
| ---- | ---- | ---- | ---- |
| 등장 (`animate-spring-in`) | 650ms | `ease-spring` | 위 16px·94%·blur 4px에서 선명하게 내려앉음 |
| 팝 (`animate-pop`) | 450ms | `ease-spring` | 배지·점: scale .3 → 1 |
| 링 (`animate-ring`) | 900ms, 200ms 지연 | `ease-out` | `--ring-color`(기본 `--color-primary`)로 14px까지 퍼짐 |
| 미끄러지는 알약 (탭·필터·Segmented) | 450ms | `ease-spring` | |
| 퇴장 | 150–250ms | `ease-in` | 토스트 `TOAST_EXIT_MS`, 패널 `PANEL_CLOSE_MS=200`과 맞물림 |
| 떠다님 | drift 12s, bob 1.6s | `ease-in-out` | 오로라 블롭, 도넛의 ↓ 아이콘 |
| FLIP 재정렬·드래그 | 그대로 | `EASE_MOVE` / `EASE_SPRING` (`motion.ts`) | 물리감이 필요한 곳은 기존 곡선 유지 |
| 패널 열림 슬라이드 | 300ms | `EASE_MOVE` | 아래 참고 |

**패널 열림 슬라이드는 `EASE_MOVE`를 유지한다.** 패널 창은 패널 크기와 정확히 같다. `ease-spring`의 1.2 오버슈트를 쓰면 시트가 목표 위치를 지나 창의 위쪽 가장자리 밖으로 밀려나 잘린다. `EASE_MOVE`는 오버슈트가 없다.

규칙:

- `transform`과 `opacity`만 애니메이션한다. 예외: `spring-in` 첫 프레임의 `filter: blur()`, `ring`의 `box-shadow`, 오로라 블롭의 `filter`(정적).
- **움직임 줄이기:** `prefers-reduced-motion: reduce`이면 `globals.css`의 한 블록이 `.aurora-blob`, `animate-spring-in`/`pop`/`ring`/`bob`을 끄고(`animation: none`), `ease-spring`을 쓰는 요소의 transition을 끈다. 요소는 최종 상태(불투명, blur 없음)로 바로 보인다. `motion.ts`는 이미 `reducedMotion()`일 때 아무것도 하지 않는다.

## 6. 창별 레이아웃

### 패널 (⌘V)

- 바탕 단색 + 툴바 뒤 오로라. 검색창·필터·아이콘 버튼은 유리.
- 모든 카드는 같은 크기의 유리 카드: **머리줄**(앱 이름 · 타입 배지 · 시간) → **내용 타일** → **아래 줄**.
  - 텍스트: 타일 + 구석의 옅은 큰따옴표 장식. 코드로 구분되면 고정폭 타일.
  - 링크: 타일 위쪽은 그라데이션 + 큰 🔗 아이콘, 아래쪽은 굵은 도메인 + URL 한 줄(길면 `…`).
  - 이미지·파일: 지금처럼 썸네일/아이콘.
- 지금 카드의 요소는 모두 유지: 아래 줄(글자 수/이미지 크기/파일 수, `missing` 경고, 복사 안내, 변환 저장 폴더), 출처 앱 워터마크, hover 📌, 고정/변환 실패/완료 테두리, 삭제 확인 오버레이, 변환 진행 표시, 고정 카드 뒤 세로 구분선.
- 움직임: 카드가 흐릿→선명하게 짧은 stagger로 들어옴, 새 카드는 보라 링 한 번, 필터 선택은 미끄러지는 알약, hover 시 살짝 떠오름. 애니메이션이 키보드 선택·복사를 막지 않는다.

### 설정

- 바탕 단색 + 탭 바 뒤 오로라. 가운데 유리 탭 바(6개 탭) + 미끄러지는 선택 알약.
- 섹션은 유리 카드, 탭을 바꾸면 차례로 들어옴.
- 테마는 미니 미리보기 카드(시스템은 대각선 반반). Segmented는 미끄러지는 알약, Switch는 스프링 썸.

### 토스트

- 단색 카드 + 옅은 오로라 빛, 아이콘은 유리 타일.
- 아래에서 흐릿→선명하게 올라옴, ✓ 배지가 pop + 초록 링(`--ring-color`).

### 변환 도넛

- 말랑한 링: 조각 6개가 네 모서리가 둥근 채로 간격을 두고 놓인다(겹치지 않음). 가운데는 살짝 흔들리는 ↓(`animate-bob`) + 짧은 드롭 안내, 파일 이름 없음.
- 포맷이 5개 이하면 조각을 60°로 고정해 12시 방향 가운데로 모으고, 남은 자리는 옅은 빈 호(`fill-donut-rest`)로 둔다. 빈 호는 드롭 대상이 아니다. 6개 이상은 링 전체를 n등분한다.
- 받침 원판: 단색 원판 + 안쪽 은은한 오로라 + 얇은 테두리 + 그림자. 흰/검은 배경 어디서도 보여야 한다.
- 원판 바깥으로 보라·하늘·분홍 후광(라이트·다크 모두).
- 다크: 조각은 바탕보다 한 톤 밝게, 위쪽 1px 빛 테두리.
- 원판이 튀어나오고 조각이 회전하며 차례로 펼쳐짐, hover 조각은 보라로 채워지며 커짐. 후광·그림자가 창(260×260) 밖으로 잘리지 않게 한다.

### 새 기능 안내

- 설정과 같은 상단 오로라, 버전 배지 pop, 제목·항목 카드가 차례로 들어옴.
- 확인 버튼은 보라→하늘 그라데이션(`from-primary to-sky`).

## 7. Do / Don't

- 색·그림자·그라데이션은 `globals.css`에만, 라이트와 다크 모두.
- 라이트/다크, 한국어/English를 모두 확인한다. 투명 창(패널·토스트·도넛)은 흰 배경과 검은 배경 위에서 확인한다.
- 네 번째 블롭, 두 번째 유리 레시피, 창마다 다른 이징을 만들지 않는다.
- 레이아웃 속성(height, top, margin)을 애니메이션하지 않는다(위 예외 제외).
