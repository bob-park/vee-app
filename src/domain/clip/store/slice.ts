import type { ClipState } from '@/domain/clip/store/clip.state';
import type { BoundState } from '@/shared/store/rootStore';

import type { SlicePattern } from 'zustand';

const createClipSlice: SlicePattern<ClipState, BoundState> = (set) => ({
  query: '',
  filter: 'all',
  app: null,
  setQuery: (query) =>
    set(
      (state) => {
        state.query = query;
      },
      false,
      'clip/setQuery',
    ),
  setFilter: (filter) =>
    set(
      (state) => {
        state.filter = filter;
      },
      false,
      'clip/setFilter',
    ),
  setApp: (app) =>
    set(
      (state) => {
        state.app = app;
      },
      false,
      'clip/setApp',
    ),
  resetSearch: () =>
    set(
      (state) => {
        state.query = '';
        state.app = null;
        state.filter = 'all';
      },
      false,
      'clip/resetSearch',
    ),
});

export default createClipSlice;
