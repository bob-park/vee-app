import { useState } from 'react';

import Button from '@/app/settings/_components/Button';
import Row from '@/app/settings/_components/Row';
import SettingSection from '@/app/settings/_components/SettingSection';
import { BADGE } from '@/app/settings/_components/badge';
import { openAutomationSettings, requestFinderAccess } from '@/domain/convert/apis/convert';
import { useFinderAccess } from '@/domain/convert/queries/convert';
import { openDiskAccessSettings, openScreenAccessSettings, requestScreenAccess } from '@/domain/settings/apis/settings';
import type { Settings } from '@/domain/settings/apis/settings.dto';
import { useDiskAccess, useScreenAccess } from '@/domain/settings/queries/settings';
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

  // hooks
  const s = useI18n().t.settings;

  // queries
  const { granted } = useDiskAccess();
  const { access } = useFinderAccess();
  const { granted: screenGranted } = useScreenAccess();

  // handle
  const handleScreenAllow = () => {
    run(screenRequested ? openScreenAccessSettings() : requestScreenAccess());
    setScreenRequested(true);
  };

  const c = s.convert;
  if (granted === null && screenGranted === null && !access) return null;
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
