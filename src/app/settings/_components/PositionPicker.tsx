import { PANEL_POSITIONS, type PanelPosition } from '@/domain/settings/apis/settings.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { arrowStep } from '@/utils/radio';

import cx from 'classnames';

// The panel bar inside each mini screen, on the edge it stands for.
const BAR: Record<PanelPosition, string> = {
  bottom: 'inset-x-1 bottom-1 h-[28%]',
  top: 'inset-x-1 top-1 h-[28%]',
  left: 'inset-y-1 left-1 w-[24%]',
  right: 'inset-y-1 right-1 w-[24%]',
};

const MOTION = 'ease-spring duration-450';

interface PositionPickerProps {
  value: PanelPosition;
  onChange: (value: PanelPosition) => void;
  /** Id of the element naming the group (the row label). */
  labelledBy: string;
}

/** Panel position as four mini screens in a row; the selected one lifts with a primary border. */
export default function PositionPicker({ value, onChange, labelledBy }: Readonly<PositionPickerProps>) {
  // hooks
  const s = useI18n().t.settings;

  const index = PANEL_POSITIONS.indexOf(value);

  // handle
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const next = arrowStep(e.key, Math.max(index, 0), PANEL_POSITIONS.length);
    if (next === null) return;
    e.preventDefault();
    onChange(PANEL_POSITIONS[next]);
    e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
  };

  return (
    <div className="grid grid-cols-4 gap-2" role="radiogroup" aria-labelledby={labelledBy} onKeyDown={handleKeyDown}>
      {PANEL_POSITIONS.map((position) => {
        const on = position === value;
        return (
          <button
            key={position}
            role="radio"
            aria-checked={on}
            tabIndex={on ? 0 : -1}
            className={cx(
              'flex w-[84px] flex-col gap-1 justify-self-center text-[11px] transition-transform',
              MOTION,
              on ? 'text-primary -translate-y-0.5 font-semibold' : 'text-muted',
            )}
            onClick={() => onChange(position)}
          >
            {/* Explicit size (84 × 10/16): older WebKit can collapse an aspect-ratio box inside a button. */}
            <span
              className={cx(
                'bg-subtle relative block h-[52.5px] w-full overflow-hidden rounded-lg border-2 transition-colors',
                MOTION,
                on ? 'border-primary' : 'border-base-300',
              )}
              aria-hidden
            >
              <span
                className={cx(
                  'absolute rounded-[3px] transition-colors',
                  MOTION,
                  BAR[position],
                  on ? 'bg-primary/70' : 'bg-muted/50',
                )}
              />
            </span>
            {s.panelPositions[position]}
          </button>
        );
      })}
    </div>
  );
}
