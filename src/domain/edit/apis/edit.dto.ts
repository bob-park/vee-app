/** One step of an edit; mirrors Rust `Op` (serde `type` tag, camelCase). */
export type EditOp =
  | { type: 'crop'; x: number; y: number; w: number; h: number }
  /** `1` turns clockwise, `-1` counter-clockwise. */
  | { type: 'rotate'; quarter: -1 | 1 }
  | { type: 'resize'; w: number; h: number }
  | { type: 'removeBg' }
  | { type: 'upscale'; scale: 2 | 4 };

/** A rendered image; mirrors Rust `EditImageDto`. `width`/`height` are full-res, the data URL may be smaller. */
export interface EditImage {
  width: number;
  height: number;
  dataUrl: string;
}
