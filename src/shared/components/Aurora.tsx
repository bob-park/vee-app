import cx from 'classnames';

/** The soft aurora band along the top of its positioned parent; the caller sets its height. */
export default function Aurora({ className }: { className?: string }) {
  return (
    <div className={cx('aurora', className)} aria-hidden>
      <i className="aurora-blob" />
      <i className="aurora-blob" />
      <i className="aurora-blob" />
    </div>
  );
}
