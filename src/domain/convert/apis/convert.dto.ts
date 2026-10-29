/** Target formats; mirrors Rust `Format` (serde names). */
export type Format =
  | 'png'
  | 'jpg'
  | 'webp'
  | 'heic'
  | 'gif'
  | 'ico'
  | 'icns'
  | 'pdf'
  | 'mp4'
  | 'webm'
  | 'mov'
  | 'mp3'
  | '720p'
  | 'hwp'
  | 'hwpx'
  | 'docx'
  | 'odt'
  | 'rtf'
  | 'txt'
  | 'html'
  | 'xlsx'
  | 'ods'
  | 'pptx'
  | 'odp'
  /** Not a format: the slice that opens a single image in the editor. */
  | 'edit';

/** One donut slice; mirrors Rust `Slice`. `needs` names the engine still missing for it. */
export interface Slice {
  format: Format;
  needs: 'ffmpeg' | 'rhwp' | 'libreoffice' | null;
}

/** Why a conversion failed; mirrors Rust `JobError`. */
export type ConvertError = 'missing' | 'engine' | 'write' | 'runtime';

/** What a toast says about converting; mirrors Rust `ConvertNote`. */
export type ConvertNote =
  | { type: 'saved'; format: Format; count: number; folder: string }
  | { type: 'failed'; name: string; error: ConvertError }
  | { type: 'notFolder'; finderBlocked: boolean }
  | { type: 'unsupported' };

/** One conversion in the panel's job list; mirrors Rust `JobView`. */
export interface ConvertJob {
  id: number;
  clipId: number;
  format: Format;
  state: 'queued' | 'running' | 'done' | 'failed';
  position: number;
  done: number;
  total: number;
  folder: string;
  error: ConvertError | null;
  /** Overall percent (0–99) of a running job whose current file reports progress. */
  percent: number | null;
  /** Seconds left, once past 3 %. */
  eta: number | null;
  /** A running video job; `cancelConvert` stops it. */
  cancellable: boolean;
}

/** Apple Events permission for Finder; mirrors `get_finder_access`. */
export type FinderAccess = 'granted' | 'denied' | 'unknown';

/** A downloadable engine; mirrors Rust `EngineId`. */
export type EngineId = 'ffmpeg' | 'rhwp';

/** Why the last engine download failed; mirrors Rust `InstallError`. */
export type InstallError = 'network' | 'checksum' | 'unpack' | 'disk';

/**
 * A downloaded engine; mirrors Rust `EngineStatus`. `error` is the last failed download, if any.
 * `downloadSize` and `update` are known once the latest release has been looked up.
 */
export type EngineStatus =
  | { state: 'missing'; error: InstallError | null; downloadSize: number | null }
  | { state: 'downloading'; received: number; total: number; speed: number; stage: 'download' | 'verify' | 'unpack' }
  | {
      state: 'installed';
      version: string;
      size: number;
      installedAt: number;
      path: string;
      update: boolean;
      error: InstallError | null;
    };

/** The `engines://status` payload; mirrors Rust `StatusEvent`. */
export interface EngineStatusEvent {
  engine: EngineId;
  status: EngineStatus;
}

/** The user's installed LibreOffice; mirrors Rust `LibreOffice`. `null` when it isn't found; `version` is `null` when it won't say. */
export type LibreOffice = { version: string | null } | null;

/** FFmpeg's H.264 encoder ids from Rust `HwEncoder`. */
export type HwEncoder = 'videotoolbox' | 'nvenc' | 'qsv' | 'amf';

/** Which encoder MP4 / MOV / 720p conversions use; mirrors Rust `VideoEncoder`. */
export interface VideoEncoder {
  /** `null` is software (CPU). */
  encoder: HwEncoder | null;
  /** The hardware encoder failed on the last job and it was re-encoded in software. */
  fellBack: boolean;
}
