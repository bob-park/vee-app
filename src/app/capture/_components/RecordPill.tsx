import { RECORD_FORMATS, type RecordFormat } from '@/domain/settings/apis/settings.dto';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { needsFfmpeg } from '@/utils/recording';

import cx from 'classnames';

import { floatClass } from './float';

interface RecordPillProps {
  /** A record selection waits here and the toolbar is shown; hidden it eases out and ignores input. */
  shown: boolean;
  format: RecordFormat;
  /** FFmpeg is installed: the formats that need it can be picked. */
  ffmpeg: boolean;
  onFormat: (format: RecordFormat) => void;
  onStart: () => void;
}

const isMac = navigator.userAgent.includes('Mac');

/** The format and Start button for a picked record selection, one row above the toolbar. Cancel is the toolbar's ✕. */
export default function RecordPill({ shown, format, ffmpeg, onFormat, onStart }: RecordPillProps) {
  // hooks
  const { t } = useI18n();

  const r = t.record;
  return (
    // Its own clicks must not start a new drag on the overlay.
    <div
      className={cx(
        'glass shadow-glass absolute bottom-[88px] left-1/2 flex -translate-x-1/2 cursor-default items-center gap-1.5 rounded-full p-1 whitespace-nowrap',
        floatClass(shown),
      )}
      inert={!shown}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <Segmented
        ariaLabel={r.format}
        value={format}
        onChange={onFormat}
        options={RECORD_FORMATS.map((f) => {
          const locked = !ffmpeg && needsFfmpeg(f, isMac);
          return { value: f, label: f.toUpperCase(), disabled: locked, title: locked ? r.ffmpegNeeded : undefined };
        })}
      />
      <button
        className="bg-rec flex items-center gap-1.5 rounded-full px-3 py-1.5 text-[12px] font-semibold text-white transition-transform duration-150 active:scale-96 motion-reduce:transition-none"
        onClick={onStart}
      >
        <span className="size-2 rounded-full bg-white" aria-hidden />
        {r.start}
      </button>
    </div>
  );
}
