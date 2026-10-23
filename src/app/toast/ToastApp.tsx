import { useEffect, useState } from 'react';

import type { ToastPayload } from '@/domain/clip/apis/clip.dto';
import type { ConvertNote } from '@/domain/convert/apis/convert.dto';
import Aurora from '@/shared/components/Aurora';
import KindIcon from '@/shared/components/KindIcon';
import type { Dict } from '@/shared/i18n/en';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { formatLabel } from '@/utils/formatLabel';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

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
    : p.pinsFit !== null
      ? [t.sidePinsTitle, t.sidePinsFit(p.pinsFit)]
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
        'border-toast-border bg-base-100 text-toast-fg shadow-toast absolute right-9 bottom-9 left-9 overflow-hidden rounded-[18px] border motion-reduce:animate-none',
        toast.out ? 'animate-toast-out motion-reduce:opacity-0' : 'animate-toast-in',
      )}
      role="status"
    >
      <Aurora className="inset-0 opacity-80" />
      <div className="relative z-1 flex items-center gap-3 py-3 pr-4 pl-3">
        <span
          className={cx(
            'glass relative flex size-[44px] flex-none items-center justify-center rounded-xl text-[22px] font-extrabold',
            p.ok ? 'text-success' : 'text-error',
          )}
        >
          {p.ok ? <KindIcon kind={p.kind} className="size-[22px]" /> : '!'}
          {/* Check badge on the icon's corner, ringed in the card colour. Two elements, because pop
              (transform) and ring (box-shadow) each need their own `animation`. */}
          {p.ok && (
            <span className="animate-ring absolute -right-[5px] -bottom-[5px] rounded-full [--ring-color:var(--color-success)]">
              <span className="animate-pop border-base-100 bg-success text-success-content flex size-[18px] items-center justify-center rounded-full border-2 text-[10px] font-extrabold [animation-delay:250ms]">
                ✓
              </span>
            </span>
          )}
        </span>
        <span className="flex min-w-0 flex-col gap-0.5">
          <b className="text-[16px]">{title}</b>
          {summary && (
            // The side-pins warning is a sentence, not a preview: it wraps instead of being cut.
            <span
              className={cx(
                'text-[13px] opacity-70',
                p.pinsFit !== null ? 'line-clamp-2 text-pretty break-keep' : 'truncate',
              )}
            >
              {summary}
            </span>
          )}
        </span>
      </div>
    </div>
  );
}
