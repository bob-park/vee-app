import type { Format } from '@/domain/convert/apis/convert.dto';

/** The donut's edit slice, which opens the editor instead of converting. */
export function isEdit(f: Format): boolean {
  return f === 'edit';
}

/** How a target format is written in the UI: `720p` as is, the rest upper case. */
export function formatLabel(f: Format): string {
  return f === '720p' ? f : f.toUpperCase();
}
