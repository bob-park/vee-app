import { useRef } from 'react';

import type { Clip } from '@/domain/clip/apis/clip.dto';
import { cancelConvert, dismissConvert, retryConvert } from '@/domain/convert/apis/convert';
import type { ConvertJob } from '@/domain/convert/apis/convert.dto';
import KindIcon from '@/shared/components/KindIcon';
import { relativeTime } from '@/shared/i18n';
import type { Dict } from '@/shared/i18n/en';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { imageTag } from '@/utils/fileThumb';
import { formatLabel } from '@/utils/formatLabel';
import { linkParts } from '@/utils/link';

import cx from 'classnames';

import { ConfirmButton } from './ConfirmButton';
import { DocIcon, FolderIcon } from './FileIcon';

interface Props {
  clip: Clip;
  /** The clip's latest conversion, drawn over the body. */
  job?: ConvertJob;
  selected: boolean;
  onSelect: () => void;
  /** Every mousemove over the card; the panel decides whether the pointer really moved. */
  onHover: () => void;
  /** `plain` copies text without its formatting (shift held). */
  onCopy: (plain: boolean) => void;
  onTogglePin: () => void;
  /** Starts dragging the card out, from the card element and the cursor position. */
  onDragOut?: (card: HTMLElement, x: number, y: number) => void;
  confirming: boolean;
  onConfirmDelete: () => void;
  onCancelDelete: () => void;
  /** Position in the open entrance's stagger (capped by the caller). */
  enterIndex: number;
}

const DRAG_THRESHOLD = 5;

function fileCount(clip: Clip): number {
  return Number(clip.meta ?? '1');
}

function badgeLabel(clip: Clip, t: Dict): string {
  switch (clip.kind) {
    case 'files':
      return clip.isDir ? t.kinds.folder : t.filesCount(fileCount(clip));
    default:
      return t.kinds[clip.kind];
  }
}

function paths(clip: Clip): string[] {
  return (clip.textPreview ?? '').split('\n');
}

function firstPath(clip: Clip): string {
  return paths(clip)[0];
}

function footLabel(clip: Clip, t: Dict): string {
  if (clip.missing) return t.missing;
  switch (clip.kind) {
    case 'image':
      return clip.meta ?? '';
    case 'files': {
      const first = firstPath(clip);
      const name = first.split(/[\\/]/).filter(Boolean).pop() ?? first;
      return t.moreFiles(name, fileCount(clip) - 1);
    }
    default:
      return t.chars(clip.charCount);
  }
}

const THUMB = 'size-full rounded-md object-contain';

/** Tilt per layer, front (first file) first. */
const LAYER_TILT = [0, 5, -7];

function FilesBody({ clip }: { clip: Clip }) {
  const { stack } = clip;
  if (stack.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        {clip.isDir ? <FolderIcon /> : <DocIcon path={firstPath(clip)} />}
      </div>
    );
  }
  if (stack.length === 1) {
    const tag = imageTag(firstPath(clip));
    return (
      <>
        <img className={THUMB} src={stack[0] ?? ''} alt="" />
        {tag && (
          <span className="bg-ext-tag absolute bottom-1.5 left-3.5 rounded-md px-1.5 py-0.5 text-[11px] font-semibold text-white">
            {tag}
          </span>
        )}
      </>
    );
  }
  return (
    <div
      className="*:border-card *:shadow-stack relative h-full *:absolute *:top-2.5 *:left-4.5 *:h-[calc(100%-14px)] *:w-[calc(100%-36px)] *:rounded-md *:border-2 *:object-cover"
      aria-hidden
    >
      {stack.map((src, i) => {
        const style = { transform: `rotate(${LAYER_TILT[i]}deg)`, zIndex: LAYER_TILT.length - i };
        return src ? (
          <img key={i} src={src} alt="" style={style} />
        ) : (
          <div key={i} className="bg-card flex items-center justify-center *:w-2/5" style={style}>
            <DocIcon path={paths(clip)[i] ?? ''} />
          </div>
        );
      })}
    </div>
  );
}

/** Keeps a press on an overlay button from selecting, copying or dragging the card. */
const STOP = {
  onMouseDown: (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
  },
  onDoubleClick: (e: React.MouseEvent) => e.stopPropagation(),
};

function JobButton({ label, onClick, children }: { label?: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      {...STOP}
      className="bg-backdrop flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[11px] leading-none"
      aria-label={label}
      tabIndex={-1}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
    >
      {children}
    </button>
  );
}

/** Progress of a conversion, over the card body (done shows only a chip). */
function JobOverlay({ job }: { job: ConvertJob }) {
  // hooks
  const { t } = useI18n();

  const fmt = formatLabel(job.format);
  if (job.state === 'done') {
    return (
      <span className="bg-primary absolute top-1.5 left-3.5 rounded-md px-1.5 py-0.5 text-[10px] font-bold text-white">
        {t.convert.saved(fmt)}
      </span>
    );
  }
  const dismiss = (
    <span className="absolute top-1.5 right-1.5">
      <JobButton label={t.convert.dismiss} onClick={() => void dismissConvert(job.id)}>
        ✕
      </JobButton>
    </span>
  );
  if (job.state === 'failed') {
    return (
      <div className="bg-job-failed absolute inset-x-2 inset-y-0 flex flex-col justify-end gap-1 rounded-md p-2 text-white">
        {dismiss}
        <div className="flex items-center justify-between gap-1.5 text-[11px] font-semibold">
          <span className="truncate">{t.convert.failed}</span>
          <JobButton onClick={() => void retryConvert(job.id)}>{t.convert.retry} ↻</JobButton>
        </div>
        {job.error && <span className="text-[10px] opacity-85">{t.convert.errors[job.error]}</span>}
      </div>
    );
  }
  const running = job.state === 'running';
  const counted = job.total > 1;
  // Video jobs report a percent; image batches fall back to finished files; otherwise it spins.
  const percent = job.percent ?? (running && counted ? Math.round((job.done / job.total) * 100) : null);
  const detail = [
    running && counted && `${job.done}/${job.total}`,
    running && job.eta !== null && t.convert.remaining(job.eta),
  ]
    .filter(Boolean)
    .join(' · ');
  return (
    <div className="bg-job-veil absolute inset-x-2 inset-y-0 flex flex-col items-center justify-center gap-1.5 rounded-md px-2 text-center text-white">
      {job.state === 'queued' && dismiss}
      {running && job.cancellable && (
        <span className="absolute top-1.5 right-1.5">
          <JobButton label={t.convert.cancel} onClick={() => void cancelConvert(job.id)}>
            ✕
          </JobButton>
        </span>
      )}
      <ProgressRing percent={job.state === 'queued' ? 0 : percent} spin={running && percent === null} />
      <span className="w-full truncate text-[11px] font-semibold">
        {job.state === 'queued' ? t.convert.queued(job.position) : t.convert.converting(fmt)}
      </span>
      {/* Always rendered so the ring doesn't jump up when the remaining time first appears. */}
      <span className="w-full truncate text-[10px] opacity-75">{detail || '\u00a0'}</span>
    </div>
  );
}

const RING_R = 36;
const RING_C = 2 * Math.PI * RING_R;

/** A ring filled to `percent` with the number in the middle; `spin` turns a partial arc when there is no number. */
function ProgressRing({ percent, spin }: { percent: number | null; spin: boolean }) {
  const offset = percent === null ? RING_C * 0.72 : RING_C * (1 - percent / 100);
  return (
    // Drawn in one viewBox so the number scales with the ring, which shrinks with a short panel (78px at most).
    <svg viewBox="0 0 80 80" className="aspect-square h-[45%] max-h-[78px] min-h-8" aria-hidden>
      <g className={cx('origin-center', spin && 'animate-spin motion-reduce:animate-none')}>
        <g transform="rotate(-90 40 40)">
          <circle cx="40" cy="40" r={RING_R} fill="none" strokeWidth="6" className="stroke-white/20" />
          <circle
            cx="40"
            cy="40"
            r={RING_R}
            fill="none"
            strokeWidth="6"
            strokeLinecap="round"
            strokeDasharray={RING_C}
            strokeDashoffset={offset}
            className="stroke-white [transition:stroke-dashoffset_400ms] motion-reduce:transition-none"
          />
        </g>
      </g>
      {percent !== null && !spin && (
        // Alphabetic baseline so the smaller % sits on the number's baseline; y=48 centres the digits.
        <text x="40" y="48" textAnchor="middle" className="fill-white text-[22px] font-bold">
          {percent}
          <tspan className="text-[13px] font-semibold opacity-80">%</tspan>
        </text>
      )}
    </svg>
  );
}

function Body({ clip }: { clip: Clip }) {
  if (clip.kind === 'image') return clip.thumb ? <img className={THUMB} src={clip.thumb} alt="" /> : null;
  if (clip.kind === 'files') return <FilesBody clip={clip} />;
  if (clip.kind === 'link') {
    const { host, rest } = linkParts(clip.textPreview ?? '');
    return (
      <div className="bg-subtle flex h-[calc(100%-8px)] flex-col overflow-hidden rounded-lg">
        <div className="from-primary/18 to-sky/22 text-primary grid min-h-0 flex-1 place-items-center bg-linear-to-br">
          <KindIcon kind="link" className="size-[30%] max-h-9" />
        </div>
        <div className="border-divider border-t px-2.5 py-1.5">
          <b className="block truncate text-[12px]">{host}</b>
          <span className="text-muted block truncate text-[11px]">{rest}</span>
        </div>
      </div>
    );
  }
  return (
    // Set apart from the card's header and footer; the wrapper anchors the quote mark.
    <div className="relative h-[calc(100%-8px)]">
      <p className="bg-subtle line-clamp-14 h-full rounded-lg px-2.5 py-2 text-[length:clamp(12px,calc(100vh/26),15px)] leading-[1.45] break-all whitespace-pre-wrap">
        {clip.textPreview}
      </p>
      <span
        className="text-primary pointer-events-none absolute right-2 -bottom-1.5 font-serif text-[44px] leading-none opacity-12"
        aria-hidden
      >
        &rdquo;
      </span>
    </div>
  );
}

export function Card({
  clip,
  job,
  selected,
  onSelect,
  onHover,
  onCopy,
  onTogglePin,
  onDragOut,
  confirming,
  onConfirmDelete,
  onCancelDelete,
  enterIndex,
}: Props) {
  // ref
  const press = useRef<{ x: number; y: number } | null>(null);

  // hooks
  const { t, locale } = useI18n();

  return (
    <div
      // `card` is the hook motion.ts finds cards by. `data-slot` (set by motion.ts) marks where a floating card came from.
      className={cx(
        'card group rounded-box glass relative flex w-[calc((100vh-72px)*0.95)] flex-none cursor-default flex-col overflow-hidden',
        // Lifts on hover, but not the empty slot a dragged card left.
        'ease-spring transition-[translate,box-shadow] duration-250 not-data-slot:hover:-translate-y-0.5',
        'data-slot:border-accent-subtle data-slot:border-2 data-slot:border-dashed data-slot:bg-transparent data-slot:shadow-none data-slot:*:invisible',
        confirming
          ? 'border-error shadow-card-confirm border-2'
          : job?.state === 'failed'
            ? 'border-error shadow-card border-2'
            : job?.state === 'done'
              ? 'border-success shadow-card border'
              : selected
                ? 'border-primary shadow-card-selected border'
                : // Idle keeps the glass hairline.
                  cx('shadow-card border', clip.pinned && 'border-pinned-border'),
      )}
      // The open entrance's stagger step, read by `[data-entering] .card` in globals.css.
      style={{ '--i': enterIndex } as React.CSSProperties}
      data-id={clip.id}
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      onDoubleClick={(e) => onCopy(e.shiftKey)}
      onMouseDown={(e) => {
        press.current = onDragOut ? { x: e.clientX, y: e.clientY } : null;
      }}
      onMouseMove={(e) => {
        onHover();
        const p = press.current;
        if (!p || !onDragOut) return;
        if ((e.buttons & 1) === 0) {
          press.current = null;
          return;
        }
        if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_THRESHOLD) return;
        press.current = null;
        onDragOut(e.currentTarget, e.clientX, e.clientY);
      }}
      onMouseUp={() => {
        press.current = null;
      }}
    >
      {/* The source app's icon, faded in from the top-left corner like a watermark. */}
      {clip.appIcon && (
        <img
          className="pointer-events-none absolute -top-3.5 -left-4.5 z-0 size-18 [mask-image:linear-gradient(135deg,#000_30%,transparent_85%)] opacity-16 [:root[data-theme=dark]_&]:opacity-22"
          src={clip.appIcon}
          alt=""
          aria-hidden
        />
      )}
      {/* The left 46px is where the backdrop icon shows through. */}
      <div className="relative z-1 flex min-h-10 items-center gap-1.5 py-2 pr-2 pl-11.5">
        <span className="text-muted min-w-0 truncate text-[11px] font-semibold">{clip.appName ?? ''}</span>
        <span
          className={cx(
            'truncate rounded-md px-1.5 py-0.5 text-[11px] font-semibold',
            clip.kind === 'files' ? 'bg-ok-bg text-ok-fg' : 'bg-badge-bg text-badge-fg',
          )}
        >
          {badgeLabel(clip, t)}
        </span>
        <span className="text-muted ml-auto text-[11px] whitespace-nowrap">
          {relativeTime(clip.lastUsedAt, locale)}
        </span>
        <button
          className={cx(
            'flex-none px-0.5 text-[12px] leading-none transition-opacity duration-120 ease-[ease]',
            clip.pinned ? 'opacity-100' : 'opacity-0 group-hover:opacity-50 hover:opacity-100',
          )}
          aria-label={clip.pinned ? t.unpin : t.pin}
          title={clip.pinned ? t.unpin : t.pin}
          tabIndex={-1}
          // Don't start a drag, select the card or copy it.
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
          onClick={(e) => {
            e.stopPropagation();
            onTogglePin();
          }}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          📌
        </button>
      </div>
      <div className="relative z-1 min-h-0 flex-1 overflow-hidden px-2">
        <Body clip={clip} />
        {job && <JobOverlay job={job} />}
      </div>
      {/* Fixed leading: the copy hint's fallback glyphs would otherwise grow the footer and nudge the body. */}
      <div
        className={cx(
          'border-divider relative z-1 flex justify-between gap-1.5 border-t px-2 py-1.5 text-[11px] leading-3 *:truncate',
          clip.missing ? 'text-error' : 'text-muted',
        )}
      >
        <span>{job?.state === 'done' ? job.folder : footLabel(clip, t)}</span>
        {selected && <span>{t.copyHint}</span>}
      </div>
      {confirming && (
        <div
          className="bg-confirm-veil absolute inset-0 z-2 flex flex-col items-center justify-center gap-3 p-3 text-center"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <p className="text-[14px] font-semibold">{t.deleteConfirm}</p>
          <div className="flex gap-2">
            <ConfirmButton onClick={onCancelDelete}>{t.cancel}</ConfirmButton>
            <ConfirmButton tone="danger" onClick={onConfirmDelete}>
              {t.delete}
            </ConfirmButton>
          </div>
          <span className="text-muted text-[11px]">{t.deleteHint}</span>
        </div>
      )}
    </div>
  );
}
