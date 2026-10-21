import Button from '@/app/settings/_components/Button';
import Row from '@/app/settings/_components/Row';
import SettingSection from '@/app/settings/_components/SettingSection';
import { BADGE } from '@/app/settings/_components/badge';
import { openAutomationSettings, requestFinderAccess } from '@/domain/convert/apis/convert';
import { useFinderAccess } from '@/domain/convert/queries/convert';
import { openDiskAccessSettings } from '@/domain/settings/apis/settings';
import type { Settings } from '@/domain/settings/apis/settings.dto';
import { useDiskAccess } from '@/domain/settings/queries/settings';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

interface Props {
  settings: Settings;
  /** Runs a command, showing its error under the settings. */
  run: (p: Promise<unknown>) => void;
}

/**
 * macOS permissions; both are null on Windows, where the tab is hidden.
 * Re-checked on focus so they update after the user returns from System Settings.
 */
export default function PermissionsSection({ settings, run }: Props) {
  // hooks
  const s = useI18n().t.settings;

  // queries
  const { granted } = useDiskAccess();
  const { access } = useFinderAccess();

  const c = s.convert;
  if (granted === null && !access) return null;
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
