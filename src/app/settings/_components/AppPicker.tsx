import { type KeyboardEvent, useEffect, useRef, useState } from 'react';

import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import Suggest from '@/shared/components/suggest/Suggest';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { nextIndex } from '@/utils/dropdownKeys';

/** Search box with the panel's `@app` suggestion popup, for picking an app to exclude. */
export function AppPicker({ apps, onPick }: { apps: App[]; onPick: (app: App) => void }) {
  // ref
  const root = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // state
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [index, setIndex] = useState(0);

  // hooks
  const { t } = useI18n();

  const q = query.trim().toLowerCase();
  const matches = q ? apps.filter((a) => a.name.toLowerCase().includes(q)) : apps;

  // useEffect
  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    return () => document.removeEventListener('pointerdown', close);
  }, [open]);

  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: 'nearest' });
  }, [index, open]);

  // handle
  const pick = (app: App | undefined) => {
    if (!app) return;
    setQuery('');
    setOpen(false);
    onPick(app);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      setOpen(false);
    } else if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') setOpen(true);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      pick(matches[index]);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      setIndex((i) => nextIndex(e.key, i, matches.length) ?? i);
    }
  };

  return (
    <div className="rounded-box bg-subtle text-muted relative flex items-center gap-1.5 px-[10px] py-1.5" ref={root}>
      <span aria-hidden>🔍</span>
      <input
        className="text-base-content min-w-0 flex-1 outline-none"
        value={query}
        placeholder={t.settings.searchApps}
        aria-label={t.settings.searchApps}
        disabled={apps.length === 0}
        spellCheck={false}
        onFocus={() => setOpen(true)}
        onChange={(e) => {
          setQuery(e.target.value);
          setIndex(0);
          setOpen(true);
        }}
        onKeyDown={onKeyDown}
      />
      {open && (
        <Suggest
          apps={matches}
          index={index}
          query={query.trim()}
          listRef={listRef}
          className="max-h-[200px] w-full"
          onHover={setIndex}
          onPick={pick}
        />
      )}
    </div>
  );
}
