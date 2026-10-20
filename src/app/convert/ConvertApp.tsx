import { useEffect, useRef, useState } from 'react';

import { convertDrop } from '@/domain/convert/apis/convert';
import type { Slice } from '@/domain/convert/apis/convert.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { DONUT, dropPoint, labelAt, sliceAt, slicePath } from '@/utils/donut';
import { formatLabel } from '@/utils/formatLabel';

import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import cx from 'classnames';

const isMac = navigator.userAgent.includes('Mac');

interface OpenPayload {
  slices: Slice[];
  name: string;
}

/** The format donut shown under the cursor while a card is dragged; dropping on a slice converts. */
export default function ConvertApp() {
  // ref
  // The drag-drop listener is added once, so it reads the slices from here.
  const slicesRef = useRef<Slice[]>([]);

  // state
  const [open, setOpen] = useState<OpenPayload | null>(null);
  const [hot, setHot] = useState<number | null>(null);
  // Bumped per open so the entrance replays; set on the first frame the (shown) window paints.
  const [play, setPlay] = useState(0);

  // hooks
  const { t } = useI18n();

  // useEffect
  useEffect(() => {
    const offOpen = listen<OpenPayload>('convert://open', (e) => {
      slicesRef.current = e.payload.slices;
      setOpen(e.payload);
      setHot(null);
      setPlay(0);
      // A hidden window paints no frames, so this waits until Rust has shown it.
      requestAnimationFrame(() => setPlay(Date.now()));
    });
    // Rust hides the window and sends this before the drop event below arrives, so it must
    // leave the slices alone; only the next `convert://open` replaces them.
    const offClose = listen('convert://close', () => setHot(null));
    const offDrag = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'leave') return setHot(null);
      const slices = slicesRef.current;
      const { x, y } = dropPoint(p.position, devicePixelRatio, isMac);
      const i = sliceAt(x, y, slices.length);
      if (p.type !== 'drop') return setHot(i);
      setHot(null);
      // A slice that still needs an engine does nothing. Rejects when Rust has no pending drop
      // for it (a stale or replaced drag); nothing to show.
      if (i !== null && !slices[i].needs) void convertDrop(slices[i].format).catch(() => undefined);
    });
    return () => {
      void offOpen.then((off) => off());
      void offClose.then((off) => off());
      void offDrag.then((off) => off());
    };
  }, []);

  if (!open) return null;
  const n = open.slices.length;
  const needsLabel = {
    ffmpeg: t.convert.needsFfmpeg,
    rhwp: t.convert.needsRhwp,
    libreoffice: t.convert.needsLibreOffice,
  };
  const half = DONUT.size / 2;
  return (
    <div
      key={play}
      className={cx(
        'relative size-[260px] motion-reduce:animate-none',
        play ? 'animate-donut-in' : 'opacity-0 motion-reduce:opacity-100',
      )}
    >
      <svg width={DONUT.size} height={DONUT.size} viewBox={`${-half} ${-half} ${DONUT.size} ${DONUT.size}`} aria-hidden>
        {/* The border keeps the white disc readable over a light desktop, as the toast's does. */}
        <circle r={DONUT.disc} className="fill-card stroke-base-300 shadow-donut" strokeWidth={1} />
        {open.slices.map(({ format, needs }, i) => {
          const l = labelAt(i, n);
          const active = i === hot && !needs;
          return (
            <g
              key={format}
              className={cx(play && 'animate-slice-in motion-reduce:animate-none', needs && 'opacity-45')}
              style={{ animationDelay: `${40 + i * 22}ms` }}
            >
              <path
                d={slicePath(i, n)}
                className={cx(
                  '[transition:fill_180ms_ease,scale_180ms_cubic-bezier(0.22,1,0.36,1)] motion-reduce:transition-none',
                  active ? 'fill-primary scale-[1.045]' : 'fill-donut',
                )}
              />
              <text
                x={l.x}
                y={needs ? l.y : l.y + 4}
                textAnchor="middle"
                className={cx(
                  'text-[12px] font-bold tracking-[-0.01em] [transition:fill_180ms_ease]',
                  active ? 'fill-primary-content' : 'fill-base-content',
                )}
              >
                {formatLabel(format)}
              </text>
              {needs && (
                <text x={l.x} y={l.y + 11} textAnchor="middle" className="fill-muted text-[8.5px]">
                  {needsLabel[needs]}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      {/* Inside the ring's hole (inner 52px); the disc behind it is the card colour already. */}
      <div className="absolute inset-[82px] flex flex-col items-center justify-center text-center">
        <b className="max-w-full truncate text-[11px]">{open.name}</b>
        <small className="text-muted text-[9.5px] leading-tight">{t.convert.dropHint}</small>
      </div>
    </div>
  );
}
