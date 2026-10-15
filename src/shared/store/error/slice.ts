import type { ErrorState } from '@/shared/store/error/error.state';
import type { BoundState } from '@/shared/store/rootStore';

import type { SlicePattern } from 'zustand';

const createErrorSlice: SlicePattern<ErrorState, BoundState> = (set) => ({
  error: undefined,
  showError: (error) =>
    set(
      (state) => {
        state.error = error;
      },
      false,
      'error/showError',
    ),
  dismissError: () =>
    set(
      (state) => {
        state.error = undefined;
      },
      false,
      'error/dismissError',
    ),
});

export default createErrorSlice;
