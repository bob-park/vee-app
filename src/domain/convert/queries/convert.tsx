import { useEffect } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';

import { getFinderAccess, listConvertJobs } from '@/domain/convert/apis/convert';
import type { ConvertJob, FinderAccess } from '@/domain/convert/apis/convert.dto';

import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

const JOBS_QUERY_KEY = ['convert', 'jobs'];
const FINDER_ACCESS_QUERY_KEY = ['convert', 'finderAccess'];

const EMPTY = new Map<number, ConvertJob>();
const byClip = (jobs: ConvertJob[]) => new Map(jobs.map((j) => [j.clipId, j]));

/**
 * The panel's convert jobs keyed by clip id. Seeded on mount and on `panel://opened`
 * (events sent while the panel was hidden still arrive, but a reload may miss some),
 * then replaced by every `convert://jobs` payload.
 */
export function useConvertJobs() {
  const queryClient = useQueryClient();

  const { data } = useQuery<ConvertJob[], Error, Map<number, ConvertJob>>({
    queryKey: JOBS_QUERY_KEY,
    queryFn: listConvertJobs,
    select: byClip,
    staleTime: 0,
  });

  useEffect(() => {
    const offJobs = listen<ConvertJob[]>('convert://jobs', (e) => queryClient.setQueryData(JOBS_QUERY_KEY, e.payload));
    const offOpened = listen('panel://opened', () => void queryClient.invalidateQueries({ queryKey: JOBS_QUERY_KEY }));

    return () => {
      void offJobs.then((fn) => fn());
      void offOpened.then((fn) => fn());
    };
  }, [queryClient]);

  return { jobs: data ?? EMPTY };
}

/**
 * Finder access (macOS); null on Windows or while unknown. Rechecked on focus (back from
 * System Settings) and on `settings://changed` (the prompt was answered).
 */
export function useFinderAccess() {
  const queryClient = useQueryClient();

  const { data } = useQuery<FinderAccess | null>({
    queryKey: FINDER_ACCESS_QUERY_KEY,
    queryFn: getFinderAccess,
    staleTime: 0,
  });

  useEffect(() => {
    const refresh = () => void queryClient.invalidateQueries({ queryKey: FINDER_ACCESS_QUERY_KEY });
    const offFocus = getCurrentWindow().onFocusChanged((e) => e.payload && refresh());
    const offSettings = listen('settings://changed', refresh);

    return () => {
      void offFocus.then((fn) => fn());
      void offSettings.then((fn) => fn());
    };
  }, [queryClient]);

  return { access: data ?? null };
}
