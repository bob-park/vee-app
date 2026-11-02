import type { Selection } from '@/domain/capture/apis/capture.dto';
import call from '@/shared/api';

/** Hides the overlay, grabs `selection` and copies it into the clipboard and history. */
export async function takeCapture(selection: Selection) {
  return call<void>('take_capture', { selection });
}

/** Hides the overlay without capturing. */
export async function cancelCapture() {
  return call<void>('cancel_capture');
}
