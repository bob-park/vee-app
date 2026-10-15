// Type-only import: erased at runtime, so node --test needs no alias resolution.
import type { Settings } from '@/domain/settings/apis/settings.dto';

export function resolveTheme(pref: Settings['theme'], prefersDark: boolean): 'light' | 'dark' {
  if (pref === 'system') {
    return prefersDark ? 'dark' : 'light';
  }

  return pref;
}
