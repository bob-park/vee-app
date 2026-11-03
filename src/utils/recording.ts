import type { RecordFormat } from '@/domain/settings/apis/settings.dto';

const pad = (n: number) => String(n).padStart(2, '0');

/** A recording's elapsed time: `00:12`, `1:02:03` past an hour. */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const [h, m, s] = [Math.floor(total / 3600), Math.floor(total / 60) % 60, total % 60];
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

/** What is left of a GIF's time limit: whole seconds rounded up, and the share used (0..1). */
export function gifRemaining(elapsedMs: number, limitMs: number): { seconds: number; share: number } {
  const left = Math.max(0, limitMs - elapsedMs);
  return { seconds: Math.ceil(left / 1000), share: Math.min(1, Math.max(0, elapsedMs / limitMs)) };
}

/** Whether `format` needs FFmpeg to record; on Windows recording needs it anyway, so only macOS asks. */
export function needsFfmpeg(format: RecordFormat, mac: boolean): boolean {
  return mac && (format === 'webm' || format === 'gif');
}

/** The format the pill starts on: the saved one, or MP4 when this Mac can't make it without FFmpeg. */
export function initialFormat(saved: RecordFormat, ffmpeg: boolean, mac: boolean): RecordFormat {
  return !ffmpeg && needsFfmpeg(saved, mac) ? 'mp4' : saved;
}
