import { useEffect, useRef, useState } from 'react';

import { convertDrop, hideConvert } from '@/domain/convert/apis/convert';
import type { Slice } from '@/domain/convert/apis/convert.dto';
import Aurora from '@/shared/components/Aurora';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { DONUT, dropPoint, labelAt, restPath, sliceAt, slicePath } from '@/utils/donut';
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
    // Clears the donut, then lets Rust hide the window once that blank frame is painted: a hidden
    // window shows its last frame when shown again, which must not be this donut. The drop event
    // below arrives after this, so `slicesRef` keeps the slices; only the next open replaces them.
    const offClose = listen<{ seq: number }>('convert://close', (e) => {
      setOpen(null);
      setHot(null);
      requestAnimationFrame(() => requestAnimationFrame(() => void hideConvert(e.payload.seq).catch(() => undefined)));
    });
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
  const rest = restPath(n);
  return (
    <div
      key={play}
      className={cx(
        'relative size-[260px] motion-reduce:animate-none',
        play ? 'animate-pop' : 'opacity-0 motion-reduce:opacity-100',
      )}
    >
      {/* The halo: disc + 4px, blurred 8px, so it fades out inside the window (see shadow-donut). */}
      <div
        className="absolute animate-[spin_8s_linear_infinite] rounded-full bg-[conic-gradient(var(--color-primary),var(--color-sky),var(--color-aurora-3),var(--color-primary))] opacity-50 blur-[8px] motion-reduce:animate-none"
        style={{ inset: half - DONUT.disc - 4 }}
      />
      {/* The plate; its edge and shadow keep it readable over a light or a black desktop alike. */}
      <div
        className="bg-base-100 shadow-donut ring-donut-edge absolute overflow-hidden rounded-full ring-1"
        style={{ inset: half - DONUT.disc }}
      >
        <Aurora className="inset-0 opacity-45" />
      </div>
      <svg
        className="relative"
        width={DONUT.size}
        height={DONUT.size}
        viewBox={`${-half} ${-half} ${DONUT.size} ${DONUT.size}`}
        aria-hidden
      >
        {/* Fewer than six formats leave the rest of the ring empty; it is drawn faintly, and hits nothing. */}
        {rest && (
          <path
            d={rest}
            className={cx('fill-donut-rest', play && 'animate-slice-in motion-reduce:animate-none')}
            style={{ animationDelay: `${40 + n * 25}ms` }}
          />
        )}
        {open.slices.map(({ format, needs }, i) => {
          const l = labelAt(i, n);
          const active = i === hot && !needs;
          return (
            <g
              key={format}
              className={cx(play && 'animate-slice-in motion-reduce:animate-none', needs && 'opacity-45')}
              style={{ animationDelay: `${40 + i * 25}ms` }}
            >
              <path
                d={slicePath(i, n)}
                className={cx(
                  'slice-lit [transition:fill_180ms_ease,scale_350ms_var(--ease-spring)] motion-reduce:transition-none',
                  active ? 'fill-primary scale-[1.07]' : 'fill-donut',
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
      {/* Inside the ring's hole (inner 54px). */}
      <div className="absolute inset-[82px] flex flex-col items-center justify-center gap-1 text-center">
        <span className="bg-accent-subtle text-primary animate-bob grid size-[30px] place-items-center rounded-full">
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.8}
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden
          >
            <path d="M7 2v10M3 8l4 4 4-4" />
          </svg>
        </span>
        <small className="text-muted text-[9.5px] leading-tight">{t.convert.dropHint}</small>
      </div>
    </div>
  );
}
