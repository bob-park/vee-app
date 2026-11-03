import { useEffect } from 'react';

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import {
  countPrunable,
  getDiskAccess,
  getScreenAccess,
  getSettings,
  getShortcutFailures,
  setSetting,
} from '@/domain/settings/apis/settings';
import type { GlobalShortcutKey, Retention, SettingKey, Settings } from '@/domain/settings/apis/settings.dto';
import type { AppError } from '@/shared/api/common.dto';
import type { QueryMutationHandle } from '@/shared/queries';

import { getCurrentWindow } from '@tauri-apps/api/window';

/** Seeded by main.tsx before the first paint; refetched on `settings://changed` (ThemeProvider). */
export const SETTINGS_QUERY_KEY = ['settings'];
const DISK_ACCESS_QUERY_KEY = ['settings', 'diskAccess'];
const SCREEN_ACCESS_QUERY_KEY = ['settings', 'screenAccess'];
/** Under SETTINGS_QUERY_KEY, so `settings://changed` (a successful shortcut save) refetches it too. */
const SHORTCUT_FAILURES_QUERY_KEY = ['settings', 'shortcutFailures'];

export function useSettings() {
  const { data, isLoading } = useQuery<Settings>({
    queryKey: SETTINGS_QUERY_KEY,
    queryFn: getSettings,
  });

  return { settings: data, isLoading };
}

export function useSettingUpdate({ onSuccess, onError }: QueryMutationHandle<void> = {}) {
  const { mutate, isPending } = useMutation<void, AppError, { key: SettingKey; value: string }>({
    mutationKey: ['settings', 'update'],
    mutationFn: ({ key, value }) => setSetting(key, value),
    // set_setting emits `settings://changed`, which refetches the settings in every window.
    onSuccess: () => {
      onSuccess?.();
    },
    onError: (err) => {
      onError?.(err);
    },
  });

  return { updateSetting: mutate, isLoading: isPending };
}

/**
 * Full Disk Access (macOS); null where the permission doesn't exist (Windows) or while unknown.
 * Re-checked on focus so it updates after the user returns from System Settings.
 */
export function useDiskAccess() {
  const queryClient = useQueryClient();

  const { data } = useQuery<boolean | null>({
    queryKey: DISK_ACCESS_QUERY_KEY,
    queryFn: getDiskAccess,
    // Rechecked on every mount: a grant made while the row was hidden must show.
    staleTime: 0,
  });

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(
      (e) => e.payload && void queryClient.invalidateQueries({ queryKey: DISK_ACCESS_QUERY_KEY }),
    );

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  return { granted: data ?? null };
}

/**
 * Screen & System Audio Recording (macOS); null where the permission doesn't exist (Windows) or while unknown.
 * Re-checked on focus, though macOS only reports a new grant after Vee relaunches.
 */
export function useScreenAccess() {
  const queryClient = useQueryClient();

  const { data } = useQuery<boolean | null>({
    queryKey: SCREEN_ACCESS_QUERY_KEY,
    queryFn: getScreenAccess,
    staleTime: 0,
  });

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(
      (e) => e.payload && void queryClient.invalidateQueries({ queryKey: SCREEN_ACCESS_QUERY_KEY }),
    );

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  return { granted: data ?? null };
}

/** Global shortcuts that failed to register; re-checked on focus and after every settings change. */
export function useShortcutFailures() {
  const queryClient = useQueryClient();

  const { data } = useQuery<GlobalShortcutKey[]>({
    queryKey: SHORTCUT_FAILURES_QUERY_KEY,
    queryFn: getShortcutFailures,
    staleTime: 0,
  });

  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(
      (e) => e.payload && void queryClient.invalidateQueries({ queryKey: SHORTCUT_FAILURES_QUERY_KEY }),
    );

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [queryClient]);

  return { failures: data ?? [] };
}

/** How many clips a retention value would delete right away; always recounted, never cached. */
export function usePrunableCount() {
  const queryClient = useQueryClient();

  const fetchPrunableCount = (value: Retention) =>
    queryClient.fetchQuery({
      queryKey: ['settings', 'prunable', value],
      queryFn: () => countPrunable(value),
      staleTime: 0,
    });

  return { fetchPrunableCount };
}
