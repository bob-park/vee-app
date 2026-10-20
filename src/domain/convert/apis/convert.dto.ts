/** Target formats, in the donut's slice order. */
export type Format = 'png' | 'jpg' | 'webp' | 'heic' | 'tiff' | 'gif' | 'bmp' | 'pdf';

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
