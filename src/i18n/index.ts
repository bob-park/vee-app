import { en, type Dict } from "./en.ts";
import { ko } from "./ko.ts";

export type Locale = "ko" | "en";

export const dicts: Record<Locale, Dict> = { ko, en };

export function resolveLocale(setting: string): Locale {
  if (setting === "ko" || setting === "en") return setting;
  return navigator.language.toLowerCase().startsWith("ko") ? "ko" : "en";
}

export function relativeTime(ms: number, locale: Locale, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const sec = Math.round((ms - now) / 1000);
  const abs = Math.abs(sec);
  if (abs < 60) return rtf.format(sec, "second");
  if (abs < 3600) return rtf.format(Math.round(sec / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(sec / 3600), "hour");
  return rtf.format(Math.round(sec / 86400), "day");
}
