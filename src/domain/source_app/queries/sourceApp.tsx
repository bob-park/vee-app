import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';

import { countAppClips, listApps, listExcludedApps, listKnownApps } from '@/domain/source_app/apis/sourceApp';
import type { App } from '@/domain/source_app/apis/sourceApp.dto';

/** Invalidated by HistorySection on `clips://changed`, `clips://added` and `settings://changed`. */
export const KNOWN_APPS_QUERY_KEY = ['sourceApp', 'known'];
export const EXCLUDED_APPS_QUERY_KEY = ['sourceApp', 'excluded'];

/**
 * The panel's `@app` suggestions. While the list is open the last results stay up until the next
 * ones arrive; the closed state caches `[]`, so reopening never shows a previous search's apps.
 */
export function useAppSuggestions(query: string, open: boolean) {
  const { data } = useQuery<App[]>({
    queryKey: ['sourceApp', 'suggest', open ? query : null],
    queryFn: () => listApps(query),
    enabled: open,
    initialData: open ? undefined : [],
    placeholderData: keepPreviousData,
    staleTime: 0,
  });

  return { suggestions: data ?? [] };
}

/** Apps that aren't excluded, including ones with no clips left. */
export function useKnownApps() {
  const { data, isLoading } = useQuery<App[]>({
    queryKey: KNOWN_APPS_QUERY_KEY,
    queryFn: listKnownApps,
    // Refetched on every mount: changes made while HistorySection was hidden have no listener.
    staleTime: 0,
  });

  return { apps: data ?? [], isLoading };
}

export function useExcludedApps() {
  const { data, isLoading } = useQuery<App[]>({
    queryKey: EXCLUDED_APPS_QUERY_KEY,
    queryFn: listExcludedApps,
    staleTime: 0,
  });

  return { excluded: data ?? [], isLoading };
}

/** How many clips an app has; always recounted, never cached. */
export function useAppClipCount() {
  const queryClient = useQueryClient();

  const fetchAppClipCount = (appId: number) =>
    queryClient.fetchQuery({
      queryKey: ['sourceApp', 'clipCount', appId],
      queryFn: () => countAppClips(appId),
      staleTime: 0,
    });

  return { fetchAppClipCount };
}
