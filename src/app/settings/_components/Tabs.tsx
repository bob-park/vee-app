import type { KeyboardEvent, ReactNode } from 'react';

import { useI18n } from '@/shared/providers/i18n/I18nProvider';

import cx from 'classnames';

export type SettingsTab = 'general' | 'panel' | 'shortcuts' | 'convert' | 'permissions' | 'history';

const isMac = navigator.userAgent.includes('Mac');

const svg = (children: ReactNode) => (
  <svg
    width="20"
    height="20"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    {children}
  </svg>
);

const ALL_TABS: { value: SettingsTab; icon: ReactNode }[] = [
  {
    value: 'general',
    icon: svg(
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" />
      </>,
    ),
  },
  {
    value: 'panel',
    icon: svg(
      <>
        <rect x="3" y="5" width="18" height="14" rx="2.5" />
        <path d="M3 13h18" />
      </>,
    ),
  },
  {
    value: 'shortcuts',
    icon: svg(<path d="M9 9V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V9Z" />),
  },
  {
    value: 'convert',
    icon: svg(<path d="M4 7h13l-3-3M20 17H7l3 3" />),
  },
  {
    value: 'permissions',
    icon: svg(<path d="M12 3l7 3v6c0 4.5-3 8-7 9-4-1-7-4.5-7-9V6l7-3Z" />),
  },
  {
    value: 'history',
    icon: svg(
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7v5l3 2" />
      </>,
    ),
  },
];

// Both permissions are macOS-only, so Windows would get an empty tab.
const TABS = ALL_TABS.filter((tab) => isMac || tab.value !== 'permissions');

export const isSettingsTab = (v: unknown): v is SettingsTab => TABS.some((tab) => tab.value === v);

export function Tabs({ value, onChange }: { value: SettingsTab; onChange: (tab: SettingsTab) => void }) {
  // hooks
  const labels = useI18n().t.settings.tabs;

  const index = TABS.findIndex((tab) => tab.value === value);

  // handle

  // Arrow keys move between tabs (WAI-ARIA tabs pattern, automatic activation).
  const onKeyDown = (e: KeyboardEvent) => {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = TABS[(index + step + TABS.length) % TABS.length].value;
    onChange(next);
    document.getElementById(`tab-${next}`)?.focus();
  };

  return (
    <div
      className="glass relative z-1 mt-1.5 mb-3.5 flex shrink-0 gap-1 self-center rounded-[14px] p-1"
      role="tablist"
      onKeyDown={onKeyDown}
    >
      {/* The sliding pill: one 72px tab plus the 4px gap per step, indexed in the filtered TABS. */}
      <span
        className="bg-accent-subtle ease-spring absolute top-1 left-1 h-[calc(100%-8px)] w-[72px] rounded-[10px] transition-transform duration-450"
        style={{ transform: `translateX(${index * 76}px)` }}
        aria-hidden
      />
      {TABS.map((tab) => (
        <button
          key={tab.value}
          id={`tab-${tab.value}`}
          role="tab"
          aria-selected={tab.value === value}
          aria-controls="settings-panel"
          tabIndex={tab.value === value ? 0 : -1}
          className={cx(
            'relative flex w-[72px] flex-col items-center gap-[3px] rounded-[10px] py-1.5 text-[11px]',
            tab.value === value ? 'text-primary font-semibold' : 'text-muted',
          )}
          onClick={() => onChange(tab.value)}
        >
          <span
            className={cx('ease-spring block transition-transform duration-450', { 'scale-110': tab.value === value })}
          >
            {tab.icon}
          </span>
          {labels[tab.value]}
        </button>
      ))}
    </div>
  );
}
