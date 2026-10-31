import type { ReactNode } from 'react';

import { BRUSH } from '@/app/editor/_hooks/useBgTools';
import type { BrushMode } from '@/domain/edit/apis/edit.dto';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

import type { BgTool } from './BgToolRail';
import ModelPrompt from './ModelPrompt';

const BRUSH_MODES: BrushMode[] = ['erase', 'restore'];

/** The neutral button beside Apply, like the footer's Cancel. */
const RESET_BUTTON =
  'rounded-box bg-subtle text-base-content px-3.5 py-1.5 text-[12px] font-semibold disabled:cursor-default disabled:opacity-50';

interface BgControlsProps {
  tool: BgTool;
  /** The model a tool needs is known to be missing; its tool shows the download prompt instead. */
  bgMissing: boolean;
  samMissing: boolean;
  isMac: boolean;
  /** Disables the Subject tool's Reset. */
  resetDisabled: boolean;
  onReset: () => void;
  brushMode: BrushMode;
  onBrushMode: (mode: BrushMode) => void;
  /** Brush radius in CSS px. */
  brushSize: number;
  onBrushSize: (size: number) => void;
  /** Auto's Apply (or Retry). */
  aiButton: ReactNode;
  /** Subject's and Brush's Apply. */
  liveApply: ReactNode;
}

/** The Remove background tab's bottom row for the current tool. */
export default function BgControls({
  tool,
  bgMissing,
  samMissing,
  isMac,
  resetDisabled,
  onReset,
  brushMode,
  onBrushMode,
  brushSize,
  onBrushSize,
  aiButton,
  liveApply,
}: BgControlsProps) {
  // hooks
  const { t } = useI18n();

  return (
    <>
      {tool === 'auto' &&
        (bgMissing ? (
          <ModelPrompt />
        ) : (
          <div className="flex items-center gap-3">
            <span className="text-muted text-[12px]">{t.editor.removeBgHint}</span>
            {aiButton}
          </div>
        ))}
      {tool === 'subject' &&
        (samMissing ? (
          <ModelPrompt />
        ) : (
          <div className="flex items-center gap-3">
            <span className="text-muted text-[12px]">{t.editor.subjectHint(isMac)}</span>
            <button className={RESET_BUTTON} disabled={resetDisabled} onClick={onReset}>
              {t.editor.subjectReset}
            </button>
            {liveApply}
          </div>
        ))}
      {tool === 'brush' && (
        <div className="flex items-center gap-3">
          <div className="glass rounded-xl p-0.5">
            <Segmented
              value={brushMode}
              onChange={onBrushMode}
              options={BRUSH_MODES.map((value) => ({ value, label: t.editor.brushModes[value] }))}
            />
          </div>
          <label className="text-muted flex items-center gap-2 text-[12px]">
            {t.editor.brushSize}
            <input
              type="range"
              className="accent-primary w-24"
              min={BRUSH.min}
              max={BRUSH.max}
              value={brushSize}
              aria-label={t.editor.brushSize}
              onChange={(e) => onBrushSize(Number(e.target.value))}
            />
            <span className="w-5 tabular-nums">{brushSize}</span>
          </label>
          {liveApply}
        </div>
      )}
    </>
  );
}
