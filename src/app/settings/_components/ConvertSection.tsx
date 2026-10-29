import { useState } from 'react';

import Button from '@/app/settings/_components/Button';
import { ConfirmModal } from '@/app/settings/_components/ConfirmModal';
import Row from '@/app/settings/_components/Row';
import SettingSection from '@/app/settings/_components/SettingSection';
import Switch from '@/app/settings/_components/Switch';
import { BADGE, BADGE_OFF } from '@/app/settings/_components/badge';
import {
  cancelEngineDownload,
  downloadEngine,
  openLibreOfficeDownload,
  removeEngine,
} from '@/domain/convert/apis/convert';
import type { EngineId } from '@/domain/convert/apis/convert.dto';
import {
  useEngineStatus,
  useEngineUpdateCheck,
  useLibreOffice,
  useVideoEncoder,
} from '@/domain/convert/queries/engines';
import type { SettingKey, Settings } from '@/domain/settings/apis/settings.dto';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

const isMac = navigator.userAgent.includes('Mac');
const KEYS: Settings['convertKey'][] = isMac ? ['alt', 'ctrl', 'shift'] : ['ctrl', 'alt', 'shift'];
const QUALITIES: Settings['videoQuality'][] = ['small', 'normal', 'original'];

const MB = 1024 * 1024;

interface Props {
  settings: Settings;
  /** Runs a command, showing its error under the settings. */
  run: (p: Promise<unknown>) => void;
  updateSetting: (v: { key: SettingKey; value: string }) => void;
}

export default function ConvertSection({ settings, run, updateSetting }: Props) {
  // hooks
  const s = useI18n().t.settings;
  useEngineUpdateCheck();

  // queries
  const { videoEncoder } = useVideoEncoder();

  const c = s.convert;
  const off = settings.dragConvert !== 'on';
  const encoders = c.ffmpeg.encoders;
  const encoderLabel =
    videoEncoder &&
    (videoEncoder.fellBack && videoEncoder.encoder
      ? c.ffmpeg.encoderFellBack(encoders[videoEncoder.encoder])
      : encoders[videoEncoder.encoder ?? 'cpu']);
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
      </SettingSection>
      <EngineSection engine="ffmpeg" index={1} run={run}>
        <Row label={c.ffmpeg.quality} hint={c.ffmpeg.qualityHint}>
          <Segmented
            value={settings.videoQuality}
            onChange={(v) => updateSetting({ key: 'videoQuality', value: v })}
            options={QUALITIES.map((q) => ({ value: q, label: c.ffmpeg.qualities[q] }))}
          />
        </Row>
        <Row label={c.ffmpeg.hwAccel} hint={c.ffmpeg.hwAccelHint}>
          <div className="flex items-center gap-2">
            {encoderLabel && <span className={BADGE}>{encoderLabel}</span>}
            <Switch
              ariaLabel={c.ffmpeg.hwAccel}
              checked={settings.videoHwAccel === 'on'}
              onChange={(checked) => updateSetting({ key: 'videoHwAccel', value: checked ? 'on' : 'off' })}
            />
          </div>
        </Row>
      </EngineSection>
      <EngineSection engine="rhwp" index={2} run={run} />
      <EngineSection engine="bgremove" index={3} run={run} />
      <LibreOfficeSection index={4} run={run} />
    </>
  );
}

/** Title and hint left, actions right; the top line of every engine state. */
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

function Caption({ children }: { children: React.ReactNode }) {
  return <div className="text-muted mx-0.5 mb-1.5 flex items-center gap-1.5 text-[11px] font-semibold">{children}</div>;
}

interface EngineSectionProps {
  engine: EngineId;
  run: Props['run'];
  /** Order on the tab, for the entrance stagger. */
  index: number;
  /** Extra rows shown only while the engine is installed. */
  children?: React.ReactNode;
}

/** A downloadable engine: download, progress, installed details, remove. */
function EngineSection({ engine, index, run, children }: EngineSectionProps) {
  // state
  const [confirmRemove, setConfirmRemove] = useState(false);

  // hooks
  const { t, locale } = useI18n();

  // queries
  const { status } = useEngineStatus(engine);

  if (!status) return null;
  const e = t.settings.convert.engine;
  const f = t.settings.convert[engine];
  const error = status.state !== 'downloading' && status.error && (
    <p className="text-error text-[11px]">{e.errors[status.error]}</p>
  );

  return (
    <>
      <Caption>{f.caption}</Caption>
      <SettingSection index={index}>
        {status.state === 'missing' && (
          <Row col>
            <Head
              title={f.title}
              hint={f.missingHint(status.downloadSize === null ? null : Math.round(status.downloadSize / MB))}
            >
              <Button onClick={() => run(downloadEngine(engine))}>{e.download}</Button>
            </Head>
            {error}
          </Row>
        )}
        {status.state === 'downloading' && (
          <Row col>
            <Head
              title={f.title}
              hint={
                status.stage === 'verify'
                  ? e.verifying
                  : status.total === 0
                    ? // No release known yet: the download looks it up first.
                      e.resolving
                    : status.stage === 'unpack'
                      ? e.unpacking
                      : e.progress(
                          status.total > 0 ? Math.min(100, Math.floor((status.received / status.total) * 100)) : 0,
                          (status.received / MB).toFixed(1),
                          (status.total / MB).toFixed(1),
                          (status.speed / MB).toFixed(1),
                        )
              }
            >
              <Button variant="gray" onClick={() => run(cancelEngineDownload(engine))}>
                {t.cancel}
              </Button>
            </Head>
            <div className="bg-subtle h-1.5 overflow-hidden rounded-full">
              <i
                className="bg-primary block h-full rounded-full"
                style={{ width: `${status.total > 0 ? Math.min(100, (status.received / status.total) * 100) : 0}%` }}
              />
            </div>
          </Row>
        )}
        {status.state === 'installed' && (
          <>
            <Row col>
              <Head
                title={
                  <span className="flex items-center gap-1.5">
                    {f.title}
                    <span className={BADGE}>{e.installed}</span>
                  </span>
                }
              >
                {status.update && (
                  <Button variant="gray" onClick={() => run(downloadEngine(engine))}>
                    {e.update}
                  </Button>
                )}
                <Button variant="danger" onClick={() => setConfirmRemove(true)}>
                  {e.remove}
                </Button>
              </Head>
              <dl className="text-muted [&_dd]:text-base-content grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[11px] [&_dd]:font-medium">
                <dt>{e.meta.version}</dt>
                <dd>{status.version}</dd>
                <dt>{e.meta.size}</dt>
                <dd>{e.mb(Math.round(status.size / MB))}</dd>
                <dt>{e.meta.installedAt}</dt>
                <dd>{new Date(status.installedAt).toLocaleDateString(locale)}</dd>
              </dl>
              {error}
            </Row>
            {children}
          </>
        )}
      </SettingSection>
      <ConfirmModal
        open={confirmRemove}
        title={f.title}
        body={f.removeConfirm}
        confirmLabel={e.remove}
        onConfirm={() => {
          run(removeEngine(engine));
          setConfirmRemove(false);
        }}
        onCancel={() => setConfirmRemove(false)}
      />
    </>
  );
}

/** The user's LibreOffice: found (with its version, when it says) or a link to its download page. */
function LibreOfficeSection({ index, run }: { index: number; run: Props['run'] }) {
  // hooks
  const l = useI18n().t.settings.convert.libreoffice;

  // queries
  const { libreOffice, isLoading } = useLibreOffice();

  const version = libreOffice?.version;

  return (
    <>
      <Caption>
        {l.caption}
        <span className="bg-accent-subtle text-primary rounded-md px-1.5 py-px text-[10px] font-semibold">
          {l.beta}
        </span>
      </Caption>
      <SettingSection index={index}>
        <Row
          label={l.title}
          hint={isLoading ? undefined : !libreOffice ? l.hint : version ? l.version(version) : undefined}
        >
          {isLoading ? null : libreOffice ? (
            <span className={BADGE}>{l.found}</span>
          ) : (
            // Kept whole: the long hint on the left would otherwise squeeze this to one character per line.
            <div className="flex shrink-0 items-center gap-2">
              <span className={BADGE_OFF}>{l.missing}</span>
              <Button onClick={() => run(openLibreOfficeDownload())}>{l.install}</Button>
            </div>
          )}
        </Row>
      </SettingSection>
    </>
  );
}
