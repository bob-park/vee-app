import { useEffect, useRef } from 'react';

import Button from '@/app/settings/_components/Button';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

/** Native modal dialog: the browser handles Esc, focus trapping and aria-modal. */
export function ConfirmModal({
  open,
  title,
  body,
  confirmLabel,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  body: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  // ref
  const ref = useRef<HTMLDialogElement>(null);

  // hooks
  const { t } = useI18n();

  // useEffect

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      className="border-base-300 bg-card text-base-content shadow-modal rounded-box backdrop:bg-backdrop m-auto w-[280px] border p-0"
      // Let `open` drive closing so Esc can't leave React out of sync.
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      // The card fills the dialog, so a click on the dialog itself is a backdrop click.
      onClick={(e) => e.target === e.currentTarget && onCancel()}
    >
      {open && (
        <div className="p-4">
          <h2 className="mb-1 text-[14px] font-semibold">{title}</h2>
          <p className="text-muted text-[12px] leading-[1.5]">{body}</p>
          <div className="mt-[14px] flex justify-end gap-1.5">
            <Button variant="gray" autoFocus onClick={onCancel}>
              {t.cancel}
            </Button>
            <Button variant="go" onClick={onConfirm}>
              {confirmLabel}
            </Button>
          </div>
        </div>
      )}
    </dialog>
  );
}
