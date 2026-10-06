import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ToastPayload } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import "./toast.css";

export function Toast() {
  const { t } = usePrefs();
  const [toast, setToast] = useState<{ payload: ToastPayload; key: number } | null>(null);

  useEffect(() => {
    const unlisten = listen<ToastPayload>("toast://show", (e) => setToast({ payload: e.payload, key: Date.now() }));
    return () => void unlisten.then((off) => off());
  }, []);

  if (!toast) return null;
  const p = toast.payload;
  if (!p.ok) {
    return (
      <div key={toast.key} className="toast toast-error" role="status">
        ⚠ {t.copyFailed}
      </div>
    );
  }
  const summary = p.image ? t.kinds.image : p.files > 0 ? t.filesCount(p.files) : p.text;
  return (
    <div key={toast.key} className="toast" role="status">
      <span className="toast-check">✓</span>
      <b>{t.copied}</b>
      {summary && <span className="toast-summary">· {summary}</span>}
    </div>
  );
}
