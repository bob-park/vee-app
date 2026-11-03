import type { RecordFormat } from '@/domain/settings/apis/settings.dto';

/** Mirrors `app::capture::{geometry, window_list}` (serde camelCase). */

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Global logical coordinates (points on macOS, DIPs on Windows). */
export interface MonitorInfo extends Rect {
  scale: number;
}

/** Monitor-local logical coordinates; lists come front to back. */
export interface WinInfo extends Rect {
  id: number;
  pid: number;
  app: string;
}

export type Selection = ({ kind: 'rect' } & Rect) | { kind: 'window'; id: number };

export type CaptureMode = 'capture' | 'record';

/** The open overlay session, from `get_capture_session` (`capture://open` only signals it). */
export interface CaptureOpen {
  mode: CaptureMode;
  monitor: MonitorInfo;
  windows: WinInfo[];
  /** Where the mouse is, if on this monitor. */
  cursor: Point | null;
}

/** The `recorder://started` payload and `get_recorder_state`; mirrors Rust `recorder::Started`. */
export interface RecorderStarted {
  /** Unix ms. */
  startedMs: number;
  format: RecordFormat;
  /** GIF's time limit; null for the other formats. */
  limitMs: number | null;
}
