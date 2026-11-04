import type { CaptureMode, RecordBlock } from '@/domain/capture/apis/capture.dto';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

interface CaptureToolbarProps {
  mode: CaptureMode;
  windowMode: boolean;
  recordBlocked: RecordBlock | null;
  onMode: (mode: CaptureMode) => void;
  onWindowMode: (on: boolean) => void;
  onCancel: () => void;
}

/** A 14px stroked icon, drawn in the current text color. */
function Icon({ d, children }: { d?: string; children?: React.ReactNode }) {
  return (
    <svg
      className="size-3.5 shrink-0 fill-none stroke-current stroke-2"
      viewBox="0 0 24 24"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {d && <path d={d} />}
      {children}
    </svg>
  );
}

/** An icon (or dot) followed by its label, for a Segmented option. */
function Option({ icon, label }: { icon: React.ReactNode; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      {icon}
      {label}
    </span>
  );
}

/** The overlay's bottom toolbar: capture/record, area/window, and ✕. Shown on the monitor under the mouse. */
export default function CaptureToolbar({
  mode,
  windowMode,
  recordBlocked,
  onMode,
  onWindowMode,
  onCancel,
}: CaptureToolbarProps) {
  // hooks
  const { t } = useI18n();

  const c = t.capture;
  const blockedTitle = {
    recording: c.recordBusy,
    unsupported: t.record.unsupported,
    needsFfmpeg: t.record.ffmpegNeeded,
  };
  return (
    // Its own clicks must not start a drag or pick a window on the overlay.
    <div
      className="glass shadow-glass absolute bottom-10 left-1/2 flex -translate-x-1/2 cursor-default items-center rounded-full p-1 whitespace-nowrap"
      onPointerDown={(e) => e.stopPropagation()}
    >
      <Segmented
        ariaLabel={c.modeLabel}
        value={mode}
        onChange={onMode}
        options={[
          {
            value: 'capture',
            label: (
              <Option
                icon={
                  <Icon d="M4 8h3l2-3h6l2 3h3v11H4z">
                    <circle cx="12" cy="13" r="3.5" />
                  </Icon>
                }
                label={c.capture}
              />
            ),
            activeClassName: 'text-primary',
          },
          {
            value: 'record',
            label: <Option icon={<span className="size-2 shrink-0 rounded-full bg-current" />} label={c.record} />,
            activeClassName: 'text-rec',
            disabled: recordBlocked !== null,
            title: recordBlocked ? blockedTitle[recordBlocked] : undefined,
          },
        ]}
      />
      <span className="bg-divider mx-1.5 h-5 w-px" aria-hidden />
      <Segmented
        ariaLabel={c.targetLabel}
        value={windowMode ? 'window' : 'area'}
        onChange={(v) => onWindowMode(v === 'window')}
        options={[
          {
            value: 'area',
            label: <Option icon={<Icon d="M4 8V4h4M16 4h4v4M20 16v4h-4M8 20H4v-4" />} label={c.area} />,
          },
          {
            value: 'window',
            label: (
              <Option
                icon={
                  <Icon d="M3 9h18">
                    <rect x="3" y="5" width="18" height="14" rx="3" />
                  </Icon>
                }
                label={c.window}
              />
            ),
          },
        ]}
      />
      <span className="bg-divider mx-1.5 h-5 w-px" aria-hidden />
      <button
        className="text-muted hover:bg-subtle grid size-7 place-items-center rounded-full"
        aria-label={c.cancel}
        title={c.cancel}
        onClick={onCancel}
      >
        <Icon d="M6 6l12 12M18 6L6 18" />
      </button>
    </div>
  );
}
