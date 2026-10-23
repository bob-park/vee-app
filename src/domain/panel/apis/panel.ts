import call from '@/shared/api';

export async function hidePanel() {
  return call<void>('hide_panel');
}

/** A pinned panel stays up when focus leaves it; unpinning leaves it open. */
export async function setPanelPinned(pinned: boolean) {
  return call<void>('set_panel_pinned', { pinned });
}

/**
 * `size` is the height (top/bottom) or width (left/right) in CSS px; the backend clamps it and keeps the
 * screen-side edge put. Rejects with code "RESIZE_OFF".
 */
export async function resizePanel(size: number, save: boolean) {
  return call<void>('resize_panel', { size, save });
}

/** Forgets the saved size of the current position's axis. */
export async function resetPanelSize() {
  return call<void>('reset_panel_size');
}

export async function revealPanel() {
  return call<void>('reveal_panel');
}

export async function openSettings() {
  return call<void>('open_settings');
}
