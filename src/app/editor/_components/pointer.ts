import type { PointerEvent } from 'react';

import type { Pt } from '@/utils/edit';

/** The pointer in px from the element's top-left. */
export const pointIn = (e: PointerEvent<Element>): Pt => {
  const r = e.currentTarget.getBoundingClientRect();
  return { x: e.clientX - r.left, y: e.clientY - r.top };
};
