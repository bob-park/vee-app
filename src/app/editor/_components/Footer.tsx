import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import type { Size } from '@/utils/edit';

interface FooterProps {
  from: Size;
  to: Size;
  /** The last failed render or copy. */
  error: string | null;
  canCopy: boolean;
  onCancel: () => void;
  onCopy: () => void;
}

const size = (s: Size) => `${s.w} × ${s.h}`;

/** The size change (or the last error) on the left, Cancel / Copy on the right. */
export default function Footer({ from, to, error, canCopy, onCancel, onCopy }: FooterProps) {
  // hooks
  const { t } = useI18n();

  return (
    <footer className="border-divider bg-card relative z-1 flex shrink-0 items-center gap-2 border-t px-4 py-2.5">
      {error ? (
        <span className="text-error min-w-0 flex-1 truncate text-[12px]" role="alert" title={error}>
          {error}
        </span>
      ) : (
        <span className="text-muted flex-1 text-[12px] tabular-nums">
          {from.w === to.w && from.h === to.h ? size(from) : `${size(from)} → ${size(to)}`}
        </span>
      )}
      <button className="rounded-box bg-subtle text-base-content px-3.5 py-1.5 font-semibold" onClick={onCancel}>
        {t.editor.cancel}
      </button>
      <button
        className="rounded-box from-primary to-sky bg-linear-to-br px-4 py-1.5 font-semibold text-white transition-transform duration-150 active:scale-96 disabled:cursor-default disabled:opacity-50 motion-reduce:transition-none"
        disabled={!canCopy}
        onClick={onCopy}
      >
        {t.editor.copy}
      </button>
    </footer>
  );
}
