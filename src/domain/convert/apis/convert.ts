import type { ConvertJob, Format } from '@/domain/convert/apis/convert.dto';
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

/** Removes a queued or failed job. */
export async function dismissConvert(id: number) {
  return call<void>('dismiss_convert', { id });
}
