// Keep in sync with IMAGE_EXTS in src-tauri/src/watcher.rs.
const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

/** Lower-cased extension of a path's file name; "" for none or dotfiles. */
function extOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** "PNG" for an image file path, null otherwise. */
export function imageTag(path: string): string | null {
  const ext = extOf(path);
  return IMAGE_EXTS.includes(ext) ? ext.toUpperCase() : null;
}

/** The label on a file icon: "PDF", "XLSX"; at most four characters, "" for none. */
export function fileExt(path: string): string {
  return extOf(path).slice(0, 4).toUpperCase();
}

const FILE_KINDS: [string, string[]][] = [
  ["#d1344b", ["pdf"]],
  ["#d9901e", ["zip", "rar", "7z", "tar", "gz", "dmg"]],
  ["#2f6fd6", ["doc", "docx", "pages", "rtf", "hwp"]],
  ["#149e61", ["xls", "xlsx", "csv", "numbers"]],
  ["#e8662c", ["ppt", "pptx", "key"]],
  ["#7132f5", ["js", "ts", "rs", "py", "json", "html", "css", "md", "txt"]],
  ["#1f9ea8", [...IMAGE_EXTS, "heic"]],
];
const OTHER_FILE = "#6b6f80";

/** Band colour of a file's icon, grouped by kind of file. */
export function fileColor(path: string): string {
  const ext = extOf(path);
  return FILE_KINDS.find(([, exts]) => exts.includes(ext))?.[0] ?? OTHER_FILE;
}

/** "Vee 2026-10-07 14.32.05" in local time — the name a dragged-out image is dropped as. */
export function dragFileName(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, "0");
  return `Vee ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
}
