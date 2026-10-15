import cx from 'classnames';

interface RowProps {
  /** Left out for a row that is all controls. */
  label?: string;
  hint?: string;
  dim?: boolean;
  /** Stacks the control under the label at full width. */
  col?: boolean;
  children: React.ReactNode;
}

/** One settings line: label and hint left, control right. */
export default function Row({ label, hint, dim, col, children }: RowProps) {
  return (
    <div
      className={cx(
        'border-divider flex justify-between border-b py-3 last:border-b-0',
        col ? 'flex-col items-stretch gap-2' : 'items-center gap-3',
        { 'opacity-45': dim },
      )}
    >
      {label && (
        <div>
          {label}
          {hint && <small className="text-muted mt-0.5 block text-[11px]">{hint}</small>}
        </div>
      )}
      {children}
    </div>
  );
}
