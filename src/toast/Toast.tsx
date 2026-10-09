import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Kind, ToastPayload } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import "./toast.css";

/** White line icons for what was copied, drawn on the coloured tile. */
const KIND_ICONS: Record<Kind, string> = {
  text: "M5 6h14M5 10h14M5 14h10M5 18h7",
  link: "M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1",
  image: "M4 5h16v14H4zM4 16l5-5 4 4 2-2 5 5M15.5 8.5h.01",
  files: "M6 3h8l4 4v14H6zM14 3v4h4",
};

function Icon({ path }: { path: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d={path} />
    </svg>
  );
}

export function Toast() {
  const { t } = usePrefs();
  const [toast, setToast] = useState<{ payload: ToastPayload; key: number; out: boolean } | null>(null);

  useEffect(() => {
    const offShow = listen<ToastPayload>("toast://show", (e) =>
      setToast({ payload: e.payload, key: Date.now(), out: false }),
    );
    const offHide = listen("toast://hide", () => setToast((cur) => cur && { ...cur, out: true }));
    return () => {
      void offShow.then((off) => off());
      void offHide.then((off) => off());
    };
  }, []);

  if (!toast) return null;
  const p = toast.payload;
  const summary = !p.ok
    ? t.copyFailedDetail
    : p.kind === "image"
      ? t.kinds.image
      : p.kind === "files"
        ? t.filesCount(p.files)
        : p.text;
  return (
    <div
      key={toast.key}
      className={["toast", !p.ok && "toast-error", toast.out && "out"].filter(Boolean).join(" ")}
      role="status"
    >
      <span className="toast-icon">
        {p.ok ? <Icon path={KIND_ICONS[p.kind]} /> : "!"}
        {p.ok && <span className="toast-check">✓</span>}
      </span>
      <span className="toast-text">
        <b>{p.ok ? (p.plain ? t.copiedPlain : t.copied) : t.copyFailed}</b>
        {summary && <span className="toast-summary">{summary}</span>}
      </span>
    </div>
  );
}
