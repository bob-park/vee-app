import type { AppError } from '@/shared/api/common.dto';

interface QueryMutationHandle<T> {
  onSuccess?: (data: T) => void;
  onError?: (err: AppError) => void;
}

export type { QueryMutationHandle };
