import cx from 'classnames';

const VARIANTS = {
  default: 'bg-accent-subtle text-primary',
  danger: 'bg-danger-subtle text-error',
  gray: 'bg-subtle text-base-content',
  /** The confirm button of a destructive modal. */
  go: 'bg-error text-white',
};

type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: keyof typeof VARIANTS;
  small?: boolean;
};

/** Tinted settings button. */
export default function Button({ variant = 'default', small, ...props }: ButtonProps) {
  return (
    <button
      {...props}
      className={cx(
        'rounded-box font-semibold whitespace-nowrap disabled:cursor-default disabled:opacity-50',
        small ? 'px-2.5 py-1.5 text-[11px]' : 'px-3.5 py-2',
        VARIANTS[variant],
      )}
    />
  );
}
