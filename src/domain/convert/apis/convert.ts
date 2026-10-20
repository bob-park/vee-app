import type { ConvertJob, EngineId, EngineStatus, FinderAccess, Format } from '@/domain/convert/apis/convert.dto';
import call from '@/shared/api';

/** Converts the dragged clip into the folder under the drop; rejects when no drop is pending. */
export async function convertDrop(format: Format) {
  return call<void>('convert_drop', { format });
}

export async function listConvertJobs() {
  return call<ConvertJob[]>('list_convert_jobs');
}

/** Requeues a failed job. */
export async function retryConvert(id: number) {
  return call<void>('retry_convert', { id });
}

/** Stops a running video job; it disappears without a toast. */
export async function cancelConvert(id: number) {
  return call<void>('cancel_convert', { id });
}

/** Removes a queued or failed job. */
export async function dismissConvert(id: number) {
  return call<void>('dismiss_convert', { id });
}

/** Whether Vee may ask Finder for its open folder (macOS); null where the permission doesn't exist (Windows). */
export async function getFinderAccess() {
  return call<FinderAccess | null>('get_finder_access');
}

/** Shows the system prompt; `settings://changed` follows once it is answered. */
export async function requestFinderAccess() {
  return call<void>('request_finder_access');
}

export async function openAutomationSettings() {
  return call<void>('open_automation_settings');
}

export async function getEngineStatus(engine: EngineId) {
  return call<EngineStatus>('get_engine_status', { engine });
}

/** Downloads (or updates) the engine; progress arrives on `engines://status`. */
export async function downloadEngine(engine: EngineId) {
  return call<void>('download_engine', { engine });
}

export async function cancelEngineDownload(engine: EngineId) {
  return call<void>('cancel_engine_download', { engine });
}

export async function removeEngine(engine: EngineId) {
  return call<void>('remove_engine', { engine });
}
