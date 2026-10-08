/** Shared motion for the panel. Everything here is a no-op under reduced motion. */

export const EASE_MOVE = "cubic-bezier(0.32, 0.72, 0, 1)";
export const EASE_SPRING = "cubic-bezier(0.34, 1.56, 0.64, 1)";

export function reducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Each card's left edge on screen, by clip id. */
export function cardLefts(row: HTMLElement | null): Map<number, number> {
  const lefts = new Map<number, number>();
  for (const el of row?.children ?? []) {
    const card = el as HTMLElement;
    lefts.set(Number(card.dataset.id), card.getBoundingClientRect().left);
  }
  return lefts;
}

/**
 * Slides cards that moved from their old place (FLIP) and pops in cards that are new,
 * ringing them once in the accent colour so a copy made elsewhere is easy to spot.
 */
export function playFlip(row: HTMLElement, before: Map<number, number>): void {
  for (const el of row.children) {
    const card = el as HTMLElement;
    const prev = before.get(Number(card.dataset.id));
    if (prev === undefined) {
      card.animate([{ transform: "scale(0.86)", opacity: 0 }, { transform: "none", opacity: 1 }], {
        duration: 360,
        easing: EASE_MOVE,
      });
      card.animate(
        [
          { boxShadow: "0 0 0 0 rgba(113, 50, 245, 0.5)" },
          { boxShadow: "0 0 0 6px rgba(113, 50, 245, 0.28)", offset: 0.3 },
          { boxShadow: "0 0 0 0 rgba(113, 50, 245, 0)" },
        ],
        { duration: 1400, easing: "ease-out" },
      );
      continue;
    }
    const dx = prev - card.getBoundingClientRect().left;
    if (dx !== 0) {
      card.animate([{ transform: `translateX(${dx}px)` }, { transform: "none" }], { duration: 360, easing: EASE_MOVE });
    }
  }
}

/**
 * Distance from the window's top edge at which a floating card hands off to the OS drag.
 * 0 = only once the cursor has left the window. 20 hands off inside the window, the path
 * known to work; switch to 0 once a native drag started outside the window is confirmed.
 */
export const HANDOFF_EDGE = 20;

/** The floating card leans into horizontal motion and settles when it stops. */
export function nextTilt(prev: number, dx: number): number {
  return Math.max(-10, Math.min(10, prev * 0.7 + dx * 0.6));
}

export function leftWindow(x: number, y: number, w: number, h: number, edge: number): boolean {
  return x < 0 || y < edge || x >= w || y >= h;
}

/** Tears down the float (or its spring-back) in progress, if any. */
let cancelActive: (() => void) | null = null;

/** Drops any floating card at once, with no animation and no OS drag. */
export function cancelFloat(): void {
  cancelActive?.();
}

/**
 * Lifts a copy of `card` that follows the cursor, leaving a dashed slot behind. When the
 * cursor leaves the window the copy is dropped and `onLeave` starts the OS drag; when the
 * button is released inside, the copy springs back into the slot.
 */
export function floatCard(card: HTMLElement, x0: number, y0: number, onLeave: () => void): void {
  // A card grabbed again mid spring-back must not clone the previous float's slot marker.
  cancelFloat();
  const r = card.getBoundingClientRect();
  const ghost = card.cloneNode(true) as HTMLElement;
  ghost.classList.add("ghost");
  Object.assign(ghost.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
  document.body.appendChild(ghost);
  // An attribute, not a class: React rewrites className whenever the card re-renders.
  card.dataset.slot = "";

  let tilt = 0;
  let lastX = x0;
  let back: Animation | null = null;
  const place = (x: number, y: number) => {
    ghost.style.transform = `translate(${x - x0}px, ${y - y0}px) rotate(${tilt}deg) scale(1.05)`;
  };
  const restore = () => {
    ghost.remove();
    delete card.dataset.slot;
    cancelActive = null;
  };
  const stop = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", drop);
    document.documentElement.removeEventListener("mouseleave", leave);
  };
  function leave() {
    stop();
    restore();
    onLeave();
  }
  function drop() {
    stop();
    back = ghost.animate([{ transform: ghost.style.transform }, { transform: "none" }], {
      duration: 320,
      easing: EASE_SPRING,
    });
    back.onfinish = restore;
  }
  function move(e: MouseEvent) {
    if ((e.buttons & 1) === 0) return drop();
    tilt = nextTilt(tilt, e.clientX - lastX);
    lastX = e.clientX;
    if (leftWindow(e.clientX, e.clientY, innerWidth, innerHeight, HANDOFF_EDGE)) return leave();
    place(e.clientX, e.clientY);
  }

  cancelActive = () => {
    stop();
    back?.cancel();
    restore();
  };
  place(x0, y0);
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", drop);
  document.documentElement.addEventListener("mouseleave", leave);
}
