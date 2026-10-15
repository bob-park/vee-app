import cx from 'classnames';

interface SegmentedProps<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  /** Fills the row, options sharing the width. */
  wide?: boolean;
  /** An option waiting for confirmation, outlined in red. */
  pending?: T;
}

/** Radio group drawn as a segmented control. */
export default function Segmented<T extends string>({ value, options, onChange, wide, pending }: SegmentedProps<T>) {
  return (
    <div className={cx('bg-subtle gap-0.5 rounded-[10px] p-0.5', wide ? 'flex' : 'inline-flex')} role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          className={cx('rounded-lg px-2.5 py-1 text-[12px]', {
            'flex-1': wide,
            'bg-card shadow-seg font-semibold': o.value === value,
            'outline-error text-error outline-[1.5px] outline-dashed': o.value === pending,
            'text-base-content': o.value === value && o.value !== pending,
            'text-muted': o.value !== value && o.value !== pending,
          })}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
