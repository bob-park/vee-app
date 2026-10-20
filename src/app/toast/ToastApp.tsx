import { useEffect, useState } from 'react';

import type { Kind, ToastPayload } from '@/domain/clip/apis/clip.dto';
import type { ConvertNote } from '@/domain/convert/apis/convert.dto';
import type { Dict } from '@/shared/i18n/en';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { formatLabel } from '@/utils/formatLabel';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

/** White line icons for what was copied, drawn on the coloured tile. */
const KIND_ICONS: Record<Kind, string> = {
  text: 'M5 6h14M5 10h14M5 14h10M5 18h7',
  link: 'M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1',
  image: 'M4 5h16v14H4zM4 16l5-5 4 4 2-2 5 5M15.5 8.5h.01',
  files: 'M6 3h8l4 4v14H6zM14 3v4h4',
};

function Icon({ path }: { path: string }) {
  return (
    <svg
      className="size-[22px]"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d={path} />
    </svg>
  );
}

function convertLines(note: ConvertNote, t: Dict): [string, string | null] {
  switch (note.type) {
    case 'saved': {
      const fmt = formatLabel(note.format);
      return [note.count > 1 ? t.convert.savedMany(note.count, fmt) : t.convert.savedOne(fmt), note.folder];
    }
    case 'failed':
      return [t.convert.failedTitle, `${note.name} · ${t.convert.errors[note.error]}`];
    case 'notFolder':
      return [t.convert.notFolder, note.finderBlocked ? t.convert.finderBlockedHint : null];
    case 'unsupported':
      return [t.convert.unsupported, null];
  }
}

export function ToastApp() {
  // state
  const [toast, setToast] = useState<{ payload: ToastPayload; key: number; out: boolean } | null>(null);

  // hooks
  const { t } = useI18n();

  // useEffect
  useEffect(() => {
    const offShow = listen<ToastPayload>('toast://show', (e) =>
      setToast({ payload: e.payload, key: Date.now(), out: false }),
    );
    const offHide = listen('toast://hide', () => setToast((cur) => cur && { ...cur, out: true }));
    return () => {
      void offShow.then((off) => off());
      void offHide.then((off) => off());
    };
  }, []);

  if (!toast) return null;
  const p = toast.payload;
  // The tile follows `ok` and `kind` for a conversion too (Rust sets them to match).
  const [title, summary] = p.convert
    ? convertLines(p.convert, t)
    : [
        p.ok ? (p.plain ? t.copiedPlain : t.copied) : t.copyFailed,
        !p.ok
          ? t.copyFailedDetail
          : p.kind === 'image'
            ? t.kinds.image
            : p.kind === 'files'
              ? t.filesCount(p.files)
              : p.text,
      ];
  return (
    // The window is larger than the card; the margin is room for the card's shadow.
    <div
      key={toast.key}
      className={cx(
        'border-toast-border bg-toast-bg text-toast-fg shadow-toast absolute right-9 bottom-9 left-9 flex items-center gap-3 rounded-2xl border py-3 pr-4 pl-3 motion-reduce:animate-none',
        toast.out ? 'animate-toast-out motion-reduce:opacity-0' : 'animate-toast-in',
      )}
      role="status"
    >
      <span
        className={cx(
          'relative flex size-[42px] flex-none items-center justify-center rounded-xl text-[22px] font-extrabold text-white',
          p.ok ? 'bg-toast-icon' : 'bg-error',
        )}
      >
        {p.ok ? <Icon path={KIND_ICONS[p.kind]} /> : '!'}
        {/* Small check badge on the icon's corner, ringed in the card colour. */}
        {p.ok && (
          <span className="animate-toast-check-in border-toast-bg bg-primary absolute -right-[5px] -bottom-[5px] flex size-[18px] items-center justify-center rounded-full border-2 text-[10px] font-extrabold text-white motion-reduce:animate-none">
            ✓
          </span>
        )}
      </span>
      <span className="flex min-w-0 flex-col gap-0.5">
        <b className="text-[16px]">{title}</b>
        {summary && <span className="truncate text-[13px] opacity-70">{summary}</span>}
      </span>
    </div>
  );
}
