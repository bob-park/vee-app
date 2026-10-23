import { useQuery } from '@tanstack/react-query';

import { getStats } from '@/domain/clip/apis/clip';
import type { Stats } from '@/domain/clip/apis/clip.dto';

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
