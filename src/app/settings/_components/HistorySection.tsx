import { useEffect, useState } from 'react';

import { useQueryClient } from '@tanstack/react-query';

import { Segmented } from '@/app/settings/SettingsApp';
import { clearHistory } from '@/domain/clip/apis/clip';
import { STATS_QUERY_KEY, useStats } from '@/domain/clip/queries/clip';
import type { Retention, SettingKey } from '@/domain/settings/apis/settings.dto';
import { usePrunableCount, useSettings } from '@/domain/settings/queries/settings';
import { setAppExcluded } from '@/domain/source_app/apis/sourceApp';
import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import {
  EXCLUDED_APPS_QUERY_KEY,
  KNOWN_APPS_QUERY_KEY,
  useAppClipCount,
  useExcludedApps,
  useKnownApps,
} from '@/domain/source_app/queries/sourceApp';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

import { listen } from '@tauri-apps/api/event';
import { ask } from '@tauri-apps/plugin-dialog';

import { AppPicker } from './AppPicker';

const RETENTIONS: Retention[] = ['off', '7', '30', '90'];

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
  // hooks
  const { t } = useI18n();

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
  { kind: 'retention'; value: Retention; count: number } | { kind: 'exclude'; app: App; count: number } | null;

interface Props {
  /** Runs a command, showing its error under the settings. */
  run: (p: Promise<unknown>) => void;
  updateSetting: (v: { key: SettingKey; value: string }) => void;
}

export function HistorySection({ run, updateSetting }: Props) {
  // state
  const [pending, setPending] = useState<Pending>(null);

  // hooks
  const { t } = useI18n();
  const queryClient = useQueryClient();

  // queries
  const { settings } = useSettings();
  const { stats } = useStats();
  const { apps } = useKnownApps();
  const { excluded } = useExcludedApps();
  const { fetchPrunableCount } = usePrunableCount();
  const { fetchAppClipCount } = useAppClipCount();

  // useEffect
  useEffect(() => {
    const refresh = () => {
      void queryClient.invalidateQueries({ queryKey: EXCLUDED_APPS_QUERY_KEY });
      void queryClient.invalidateQueries({ queryKey: KNOWN_APPS_QUERY_KEY });
      void queryClient.invalidateQueries({ queryKey: STATS_QUERY_KEY });
    };
    const offClips = listen('clips://changed', refresh);
    const offAdded = listen('clips://added', refresh);
    const offSettings = listen('settings://changed', refresh);
    return () => {
      void offClips.then((off) => off());
      void offAdded.then((off) => off());
      void offSettings.then((off) => off());
    };
  }, [queryClient]);

  // handle
  const s = t.settings;

  const pickRetention = async (value: Retention) => {
    setPending(null);
    if (value === settings?.retention) return;
    const count = await fetchPrunableCount(value);
    if (count === 0) updateSetting({ key: 'retention', value });
    else setPending({ kind: 'retention', value, count });
  };

  const pickApp = async (id: number) => {
    setPending(null);
    const app = apps.find((a) => a.id === id);
    if (!app) return;
    const count = await fetchAppClipCount(id);
    if (count === 0) run(setAppExcluded(id, true));
    else setPending({ kind: 'exclude', app, count });
  };

  /** Recounts first: the box may have sat open while more items aged out or were copied. */
  const confirm = async () => {
    if (!pending) return;
    const count =
      pending.kind === 'retention' ? await fetchPrunableCount(pending.value) : await fetchAppClipCount(pending.app.id);
    if (count > pending.count) {
      setPending({ ...pending, count });
      return;
    }
    if (pending.kind === 'retention') updateSetting({ key: 'retention', value: pending.value });
    else run(setAppExcluded(pending.app.id, true));
    setPending(null);
  };

  const clear = async () => {
    setPending(null);
    const pinned = stats?.pinned ?? 0;
    const message = pinned > 0 ? s.clearConfirmKeepPinned(pinned) : s.clearConfirm;
    if (await ask(message, { title: 'Vee', kind: 'warning' })) run(clearHistory());
  };

  return (
    <>
      <section>
        <div className="setting-row col">
          <div>
            {s.retention}
            <small>{s.retentionHint}</small>
          </div>
          <Segmented
            className="wide"
            value={settings?.retention ?? 'off'}
            pending={pending?.kind === 'retention' ? pending.value : undefined}
            onChange={(v) => void pickRetention(v)}
            options={RETENTIONS.map((r) => ({ value: r, label: s.retentionOptions[r] }))}
          />
          {pending?.kind === 'retention' && (
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
          {excluded.length > 0 && (
            <div className="app-tags">
              {excluded.map((a) => (
                <span key={a.id} className="app-chip">
                  {a.icon && <img src={a.icon} alt="" />}
                  {a.name}
                  <button aria-label={s.includeApp(a.name)} onClick={() => run(setAppExcluded(a.id, false))}>
                    ×
                  </button>
                </span>
              ))}
            </div>
          )}
          <AppPicker apps={apps} onPick={(a) => void pickApp(a.id)} />
          {pending?.kind === 'exclude' && (
            <ConfirmBox
              message={s.excludeConfirm(pending.app.name, pending.count)}
              confirmLabel={s.deleteAndExclude}
              onConfirm={() => void confirm()}
              onCancel={() => setPending(null)}
            />
          )}
        </div>
        <div className="setting-row">
          <span className="stat">{stats ? s.stats(stats.count, formatBytes(stats.imageBytes)) : ''}</span>
          <button className="btn danger" onClick={() => void clear()}>
            {s.clearHistory}
          </button>
        </div>
      </section>
    </>
  );
}
