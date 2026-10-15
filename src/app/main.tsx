import { StrictMode } from 'react';

import { PanelApp } from '@/app/panel/PanelApp';
import { SettingsApp } from '@/app/settings/SettingsApp';
import { ToastApp } from '@/app/toast/ToastApp';
import { WhatsNewApp } from '@/app/whats-new/WhatsNewApp';
import { getSettings } from '@/domain/settings/apis/settings';
import { SETTINGS_QUERY_KEY } from '@/domain/settings/queries/settings';
import RQProvider, { queryClient } from '@/shared/components/queries/RQProvider';
import { resolveLocale } from '@/shared/i18n';
import I18nProvider from '@/shared/providers/i18n/I18nProvider';
import ThemeProvider, { applyTheme } from '@/shared/providers/theme/ThemeProvider';

import { getCurrentWindow } from '@tauri-apps/api/window';
import { createRoot } from 'react-dom/client';

import './theme.css';

const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
const View =
  label === 'panel' ? PanelApp : label === 'toast' ? ToastApp : label === 'whats-new' ? WhatsNewApp : SettingsApp;

// Seed settings before the first paint so there is no flash of the wrong theme or language.
// On failure the query fetches (and retries) on mount instead.
const settings = await getSettings().catch(() => undefined);
if (settings) {
  queryClient.setQueryData(SETTINGS_QUERY_KEY, settings);
  applyTheme(settings.theme);
  document.documentElement.lang = resolveLocale(settings.locale, navigator.language);
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RQProvider>
      <ThemeProvider>
        <I18nProvider>
          <View />
        </I18nProvider>
      </ThemeProvider>
    </RQProvider>
  </StrictMode>,
);
