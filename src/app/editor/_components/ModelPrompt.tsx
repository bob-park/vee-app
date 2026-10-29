import { openSettings } from '@/domain/panel/apis/panel';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

/** In place of a tool whose AI model isn't downloaded: a pointer to Settings › Convert. */
export default function ModelPrompt() {
  // hooks
  const { t } = useI18n();

  return (
    <div className="flex items-center gap-3">
      <span className="text-muted text-[12px]">{t.editor.needsModel}</span>
      <button
        className="rounded-box bg-accent-subtle text-primary px-3.5 py-1.5 text-[12px] font-semibold"
        onClick={() => void openSettings('convert')}
      >
        {t.editor.openSettings}
      </button>
    </div>
  );
}
