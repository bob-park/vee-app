import type { KeyboardEvent, ReactNode } from "react";
import { usePrefs } from "../prefs.tsx";

export type SettingsTab = "general" | "panel" | "shortcuts" | "history";

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

const TABS: { value: SettingsTab; icon: ReactNode }[] = [
  {
    value: "general",
    icon: svg(
      <>
        <circle cx="12" cy="12" r="3" />
        <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1" />
      </>,
    ),
  },
  {
    value: "panel",
    icon: svg(
      <>
        <rect x="3" y="5" width="18" height="14" rx="2.5" />
        <path d="M3 13h18" />
      </>,
    ),
  },
  {
    value: "shortcuts",
    icon: svg(<path d="M9 9V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V9Z" />),
  },
  {
    value: "history",
    icon: svg(
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7v5l3 2" />
      </>,
    ),
  },
];

export function Tabs({ value, onChange }: { value: SettingsTab; onChange: (tab: SettingsTab) => void }) {
  const labels = usePrefs().t.settings.tabs;

  // Arrow keys move between tabs (WAI-ARIA tabs pattern, automatic activation).
  const onKeyDown = (e: KeyboardEvent) => {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const i = TABS.findIndex((tab) => tab.value === value);
    const next = TABS[(i + step + TABS.length) % TABS.length].value;
    onChange(next);
    document.getElementById(`tab-${next}`)?.focus();
  };

  return (
    <div className="tabs" role="tablist" onKeyDown={onKeyDown}>
      {TABS.map((tab) => (
        <button
          key={tab.value}
          id={`tab-${tab.value}`}
          role="tab"
          aria-selected={tab.value === value}
          aria-controls="settings-panel"
          tabIndex={tab.value === value ? 0 : -1}
          className={tab.value === value ? "on" : undefined}
          onClick={() => onChange(tab.value)}
        >
          {tab.icon}
          {labels[tab.value]}
        </button>
      ))}
    </div>
  );
}
