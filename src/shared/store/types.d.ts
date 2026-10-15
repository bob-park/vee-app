import { StateCreator } from 'zustand';

declare module 'zustand' {
  type SlicePattern<T, S = T> = StateCreator<S & T, [['zustand/immer', never], ['zustand/devtools', never]], [], T>;
}
