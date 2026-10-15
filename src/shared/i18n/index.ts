import { type Dict, en } from './en';
import { ko } from './ko';
import type { Locale } from './locale';

export const dicts: Record<Locale, Dict> = { ko, en };

export type { Dict, Locale };
export { resolveLocale } from './locale';

export function relativeTime(ms: number, locale: Locale, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
  const sec = Math.round((ms - now) / 1000);
  const abs = Math.abs(sec);
  if (abs < 60) return rtf.format(sec, 'second');
  if (abs < 3600) return rtf.format(Math.round(sec / 60), 'minute');
  if (abs < 86400) return rtf.format(Math.round(sec / 3600), 'hour');
  return rtf.format(Math.round(sec / 86400), 'day');
}
