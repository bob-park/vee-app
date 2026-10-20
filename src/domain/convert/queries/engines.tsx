import { useEffect } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';

import { getFfmpegStatus } from '@/domain/convert/apis/convert';
import type { FfmpegStatus } from '@/domain/convert/apis/convert.dto';

import { listen } from '@tauri-apps/api/event';

const FFMPEG_QUERY_KEY = ['engines', 'ffmpeg'];

/** The downloaded FFmpeg; seeded on mount, then replaced by every `engines://ffmpeg` payload. */
export function useFfmpegStatus() {
  const queryClient = useQueryClient();

  const { data } = useQuery<FfmpegStatus>({
    queryKey: FFMPEG_QUERY_KEY,
    queryFn: getFfmpegStatus,
    staleTime: 0,
  });

  useEffect(() => {
    const off = listen<FfmpegStatus>('engines://ffmpeg', (e) => queryClient.setQueryData(FFMPEG_QUERY_KEY, e.payload));

    return () => void off.then((fn) => fn());
  }, [queryClient]);

  return { ffmpeg: data ?? null };
}
