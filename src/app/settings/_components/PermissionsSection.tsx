import { useState } from 'react';

import Button from '@/app/settings/_components/Button';
import Row from '@/app/settings/_components/Row';
import SettingSection from '@/app/settings/_components/SettingSection';
import { BADGE } from '@/app/settings/_components/badge';
import { openAutomationSettings, requestFinderAccess } from '@/domain/convert/apis/convert';
import { useFinderAccess } from '@/domain/convert/queries/convert';
import {
  openDiskAccessSettings,
  openInputAccessSettings,
  openScreenAccessSettings,
  requestInputAccess,
  requestScreenAccess,
} from '@/domain/settings/apis/settings';
import type { Settings } from '@/domain/settings/apis/settings.dto';
import { useDiskAccess, useInputAccess, useScreenAccess } from '@/domain/settings/queries/settings';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

interface Props {
  settings: Settings;
  /** Runs a command, showing its error under the settings. */
  run: (p: Promise<unknown>) => void;
}

/**
 * macOS permissions; all are null on Windows, where the tab is hidden.
 * Re-checked on focus so they update after the user returns from System Settings.
 */
export default function PermissionsSection({ settings, run }: Props) {
  // state
  // macOS shows its prompt only once, so later presses open System Settings.
  const [screenRequested, setScreenRequested] = useState(false);
  const [inputRequested, setInputRequested] = useState(false);

  // hooks
  const s = useI18n().t.settings;

  // queries
  const { granted } = useDiskAccess();
  const { access } = useFinderAccess();
  const { granted: screenGranted } = useScreenAccess();
  const { granted: inputGranted } = useInputAccess();

  // handle
  const handleScreenAllow = () => {
    run(screenRequested ? openScreenAccessSettings() : requestScreenAccess());
    setScreenRequested(true);
  };

  // On without access, or the button was used: a grant may need a relaunch to show.
  const inputAsked = inputRequested || settings.autoNextPaste === 'on';

  // The shortcut may have used macOS's one-time prompt already, and nothing tells us. The request is
  // a no-op then (and lists Vee in System Settings otherwise), so always open System Settings too.
  const handleInputAllow = () => {
    run(requestInputAccess().then(() => openInputAccessSettings()));
    setInputRequested(true);
  };

  const c = s.convert;
  if (granted === null && screenGranted === null && inputGranted === null && !access) return null;
  return (
    <SettingSection>
      {granted !== null && (
        <Row label={s.diskAccess} hint={s.diskAccessHint}>
          {granted ? (
            <span className={BADGE}>{s.diskAccessGranted}</span>
          ) : (
            <Button onClick={() => void openDiskAccessSettings()}>{s.diskAccessGrant}</Button>
          )}
        </Row>
      )}
      {screenGranted !== null && (
        <Row
          label={s.screenAccess}
          hint={screenRequested && !screenGranted ? s.screenAccessRelaunchHint : s.screenAccessHint}
        >
          {screenGranted ? (
            <span className={BADGE}>{c.allowed}</span>
          ) : (
            <Button onClick={handleScreenAllow}>{screenRequested ? c.openSystemSettings : c.allow}</Button>
          )}
        </Row>
      )}
      {inputGranted !== null && (
        <Row
          label={s.inputAccess}
          hint={inputAsked && !inputGranted ? s.inputAccessRelaunchHint : s.inputAccessHint}
          dim={settings.autoNextPaste !== 'on'}
        >
          {inputGranted ? (
            <span className={BADGE}>{c.allowed}</span>
          ) : (
            <Button onClick={handleInputAllow}>{c.openSystemSettings}</Button>
          )}
        </Row>
      )}
      {access && (
        <Row label={c.finderAccess} hint={c.finderAccessHint} dim={settings.dragConvert !== 'on'}>
          {access === 'granted' ? (
            <span className={BADGE}>{c.allowed}</span>
          ) : access === 'denied' ? (
            <Button onClick={() => run(openAutomationSettings())}>{c.openSystemSettings}</Button>
          ) : (
            <Button onClick={() => run(requestFinderAccess())}>{c.allow}</Button>
          )}
        </Row>
      )}
    </SettingSection>
  );
}
