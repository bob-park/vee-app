import type { Kind } from '@/domain/clip/apis/clip.dto';

/** Line icons for what was copied, drawn in `currentColor`. */
export const KIND_PATHS: Record<Kind, string> = {
  text: 'M5 6h14M5 10h14M5 14h10M5 18h7',
  link: 'M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1',
  image: 'M4 5h16v14H4zM4 16l5-5 4 4 2-2 5 5M15.5 8.5h.01',
  files: 'M6 3h8l4 4v14H6zM14 3v4h4',
};

export default function KindIcon({ kind, className }: { kind: Kind; className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d={KIND_PATHS[kind]} />
    </svg>
  );
}
