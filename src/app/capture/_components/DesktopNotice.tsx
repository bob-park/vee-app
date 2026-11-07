import type { CaptureMode } from '@/domain/capture/apis/capture.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

import cx from 'classnames';

import { floatClass } from './float';

interface DesktopNoticeProps {
  /** A window-mode click just hit no window; hidden it eases out. */
  shown: boolean;
  mode: CaptureMode;
  /** The record pill is up, so this sits a row higher instead of covering it. */
  abovePill: boolean;
}

/** Says a window-mode click on the desktop picked nothing. The capture toast window sits under the overlay, so this is drawn inside it. */
export default function DesktopNotice({ shown, mode, abovePill }: DesktopNoticeProps) {
  // hooks
  const { t } = useI18n();

  const text = mode === 'record' ? t.capture.noWindowRecord : t.capture.noWindowCapture;
  return (
    <div
      className={cx(
        'glass shadow-glass text-base-content pointer-events-none absolute left-1/2 -translate-x-1/2 rounded-full px-3 py-1.5 text-[12px] font-medium whitespace-nowrap',
        abovePill ? 'bottom-[136px]' : 'bottom-[88px]',
        floatClass(shown),
      )}
    >
      {/* Stays while it fades out; the live region below says it instead. */}
      <span aria-hidden>{text}</span>
      {/* Filled only while shown, so each click is announced, not the hidden text. */}
      <span className="sr-only" role="status" aria-live="polite">
        {shown && text}
      </span>
    </div>
  );
}
