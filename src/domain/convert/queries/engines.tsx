import { useEffect } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';

import { getEngineStatus, getLibreOffice } from '@/domain/convert/apis/convert';
import type { EngineId, EngineStatus, EngineStatusEvent, LibreOffice } from '@/domain/convert/apis/convert.dto';

import { listen } from '@tauri-apps/api/event';

const engineQueryKey = (engine: EngineId) => ['engines', engine];

/** A downloaded engine; seeded on mount, then replaced by every `engines://status` payload for it. */
export function useEngineStatus(engine: EngineId) {
  const queryClient = useQueryClient();

  const { data } = useQuery<EngineStatus>({
    queryKey: engineQueryKey(engine),
    queryFn: () => getEngineStatus(engine),
    staleTime: 0,
  });

  useEffect(() => {
    const off = listen<EngineStatusEvent>('engines://status', (e) => {
      if (e.payload.engine === engine) queryClient.setQueryData(engineQueryKey(engine), e.payload.status);
    });

    return () => void off.then((fn) => fn());
  }, [queryClient, engine]);

  return { status: data ?? null };
}

/** The user's LibreOffice; `staleTime: 0` re-checks it each time the Convert tab mounts. */
export function useLibreOffice() {
  const { data, isLoading } = useQuery<LibreOffice>({
    queryKey: ['engines', 'libreoffice'],
    queryFn: getLibreOffice,
    staleTime: 0,
  });

  return { libreOffice: data ?? null, isLoading };
}
