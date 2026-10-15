import { useQuery } from '@tanstack/react-query';

import { getWhatsNew } from '@/domain/whats_new/apis/whatsNew';
import type { WhatsNew } from '@/domain/whats_new/apis/whatsNew.dto';

export function useWhatsNew() {
  const { data, isError } = useQuery<WhatsNew | null>({
    queryKey: ['whatsNew'],
    queryFn: getWhatsNew,
    // A failure dismisses the window right away, as before.
    retry: false,
  });

  return { whatsNew: data ?? null, isError };
}
