import { type KeyboardEvent, useState } from 'react';

import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { acceleratorFromEvent, formatAccelerator } from '@/utils/accelerator';

const isMac = navigator.userAgent.includes('Mac');

interface Props {
  value: string;
  onSave: (accel: string) => Promise<void>;
  /** Combos already bound elsewhere; recording one shows an error instead of saving. */
  taken: string[];
}

export function ShortcutRecorder({ value, onSave, taken }: Props) {
  // state
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // hooks
  const { t } = useI18n();

  // handle

  const onKeyDown = async (e: KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    if (e.key === 'Escape') {
      setRecording(false);
      return;
    }
    const accel = acceleratorFromEvent(e, isMac);
    if (!accel) return;
    setRecording(false);
    if (accel !== value && taken.includes(accel)) {
      setError(t.settings.shortcutTaken);
      return;
    }
    try {
      await onSave(accel);
      setError(null);
    } catch {
      setError(t.settings.shortcutFailed);
    }
  };

  return (
    <div className="recorder">
      <button
        className={recording ? 'kbd recording' : 'kbd'}
        onClick={(e) => {
          setRecording(true);
          // WebKit does not focus a clicked <button>, so keydown/blur would never reach it.
          e.currentTarget.focus();
          setError(null);
        }}
        onKeyDown={(e) => void onKeyDown(e)}
        onBlur={() => setRecording(false)}
      >
        {recording ? t.settings.recording : formatAccelerator(value, isMac)}
      </button>
      {error && (
        <span className="error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
