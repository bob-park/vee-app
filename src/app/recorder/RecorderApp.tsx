import { useEffect, useState } from 'react';

import { cancelRecording, getRecorderState, stopRecording } from '@/domain/capture/apis/capture';
import type { RecorderStarted } from '@/domain/capture/apis/capture.dto';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { formatElapsed, gifRemaining } from '@/utils/recording';

import { listen } from '@tauri-apps/api/event';

/** How often the timer redraws; under a second so it never skips one. */
const TICK_MS = 250;

/** The recording pill: elapsed time, GIF time left, stop and cancel. Rust shows and hides the window. */
export default function RecorderApp() {
  // state
  const [started, setStarted] = useState<RecorderStarted | null>(null);
  const [now, setNow] = useState(() => Date.now());

  // hooks
  const r = useI18n().t.record;

  // useEffect
  useEffect(() => {
    // Ask on mount too, in case the window loaded after `recorder://started` was sent.
    void getRecorderState().then((s) => s && setStarted(s));
    const off = listen<RecorderStarted>('recorder://started', (e) => {
      setNow(Date.now());
      setStarted(e.payload);
    });
    return () => void off.then((fn) => fn());
  }, []);

  useEffect(() => {
    if (!started) return;
    const id = setInterval(() => setNow(Date.now()), TICK_MS);
    return () => clearInterval(id);
  }, [started]);

  // handle
  const handleStop = () => {
    setStarted(null);
    void stopRecording();
  };

  const handleCancel = () => {
    setStarted(null);
    void cancelRecording();
  };

  if (!started) return null;
  const elapsed = now - started.startedMs;
  const gif = started.limitMs !== null ? gifRemaining(elapsed, started.limitMs) : null;
  return (
    <div className="flex h-screen items-center justify-center">
      <div className="glass text-base-content shadow-glass flex items-center gap-1.5 rounded-full p-1.5">
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
          className="bg-rec grid size-7.5 place-items-center rounded-full text-white hover:brightness-108"
          aria-label={r.stop}
          title={r.stop}
          onClick={handleStop}
        >
          <svg className="size-4" viewBox="0 0 24 24" aria-hidden>
            <rect x="6" y="6" width="12" height="12" rx="2.5" fill="currentColor" />
          </svg>
        </button>
        <button
          className="hover:bg-subtle grid size-7.5 place-items-center rounded-full"
          aria-label={r.cancel}
          title={r.cancel}
          onClick={handleCancel}
        >
          <svg className="size-4" viewBox="0 0 24 24" aria-hidden>
            <path d="M6 6l12 12M18 6L6 18" stroke="currentColor" strokeWidth={2} strokeLinecap="round" fill="none" />
          </svg>
        </button>
      </div>
    </div>
  );
}
