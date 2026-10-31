import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

/** The editor's tools. `removeBg` and `upscale` show their result next to the original before it's applied. */
export type EditorTab = 'crop' | 'rotate' | 'resize' | 'removeBg' | 'upscale';

const TABS: EditorTab[] = ['crop', 'rotate', 'resize', 'removeBg', 'upscale'];

interface EditorHeaderProps {
  isMac: boolean;
  tab: EditorTab;
  onTab: (tab: EditorTab) => void;
  /** Disables every tab button, while a slow AI step renders. */
  disabled: boolean;
}

/** The title row (macOS only) and the tab bar. */
export default function EditorHeader({ isMac, tab, onTab, disabled }: EditorHeaderProps) {
  // hooks
  const { t } = useI18n();

  return (
    <>
      {/* macOS overlays the traffic lights on this row (`titleBarStyle: Overlay`); Windows keeps its native bar. */}
      {isMac && (
        <div
          className="relative z-1 flex h-7 shrink-0 items-center justify-center font-semibold select-none"
          data-tauri-drag-region
        >
          {t.editor.title}
        </div>
      )}
      <div className="relative z-1 flex shrink-0 justify-center pt-1 pb-3">
        {/* Disabled (natively, for every tab button) while a slow AI step renders. */}
        <fieldset className="glass rounded-xl p-0.5 disabled:opacity-60" disabled={disabled}>
          <Segmented
            value={tab}
            onChange={onTab}
            options={TABS.map((value) => ({ value, label: t.editor.tabs[value] }))}
          />
        </fieldset>
      </div>
    </>
  );
}
