import { useState, type ReactNode } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { ShortcutRecorder } from "./ShortcutRecorder.tsx";
import "./settings.css";

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div>
        {label}
        {hint && <small>{hint}</small>}
      </div>
      {children}
    </div>
  );
}

function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <div className="seg" role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={o.value === value ? "on" : ""}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Settings() {
  const { settings, t } = usePrefs();
  const s = t.settings;
  const [error, setError] = useState<string | null>(null);
  const run = (p: Promise<unknown>) => void p.then(() => setError(null)).catch((e) => setError(String(e)));

  const clear = async () => {
    if (await ask(s.clearConfirm, { title: "Vee", kind: "warning" })) run(api.clearHistory());
  };

  return (
    <main className="settings">
      <h1>{s.title}</h1>
      <section>
        <Row label={s.theme}>
          <Segmented
            value={settings.theme}
            onChange={(v) => run(api.setSetting("theme", v))}
            options={[
              { value: "system", label: s.system },
              { value: "light", label: s.light },
              { value: "dark", label: s.dark },
            ]}
          />
        </Row>
        <Row label={s.language}>
          <Segmented
            value={settings.locale}
            onChange={(v) => run(api.setSetting("locale", v))}
            options={[
              { value: "system", label: s.system },
              { value: "ko", label: "한국어" },
              { value: "en", label: "English" },
            ]}
          />
        </Row>
        <Row label={s.launchAtLogin}>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label={s.launchAtLogin}
            checked={settings.autostart}
            onChange={(e) => run(api.setAutostart(e.target.checked))}
          />
        </Row>
        <Row label={s.shortcut} hint={s.shortcutHint}>
          <ShortcutRecorder value={settings.shortcut} />
        </Row>
      </section>
      <section>
        <Row label={s.history}>
          <button className="btn danger" onClick={() => void clear()}>
            {s.clearHistory}
          </button>
        </Row>
      </section>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </main>
  );
}
