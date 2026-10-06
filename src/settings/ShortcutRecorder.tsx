import { useState, type KeyboardEvent } from "react";
import { api } from "../api.ts";
import { usePrefs } from "../prefs.tsx";
import { acceleratorFromEvent, formatAccelerator } from "./accelerator.ts";

const isMac = navigator.userAgent.includes("Mac");

export function ShortcutRecorder({ value }: { value: string }) {
  const { t } = usePrefs();
  const [recording, setRecording] = useState(false);
  const [failed, setFailed] = useState(false);

  const onKeyDown = async (e: KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      setRecording(false);
      return;
    }
    const accel = acceleratorFromEvent(e, isMac);
    if (!accel) return;
    setRecording(false);
    try {
      await api.setShortcut(accel);
      setFailed(false);
    } catch {
      setFailed(true);
    }
  };

  return (
    <div className="recorder">
      <button
        className={recording ? "kbd recording" : "kbd"}
        onClick={() => {
          setRecording(true);
          setFailed(false);
        }}
        onKeyDown={(e) => void onKeyDown(e)}
        onBlur={() => setRecording(false)}
      >
        {recording ? t.settings.recording : formatAccelerator(value, isMac)}
      </button>
      {failed && (
        <span className="error" role="alert">
          {t.settings.shortcutFailed}
        </span>
      )}
    </div>
  );
}
