import { fileColor, fileExt } from "./fileThumb.ts";

/** A document with a folded corner and the file's extension on a coloured band. */
export function DocIcon({ path }: { path: string }) {
  const ext = fileExt(path);
  return (
    <svg className="file-icon" viewBox="0 0 52 64" aria-hidden>
      <path className="file-paper" d="M4 2h30l14 14v44a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z" />
      <path className="file-fold" d="M34 2v12a2 2 0 0 0 2 2h12" />
      {ext && (
        <>
          <rect x="6" y="38" width="40" height="16" rx="4" fill={fileColor(path)} />
          <text className="file-ext" x="26" y="50" textAnchor="middle">
            {ext}
          </text>
        </>
      )}
    </svg>
  );
}

export function FolderIcon() {
  return (
    <svg className="folder-icon" viewBox="0 0 70 54" aria-hidden>
      <path d="M4 6a4 4 0 0 1 4-4h16l6 6h32a4 4 0 0 1 4 4v36a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4z" fill="#4c9be8" />
      <path d="M4 16h62v32a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4z" fill="#6fb3f2" />
    </svg>
  );
}
