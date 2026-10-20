import { useEffect, useRef, useState } from 'react';

import { convertDrop } from '@/domain/convert/apis/convert';
import type { Format } from '@/domain/convert/apis/convert.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { DONUT, dropPoint, labelAt, sliceAt, slicePath } from '@/utils/donut';

import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import cx from 'classnames';

const isMac = navigator.userAgent.includes('Mac');

interface OpenPayload {
  formats: Format[];
  name: string;
}

/** The format donut shown under the cursor while a card is dragged; dropping on a slice converts. */
export default function ConvertApp() {
  // ref
  // The drag-drop listener is added once, so it reads the formats from here.
  const formatsRef = useRef<Format[]>([]);

  // state
  const [open, setOpen] = useState<OpenPayload | null>(null);
  const [hot, setHot] = useState<number | null>(null);

  // hooks
  const { t } = useI18n();

  // useEffect
  useEffect(() => {
    const offOpen = listen<OpenPayload>('convert://open', (e) => {
      formatsRef.current = e.payload.formats;
      setOpen(e.payload);
      setHot(null);
    });
    // Rust hides the window and sends this before the drop event below arrives, so it must
    // leave the formats alone; only the next `convert://open` replaces them.
    const offClose = listen('convert://close', () => setHot(null));
    const offDrag = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'leave') return setHot(null);
      const formats = formatsRef.current;
      const { x, y } = dropPoint(p.position, devicePixelRatio, isMac);
      const i = sliceAt(x, y, formats.length);
      if (p.type !== 'drop') return setHot(i);
      setHot(null);
      // Rejects when Rust has no pending drop for it (a stale or replaced drag); nothing to show.
      if (i !== null) void convertDrop(formats[i]).catch(() => undefined);
    });
    return () => {
      void offOpen.then((off) => off());
      void offClose.then((off) => off());
      void offDrag.then((off) => off());
    };
  }, []);

  if (!open) return null;
  const n = open.formats.length;
  const half = DONUT.size / 2;
  return (
    <div className="relative size-[260px]">
      <svg width={DONUT.size} height={DONUT.size} viewBox={`${-half} ${-half} ${DONUT.size} ${DONUT.size}`} aria-hidden>
        {open.formats.map((f, i) => {
          const l = labelAt(i, n);
          return (
            <g key={f}>
              <path d={slicePath(i, n)} className={cx('stroke-base-300', i === hot ? 'fill-primary' : 'fill-donut')} />
              <text
                x={l.x}
                y={l.y + 4}
                textAnchor="middle"
                className={cx('text-[12px] font-bold', i === hot ? 'fill-primary-content' : 'fill-base-content')}
              >
                {f.toUpperCase()}
              </text>
            </g>
          );
        })}
      </svg>
      <div className="bg-card shadow-card absolute inset-[90px] flex flex-col items-center justify-center rounded-full px-2 text-center">
        <b className="max-w-full truncate text-[12px]">{open.name}</b>
        <small className="text-muted text-[10px] leading-tight">{t.convert.dropHint}</small>
      </div>
    </div>
  );
}
