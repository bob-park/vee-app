import { arrowStep } from '@/utils/radio';

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

/** Radio group drawn as a segmented control, with a thumb that slides to the selection. */
export default function Segmented<T extends string>({ value, options, onChange, wide, pending }: SegmentedProps<T>) {
  const index = options.findIndex((o) => o.value === value);

  // handle
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const next = arrowStep(e.key, Math.max(index, 0), options.length);
    if (next === null) return;
    e.preventDefault();
    onChange(options[next].value);
    e.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')[next]?.focus();
  };

  return (
    <div
      className={cx(
        'bg-subtle relative auto-cols-fr grid-flow-col rounded-[10px] p-0.5',
        wide ? 'grid' : 'inline-grid',
      )}
      role="radiogroup"
      onKeyDown={handleKeyDown}
    >
      {index >= 0 && (
        // One cell wide (no gaps), so translating by 100% steps exactly one option.
        <span
          className="bg-card shadow-seg ease-spring absolute inset-y-0.5 left-0.5 rounded-lg transition-transform duration-450"
          style={{ width: `calc((100% - 4px) / ${options.length})`, transform: `translateX(${index * 100}%)` }}
          aria-hidden
        />
      )}
      {options.map((o, i) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={i === index}
          tabIndex={i === index || (index < 0 && i === 0) ? 0 : -1}
          className={cx('relative rounded-lg px-2.5 py-1 text-[12px]', {
            'font-semibold': i === index,
            'outline-error text-error outline-[1.5px] outline-dashed': o.value === pending,
            'text-base-content': i === index && o.value !== pending,
            'text-muted': i !== index && o.value !== pending,
          })}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
