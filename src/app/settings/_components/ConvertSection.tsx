import { useState } from 'react';

import Button from '@/app/settings/_components/Button';
import { ConfirmModal } from '@/app/settings/_components/ConfirmModal';
import Row from '@/app/settings/_components/Row';
import Segmented from '@/app/settings/_components/Segmented';
import SettingSection from '@/app/settings/_components/SettingSection';
import Switch from '@/app/settings/_components/Switch';
import {
  cancelFfmpegDownload,
  downloadFfmpeg,
  openAutomationSettings,
  removeFfmpeg,
  requestFinderAccess,
} from '@/domain/convert/apis/convert';
import { useFinderAccess } from '@/domain/convert/queries/convert';
import { useFfmpegStatus } from '@/domain/convert/queries/engines';
import type { SettingKey, Settings } from '@/domain/settings/apis/settings.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

const isMac = navigator.userAgent.includes('Mac');
const KEYS: Settings['convertKey'][] = isMac ? ['alt', 'ctrl', 'shift'] : ['ctrl', 'alt', 'shift'];
const QUALITIES: Settings['videoQuality'][] = ['small', 'normal', 'original'];

const MB = 1024 * 1024;

const BADGE = 'bg-ok-bg text-ok-fg rounded-md px-2 py-0.5 text-[11px] font-semibold';

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
    <>
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
              <span className={BADGE}>{c.allowed}</span>
            ) : access === 'denied' ? (
              <Button onClick={() => run(openAutomationSettings())}>{c.openSystemSettings}</Button>
            ) : (
              <Button onClick={() => run(requestFinderAccess())}>{c.allow}</Button>
            )}
          </Row>
        )}
      </SettingSection>
      <FfmpegSection settings={settings} run={run} updateSetting={updateSetting} />
    </>
  );
}

/** Title and hint left, actions right; the top line of every FFmpeg state. */
function Head({ title, hint, children }: { title: React.ReactNode; hint?: string; children?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div>
        {title}
        {hint && <small className="text-muted mt-0.5 block text-[11px]">{hint}</small>}
      </div>
      {children && <div className="flex gap-1.5">{children}</div>}
    </div>
  );
}

function FfmpegSection({ settings, run, updateSetting }: Props) {
  // state
  const [confirmRemove, setConfirmRemove] = useState(false);

  // hooks
  const { t, locale } = useI18n();

  // queries
  const { ffmpeg } = useFfmpegStatus();

  if (!ffmpeg) return null;
  const f = t.settings.convert.ffmpeg;
  const error = ffmpeg.state !== 'downloading' && ffmpeg.error && (
    <p className="text-error text-[11px]">{f.errors[ffmpeg.error]}</p>
  );

  return (
    <>
      <div className="text-muted mx-0.5 mb-1.5 text-[11px] font-semibold">{f.caption}</div>
      <SettingSection>
        {ffmpeg.state === 'missing' && (
          <Row col>
            <Head title={f.title} hint={f.missingHint(f.mb(Math.round(ffmpeg.downloadSize / MB)))}>
              <Button onClick={() => run(downloadFfmpeg())}>{f.download}</Button>
            </Head>
            {error}
          </Row>
        )}
        {ffmpeg.state === 'downloading' && (
          <Row col>
            <Head
              title={f.title}
              hint={
                ffmpeg.stage === 'verify'
                  ? f.verifying
                  : f.progress(
                      ffmpeg.total > 0 ? Math.min(100, Math.floor((ffmpeg.received / ffmpeg.total) * 100)) : 0,
                      (ffmpeg.received / MB).toFixed(1),
                      (ffmpeg.total / MB).toFixed(1),
                      (ffmpeg.speed / MB).toFixed(1),
                    )
              }
            >
              <Button variant="gray" onClick={() => run(cancelFfmpegDownload())}>
                {t.cancel}
              </Button>
            </Head>
            <div className="bg-subtle h-1.5 overflow-hidden rounded-full">
              <i
                className="bg-primary block h-full rounded-full"
                style={{ width: `${ffmpeg.total > 0 ? Math.min(100, (ffmpeg.received / ffmpeg.total) * 100) : 0}%` }}
              />
            </div>
          </Row>
        )}
        {ffmpeg.state === 'installed' && (
          <>
            <Row col>
              <Head
                title={
                  <span className="flex items-center gap-1.5">
                    {f.title}
                    <span className={BADGE}>{f.installed}</span>
                  </span>
                }
              >
                {ffmpeg.update && (
                  <Button variant="gray" onClick={() => run(downloadFfmpeg())}>
                    {f.update}
                  </Button>
                )}
                <Button variant="danger" onClick={() => setConfirmRemove(true)}>
                  {f.remove}
                </Button>
              </Head>
              <dl className="text-muted [&_dd]:text-base-content grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[11px] [&_dd]:font-medium">
                <dt>{f.meta.version}</dt>
                <dd>{ffmpeg.version}</dd>
                <dt>{f.meta.size}</dt>
                <dd>{f.mb(Math.round(ffmpeg.size / MB))}</dd>
                <dt>{f.meta.installedAt}</dt>
                <dd>{new Date(ffmpeg.installedAt).toLocaleDateString(locale)}</dd>
              </dl>
              {error}
            </Row>
            <Row label={f.quality} hint={f.qualityHint}>
              <Segmented
                value={settings.videoQuality}
                onChange={(v) => updateSetting({ key: 'videoQuality', value: v })}
                options={QUALITIES.map((q) => ({ value: q, label: f.qualities[q] }))}
              />
            </Row>
          </>
        )}
      </SettingSection>
      <ConfirmModal
        open={confirmRemove}
        title={f.title}
        body={f.removeConfirm}
        confirmLabel={f.remove}
        onConfirm={() => {
          run(removeFfmpeg());
          setConfirmRemove(false);
        }}
        onCancel={() => setConfirmRemove(false)}
      />
    </>
  );
}
