import type { Format } from '@/domain/convert/apis/convert.dto';

/** How a target format is written in the UI: `720p` as is, the rest upper case. */
export function formatLabel(f: Format): string {
  return f === '720p' ? f : f.toUpperCase();
}
