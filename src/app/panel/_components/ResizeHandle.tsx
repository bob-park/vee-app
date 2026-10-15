import { useEffect, useRef, useState, type PointerEvent } from "react";
import { resizePanel } from "@/domain/panel/apis/panel";
import { usePrefs } from "@/prefs";

interface Drag {
  startY: number;
  startH: number;
  h: number;
  frame: number;
}

/** A grab bar on the panel's top edge. screenY is in CSS px, which match design points on macOS and Windows. */
export function ResizeHandle() {
  const { settings } = usePrefs();
  const drag = useRef<Drag | null>(null);
  const [active, setActive] = useState(false);
  // Unmounted mid-drag (panel closing, setting flipped): drop the pending frame so it can't resize.
  useEffect(
    () => () => {
      if (drag.current) cancelAnimationFrame(drag.current.frame);
      drag.current = null;
    },
    [],
  );
  if (settings.panelResize !== "on") return null;

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
      className={active ? "resize-handle active" : "resize-handle"}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
    />
  );
}
