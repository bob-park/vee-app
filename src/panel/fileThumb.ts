// Keep in sync with IMAGE_EXTS in src-tauri/src/watcher.rs.
const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"];

/** "PNG" for an image file path, null otherwise. */
export function imageTag(path: string): string | null {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  return IMAGE_EXTS.includes(ext) ? ext.toUpperCase() : null;
}
