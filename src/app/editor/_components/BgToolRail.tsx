import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { arrowStep } from '@/utils/radio';

import cx from 'classnames';

export type BgTool = 'auto' | 'subject' | 'brush';

const TOOLS: BgTool[] = ['auto', 'subject', 'brush'];

const ICONS: Record<BgTool, React.ReactNode> = {
  auto: <path d="M12 3l1.8 4.2L18 9l-4.2 1.8L12 15l-1.8-4.2L6 9l4.2-1.8z" />,
  subject: <path d="M4 4l7 16 2-7 7-2z" />,
  brush: (
    <>
      <path d="M18 3l3 3-9 9-3-3z" />
      <path d="M9 12c-3 0-4 2-4 4s-1 3-2 3c3 1 7 0 7-3" />
    </>
  ),
};

interface BgToolRailProps {
  value: BgTool;
  onChange: (value: BgTool) => void;
  disabled: boolean;
}

/** The Remove background tools as a vertical glass radio group at the stage's left edge. */
export default function BgToolRail({ value, onChange, disabled }: BgToolRailProps) {
  // hooks
  const { t } = useI18n();

  // handle
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const next = arrowStep(e.key, TOOLS.indexOf(value), TOOLS.length);
    if (next === null || disabled) return;
    e.preventDefault();
    onChange(TOOLS[next]);
    e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
  };

  return (
    <div
      className="glass absolute top-1/2 left-0 z-2 flex -translate-y-1/2 flex-col gap-1 rounded-xl p-1"
      role="radiogroup"
      aria-label={t.editor.tabs.removeBg}
      onKeyDown={handleKeyDown}
    >
      {TOOLS.map((tool) => (
        <button
          key={tool}
          role="radio"
          aria-checked={tool === value}
          aria-label={t.editor.bgTools[tool]}
          title={t.editor.bgTools[tool]}
          tabIndex={tool === value ? 0 : -1}
          disabled={disabled}
          className={cx('flex size-7.5 items-center justify-center rounded-[9px] disabled:opacity-50', {
            'bg-card text-primary shadow-seg': tool === value,
            'text-muted': tool !== value,
          })}
          onClick={() => onChange(tool)}
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinejoin="round"
            aria-hidden
          >
            {ICONS[tool]}
          </svg>
        </button>
      ))}
    </div>
  );
}
