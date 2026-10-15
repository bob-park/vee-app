import { useEffect } from 'react';

import { useQueryClient } from '@tanstack/react-query';

import type { Settings } from '@/domain/settings/apis/settings.dto';
import { SETTINGS_QUERY_KEY, useSettings } from '@/domain/settings/queries/settings';
import { resolveTheme } from '@/utils/theme';

import { listen } from '@tauri-apps/api/event';

const DARK_QUERY = '(prefers-color-scheme: dark)';

export function applyTheme(pref: Settings['theme']) {
  document.documentElement.dataset.theme = resolveTheme(pref, window.matchMedia(DARK_QUERY).matches);
}

export default function ThemeProvider({ children }: Readonly<{ children: React.ReactNode }>) {
  // hooks
  const queryClient = useQueryClient();

  // queries
  const { settings } = useSettings();

  const theme = settings?.theme ?? 'system';

  // useEffect
  useEffect(() => {
    applyTheme(theme);

    if (theme !== 'system') {
      return;
    }

    const media = window.matchMedia(DARK_QUERY);
    const handleChange = () => applyTheme(theme);
    media.addEventListener('change', handleChange);

    return () => media.removeEventListener('change', handleChange);
  }, [theme]);

  useEffect(() => {
    // Any window may change a setting; each window has its own query cache.
    const unlisten = listen('settings://changed', () => {
      void queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  return children;
}
