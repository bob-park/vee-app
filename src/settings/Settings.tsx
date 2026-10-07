import { useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type UpdateStatus } from "../api.ts";
import { relativeTime } from "../i18n/index.ts";
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

function UpdateRow() {
  const { settings, t, locale } = usePrefs();
  const s = t.settings;
  const [status, setStatus] = useState<UpdateStatus>({ status: "idle" });

  useEffect(() => {
    void api.getUpdateStatus().then(setStatus);
    const unlisten = listen<UpdateStatus>("update://status", (e) => setStatus(e.payload));
    return () => void unlisten.then((off) => off());
  }, []);

  const detail = (() => {
    switch (status.status) {
      case "idle":
        return s.notChecked;
      case "checking":
        return s.checking;
      case "upToDate":
        return `${s.upToDate} · ${relativeTime(status.checkedAt, locale)}`;
      case "failed":
        return `${s.checkFailed} · ${relativeTime(status.checkedAt, locale)}`;
      case "ready":
        return s.updateReady(status.version);
    }
  })();

  return (
    <Row label={s.updates} hint={`${s.version(settings.version)} · ${detail}`}>
      {status.status === "ready" ? (
        <button className="btn" onClick={() => void api.installUpdate()}>
          {s.restartToUpdate}
        </button>
      ) : (
        <button className="btn" disabled={status.status === "checking"} onClick={() => void api.checkUpdate()}>
          {s.checkNow}
        </button>
      )}
    </Row>
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
        <Row label={s.copySound}>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label={s.copySound}
            checked={settings.sound === "on"}
            onChange={(e) => run(api.setSetting("sound", e.target.checked ? "on" : "off"))}
          />
        </Row>
        <Row label={s.confirmDelete} hint={s.confirmDeleteHint}>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label={s.confirmDelete}
            checked={settings.confirmDelete === "on"}
            onChange={(e) => run(api.setSetting("confirmDelete", e.target.checked ? "on" : "off"))}
          />
        </Row>
        <Row label={s.shortcut} hint={s.shortcutHint}>
          <ShortcutRecorder value={settings.shortcut} />
        </Row>
      </section>
      <section>
        <UpdateRow />
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
