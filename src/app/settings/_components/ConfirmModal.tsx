import { useEffect, useRef } from 'react';

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
      className="modal"
      // Let `open` drive closing so Esc can't leave React out of sync.
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
      // The card fills the dialog, so a click on the dialog itself is a backdrop click.
      onClick={(e) => e.target === e.currentTarget && onCancel()}
    >
      {open && (
        <div className="modal-card">
          <h2>{title}</h2>
          <p>{body}</p>
          <div className="confirm-acts">
            <button className="btn gray" autoFocus onClick={onCancel}>
              {t.cancel}
            </button>
            <button className="btn go" onClick={onConfirm}>
              {confirmLabel}
            </button>
          </div>
        </div>
      )}
    </dialog>
  );
}
