import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, type App, type Retention, type Stats } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { Dropdown } from "./Dropdown.tsx";
import { Segmented } from "./Settings.tsx";

const RETENTIONS: Retention[] = ["off", "7", "30", "90"];

function formatBytes(n: number): string {
  if (n < 1024 * 1024) return `${Math.round(n / 1024)}KB`;
  if (n < 1024 * 1024 * 1024) return `${Math.round(n / 1024 / 1024)}MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(1)}GB`;
}

function ConfirmBox({
  message,
  confirmLabel,
  onConfirm,
  onCancel,
}: {
  message: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const { t } = usePrefs();
  return (
    <div className="confirm-box" role="alertdialog">
      <p>{message}</p>
      <div className="confirm-acts">
        <button className="btn gray" onClick={onCancel}>
          {t.cancel}
        </button>
        <button className="btn danger" onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </div>
  );
}

/** One confirmation at a time: picking anything else cancels the open one. */
type Pending =
  | { kind: "retention"; value: Retention; count: number }
  | { kind: "exclude"; app: App; count: number }
  | null;

export function HistorySection({ run }: { run: (p: Promise<unknown>) => void }) {
  const { settings, t } = usePrefs();
  const s = t.settings;
  const [pending, setPending] = useState<Pending>(null);
  const [excluded, setExcluded] = useState<App[]>([]);
  const [apps, setApps] = useState<App[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);

  const refresh = useCallback(() => {
    void Promise.all([api.listExcludedApps(), api.listKnownApps(), api.getStats()]).then(([ex, all, st]) => {
      setExcluded(ex);
      setApps(all);
      setStats(st);
    });
  }, []);

  useEffect(() => {
    refresh();
    const offClips = listen("clips://changed", refresh);
    const offSettings = listen("settings://changed", refresh);
    return () => {
      void offClips.then((off) => off());
      void offSettings.then((off) => off());
    };
  }, [refresh]);

  const pickRetention = async (value: Retention) => {
    setPending(null);
    if (value === settings.retention) return;
    const count = await api.countPrunable(value);
    if (count === 0) run(api.setSetting("retention", value));
    else setPending({ kind: "retention", value, count });
  };

  const pickApp = async (id: number) => {
    setPending(null);
    const app = apps.find((a) => a.id === id);
    if (!app) return;
    const count = await api.countAppClips(id);
    if (count === 0) run(api.setAppExcluded(id, true));
    else setPending({ kind: "exclude", app, count });
  };

  /** Recounts first: the box may have sat open while more items aged out or were copied. */
  const confirm = async () => {
    if (!pending) return;
    const count =
      pending.kind === "retention" ? await api.countPrunable(pending.value) : await api.countAppClips(pending.app.id);
    if (count > pending.count) {
      setPending({ ...pending, count });
      return;
    }
    if (pending.kind === "retention") run(api.setSetting("retention", pending.value));
    else run(api.setAppExcluded(pending.app.id, true));
    setPending(null);
  };

  const clear = async () => {
    setPending(null);
    const pinned = stats?.pinned ?? 0;
    const message = pinned > 0 ? s.clearConfirmKeepPinned(pinned) : s.clearConfirm;
    if (await ask(message, { title: "Vee", kind: "warning" })) run(api.clearHistory());
  };


  return (
    <>
      <h2 className="sec-title">{s.historySection}</h2>
      <section>
        <div className="setting-row col">
          <div>
            {s.retention}
            <small>{s.retentionHint}</small>
          </div>
          <Segmented
            className="wide"
            value={settings.retention}
            pending={pending?.kind === "retention" ? pending.value : undefined}
            onChange={(v) => void pickRetention(v)}
            options={RETENTIONS.map((r) => ({ value: r, label: s.retentionOptions[r] }))}
          />
          {pending?.kind === "retention" && (
            <ConfirmBox
              message={s.retentionConfirm(s.retentionOptions[pending.value], pending.count)}
              confirmLabel={s.deleteAndApply}
              onConfirm={() => void confirm()}
              onCancel={() => setPending(null)}
            />
          )}
        </div>
        <div className="setting-row col">
          <div>
            {s.excludedApps}
            <small>{s.excludedAppsHint}</small>
          </div>
          <div className="app-tags">
            {excluded.map((a) => (
              <span key={a.id} className="app-chip">
                {a.icon && <img src={a.icon} alt="" />}
                {a.name}
                <button aria-label={s.includeApp(a.name)} onClick={() => run(api.setAppExcluded(a.id, false))}>
                  ×
                </button>
              </span>
            ))}
            <Dropdown
              className="add-app"
              ariaLabel={s.addApp}
              placeholder={s.addApp}
              value={null}
              disabled={apps.length === 0}
              options={apps.map((a) => ({ value: a.id, label: a.name, icon: a.icon }))}
              onChange={(id) => void pickApp(id)}
            />
          </div>
          {pending?.kind === "exclude" && (
            <ConfirmBox
              message={s.excludeConfirm(pending.app.name, pending.count)}
              confirmLabel={s.deleteAndExclude}
              onConfirm={() => void confirm()}
              onCancel={() => setPending(null)}
            />
          )}
        </div>
        <div className="setting-row">
          <span className="stat">{stats ? s.stats(stats.count, formatBytes(stats.imageBytes)) : ""}</span>
          <button className="btn danger" onClick={() => void clear()}>
            {s.clearHistory}
          </button>
        </div>
      </section>
    </>
  );
}
