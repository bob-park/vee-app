import type { Settings } from '@/domain/settings/apis/settings.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { arrowStep } from '@/utils/radio';

import cx from 'classnames';

type Theme = Settings['theme'];

const THEMES: Theme[] = ['system', 'light', 'dark'];

// Fixed per depicted theme, so a preview looks the same in either app theme.
const PREVIEW: Record<Theme, string> = {
  system: 'bg-[linear-gradient(135deg,var(--color-preview-light)_50%,var(--color-preview-dark)_50%)]',
  light: 'bg-preview-light',
  dark: 'bg-preview-dark',
};

const MOTION = 'ease-spring duration-450';

interface ThemePickerProps {
  value: Theme;
  onChange: (value: Theme) => void;
  /** Id of the element naming the group (the row label). */
  labelledBy: string;
}

/** Theme choice as preview tiles; the selected one lifts with a primary border. */
export default function ThemePicker({ value, onChange, labelledBy }: Readonly<ThemePickerProps>) {
  // hooks
  const s = useI18n().t.settings;

  const index = THEMES.indexOf(value);

  // handle
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const next = arrowStep(e.key, Math.max(index, 0), THEMES.length);
    if (next === null) return;
    e.preventDefault();
    onChange(THEMES[next]);
    e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
  };

  return (
    <div className="flex gap-2" role="radiogroup" aria-labelledby={labelledBy} onKeyDown={handleKeyDown}>
      {THEMES.map((theme) => {
        const on = theme === value;
        return (
          <button
            key={theme}
            role="radio"
            aria-checked={on}
            tabIndex={on ? 0 : -1}
            className={cx(
              'flex w-[74px] flex-col gap-1 text-[11px] transition-transform',
              MOTION,
              on ? 'text-primary -translate-y-0.5 font-semibold' : 'text-muted',
            )}
            onClick={() => onChange(theme)}
          >
            <span
              className={cx(
                'shadow-seg relative h-12 overflow-hidden rounded-[9px] border-2 transition-colors',
                MOTION,
                PREVIEW[theme],
                on ? 'border-primary' : 'border-transparent',
              )}
              aria-hidden
            >
              <span className="bg-muted/50 absolute top-2 left-2 h-[5px] w-[26px] rounded-[3px]" />
              <span className="bg-primary/35 absolute inset-x-2 bottom-2 h-3.5 rounded" />
            </span>
            {s[theme]}
          </button>
        );
      })}
    </div>
  );
}
