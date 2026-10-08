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
