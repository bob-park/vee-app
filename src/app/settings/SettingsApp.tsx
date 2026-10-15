import { type ReactNode, useEffect, useState } from 'react';

import { ConfirmModal } from '@/app/settings/_components/ConfirmModal';
import { Dropdown } from '@/app/settings/_components/Dropdown';
import { HistorySection } from '@/app/settings/_components/HistorySection';
import { ShortcutRecorder } from '@/app/settings/_components/ShortcutRecorder';
import { type SettingsTab, Tabs } from '@/app/settings/_components/Tabs';
import { resetPanelHeight } from '@/domain/panel/apis/panel';
import {
  openDiskAccessSettings,
  previewSound,
  setAutostart,
  setSetting,
  setShortcut,
} from '@/domain/settings/apis/settings';
import { SOUND_NAMES, type SoundName } from '@/domain/settings/apis/settings.dto';
import { useDiskAccess, useSettingUpdate, useSettings } from '@/domain/settings/queries/settings';
import { checkUpdate, installUpdate } from '@/domain/updater/apis/updater';
import { useUpdateStatus } from '@/domain/updater/queries/updater';
import { relativeTime } from '@/shared/i18n';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { errorText } from '@/utils/errorText';
import { PANEL_KEYS } from '@/utils/keys';

import { listen } from '@tauri-apps/api/event';

import './_components/settings.css';

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
    <div className={dim ? 'setting-row dim' : 'setting-row'}>
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
    <div className={className ? `seg ${className}` : 'seg'} role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={[o.value === value && 'on', o.value === pending && 'pending'].filter(Boolean).join(' ')}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

function UpdateRow({ version }: { version: string }) {
  // hooks
  const { t, locale } = useI18n();

  // queries
  const { status } = useUpdateStatus();

  const s = t.settings;

  const detail = (() => {
    switch (status.status) {
      case 'idle':
        return s.notChecked;
      case 'checking':
        return s.checking;
      case 'upToDate':
        return `${s.upToDate} · ${relativeTime(status.checkedAt, locale)}`;
      case 'failed':
        return `${s.checkFailed} · ${relativeTime(status.checkedAt, locale)}`;
      case 'ready':
        return s.updateReady(status.version);
    }
  })();

  return (
    <Row label={s.updates} hint={`${s.version(version)} · ${detail}`}>
      {status.status === 'ready' ? (
        <button className="btn" onClick={() => void installUpdate()}>
          {s.restartToUpdate}
        </button>
      ) : (
        <button className="btn" disabled={status.status === 'checking'} onClick={() => void checkUpdate()}>
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
  // hooks
  const s = useI18n().t.settings;

  // queries
  const { granted } = useDiskAccess();

  if (granted === null) return null;
  return (
    <section>
      <Row label={s.diskAccess} hint={s.diskAccessHint}>
        {granted ? (
          <span className="granted">{s.diskAccessGranted}</span>
        ) : (
          <button className="btn" onClick={() => void openDiskAccessSettings()}>
            {s.diskAccessGrant}
          </button>
        )}
      </Row>
    </section>
  );
}

export function SettingsApp() {
  // state
  const [error, setError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [tab, setTab] = useState<SettingsTab>('general');

  // hooks
  const { t } = useI18n();

  // queries
  const { settings } = useSettings();
  const { updateSetting } = useSettingUpdate({
    onSuccess: () => setError(null),
    onError: (e) => setError(errorText(e)),
  });

  // useEffect
  // The window is only hidden on close, so state survives; every open starts on the general tab.
  useEffect(() => {
    const unlisten = listen('settings://opened', () => changeTab('general'));
    return () => void unlisten.then((off) => off());
  }, []);

  // handle
  const changeTab = (next: SettingsTab) => {
    setTab(next);
    setError(null);
  };
  const run = (p: Promise<unknown>) => void p.then(() => setError(null)).catch((e) => setError(errorText(e)));

  const s = t.settings;
  if (!settings) return null;

  return (
    <main className="settings">
      <Tabs value={tab} onChange={changeTab} />
      <div className="tab-body" id="settings-panel" role="tabpanel" aria-labelledby={`tab-${tab}`}>
        {tab === 'general' && (
          <section>
            <Row label={s.theme}>
              <Segmented
                value={settings.theme}
                onChange={(v) => updateSetting({ key: 'theme', value: v })}
                options={[
                  { value: 'system', label: s.system },
                  { value: 'light', label: s.light },
                  { value: 'dark', label: s.dark },
                ]}
              />
            </Row>
            <Row label={s.language}>
              <Segmented
                value={settings.locale}
                onChange={(v) => updateSetting({ key: 'locale', value: v })}
                options={[
                  { value: 'system', label: s.system },
                  { value: 'ko', label: '한국어' },
                  { value: 'en', label: 'English' },
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
                onChange={(e) => run(setAutostart(e.target.checked))}
              />
            </Row>
            <UpdateRow version={settings.version} />
          </section>
        )}
        {tab === 'panel' && (
          <section>
            <Row label={s.copySound}>
              <input
                type="checkbox"
                role="switch"
                className="switch"
                aria-label={s.copySound}
                checked={settings.sound === 'on'}
                onChange={(e) => updateSetting({ key: 'sound', value: e.target.checked ? 'on' : 'off' })}
              />
            </Row>
            <Row label={s.soundName} hint={s.soundNameHint} dim={settings.sound !== 'on'}>
              <div className="sound-pick">
                <Dropdown
                  ariaLabel={s.soundName}
                  value={settings.soundName}
                  disabled={settings.sound !== 'on'}
                  options={SOUND_NAMES.map((name) => ({ value: name, label: s.sounds[name] }))}
                  onChange={(name: SoundName) => {
                    updateSetting({ key: 'soundName', value: name });
                    void previewSound(name);
                  }}
                />
                <button
                  className="btn play"
                  aria-label={s.preview}
                  title={s.preview}
                  disabled={settings.sound !== 'on' || settings.soundName === 'none'}
                  onClick={() => void previewSound(settings.soundName)}
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
                checked={settings.confirmDelete === 'on'}
                onChange={(e) => updateSetting({ key: 'confirmDelete', value: e.target.checked ? 'on' : 'off' })}
              />
            </Row>
            <Row label={s.panelResize} hint={s.panelResizeHint}>
              <input
                type="checkbox"
                role="switch"
                className="switch"
                aria-label={s.panelResize}
                checked={settings.panelResize === 'on'}
                onChange={(e) => updateSetting({ key: 'panelResize', value: e.target.checked ? 'on' : 'off' })}
              />
            </Row>
            {/* Stays enabled while resizing is off: the saved size is kept. */}
            <Row label={s.panelSizeReset}>
              <button className="btn" disabled={settings.panelHeightRatio === ''} onClick={() => setConfirmReset(true)}>
                {s.reset}
              </button>
            </Row>
          </section>
        )}
        {tab === 'shortcuts' && (
          <section>
            <Row label={s.shortcut} hint={s.shortcutHint}>
              <ShortcutRecorder
                value={settings.shortcut}
                onSave={setShortcut}
                taken={[...PANEL_KEYS, settings.panelPinShortcut, settings.resetSearchShortcut]}
              />
            </Row>
            <Row label={s.panelPinShortcut} hint={s.panelPinShortcutHint}>
              <ShortcutRecorder
                value={settings.panelPinShortcut}
                onSave={(accel) => setSetting('panelPinShortcut', accel)}
                taken={[...PANEL_KEYS, settings.shortcut, settings.resetSearchShortcut]}
              />
            </Row>
            <Row label={s.resetSearchShortcut} hint={s.resetSearchShortcutHint}>
              <ShortcutRecorder
                value={settings.resetSearchShortcut}
                onSave={(accel) => setSetting('resetSearchShortcut', accel)}
                taken={[...PANEL_KEYS, settings.shortcut, settings.panelPinShortcut]}
              />
            </Row>
          </section>
        )}
        {tab === 'history' && (
          <>
            <DiskAccessRow />
            <HistorySection run={run} updateSetting={updateSetting} />
          </>
        )}
      </div>
      <ConfirmModal
        open={confirmReset}
        title={s.panelSizeReset}
        body={s.panelSizeResetBody}
        confirmLabel={s.reset}
        onConfirm={() => {
          run(resetPanelHeight());
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
