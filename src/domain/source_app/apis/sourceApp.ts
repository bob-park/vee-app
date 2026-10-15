import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import call from '@/shared/api';

export async function listApps(query: string) {
  return call<App[]>('list_apps', { query });
}

/** Apps that aren't excluded, including ones with no clips left. */
export async function listKnownApps() {
  return call<App[]>('list_known_apps');
}

export async function countAppClips(appId: number) {
  return call<number>('count_app_clips', { appId });
}

export async function setAppExcluded(appId: number, excluded: boolean) {
  return call<void>('set_app_excluded', { appId, excluded });
}

export async function listExcludedApps() {
  return call<App[]>('list_excluded_apps');
}
