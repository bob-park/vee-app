import cx from 'classnames';

/** Bottom floats (toolbar, record pill): `.glass` with a denser fill, since they sit over any desktop. They stay mounted and ease in and out instead of popping; out is quicker. */
export const floatClass = (shown: boolean) =>
  cx(
    'bg-glass-float ease-spring transition-[opacity,translate,scale] motion-reduce:transition-none',
    shown ? 'duration-250' : 'pointer-events-none translate-y-2 scale-95 opacity-0 duration-150',
  );
