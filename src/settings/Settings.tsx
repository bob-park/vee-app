import { useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, SOUND_NAMES, type SoundName, type UpdateStatus } from "../api.ts";
import { relativeTime } from "../i18n/index.ts";
import { usePrefs } from "../prefs.tsx";
import { HistorySection } from "./HistorySection.tsx";
import { ShortcutRecorder } from "./ShortcutRecorder.tsx";
import "./settings.css";

export function Row({
  label,
  hint,
  dim,
  children,
}: {
  label: string;
  hint?: string;
  dim?: boolean;
  children: ReactNode;
}) {
  return (
    <div className={dim ? "setting-row dim" : "setting-row"}>
      <div>
        {label}
        {hint && <small>{hint}</small>}
      </div>
      {children}
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  className,
  pending,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  className?: string;
  /** An option waiting for confirmation, outlined in red. */
  pending?: T;
}) {
  return (
    <div className={className ? `seg ${className}` : "seg"} role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={[o.value === value && "on", o.value === pending && "pending"].filter(Boolean).join(" ")}
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

/** macOS only; re-checked on focus so it updates after the user returns from System Settings. */
function DiskAccessRow() {
  const s = usePrefs().t.settings;
  const [granted, setGranted] = useState<boolean | null>(null);

  useEffect(() => {
    const check = () => void api.getDiskAccess().then(setGranted);
    check();
    const unlisten = getCurrentWindow().onFocusChanged((e) => e.payload && check());
    return () => void unlisten.then((off) => off());
  }, []);

  if (granted === null) return null;
  return (
    <Row label={s.diskAccess} hint={s.diskAccessHint}>
      {granted ? (
        <span className="granted">{s.diskAccessGranted}</span>
      ) : (
        <button className="btn" onClick={() => void api.openDiskAccessSettings()}>
          {s.diskAccessGrant}
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
        <Row label={s.soundName} hint={s.soundNameHint} dim={settings.sound !== "on"}>
          <div className="sound-pick">
            <select
              className="select"
              aria-label={s.soundName}
              value={settings.soundName}
              disabled={settings.sound !== "on"}
              onChange={(e) => {
                const name = e.target.value as SoundName;
                run(api.setSetting("soundName", name));
                void api.previewSound(name);
              }}
            >
              {SOUND_NAMES.map((name) => (
                <option key={name} value={name}>
                  {s.sounds[name]}
                </option>
              ))}
            </select>
            <button
              className="btn play"
              aria-label={s.preview}
              title={s.preview}
              disabled={settings.sound !== "on" || settings.soundName === "none"}
              onClick={() => void api.previewSound(settings.soundName)}
            >
              ▶
            </button>
          </div>
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
        <DiskAccessRow />
        <Row label={s.shortcut} hint={s.shortcutHint}>
          <ShortcutRecorder value={settings.shortcut} />
        </Row>
        <UpdateRow />
      </section>
      <HistorySection run={run} />
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </main>
  );
}
