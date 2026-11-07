import { useEffect, useRef, useState } from 'react';

import type { Rect } from '@/domain/capture/apis/capture.dto';
import { type Glide, easeMove, lerpRect, nextGlide } from '@/utils/motion';

/** The window highlight's move to the next window: quick, so it never trails the pointer. */
const GLIDE_MS = 180;

interface Drawn {
  rect: Rect | null;
  shown: boolean;
  /** The parts in front of the target, left untinted; the last ones stay while it fades out. */
  covered: Rect[];
}

/** `target` as drawn: glides to each new rect, keeps the last one (hidden) when it goes so it can fade out. */
export function useGlide(target: Rect | null, covered: Rect[], ms = GLIDE_MS): Drawn {
  // ref
  const glideRef = useRef<Glide | null>(null);

  // state
  const [drawn, setDrawn] = useState<Drawn>({ rect: null, shown: false, covered: [] });

  // useEffect
  // By value: the caller builds a new rect every render.
  const { x, y, width, height } = target ?? { x: NaN, y: NaN, width: NaN, height: NaN };
  // The parts change only with the target, so they need no deps of their own.
  const coveredRef = useRef(covered);
  coveredRef.current = covered;
  useEffect(() => {
    const next = nextGlide(glideRef.current, target && { x, y, width, height }, coveredRef.current);
    glideRef.current = next;
    if (!next) return;
    // Gone mid-glide, it fades out where it is drawn; reduced motion skips the glide to the end.
    if (!next.restart || matchMedia('(prefers-reduced-motion: reduce)').matches) {
      const rect = next.restart ? next.to : next.drawn;
      glideRef.current = { ...next, drawn: rect };
      setDrawn({ rect, shown: next.shown, covered: next.covered });
      return;
    }
    const start = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / ms);
      const rect = lerpRect(next.from, next.to, easeMove(k));
      glideRef.current = { ...next, drawn: rect };
      setDrawn({ rect, shown: true, covered: next.covered });
      if (k < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [target === null, x, y, width, height, ms]);

  return drawn;
}
