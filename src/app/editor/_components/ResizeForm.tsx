import { useState } from 'react';

import type { EditOp } from '@/domain/edit/apis/edit.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { MAX_SIDE, type Size, resizeKeep } from '@/utils/edit';

import cx from 'classnames';

/** A whole number of px in 1..MAX_SIDE, or null. */
const parseSide = (v: string) => {
  const n = Number(v);
  return v.trim() !== '' && Number.isInteger(n) && n >= 1 && n <= MAX_SIDE ? n : null;
};

interface ResizeFormProps {
  /** The image's current full-res size; the ratio follows it. */
  size: Size;
  /** The typed size as a resize op, or null when it's invalid or unchanged. */
  onPending: (op: EditOp | null) => void;
  onApply: () => void;
}

/** Width × height inputs with a keep-ratio toggle (on by default). */
export default function ResizeForm({ size, onPending, onApply }: ResizeFormProps) {
  // state
  const [w, setW] = useState(String(size.w));
  const [h, setH] = useState(String(size.h));
  const [locked, setLocked] = useState(true);

  // hooks
  const { t } = useI18n();

  const valid = parseSide(w) !== null && parseSide(h) !== null;
  const changed = valid && (parseSide(w) !== size.w || parseSide(h) !== size.h);

  // handle
  const update = (nextW: string, nextH: string) => {
    setW(nextW);
    setH(nextH);
    const [pw, ph] = [parseSide(nextW), parseSide(nextH)];
    onPending(pw && ph && (pw !== size.w || ph !== size.h) ? { type: 'resize', w: pw, h: ph } : null);
  };

  const handleSide = (side: 'w' | 'h', value: string) => {
    const n = parseSide(value);
    if (!locked || n === null) return update(side === 'w' ? value : w, side === 'h' ? value : h);
    const next = resizeKeep(size, { [side]: n }, true);
    update(side === 'w' ? value : String(next.w), side === 'h' ? value : String(next.h));
  };

  const field = (side: 'w' | 'h') => (
    <label className="flex items-center gap-1.5 text-[12px]">
      <span className="text-muted">{side === 'w' ? t.editor.width : t.editor.height}</span>
      <input
        type="number"
        min={1}
        max={MAX_SIDE}
        className={cx(
          'bg-card text-base-content w-20 rounded-md border px-2 py-1 outline-none',
          parseSide(side === 'w' ? w : h) === null ? 'border-error' : 'border-base-300 focus:border-primary',
        )}
        value={side === 'w' ? w : h}
        onChange={(e) => handleSide(side, e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.metaKey && !e.ctrlKey && changed) onApply();
        }}
      />
    </label>
  );

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="flex items-center gap-2.5">
        {field('w')}
        <span className="text-muted">×</span>
        {field('h')}
        <button
          aria-pressed={locked}
          className={cx(
            'rounded-box px-2.5 py-1.5 text-[11px] font-semibold',
            locked ? 'bg-accent-subtle text-primary' : 'bg-subtle text-muted',
          )}
          onClick={() => setLocked(!locked)}
        >
          {t.editor.keepRatio}
        </button>
        <button
          className="rounded-box bg-accent-subtle text-primary px-3.5 py-1.5 text-[12px] font-semibold disabled:cursor-default disabled:opacity-50"
          disabled={!changed}
          onClick={onApply}
        >
          {t.editor.apply}
        </button>
      </div>
      {!valid && <p className="text-error text-[11px]">{t.editor.sizeHint(MAX_SIDE)}</p>}
    </div>
  );
}
