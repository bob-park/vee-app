import { type KeyboardEvent, useEffect, useId, useRef, useState } from 'react';

import { nextIndex } from '@/utils/dropdownKeys';

import cx from 'classnames';

export interface DropdownOption<T> {
  value: T;
  label: string;
  icon?: string | null;
}

/** Themed replacement for <select>: the native popup ignores our colors (white-on-white on Windows dark mode). */
export function Dropdown<T extends string | number>({
  value,
  options,
  onChange,
  ariaLabel,
  placeholder,
  disabled,
  className,
}: {
  value: T | null;
  options: DropdownOption<T>[];
  onChange: (v: T) => void;
  ariaLabel: string;
  /** Shown on the button instead of the selected label, e.g. an "Add…" picker. */
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}) {
  // ref
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);

  // state
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);

  // hooks
  const id = useId();

  const selected = options.findIndex((o) => o.value === value);

  // useEffect
  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [open]);

  useEffect(() => {
    if (open) list.current?.children[active]?.scrollIntoView({ block: 'nearest' });
  }, [open, active]);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  // handle
  const show = () => {
    setActive(selected);
    setOpen(true);
  };

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };

  const pick = (i: number) => {
    close();
    if (options[i] && options[i].value !== value) onChange(options[i].value);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        show();
      }
      return;
    }
    if (e.key === 'Escape' || e.key === 'Tab') {
      if (e.key === 'Escape') e.preventDefault();
      close();
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      pick(active);
    } else {
      const next = nextIndex(e.key, active, options.length);
      if (next !== null) {
        e.preventDefault();
        setActive(next);
      }
    }
  };

  const current = options[selected];
  return (
    <div className={cx('relative', className)} ref={root}>
      <button
        ref={button}
        type="button"
        className="border-base-300 bg-subtle text-base-content aria-expanded:border-primary inline-flex items-center gap-2 rounded-[10px] border px-[10px] py-[5px] text-[12px] disabled:cursor-default"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-activedescendant={open && active >= 0 ? `${id}-${active}` : undefined}
        disabled={disabled}
        onClick={() => (open ? close() : show())}
        onKeyDown={onKeyDown}
      >
        <span>{placeholder ?? current?.label}</span>
        <span className="text-muted text-[10px]" aria-hidden>
          ▾
        </span>
      </button>
      {open && (
        <ul
          ref={list}
          id={id}
          role="listbox"
          aria-label={ariaLabel}
          className="border-base-300 bg-card shadow-dropdown absolute top-[calc(100%+4px)] right-0 z-10 max-h-[220px] min-w-full overflow-y-auto rounded-[10px] border p-1 text-[12px]"
        >
          {options.map((o, i) => (
            <li
              key={o.value}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === selected}
              className={cx(
                'flex cursor-pointer items-center gap-1.5 rounded-[7px] px-[10px] py-[5px] whitespace-nowrap',
                i === selected ? 'bg-accent-subtle text-primary font-semibold' : { 'bg-subtle': i === active },
              )}
              onPointerEnter={() => setActive(i)}
              onClick={() => pick(i)}
            >
              {o.icon && <img className="size-3.5" src={o.icon} alt="" />}
              <span>{o.label}</span>
              {i === selected && (
                <span className="ml-auto pl-3" aria-hidden>
                  ✓
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
