import type { ReactNode } from 'react';

import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import type { Size } from '@/utils/edit';

import cx from 'classnames';

interface StageImageProps {
  /** Displayed size of the image, in CSS px. */
  view: Size;
  src: string;
  /** Subject's cut, drawn over the dimmed image. */
  overlaySrc: string | null;
  busy: boolean;
  /** Dims the image under Subject's cut. */
  dimmed: boolean;
  /** A checkerboard behind the image, so erased parts read as transparent. */
  checker: boolean;
  spinner: boolean;
  /** The tool layers (crop box, subject marks, brush). */
  children: ReactNode;
}

/** The working image with the current tool's layer on top. */
export default function StageImage({
  view,
  src,
  overlaySrc,
  busy,
  dimmed,
  checker,
  spinner,
  children,
}: StageImageProps) {
  // hooks
  const { t } = useI18n();

  return (
    <div className="relative" style={{ width: view.w, height: view.h }}>
      {/* Brush paints on the latest render; over a checkerboard so erased parts read as transparent. */}
      <img
        src={src}
        alt=""
        draggable={false}
        className={cx('size-full rounded-md transition-opacity duration-200 select-none', {
          'opacity-60': busy,
          'opacity-35': dimmed,
          'bg-checker': checker,
        })}
      />
      {/* Subject: the cut on top of the dimmed original, so what's left out still shows faintly. */}
      {overlaySrc && (
        <img src={overlaySrc} alt="" draggable={false} className="absolute inset-0 size-full rounded-md select-none" />
      )}
      {spinner && (
        <div className="absolute inset-0 flex items-center justify-center" role="status" aria-label={t.editor.working}>
          <span className="glass rounded-full p-2">
            <svg
              className="text-primary size-5 animate-spin motion-reduce:animate-none"
              viewBox="0 0 24 24"
              fill="none"
              aria-hidden
            >
              <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.25" strokeWidth="3" />
              <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
            </svg>
          </span>
        </div>
      )}
      {children}
    </div>
  );
}
