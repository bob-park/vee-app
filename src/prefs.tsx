import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Settings } from "./api.ts";
import { dicts, resolveLocale, type Locale } from "./i18n/index.ts";
import type { Dict } from "./i18n/en.ts";

interface Prefs {
  settings: Settings;
  locale: Locale;
  t: Dict;
  reload: () => Promise<void>;
}

const PrefsContext = createContext<Prefs | null>(null);
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

export function PrefsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [systemDark, setSystemDark] = useState(darkQuery.matches);

  const reload = useCallback(async () => {
    setSettings(await api.getSettings());
  }, []);

  useEffect(() => {
    void reload();
    const unlisten = listen("settings://changed", () => void reload());
    const onChange = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    darkQuery.addEventListener("change", onChange);
    return () => {
      void unlisten.then((off) => off());
      darkQuery.removeEventListener("change", onChange);
    };
  }, [reload]);

  const theme = settings?.theme === "light" || settings?.theme === "dark" ? settings.theme : systemDark ? "dark" : "light";
  const locale = resolveLocale(settings?.locale ?? "system");

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = locale;
  }, [theme, locale]);

  if (!settings) return null;
  return <PrefsContext.Provider value={{ settings, locale, t: dicts[locale], reload }}>{children}</PrefsContext.Provider>;
}

export function usePrefs(): Prefs {
  const prefs = useContext(PrefsContext);
  if (!prefs) throw new Error("usePrefs must be used inside PrefsProvider");
  return prefs;
}
