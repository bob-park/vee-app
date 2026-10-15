import { type ReactNode, useRef } from 'react';

import type { Clip } from '@/domain/clip/apis/clip.dto';
import { relativeTime } from '@/shared/i18n';
import type { Dict } from '@/shared/i18n/en';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { imageTag } from '@/utils/fileThumb';

import cx from 'classnames';

import { DocIcon, FolderIcon } from './FileIcon';

interface Props {
  clip: Clip;
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
}

const DRAG_THRESHOLD = 5;

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

function Body({ clip }: { clip: Clip }) {
  if (clip.kind === 'image') return clip.thumb ? <img className={THUMB} src={clip.thumb} alt="" /> : null;
  if (clip.kind === 'files') return <FilesBody clip={clip} />;
  return (
    // Set apart from the card's header and footer.
    <p
      className={cx(
        'bg-subtle line-clamp-14 h-[calc(100%-8px)] rounded-lg px-2.5 py-2 text-[length:clamp(12px,calc(100vh/26),15px)] leading-[1.45] break-all whitespace-pre-wrap',
        { 'text-primary': clip.kind === 'link' },
      )}
    >
      {clip.textPreview}
    </p>
  );
}

export function Card({
  clip,
  selected,
  onSelect,
  onHover,
  onCopy,
  onTogglePin,
  onDragOut,
  confirming,
  onConfirmDelete,
  onCancelDelete,
}: Props) {
  // ref
  const press = useRef<{ x: number; y: number } | null>(null);

  // hooks
  const { t, locale } = useI18n();

  return (
    <div
      // `card` is the hook motion.ts finds cards by. `data-slot` (set by motion.ts) marks where a floating card came from.
      className={cx(
        'card group rounded-box bg-card relative flex w-[calc((100vh-72px)*0.95)] flex-none cursor-default flex-col overflow-hidden',
        'data-slot:border-accent-subtle data-slot:border-2 data-slot:border-dashed data-slot:bg-transparent data-slot:shadow-none data-slot:*:invisible',
        confirming
          ? 'border-error shadow-card-confirm border-2'
          : selected
            ? 'border-primary shadow-card-selected border'
            : cx('shadow-card border', clip.pinned ? 'border-pinned-border' : 'border-base-300'),
      )}
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
      </div>
      <div
        className={cx(
          'border-divider relative z-1 flex justify-between gap-1.5 border-t px-2 py-1.5 text-[11px] *:truncate',
          clip.missing ? 'text-error' : 'text-muted',
        )}
      >
        <span>{footLabel(clip, t)}</span>
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
