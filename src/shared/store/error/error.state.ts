import type { AppError } from '@/shared/api/common.dto';

export interface ErrorState {
  /** The last failed action. No UI reads it yet: each screen shows its errors where it always has. */
  error?: AppError;
  showError: (error: AppError) => void;
  dismissError: () => void;
}
