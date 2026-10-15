import { useEffect } from 'react';

import { useQuery, useQueryClient } from '@tanstack/react-query';

import { getUpdateStatus } from '@/domain/updater/apis/updater';
import type { UpdateStatus } from '@/domain/updater/apis/updater.dto';

import { listen } from '@tauri-apps/api/event';

const STATUS_QUERY_KEY = ['updater', 'status'];

/** Current status, kept live by the `update://status` event (also fired by tray-initiated checks). */
export function useUpdateStatus() {
  const queryClient = useQueryClient();

  const { data } = useQuery<UpdateStatus>({
    queryKey: STATUS_QUERY_KEY,
    queryFn: getUpdateStatus,
    // Refetched on every mount: a tray-initiated check while UpdateRow was hidden had no listener.
    staleTime: 0,
  });

  useEffect(() => {
    const unlisten = listen<UpdateStatus>('update://status', (e) =>
      queryClient.setQueryData(STATUS_QUERY_KEY, e.payload),
    );

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  return { status: data ?? ({ status: 'idle' } as UpdateStatus) };
}
