import type { CaptureOpen, Selection } from '@/domain/capture/apis/capture.dto';
import call from '@/shared/api';

/** Claims the open overlay session, which makes Rust show the overlay; null when none is open. */
export async function getCaptureSession() {
  return call<CaptureOpen | null>('get_capture_session');
}

/** Hides the overlay, grabs `selection` and copies it into the clipboard and history. */
export async function takeCapture(selection: Selection) {
  return call<void>('take_capture', { selection });
}

/** Hides the overlay without capturing. */
export async function cancelCapture() {
  return call<void>('cancel_capture');
}
