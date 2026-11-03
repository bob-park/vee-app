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

/** A video camera, in `KindIcon`'s line style: the tile for a screen recording or a video conversion. */
const VIDEO_PATH = 'M3 7h12v10H3zM15 10.5l6-3.5v10l-6-3.5z';
const VIDEO_FORMATS = new Set<string>(['mp4', 'mov', 'webm', 'gif']);

/** Whether the note is about a video: a recording, or a file saved as a video format. */
function isVideo(note: ConvertNote): boolean {
  return note.type.startsWith('record') || ('format' in note && VIDEO_FORMATS.has(note.format));
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
    case 'recordFolderMissing':
      return [t.convert.savedOne(formatLabel(note.format)), t.record.folderMissing];
    case 'recordFailed':
      return [t.record.failed, null];
    case 'recordUnsupported':
      return [t.record.unsupported, null];
    case 'recordNeedsFfmpeg':
      return [t.record.needsFfmpeg, t.record.needsFfmpegHint];
    case 'recordConverting':
      return [t.record.title, t.record.converting(formatLabel(note.format))];
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
  // Still working: no check badge until the result's toast.
  const converting = p.convert?.type === 'recordConverting';
  const done = p.ok && !converting;
  const video = p.convert ? isVideo(p.convert) : false;
  // The tile follows `ok` and `kind` for a conversion too (Rust sets them to match).
  const [title, summary] = p.convert
    ? convertLines(p.convert, t)
    : p.pinsFit !== null
      ? [t.sidePinsTitle, t.sidePinsFit(p.pinsFit)]
      : p.dragMissing
        ? [t.dragMissing, t.copyFailedDetail]
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
            converting ? 'text-primary' : p.ok ? 'text-success' : 'text-error',
          )}
        >
          {!p.ok ? (
            '!'
          ) : video ? (
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
              <path d={VIDEO_PATH} />
            </svg>
          ) : (
            <KindIcon kind={p.kind} className="size-[22px]" />
          )}
          {/* Converting: a spinning ring on the corner where the check will go. */}
          {converting && (
            <span className="border-base-100 bg-base-100 absolute -right-[5px] -bottom-[5px] flex size-[18px] items-center justify-center rounded-full border-2">
              <span className="border-primary size-full animate-spin rounded-full border-2 border-t-transparent motion-reduce:animate-none" />
            </span>
          )}
          {/* Check badge on the icon's corner, ringed in the card colour. Two elements, because pop
              (transform) and ring (box-shadow) each need their own `animation`. */}
          {done && (
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
            <span className={cx('text-[13px] opacity-70', p.pinsFit !== null ? 'line-clamp-2' : 'truncate')}>
              {summary}
            </span>
          )}
        </span>
      </div>
    </div>
  );
}
