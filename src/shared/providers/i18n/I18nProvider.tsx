import { createContext, useContext, useEffect } from 'react';

import { useSettings } from '@/domain/settings/queries/settings';
import { type Dict, type Locale, dicts, resolveLocale } from '@/shared/i18n';

interface I18n {
  locale: Locale;
  t: Dict;
}

const I18nContext = createContext<I18n>({ locale: 'en', t: dicts.en });

export function useI18n() {
  return useContext(I18nContext);
}

export default function I18nProvider({ children }: Readonly<{ children: React.ReactNode }>) {
  // queries
  const { settings } = useSettings();

  const locale = resolveLocale(settings?.locale ?? 'system', navigator.language);

  // useEffect
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  return <I18nContext.Provider value={{ locale, t: dicts[locale] }}>{children}</I18nContext.Provider>;
}
