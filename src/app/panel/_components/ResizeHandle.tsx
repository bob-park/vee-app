import { type PointerEvent, useEffect, useRef, useState } from 'react';

import { resizePanel } from '@/domain/panel/apis/panel';
import { useSettings } from '@/domain/settings/queries/settings';

import cx from 'classnames';

interface Drag {
  startY: number;
  startH: number;
  h: number;
  frame: number;
}

/** A grab bar on the panel's top edge. screenY is in CSS px, which match design points on macOS and Windows. */
export function ResizeHandle() {
  // ref
  const drag = useRef<Drag | null>(null);

  // state
  const [active, setActive] = useState(false);

  // queries
  const { settings } = useSettings();

  // useEffect
  // Unmounted mid-drag (panel closing, setting flipped): drop the pending frame so it can't resize.
  useEffect(
    () => () => {
      if (drag.current) cancelAnimationFrame(drag.current.frame);
      drag.current = null;
    },
    [],
  );
  if (settings?.panelResize !== 'on') return null;

  // handle

  // No preventDefault here: that would swallow the mousedown the panel uses to keep focus in the search box.
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { startY: e.screenY, startH: window.innerHeight, h: window.innerHeight, frame: 0 };
    setActive(true);
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    d.h = d.startH + (d.startY - e.screenY);
    // One resize per frame; a "RESIZE_OFF" race just drops the call.
    if (!d.frame) {
      d.frame = requestAnimationFrame(() => {
        d.frame = 0;
        resizePanel(d.h, false).catch(() => {});
      });
    }
  };

  // A cancelled drag still saves: the window is already at the last height, so keep what the user sees.
  const onPointerEnd = () => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    cancelAnimationFrame(d.frame);
    setActive(false);
    if (d.h !== d.startH) resizePanel(d.h, true).catch(() => {});
  };

  return (
    <div
      // A drag strip along the top edge with a small grab bar in the middle.
      className={cx(
        "absolute inset-x-0 top-0 z-1 h-1.5 cursor-ns-resize touch-none select-none after:absolute after:top-px after:left-1/2 after:-ml-3.5 after:h-1 after:w-7 after:rounded-xs after:content-['']",
        active ? 'after:bg-primary' : 'after:bg-grab hover:after:bg-primary',
      )}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
    />
  );
}
