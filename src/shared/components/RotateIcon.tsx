/** Two-tone rotate icon: a tinted rounded square with the arrow passing over its top. `currentColor` only. */
export default function RotateIcon({ dir }: { dir: 'ccw' | 'cw' }) {
  const ccw = dir === 'ccw';
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden>
      <rect x={ccw ? 8 : 4} y="9" width="12" height="12" rx="3" className="fill-current opacity-22" stroke="none" />
      <rect x={ccw ? 8 : 4} y="9" width="12" height="12" rx="3" />
      <path d={ccw ? 'M3.5 12V9a5 5 0 0 1 5-5H12' : 'M20.5 12V9a5 5 0 0 0-5-5H12'} strokeLinecap="round" />
      <path d={ccw ? 'M10 1.8 12.4 4 10 6.2' : 'M14 1.8 11.6 4 14 6.2'} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
