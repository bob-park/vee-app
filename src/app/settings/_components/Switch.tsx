interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  ariaLabel: string;
}

/** On/off switch. */
export default function Switch({ checked, onChange, ariaLabel }: SwitchProps) {
  return (
    <input
      type="checkbox"
      role="switch"
      aria-label={ariaLabel}
      checked={checked}
      onChange={(e) => onChange(e.target.checked)}
      className="bg-base-300 checked:bg-primary relative h-5 w-9 cursor-pointer appearance-none rounded-[10px] [transition:background_200ms] after:absolute after:top-0.5 after:left-0.5 after:size-4 after:rounded-full after:bg-white after:content-[''] after:[transition:translate_450ms_var(--ease-spring)] checked:after:translate-x-4"
    />
  );
}
