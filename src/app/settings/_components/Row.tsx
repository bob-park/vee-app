import cx from 'classnames';

interface RowProps {
  /** Left out for a row that is all controls. */
  label?: string;
  hint?: string;
  dim?: boolean;
  /** Stacks the control under the label at full width. */
  col?: boolean;
  /** Id on the label, for a control that names itself by it. */
  labelId?: string;
  /** Cuts a long hint (a path) to one line with an ellipsis; the full text is its tooltip. */
  truncateHint?: boolean;
  children: React.ReactNode;
}

/** One settings line: label and hint left, control right. */
export default function Row({ label, hint, dim, col, labelId, truncateHint, children }: RowProps) {
  return (
    <div
      className={cx(
        'border-divider flex justify-between border-b py-3 last:border-b-0',
        col ? 'flex-col items-stretch gap-2' : 'items-center gap-3',
        { 'opacity-45': dim },
      )}
    >
      {label && (
        <div id={labelId} className={cx({ 'min-w-0': truncateHint })}>
          {label}
          {hint && (
            <small
              className={cx('text-muted mt-0.5 block text-[11px]', { truncate: truncateHint })}
              title={truncateHint ? hint : undefined}
            >
              {hint}
            </small>
          )}
        </div>
      )}
      {children}
    </div>
  );
}
