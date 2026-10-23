import { type PointerEvent, useEffect, useRef, useState } from 'react';

import { resizePanel } from '@/domain/panel/apis/panel';
import type { PanelPosition } from '@/domain/settings/apis/settings.dto';
import { useSettings } from '@/domain/settings/queries/settings';
import { type DragStart, dragSize, isSide } from '@/utils/panelEdge';

import cx from 'classnames';

interface Drag {
  start: DragStart;
  size: number;
  frame: number;
}

// The strip lies on the panel's inner edge, the one facing away from the screen edge it is docked to;
// `after:` is the small grab bar in its middle.
const STRIP: Record<PanelPosition, string> = {
  bottom: 'inset-x-0 top-0 h-1.5 cursor-ns-resize after:top-px after:left-1/2 after:-ml-3.5 after:h-1 after:w-7',
  top: 'inset-x-0 bottom-0 h-1.5 cursor-ns-resize after:bottom-px after:left-1/2 after:-ml-3.5 after:h-1 after:w-7',
  left: 'inset-y-0 right-0 w-1.5 cursor-ew-resize after:right-px after:top-1/2 after:-mt-3.5 after:h-7 after:w-1',
  right: 'inset-y-0 left-0 w-1.5 cursor-ew-resize after:left-px after:top-1/2 after:-mt-3.5 after:h-7 after:w-1',
};

/**
 * A grab bar on the panel's inner edge: it drags the height when docked top/bottom, the width when docked
 * left/right. screenX/Y are in CSS px, which match design points on macOS and Windows.
 */
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
  const position = settings.panelPosition;

  // handle

  // No preventDefault here: that would swallow the mousedown the panel uses to keep focus in the search box.
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const size = isSide(position) ? window.innerWidth : window.innerHeight;
    drag.current = { start: { size, x: e.screenX, y: e.screenY }, size, frame: 0 };
    setActive(true);
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    d.size = dragSize(position, d.start, e.screenX, e.screenY);
    // One resize per frame; a "RESIZE_OFF" race just drops the call.
    if (!d.frame) {
      d.frame = requestAnimationFrame(() => {
        d.frame = 0;
        resizePanel(d.size, false).catch(() => {});
      });
    }
  };

  // A cancelled drag still saves: the window is already at the last size, so keep what the user sees.
  const onPointerEnd = () => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    cancelAnimationFrame(d.frame);
    setActive(false);
    if (d.size !== d.start.size) resizePanel(d.size, true).catch(() => {});
  };

  return (
    <div
      className={cx(
        "absolute z-1 touch-none select-none after:absolute after:rounded-xs after:content-['']",
        STRIP[position],
        active ? 'after:bg-primary' : 'after:bg-grab hover:after:bg-primary',
      )}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
    />
  );
}
