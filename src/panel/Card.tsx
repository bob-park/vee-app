import type { Clip } from "../api.ts";
import type { Dict } from "../i18n/en.ts";
import { relativeTime } from "../i18n/index.ts";
import { usePrefs } from "../prefs.tsx";

interface Props {
  clip: Clip;
  selected: boolean;
  onSelect: () => void;
  onCopy: () => void;
}

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

function footLabel(clip: Clip, t: Dict): string {
  if (clip.missing) return t.missing;
  switch (clip.kind) {
    case "image":
      return clip.meta ?? "";
    case "files": {
      const first = (clip.textPreview ?? "").split("\n")[0];
      const name = first.split(/[\\/]/).filter(Boolean).pop() ?? first;
      return t.moreFiles(name, fileCount(clip) - 1);
    }
    default:
      return t.chars(clip.charCount);
  }
}

function Body({ clip }: { clip: Clip }) {
  if (clip.kind === "image") return clip.thumb ? <img className="thumb" src={clip.thumb} alt="" /> : null;
  if (clip.kind === "files") return <div className={clip.isDir ? "doc folder" : "doc"} aria-hidden />;
  return <p className={clip.kind === "link" ? "preview link" : "preview"}>{clip.textPreview}</p>;
}

export function Card({ clip, selected, onSelect, onCopy }: Props) {
  const { t, locale } = usePrefs();
  return (
    <div
      className={selected ? "card selected" : "card"}
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      onDoubleClick={onCopy}
    >
      <div className="card-head">
        {clip.appIcon ? (
          <img className="app-icon" src={clip.appIcon} alt={clip.appName ?? ""} title={clip.appName ?? ""} />
        ) : (
          <span className="app-icon app-icon-empty" aria-hidden />
        )}
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
    </div>
  );
}
