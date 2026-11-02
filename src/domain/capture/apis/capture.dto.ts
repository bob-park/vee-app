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
}
