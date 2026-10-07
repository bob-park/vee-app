// Keep in sync with IMAGE_EXTS in src-tauri/src/watcher.rs.
const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

/** "PNG" for an image file path, null otherwise. */
export function imageTag(path: string): string | null {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  return IMAGE_EXTS.includes(ext) ? ext.toUpperCase() : null;
}

/** "Vee 2026-10-07 14.32.05" in local time — the name a dragged-out image is dropped as. */
export function dragFileName(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, "0");
  return `Vee ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
}
