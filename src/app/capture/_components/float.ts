import cx from 'classnames';

/** Bottom floats (toolbar, record pill) stay mounted and ease in and out instead of popping; out is quicker. */
export const floatClass = (shown: boolean) =>
  cx(
    'ease-spring transition-[opacity,translate,scale] motion-reduce:transition-none',
    shown ? 'duration-250' : 'pointer-events-none translate-y-2 scale-95 opacity-0 duration-150',
  );
