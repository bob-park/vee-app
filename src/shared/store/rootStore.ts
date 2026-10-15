import type { ClipState } from '@/domain/clip/store/clip.state';
import createClipSlice from '@/domain/clip/store/slice';

import { create } from 'zustand';
import { devtools } from 'zustand/middleware';
import { immer } from 'zustand/middleware/immer';

// Register slices here: spread `createXxxSlice(...a)` and add `XxxState` to BoundState.
export const useStore = create<BoundState>()(
  devtools(
    immer((...a) => ({
      ...createClipSlice(...a),
    })),
    { name: 'vee', enabled: import.meta.env.DEV },
  ),
);

export type BoundState = ClipState;
