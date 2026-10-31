import type { ReactNode } from 'react';

import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

const SCALES = ['2', '4'] as const;
const ZOOMS = ['fit', 'actual'] as const;

export type UpscaleScale = (typeof SCALES)[number];
export type UpscaleZoom = (typeof ZOOMS)[number];

interface UpscaleControlsProps {
  scale: UpscaleScale;
  onScale: (scale: UpscaleScale) => void;
  zoom: UpscaleZoom;
  onZoom: (zoom: UpscaleZoom) => void;
  /** What "100%" is in percent of the full-res image. */
  actualPct: number;
  /** Apply (or Retry). */
  aiButton: ReactNode;
}

/** The Upscale tab's bottom row: scale, zoom and Apply. */
export default function UpscaleControls({ scale, onScale, zoom, onZoom, actualPct, aiButton }: UpscaleControlsProps) {
  // hooks
  const { t } = useI18n();

  return (
    <div className="flex items-center gap-2.5">
      <div className="glass rounded-xl p-0.5">
        <Segmented
          value={scale}
          onChange={onScale}
          options={SCALES.map((value) => ({ value, label: t.editor.scales[value] }))}
        />
      </div>
      <div className="glass rounded-xl p-0.5">
        <Segmented
          value={zoom}
          onChange={onZoom}
          options={ZOOMS.map((value) => ({
            value,
            label: value === 'fit' ? t.editor.zoom.fit : t.editor.zoom.actual(actualPct),
          }))}
        />
      </div>
      {aiButton}
    </div>
  );
}
