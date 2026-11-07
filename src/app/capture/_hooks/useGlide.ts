import { useEffect, useRef, useState } from 'react';

import type { Rect } from '@/domain/capture/apis/capture.dto';
import { type Glide, easeMove, lerpRect, nextGlide } from '@/utils/motion';

/** The window highlight's move to the next window: quick, so it never trails the pointer. */
const GLIDE_MS = 180;

/** `target` as drawn: glides to each new rect, keeps the last one (hidden) when it goes so it can fade out. */
export function useGlide(target: Rect | null, ms = GLIDE_MS): { rect: Rect | null; shown: boolean } {
  // ref
  const glideRef = useRef<Glide | null>(null);

  // state
  const [drawn, setDrawn] = useState<{ rect: Rect | null; shown: boolean }>({ rect: null, shown: false });

  // useEffect
  // By value: the caller builds a new rect every render.
  const { x, y, width, height } = target ?? { x: NaN, y: NaN, width: NaN, height: NaN };
  useEffect(() => {
    const next = nextGlide(glideRef.current, target && { x, y, width, height });
    glideRef.current = next;
    if (!next) return;
    if (!next.restart || matchMedia('(prefers-reduced-motion: reduce)').matches) {
      glideRef.current = { ...next, drawn: next.to };
      setDrawn({ rect: next.to, shown: next.shown });
      return;
    }
    const start = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / ms);
      const rect = lerpRect(next.from, next.to, easeMove(k));
      glideRef.current = { ...next, drawn: rect };
      setDrawn({ rect, shown: true });
      if (k < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [target === null, x, y, width, height, ms]);

  return drawn;
}
