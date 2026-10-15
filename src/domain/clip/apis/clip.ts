import type { Clip, Filter, Stats } from "@/domain/clip/apis/clip.dto";
import call from "@/shared/api";

export async function listClips(query: string, kind: Filter, appId: number | null, offset: number, limit: number) {
  return call<Clip[]>("list_clips", { query, kind, appId, offset, limit });
}

export async function copyClip(id: number, plain = false) {
  return call<void>("copy_clip", { id, plain });
}

/** Pinned clips in pin order; searches and filters never apply. */
export async function listPinned() {
  return call<Clip[]>("list_pinned");
}

/** Rejects with code "PIN_LIMIT" when 3 clips are already pinned. */
export async function setPinned(id: number, pinned: boolean) {
  return call<void>("set_pinned", { id, pinned });
}

export async function getStats() {
  return call<Stats>("get_stats");
}

/** `image` is the dragged card as a base64 PNG (see cardImage.ts); without it the thumbnail is dragged. */
export async function startDrag(id: number, name: string, image: string | null) {
  return call<void>("start_drag", { id, name, image });
}

export async function deleteClip(id: number) {
  return call<void>("delete_clip", { id });
}

export async function clearHistory() {
  return call<void>("clear_history");
}
