import { useEffect } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';

import { getStats, listPinned } from '@/domain/clip/apis/clip';
import type { Clip, Stats } from '@/domain/clip/apis/clip.dto';

import { listen } from '@tauri-apps/api/event';

/** Invalidated by HistorySection on `clips://changed`, `clips://added` and `settings://changed`. */
export const STATS_QUERY_KEY = ['clip', 'stats'];

export function useStats() {
  const { data, isLoading } = useQuery<Stats>({
    queryKey: STATS_QUERY_KEY,
    queryFn: getStats,
    // Refetched on every mount: changes made while HistorySection was hidden have no listener.
    staleTime: 0,
  });

  return { stats: data, isLoading };
}

const PINNED_QUERY_KEY = ['clip', 'pinned'];

/** How many clips are pinned; refetched on `clips://changed`, which pinning and unpinning send. */
export function usePinnedCount() {
  const queryClient = useQueryClient();

  const { data } = useQuery<Clip[], Error, number>({
    queryKey: PINNED_QUERY_KEY,
    queryFn: listPinned,
    select: (clips) => clips.length,
    staleTime: 0,
  });

  useEffect(() => {
    const off = listen('clips://changed', () => void queryClient.invalidateQueries({ queryKey: PINNED_QUERY_KEY }));
    return () => void off.then((fn) => fn());
  }, [queryClient]);

  return { pinnedCount: data ?? 0 };
}
