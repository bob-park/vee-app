import { useEffect, useState } from 'react';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

/** How far the border sits outside the recorded area, so it never falls inside it. */
const OUTSET = 3;

/**
 * Covers the recorded monitor: dims all but the recorded area and dashes its edge, so the area stands out.
 * Rust places it, lets clicks through and keeps it out of the recording.
 */
export default function RecFrameApp() {
  // state
  const [area, setArea] = useState<[number, number, number, number] | null>(null);
  /** False once the recording ends: the frame fades out, then Rust hides the window. */
  const [shown, setShown] = useState(false);

  // useEffect
  useEffect(() => {
    // Null once the recording ends. The old area stays drawn but transparent, so the next show never flashes it.
    const off = listen<[number, number, number, number] | null>('recframe://area', (e) => {
      setShown(e.payload !== null);
      if (e.payload) setArea(e.payload);
    });
    return () => void off.then((fn) => fn());
  }, []);

  if (!area) return null;
  const [x, y, w, h] = area;
  const o = OUTSET;
  return (
    // A new area remounts, which runs the fade in again.
    <svg
      key={area.join()}
      className={cx(
        'animate-fade-in fixed inset-0 size-full transition-opacity duration-150 motion-reduce:animate-none motion-reduce:transition-none',
        !shown && 'opacity-0',
      )}
      aria-hidden
    >
      <path
        className="fill-black/35"
        fillRule="evenodd"
        d={`M0 0H${window.innerWidth}V${window.innerHeight}H0Z M${x - o} ${y - o}h${w + 2 * o}v${h + 2 * o}h${-(w + 2 * o)}Z`}
      />
      <rect
        className="stroke-rec fill-none"
        strokeWidth={2}
        strokeDasharray="6 4"
        x={x - o + 1}
        y={y - o + 1}
        width={w + 2 * o - 2}
        height={h + 2 * o - 2}
      />
    </svg>
  );
}
