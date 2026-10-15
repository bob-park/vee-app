// Type-only import: erased at runtime, so node --test needs no alias resolution.
import type { Settings } from '@/domain/settings/apis/settings.dto';

export type Locale = 'ko' | 'en';

/** "system" follows the OS language (`navigator.language` in the webview). */
export function resolveLocale(pref: Settings['locale'], systemLanguage: string): Locale {
  if (pref === 'ko' || pref === 'en') return pref;
  return systemLanguage.toLowerCase().startsWith('ko') ? 'ko' : 'en';
}
