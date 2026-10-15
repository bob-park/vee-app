import call from "@/shared/api";

export async function hidePanel() {
  return call<void>("hide_panel");
}

/** A pinned panel stays up when focus leaves it; unpinning leaves it open. */
export async function setPanelPinned(pinned: boolean) {
  return call<void>("set_panel_pinned", { pinned });
}

/** `height` in CSS px; the backend clamps it and keeps the bottom edge put. Rejects with code "RESIZE_OFF". */
export async function resizePanel(height: number, save: boolean) {
  return call<void>("resize_panel", { height, save });
}

export async function resetPanelHeight() {
  return call<void>("reset_panel_height");
}

export async function revealPanel() {
  return call<void>("reveal_panel");
}

export async function openSettings() {
  return call<void>("open_settings");
}
