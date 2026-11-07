import { useEffect, useState } from 'react';

import { cancelRecording, getRecorderState, stopRecording } from '@/domain/capture/apis/capture';
import type { RecorderStarted } from '@/domain/capture/apis/capture.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { formatElapsed, gifRemaining } from '@/utils/recording';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

type Tip = 'stop' | 'cancel';

/** How often the timer redraws; under a second so it never skips one. */
const TICK_MS = 250;

/** The recording pill: elapsed time, GIF time left, stop and cancel. Rust shows and hides the window. */
export default function RecorderApp() {
  // state
  const [started, setStarted] = useState<RecorderStarted | null>(null);
  const [now, setNow] = useState(() => Date.now());
  /** The button under the mouse, for its tooltip. */
  const [tip, setTip] = useState<Tip | null>(null);
  /** Stopped: the pill fades out (its `duration-150`, `CROSSFADE_MS` in recorder/mod.rs), then Rust hides it. */
  const [leaving, setLeaving] = useState(false);

  // hooks
  const r = useI18n().t.record;

  // useEffect
  useEffect(() => {
    // Ask on mount too, in case the window loaded after `recorder://started` was sent.
    void getRecorderState().then((s) => s && setStarted(s));
    const off = listen<RecorderStarted>('recorder://started', (e) => {
      setNow(Date.now());
      setStarted(e.payload);
      setLeaving(false);
      // The last pill's tooltip must not come up on the new one before the mouse moves.
      setTip(null);
    });
    // Rust is hiding the pill (stop, cancel, GIF limit): it fades out on the last time, which then stops.
    // `recorder://started` replaces it before the pill is shown again, so this recording's time never comes back.
    const offStopped = listen('recorder://stopped', () => setLeaving(true));
    // macOS: the pill isn't the key window, so it gets no hover; Rust sends the cursor instead.
    const offPointer = listen<{ x: number; y: number } | null>('recorder://pointer', (e) => {
      const el = e.payload && document.elementFromPoint(e.payload.x, e.payload.y)?.closest<HTMLElement>('[data-tip]');
      setTip((el?.dataset.tip as Tip | undefined) ?? null);
    });
    return () => {
      void off.then((fn) => fn());
      void offStopped.then((fn) => fn());
      void offPointer.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (!started || leaving) return;
    const id = setInterval(() => setNow(Date.now()), TICK_MS);
    return () => clearInterval(id);
  }, [started, leaving]);

  // handle
  const handleStop = () => {
    setLeaving(true);
    void stopRecording();
  };

  const handleCancel = () => {
    setLeaving(true);
    void cancelRecording();
  };

  const tipFor = (key: Tip, label: string) => ({
    'data-tip': key,
    'aria-label': label,
    onPointerEnter: () => setTip(key),
    onPointerLeave: () => setTip(null),
  });

  if (!started) return null;
  const elapsed = now - started.startedMs;
  const gif = started.limitMs !== null ? gifRemaining(elapsed, started.limitMs) : null;
  return (
    <div className="flex h-screen items-center justify-center">
      {/* Solid, not glass: it floats over any desktop and must be read (aurora-glass §3). Each recording remounts
          it, so it fades in again; stopped, it ignores clicks so stop can't go twice. */}
      <div
        key={started.startedMs}
        className={cx(
          'bg-card border-toast-border text-base-content shadow-toast animate-fade-in flex items-center gap-1.5 rounded-full border p-1.5 transition-[opacity,scale] duration-150 ease-in motion-reduce:animate-none motion-reduce:transition-none',
          leaving && 'pointer-events-none scale-95 opacity-0',
        )}
        inert={leaving}
      >
        <span className="bg-rec animate-rec-pulse ml-2 size-2.25 rounded-full" aria-hidden />
        <span className="min-w-11 text-[13px] font-semibold tabular-nums" role="timer">
          {formatElapsed(elapsed)}
        </span>
        {gif && (
          <div className="flex min-w-21 flex-col gap-0.75">
            <small className="text-muted text-[10.5px]">{r.gifLeft(gif.seconds)}</small>
            <div className="bg-subtle h-1 overflow-hidden rounded-xs">
              <div
                className="from-primary to-sky h-full origin-left rounded-xs bg-linear-to-r"
                style={{ transform: `scaleX(${gif.share})` }}
              />
            </div>
          </div>
        )}
        <span className="bg-base-300 mx-0.5 h-4.5 w-px" aria-hidden />
        <button
          className="bg-rec relative grid size-7.5 cursor-pointer place-items-center rounded-full text-white hover:brightness-108"
          {...tipFor('stop', r.stop)}
          onClick={handleStop}
        >
          <Tooltip label={r.stop} shown={tip === 'stop'} />
          <svg className="size-4" viewBox="0 0 24 24" aria-hidden>
            <rect x="6" y="6" width="12" height="12" rx="2.5" fill="currentColor" />
          </svg>
        </button>
        <button
          className="hover:bg-subtle relative grid size-7.5 cursor-pointer place-items-center rounded-full"
          {...tipFor('cancel', r.cancel)}
          onClick={handleCancel}
        >
          <Tooltip label={r.cancel} shown={tip === 'cancel'} />
          <svg className="size-4" viewBox="0 0 24 24" aria-hidden>
            <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" strokeWidth={2} strokeLinecap="round" fill="none" />
          </svg>
        </button>
      </div>
    </div>
  );
}

/** Above the button, in the panel toolbar's tip style; the window leaves room for it above the pill. */
function Tooltip({ label, shown }: { label: string; shown: boolean }) {
  return (
    <span
      className={cx(
        'bg-tip pointer-events-none absolute bottom-[calc(100%+8px)] left-1/2 -translate-x-1/2 rounded-lg px-2 py-1 text-[11px] whitespace-nowrap text-white transition-opacity duration-150 motion-reduce:transition-none',
        shown ? 'opacity-100' : 'opacity-0',
      )}
      aria-hidden
    >
      {label}
    </span>
  );
}
