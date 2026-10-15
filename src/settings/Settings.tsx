import { useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PANEL_KEYS } from "../panel/keys.ts";
import { api, SOUND_NAMES, type SoundName, type UpdateStatus } from "../api.ts";
import { relativeTime } from "../i18n/index.ts";
import { usePrefs } from "../prefs.tsx";
import { errorText } from "../utils/errorText.ts";
import { ConfirmModal } from "./ConfirmModal.tsx";
import { Dropdown } from "./Dropdown.tsx";
import { HistorySection } from "./HistorySection.tsx";
import { ShortcutRecorder } from "./ShortcutRecorder.tsx";
import { Tabs, type SettingsTab } from "./Tabs.tsx";
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

/**
 * macOS only; re-checked on focus so it updates after the user returns from System Settings.
 * Brings its own card so no empty card shows while the state is unknown.
 */
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
    <section>
      <Row label={s.diskAccess} hint={s.diskAccessHint}>
        {granted ? (
          <span className="granted">{s.diskAccessGranted}</span>
        ) : (
          <button className="btn" onClick={() => void api.openDiskAccessSettings()}>
            {s.diskAccessGrant}
          </button>
        )}
      </Row>
    </section>
  );
}

export function Settings() {
  const { settings, t } = usePrefs();
  const s = t.settings;
  const [error, setError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [tab, setTab] = useState<SettingsTab>("general");
  const changeTab = (next: SettingsTab) => {
    setTab(next);
    setError(null);
  };
  // The window is only hidden on close, so state survives; every open starts on the general tab.
  useEffect(() => {
    const unlisten = listen("settings://opened", () => changeTab("general"));
    return () => void unlisten.then((off) => off());
  }, []);
  const run = (p: Promise<unknown>) => void p.then(() => setError(null)).catch((e) => setError(errorText(e)));

  return (
    <main className="settings">
      <Tabs value={tab} onChange={changeTab} />
      <div className="tab-body" id="settings-panel" role="tabpanel" aria-labelledby={`tab-${tab}`}>
        {tab === "general" && (
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
            <UpdateRow />
          </section>
        )}
        {tab === "panel" && (
          <section>
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
                <Dropdown
                  ariaLabel={s.soundName}
                  value={settings.soundName}
                  disabled={settings.sound !== "on"}
                  options={SOUND_NAMES.map((name) => ({ value: name, label: s.sounds[name] }))}
                  onChange={(name: SoundName) => {
                    run(api.setSetting("soundName", name));
                    void api.previewSound(name);
                  }}
                />
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
            <Row label={s.panelResize} hint={s.panelResizeHint}>
              <input
                type="checkbox"
                role="switch"
                className="switch"
                aria-label={s.panelResize}
                checked={settings.panelResize === "on"}
                onChange={(e) => run(api.setSetting("panelResize", e.target.checked ? "on" : "off"))}
              />
            </Row>
            {/* Stays enabled while resizing is off: the saved size is kept. */}
            <Row label={s.panelSizeReset}>
              <button className="btn" disabled={settings.panelHeightRatio === ""} onClick={() => setConfirmReset(true)}>
                {s.reset}
              </button>
            </Row>
          </section>
        )}
        {tab === "shortcuts" && (
          <section>
            <Row label={s.shortcut} hint={s.shortcutHint}>
              <ShortcutRecorder
                value={settings.shortcut}
                onSave={api.setShortcut}
                taken={[...PANEL_KEYS, settings.panelPinShortcut, settings.resetSearchShortcut]}
              />
            </Row>
            <Row label={s.panelPinShortcut} hint={s.panelPinShortcutHint}>
              <ShortcutRecorder
                value={settings.panelPinShortcut}
                onSave={(accel) => api.setSetting("panelPinShortcut", accel)}
                taken={[...PANEL_KEYS, settings.shortcut, settings.resetSearchShortcut]}
              />
            </Row>
            <Row label={s.resetSearchShortcut} hint={s.resetSearchShortcutHint}>
              <ShortcutRecorder
                value={settings.resetSearchShortcut}
                onSave={(accel) => api.setSetting("resetSearchShortcut", accel)}
                taken={[...PANEL_KEYS, settings.shortcut, settings.panelPinShortcut]}
              />
            </Row>
          </section>
        )}
        {tab === "history" && (
          <>
            <DiskAccessRow />
            <HistorySection run={run} />
          </>
        )}
      </div>
      <ConfirmModal
        open={confirmReset}
        title={s.panelSizeReset}
        body={s.panelSizeResetBody}
        confirmLabel={s.reset}
        onConfirm={() => {
          run(api.resetPanelHeight());
          setConfirmReset(false);
        }}
        onCancel={() => setConfirmReset(false)}
      />
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </main>
  );
}
