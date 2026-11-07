import type { CaptureMode, CaptureOpen, RecorderStarted, Selection } from '@/domain/capture/apis/capture.dto';
import type { RecordFormat } from '@/domain/settings/apis/settings.dto';
import call from '@/shared/api';

/** Claims the open overlay session, which makes Rust show the overlay; null when none is open. */
export async function getCaptureSession() {
  return call<CaptureOpen | null>('get_capture_session');
}

/** Hides the overlay, grabs `selection` and copies it into the clipboard and history. */
export async function takeCapture(selection: Selection) {
  return call<void>('take_capture', { selection });
}

/** Switches the open overlay between capture and record; returns the mode now in effect. */
export async function setCaptureMode(mode: CaptureMode) {
  return call<CaptureMode>('set_capture_mode', { mode });
}

/** Makes this overlay show its mode's cursor now; macOS otherwise waits for the mouse to move. */
export async function refreshCursor() {
  return call<void>('refresh_cursor');
}

/** Hides the overlay without capturing. */
export async function cancelCapture() {
  return call<void>('cancel_capture');
}

/** Hides the record overlay and records `selection` as `format`. */
export async function startRecording(selection: Selection, format: RecordFormat) {
  return call<void>('start_recording', { selection, format });
}

/** The running recording, or null; for a pill that missed `recorder://started`. */
export async function getRecorderState() {
  return call<RecorderStarted | null>('get_recorder_state');
}

/** Stops and saves the recording. */
export async function stopRecording() {
  return call<void>('stop_recording');
}

/** Stops the recording and drops the file. */
export async function cancelRecording() {
  return call<void>('cancel_recording');
}
