/** One step of an edit; mirrors Rust `Op` (serde `type` tag, camelCase). */
export type EditOp =
  | { type: 'crop'; x: number; y: number; w: number; h: number }
  /** `1` turns clockwise, `-1` counter-clockwise. */
  | { type: 'rotate'; quarter: -1 | 1 }
  | { type: 'resize'; w: number; h: number }
  | { type: 'removeBg' }
  | { type: 'upscale'; scale: 2 | 4 }
  | { type: 'subject'; points: SubjectPoint[] }
  | { type: 'brush'; strokes: Stroke[] };

/** A click on the image for the subject cut, in image px; `keep: false` marks background. Mirrors Rust `SubjectPoint`. */
export interface SubjectPoint {
  x: number;
  y: number;
  keep: boolean;
}

export type BrushMode = 'erase' | 'restore';

/** One brush drag in image px; `points` are `[x, y]` pairs. Mirrors Rust `Stroke`. */
export interface Stroke {
  mode: BrushMode;
  radius: number;
  points: [number, number][];
}

/** A rendered image; mirrors Rust `EditImageDto`. `width`/`height` are full-res, the data URL may be smaller. */
export interface EditImage {
  width: number;
  height: number;
  dataUrl: string;
}
