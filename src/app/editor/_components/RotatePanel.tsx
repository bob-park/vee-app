import RotateIcon from '@/shared/components/RotateIcon';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

/** Two buttons that turn the image a quarter at once. */
export default function RotatePanel({
  disabled,
  onRotate,
}: {
  disabled: boolean;
  onRotate: (quarter: -1 | 1) => void;
}) {
  // hooks
  const { t } = useI18n();

  return (
    <div className="flex justify-center gap-2.5">
      {([-1, 1] as const).map((q) => (
        <button
          key={q}
          className="rounded-box bg-card border-base-300 text-primary inline-flex items-center gap-1.5 border px-3 py-1.5 font-semibold transition-transform duration-150 active:scale-96 disabled:cursor-default disabled:opacity-50 motion-reduce:transition-none"
          disabled={disabled}
          aria-label={t.editor.rotateLabel(t.editor.turns[q < 0 ? 'ccw' : 'cw'])}
          onClick={() => onRotate(q)}
        >
          <RotateIcon dir={q < 0 ? 'ccw' : 'cw'} />
          <span className="text-base-content text-[12px]">{t.editor.turns[q < 0 ? 'ccw' : 'cw']}</span>
        </button>
      ))}
    </div>
  );
}
