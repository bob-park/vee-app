/** Target formats, in the donut's slice order. */
export type Format = 'png' | 'jpg' | 'webp' | 'heic' | 'tiff' | 'gif' | 'bmp' | 'pdf';

/** What a toast says about converting; mirrors Rust `ConvertNote`. */
export type ConvertNote =
  | { type: 'saved'; format: Format; count: number; folder: string }
  | { type: 'failed'; name: string }
  | { type: 'notFolder'; finderBlocked: boolean }
  | { type: 'unsupported' };
