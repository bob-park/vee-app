import Button from '@/app/settings/_components/Button';
import Row from '@/app/settings/_components/Row';
import Segmented from '@/app/settings/_components/Segmented';
import SettingSection from '@/app/settings/_components/SettingSection';
import Switch from '@/app/settings/_components/Switch';
import { openAutomationSettings, requestFinderAccess } from '@/domain/convert/apis/convert';
import { useFinderAccess } from '@/domain/convert/queries/convert';
import type { SettingKey, Settings } from '@/domain/settings/apis/settings.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

const isMac = navigator.userAgent.includes('Mac');
const KEYS: Settings['convertKey'][] = isMac ? ['alt', 'ctrl', 'shift'] : ['ctrl', 'alt', 'shift'];

interface Props {
  settings: Settings;
  /** Runs a command, showing its error under the settings. */
  run: (p: Promise<unknown>) => void;
  updateSetting: (v: { key: SettingKey; value: string }) => void;
}

export default function ConvertSection({ settings, run, updateSetting }: Props) {
  // hooks
  const s = useI18n().t.settings;

  // queries
  const { access } = useFinderAccess();

  const c = s.convert;
  const off = settings.dragConvert !== 'on';
  const labels = isMac ? c.keys.mac : c.keys.win;

  return (
    <SettingSection>
      <Row label={c.dragConvert} hint={c.dragConvertHint}>
        <Switch
          ariaLabel={c.dragConvert}
          checked={!off}
          onChange={(checked) => updateSetting({ key: 'dragConvert', value: checked ? 'on' : 'off' })}
        />
      </Row>
      <Row label={c.key} hint={c.keyHint} dim={off}>
        <Segmented
          value={settings.convertKey}
          onChange={(v) => updateSetting({ key: 'convertKey', value: v })}
          options={KEYS.map((k) => ({ value: k, label: labels[k] }))}
        />
      </Row>
      {/* macOS only; null on Windows. */}
      {access && (
        <Row label={c.finderAccess} hint={c.finderAccessHint} dim={off}>
          {access === 'granted' ? (
            <span className="bg-ok-bg text-ok-fg rounded-md px-2 py-0.5 text-[11px] font-semibold">{c.allowed}</span>
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
