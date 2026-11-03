import { useEffect, useState } from 'react';

import Button from '@/app/settings/_components/Button';
import { ConfirmModal } from '@/app/settings/_components/ConfirmModal';
import ConvertSection from '@/app/settings/_components/ConvertSection';
import { Dropdown } from '@/app/settings/_components/Dropdown';
import { HistorySection } from '@/app/settings/_components/HistorySection';
import PermissionsSection from '@/app/settings/_components/PermissionsSection';
import PositionPicker from '@/app/settings/_components/PositionPicker';
import Row from '@/app/settings/_components/Row';
import SettingSection from '@/app/settings/_components/SettingSection';
import { ShortcutRecorder } from '@/app/settings/_components/ShortcutRecorder';
import Switch from '@/app/settings/_components/Switch';
import { type SettingsTab, Tabs, isSettingsTab } from '@/app/settings/_components/Tabs';
import ThemePicker from '@/app/settings/_components/ThemePicker';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import { resetPanelSize } from '@/domain/panel/apis/panel';
import {
  openKeyboardSettings,
  previewSound,
  setAutostart,
  setSetting,
  setShortcut,
} from '@/domain/settings/apis/settings';
import {
  type GlobalShortcutKey,
  RECORD_FORMATS,
  type RecordFormat,
  SOUND_NAMES,
  type SoundName,
} from '@/domain/settings/apis/settings.dto';
import { useSettingUpdate, useSettings, useShortcutFailures } from '@/domain/settings/queries/settings';
import { checkUpdate, installUpdate } from '@/domain/updater/apis/updater';
import { useUpdateStatus } from '@/domain/updater/queries/updater';
import Aurora from '@/shared/components/Aurora';
import Segmented from '@/shared/components/Segmented';
import { type Dict, relativeTime } from '@/shared/i18n';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { errorText } from '@/utils/errorText';
import { PANEL_KEYS } from '@/utils/keys';
import { isSide } from '@/utils/panelEdge';
import { needsFfmpeg } from '@/utils/recording';

import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

function UpdateRow({ version }: { version: string }) {
  // state
  /** Restart was pressed; the app re-checks and may download a newer version before restarting. */
  const [installing, setInstalling] = useState(false);
  const [failed, setFailed] = useState(false);

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

  // handle
  const install = () => {
    setFailed(false);
    setInstalling(true);
    // Resolves only on failure: success restarts the app.
    void installUpdate().catch(() => {
      setFailed(true);
      setInstalling(false);
    });
  };

  return (
    <Row label={s.updates} hint={`${s.version(version)} · ${failed ? t.updateFailed : detail}`}>
      {status.status === 'ready' ? (
        <Button disabled={installing} onClick={install}>
          {installing ? t.updateInstalling : s.restartToUpdate}
        </Button>
      ) : (
        <Button disabled={status.status === 'checking'} onClick={() => void checkUpdate()}>
          {s.checkNow}
        </Button>
      )}
    </Row>
  );
}

const isMac = navigator.userAgent.includes('Mac');

/** Every recorder on the shortcuts tab; each refuses the others' combos. */
const SHORTCUT_KEYS = [
  'shortcut',
  'captureShortcut',
  'recordShortcut',
  'panelPinShortcut',
  'resetSearchShortcut',
  'pinnedCopy1Shortcut',
  'pinnedCopy2Shortcut',
  'pinnedCopy3Shortcut',
] as const;
type ShortcutKey = (typeof SHORTCUT_KEYS)[number];

/** OS-wide shortcuts, saved through `set_shortcut`; the rest only act while the panel has focus. */
const isGlobal = (key: ShortcutKey): key is GlobalShortcutKey =>
  key === 'shortcut' || key === 'captureShortcut' || key === 'recordShortcut';

function shortcutLabel(s: Dict['settings'], key: ShortcutKey) {
  switch (key) {
    case 'shortcut':
      return { label: s.shortcut, hint: s.shortcutHint };
    case 'captureShortcut':
      return { label: s.captureShortcut, hint: s.captureShortcutHint };
    case 'recordShortcut':
      return { label: s.recordShortcut, hint: s.recordShortcutHint };
    case 'panelPinShortcut':
      return { label: s.panelPinShortcut, hint: s.panelPinShortcutHint };
    case 'resetSearchShortcut':
      return { label: s.resetSearchShortcut, hint: s.resetSearchShortcutHint };
    default: {
      const n = Number(key.charAt('pinnedCopy'.length));
      return { label: s.pinnedCopyShortcut(n), hint: s.pinnedCopyShortcutHint(n) };
    }
  }
}

export function SettingsApp() {
  // state
  const [error, setError] = useState<string | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [tab, setTab] = useState<SettingsTab>('general');
  /** Globals whose last save failed to register; cleared by a successful save. */
  const [saveFailed, setSaveFailed] = useState<GlobalShortcutKey[]>([]);

  // hooks
  const { t } = useI18n();

  // queries
  const { settings } = useSettings();
  const { updateSetting } = useSettingUpdate({
    onSuccess: () => setError(null),
    onError: (e) => setError(errorText(e)),
  });
  const { failures } = useShortcutFailures();
  const { status: ffmpegStatus } = useEngineStatus('ffmpeg');

  // useEffect
  // The window is only hidden on close, so state survives; every open starts on the tab it names.
  useEffect(() => {
    const unlisten = listen<unknown>('settings://opened', (e) =>
      changeTab(isSettingsTab(e.payload) ? e.payload : 'general'),
    );
    return () => void unlisten.then((off) => off());
  }, []);

  // handle
  const changeTab = (next: SettingsTab) => {
    setTab(next);
    setError(null);
  };
  const run = (p: Promise<unknown>) => void p.then(() => setError(null)).catch((e) => setError(errorText(e)));
  /** A failed global save shows the in-use warning under its row instead of the recorder's own error. */
  const saveGlobal = async (key: GlobalShortcutKey, accel: string) => {
    try {
      await setShortcut(key, accel);
      setSaveFailed((keys) => keys.filter((k) => k !== key));
    } catch {
      setSaveFailed((keys) => (keys.includes(key) ? keys : [...keys, key]));
    }
  };
  const pickRecordFolder = async () => {
    const dir = await open({ directory: true, defaultPath: settings?.recordFolder || undefined });
    if (typeof dir === 'string') updateSetting({ key: 'recordFolder', value: dir });
  };

  const s = t.settings;
  if (!settings) return null;
  const side = isSide(settings.panelPosition);
  const ffmpegMissing = ffmpegStatus !== null && ffmpegStatus.state !== 'installed' && needsFfmpeg('webm', isMac);

  return (
    <main className="relative flex h-full flex-col">
      <Aurora className="inset-x-0 top-0 h-[110px]" />
      {/* macOS overlays the traffic lights on this row (`titleBarStyle: Overlay`); Windows keeps its native bar. */}
      {isMac && (
        <div
          className="relative z-1 flex h-7 shrink-0 items-center justify-center font-semibold select-none"
          data-tauri-drag-region
        >
          {s.title}
        </div>
      )}
      <Tabs value={tab} onChange={changeTab} />
      {/* Extra bottom room so the app picker's popup near the end never runs past the window. */}
      {/* Keyed by tab so its sections remount and spring in again on every switch. */}
      <div
        key={tab}
        className="relative z-1 flex-1 overflow-y-auto px-7 pt-4 pb-32"
        id="settings-panel"
        role="tabpanel"
        aria-labelledby={`tab-${tab}`}
      >
        {tab === 'general' && (
          <SettingSection>
            <Row label={s.theme} labelId="theme-label">
              <ThemePicker
                value={settings.theme}
                onChange={(v) => updateSetting({ key: 'theme', value: v })}
                labelledBy="theme-label"
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
              <Switch
                ariaLabel={s.launchAtLogin}
                checked={settings.autostart}
                onChange={(checked) => run(setAutostart(checked))}
              />
            </Row>
            <Row label={s.recordFolder} hint={settings.recordFolder || s.downloads} truncateHint>
              <Button onClick={() => run(pickRecordFolder())}>{s.change}</Button>
            </Row>
            <Row
              label={s.recordFormat}
              hint={ffmpegMissing ? `${s.recordFormatHint} · ${s.recordFormatFfmpegHint}` : s.recordFormatHint}
            >
              <Segmented
                ariaLabel={s.recordFormat}
                value={settings.recordFormat}
                onChange={(v: RecordFormat) => updateSetting({ key: 'recordFormat', value: v })}
                options={RECORD_FORMATS.map((f) => ({ value: f, label: f.toUpperCase() }))}
              />
            </Row>
            <UpdateRow version={settings.version} />
          </SettingSection>
        )}
        {tab === 'panel' && (
          <SettingSection>
            <Row col label={s.panelPosition} hint={s.panelPositionHint} labelId="position-label">
              <PositionPicker
                value={settings.panelPosition}
                onChange={(v) => updateSetting({ key: 'panelPosition', value: v })}
                labelledBy="position-label"
              />
            </Row>
            <Row label={s.copySound}>
              <Switch
                ariaLabel={s.copySound}
                checked={settings.sound === 'on'}
                onChange={(checked) => updateSetting({ key: 'sound', value: checked ? 'on' : 'off' })}
              />
            </Row>
            <Row label={s.soundName} hint={s.soundNameHint} dim={settings.sound !== 'on'}>
              <div className="flex items-center gap-1.5">
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
                <Button
                  small
                  aria-label={s.preview}
                  title={s.preview}
                  disabled={settings.sound !== 'on' || settings.soundName === 'none'}
                  onClick={() => void previewSound(settings.soundName)}
                >
                  ▶
                </Button>
              </div>
            </Row>
            <Row label={s.confirmDelete} hint={s.confirmDeleteHint}>
              <Switch
                ariaLabel={s.confirmDelete}
                checked={settings.confirmDelete === 'on'}
                onChange={(checked) => updateSetting({ key: 'confirmDelete', value: checked ? 'on' : 'off' })}
              />
            </Row>
            <Row label={s.panelResize} hint={s.panelResizeHints[settings.panelPosition]}>
              <Switch
                ariaLabel={s.panelResize}
                checked={settings.panelResize === 'on'}
                onChange={(checked) => updateSetting({ key: 'panelResize', value: checked ? 'on' : 'off' })}
              />
            </Row>
            {/* Stays enabled while resizing is off: the saved size is kept. */}
            <Row label={s.panelSizeReset}>
              <Button
                disabled={(side ? settings.panelWidthRatio : settings.panelHeightRatio) === ''}
                onClick={() => setConfirmReset(true)}
              >
                {s.reset}
              </Button>
            </Row>
          </SettingSection>
        )}
        {tab === 'shortcuts' && (
          <SettingSection>
            {SHORTCUT_KEYS.map((key) => (
              <Row key={key} label={shortcutLabel(s, key).label} hint={shortcutLabel(s, key).hint}>
                <div className="flex flex-col items-end gap-1.5">
                  <ShortcutRecorder
                    value={settings[key]}
                    onSave={isGlobal(key) ? (accel) => saveGlobal(key, accel) : (accel) => setSetting(key, accel)}
                    taken={[...PANEL_KEYS, ...SHORTCUT_KEYS.filter((k) => k !== key).map((k) => settings[k])]}
                  />
                  {isGlobal(key) && (failures.includes(key) || saveFailed.includes(key)) && (
                    <>
                      <span className="text-error max-w-[260px] text-right text-[12px]" role="alert">
                        {s.shortcutInUse}
                      </span>
                      {isMac && (
                        <Button small onClick={() => run(openKeyboardSettings())}>
                          {s.openKeyboardShortcuts}
                        </Button>
                      )}
                    </>
                  )}
                </div>
              </Row>
            ))}
          </SettingSection>
        )}
        {tab === 'convert' && <ConvertSection settings={settings} run={run} updateSetting={updateSetting} />}
        {tab === 'permissions' && <PermissionsSection settings={settings} run={run} />}
        {tab === 'history' && <HistorySection run={run} updateSetting={updateSetting} />}
      </div>
      <ConfirmModal
        open={confirmReset}
        title={s.panelSizeReset}
        body={s.panelSizeResetBodies[side ? 'width' : 'height']}
        confirmLabel={s.reset}
        onConfirm={() => {
          run(resetPanelSize());
          setConfirmReset(false);
        }}
        onCancel={() => setConfirmReset(false)}
      />
      {/* Same side padding as the tab body, which the error sits below. */}
      {error && (
        <p className="text-error px-7 pt-2 pb-3 text-[12px]" role="alert">
          {error}
        </p>
      )}
    </main>
  );
}
