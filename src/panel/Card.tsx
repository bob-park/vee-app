import { useRef } from "react";
import type { Clip } from "../api.ts";
import type { Dict } from "../i18n/en.ts";
import { relativeTime } from "../i18n/index.ts";
import { usePrefs } from "../prefs.tsx";
import { DocIcon, FolderIcon } from "./FileIcon.tsx";
import { imageTag } from "./fileThumb.ts";

interface Props {
  clip: Clip;
  selected: boolean;
  onSelect: () => void;
  onCopy: () => void;
  /** Starts dragging the card out, from the card element and the cursor position. */
  onDragOut?: (card: HTMLElement, x: number, y: number) => void;
  confirming: boolean;
  onConfirmDelete: () => void;
  onCancelDelete: () => void;
}

const DRAG_THRESHOLD = 5;

function fileCount(clip: Clip): number {
  return Number(clip.meta ?? "1");
}

function badgeLabel(clip: Clip, t: Dict): string {
  switch (clip.kind) {
    case "files":
      return clip.isDir ? t.kinds.folder : t.filesCount(fileCount(clip));
    default:
      return t.kinds[clip.kind];
  }
}

function paths(clip: Clip): string[] {
  return (clip.textPreview ?? "").split("\n");
}

function firstPath(clip: Clip): string {
  return paths(clip)[0];
}

function footLabel(clip: Clip, t: Dict): string {
  if (clip.missing) return t.missing;
  switch (clip.kind) {
    case "image":
      return clip.meta ?? "";
    case "files": {
      const first = firstPath(clip);
      const name = first.split(/[\\/]/).filter(Boolean).pop() ?? first;
      return t.moreFiles(name, fileCount(clip) - 1);
    }
    default:
      return t.chars(clip.charCount);
  }
}

/** Tilt per layer, front (first file) first. */
const LAYER_TILT = [0, 5, -7];

function FilesBody({ clip }: { clip: Clip }) {
  const { stack } = clip;
  if (stack.length === 0) {
    return <div className="icon-box">{clip.isDir ? <FolderIcon /> : <DocIcon path={firstPath(clip)} />}</div>;
  }
  if (stack.length === 1) {
    const tag = imageTag(firstPath(clip));
    return (
      <>
        <img className="thumb" src={stack[0] ?? ""} alt="" />
        {tag && <span className="ext-tag">{tag}</span>}
      </>
    );
  }
  return (
    <div className="stack" aria-hidden>
      {stack.map((src, i) => {
        const style = { transform: `rotate(${LAYER_TILT[i]}deg)`, zIndex: LAYER_TILT.length - i };
        return src ? (
          <img key={i} src={src} alt="" style={style} />
        ) : (
          <div key={i} className="stack-doc" style={style}>
            <DocIcon path={paths(clip)[i] ?? ""} />
          </div>
        );
      })}
    </div>
  );
}

function Body({ clip }: { clip: Clip }) {
  if (clip.kind === "image") return clip.thumb ? <img className="thumb" src={clip.thumb} alt="" /> : null;
  if (clip.kind === "files") return <FilesBody clip={clip} />;
  return <p className={clip.kind === "link" ? "preview link" : "preview"}>{clip.textPreview}</p>;
}

export function Card({ clip, selected, onSelect, onCopy, onDragOut, confirming, onConfirmDelete, onCancelDelete }: Props) {
  const { t, locale } = usePrefs();
  const press = useRef<{ x: number; y: number } | null>(null);
  return (
    <div
      className={["card", selected && "selected", confirming && "confirming"].filter(Boolean).join(" ")}
      data-id={clip.id}
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      onDoubleClick={onCopy}
      onMouseDown={(e) => {
        press.current = onDragOut ? { x: e.clientX, y: e.clientY } : null;
      }}
      onMouseMove={(e) => {
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
      {clip.appIcon && <img className="card-bg" src={clip.appIcon} alt="" aria-hidden />}
      <div className="card-head">
        <span className="app-name">{clip.appName ?? ""}</span>
        <span className={clip.kind === "files" ? "badge badge-file" : "badge"}>{badgeLabel(clip, t)}</span>
        <span className="time">{relativeTime(clip.lastUsedAt, locale)}</span>
      </div>
      <div className="card-body">
        <Body clip={clip} />
      </div>
      <div className={clip.missing ? "card-foot missing" : "card-foot"}>
        <span>{footLabel(clip, t)}</span>
        {selected && <span>{t.copyHint}</span>}
      </div>
      {confirming && (
        <div
          className="confirm"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => e.stopPropagation()}
        >
          <p>{t.deleteConfirm}</p>
          <div className="confirm-btns">
            <button className="confirm-btn" onClick={onCancelDelete}>
              {t.cancel}
            </button>
            <button className="confirm-btn danger" onClick={onConfirmDelete}>
              {t.delete}
            </button>
          </div>
          <span className="confirm-hint">{t.deleteHint}</span>
        </div>
      )}
    </div>
  );
}
