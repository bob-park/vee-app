import type { ReactNode } from 'react';

import cx from 'classnames';

/** A button in a confirmation: the card's delete prompt and the toolbar's update prompt. */
export function ConfirmButton({
  tone,
  tabIndex,
  onClick,
  children,
}: {
  tone?: 'danger' | 'go';
  tabIndex?: number;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      className={cx('rounded-box px-3.5 py-2 text-[12px] font-semibold', {
        'bg-subtle': !tone,
        'bg-error text-white': tone === 'danger',
        'bg-ok-fg text-card': tone === 'go',
      })}
      onClick={onClick}
      tabIndex={tabIndex}
    >
      {children}
    </button>
  );
}
