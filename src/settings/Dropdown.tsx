import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { nextIndex } from "./dropdownKeys.ts";

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
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const id = useId();
  const selected = options.findIndex((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);

  useEffect(() => {
    if (open) list.current?.children[active]?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

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
      if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        show();
      }
      return;
    }
    if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.preventDefault();
      close();
    } else if (e.key === "Enter" || e.key === " ") {
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
    <div className={["dropdown", className].filter(Boolean).join(" ")} ref={root}>
      <button
        ref={button}
        type="button"
        className="select"
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
        <span className="chev" aria-hidden>
          ▾
        </span>
      </button>
      {open && (
        <ul ref={list} id={id} role="listbox" aria-label={ariaLabel} className="dropdown-list">
          {options.map((o, i) => (
            <li
              key={o.value}
              id={`${id}-${i}`}
              role="option"
              aria-selected={i === selected}
              className={i === active ? "active" : undefined}
              onPointerEnter={() => setActive(i)}
              onClick={() => pick(i)}
            >
              {o.icon && <img src={o.icon} alt="" />}
              <span>{o.label}</span>
              {i === selected && <span className="check" aria-hidden>✓</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
