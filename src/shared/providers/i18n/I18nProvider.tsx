import { createContext, useContext, useEffect } from 'react';

import { useQueryClient } from '@tanstack/react-query';

import { SETTINGS_QUERY_KEY, useSettings } from '@/domain/settings/queries/settings';
import { type Dict, type Locale, dicts, resolveLocale } from '@/shared/i18n';

import { listen } from '@tauri-apps/api/event';

interface I18n {
  locale: Locale;
  t: Dict;
}

const I18nContext = createContext<I18n>({ locale: 'en', t: dicts.en });

export function useI18n() {
  return useContext(I18nContext);
}

export default function I18nProvider({ children }: Readonly<{ children: React.ReactNode }>) {
  // hooks
  const queryClient = useQueryClient();

  // queries
  const { settings } = useSettings();

  const locale = resolveLocale(settings?.locale ?? 'system', navigator.language);

  // useEffect
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    // Any window may change a setting; each window has its own query cache.
    const unlisten = listen('settings://changed', () => {
      void queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  return <I18nContext.Provider value={{ locale, t: dicts[locale] }}>{children}</I18nContext.Provider>;
}
