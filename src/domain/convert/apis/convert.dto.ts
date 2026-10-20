/** Target formats; mirrors Rust `Format` (serde names). */
export type Format =
  'png' | 'jpg' | 'webp' | 'heic' | 'tiff' | 'gif' | 'bmp' | 'pdf' | 'mp4' | 'webm' | 'mov' | 'mp3' | '720p';

/** One donut slice; mirrors Rust `Slice`. `needs` names the engine still missing for it. */
export interface Slice {
  format: Format;
  needs: 'ffmpeg' | null;
}

/** Why a conversion failed; mirrors Rust `JobError`. */
export type ConvertError = 'missing' | 'engine' | 'write';

/** What a toast says about converting; mirrors Rust `ConvertNote`. */
export type ConvertNote =
  | { type: 'saved'; format: Format; count: number; folder: string }
  | { type: 'failed'; name: string; error: ConvertError }
  | { type: 'notFolder'; finderBlocked: boolean }
  | { type: 'unsupported' };

/** One conversion in the panel's job list; mirrors Rust `JobView`. */
export interface ConvertJob {
  id: number;
  clipId: number;
  format: Format;
  state: 'queued' | 'running' | 'done' | 'failed';
  position: number;
  done: number;
  total: number;
  folder: string;
  error: ConvertError | null;
}

/** Apple Events permission for Finder; mirrors `get_finder_access`. */
export type FinderAccess = 'granted' | 'denied' | 'unknown';
