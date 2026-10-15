import { type ReactNode, useEffect } from 'react';

import { dismissWhatsNew } from '@/domain/whats_new/apis/whatsNew';
import { useWhatsNew } from '@/domain/whats_new/queries/whatsNew';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { type Block, type Inline, parseNotes } from '@/utils/notes';

function inlines(list: Inline[]) {
  return list.map((s, i) =>
    s.kind === 'bold' ? (
      <b key={i}>{s.text}</b>
    ) : s.kind === 'code' ? (
      // Keycap look, like the panel's shortcut tooltips but on the window surface.
      <kbd
        key={i}
        className="border-base-300 bg-card rounded-[5px] border border-b-2 px-[5px] py-px font-sans text-[11px] whitespace-nowrap"
      >
        {s.text}
      </kbd>
    ) : (
      s.text
    ),
  );
}

/** Blocks as elements; runs of items share one list. */
function render(blocks: Block[]) {
  const out: ReactNode[] = [];
  let items: ReactNode[] = [];
  const flush = () => {
    if (items.length)
      out.push(
        <ul key={out.length} className="list-disc pl-4">
          {items}
        </ul>,
      );
    items = [];
  };
  blocks.forEach((b, i) => {
    if (b.kind === 'item') {
      items.push(
        <li key={i} className="my-1" style={{ marginLeft: b.depth * 16 }}>
          {inlines(b.inlines)}
        </li>,
      );
      return;
    }
    flush();
    out.push(
      b.kind === 'heading' ? (
        b.level === 2 ? (
          <h2 key={out.length} className="mt-3 mb-1 text-[14px] font-bold">
            {inlines(b.inlines)}
          </h2>
        ) : (
          <h3 key={out.length} className="text-muted mt-3.5 mb-1 text-[12px] font-semibold">
            {inlines(b.inlines)}
          </h3>
        )
      ) : (
        <p key={out.length} className="my-1">
          {inlines(b.inlines)}
        </p>
      ),
    );
  });
  flush();
  return out;
}

export function WhatsNewApp() {
  // hooks
  const { t } = useI18n();

  // queries
  const { whatsNew: data, isError } = useWhatsNew();

  // useEffect
  useEffect(() => {
    if (isError) void dismissWhatsNew();
  }, [isError]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Enter' && e.key !== 'Escape') return;
      // Keeps Enter from also clicking the focused button.
      e.preventDefault();
      void dismissWhatsNew();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  if (!data) return null;
  return (
    <div className="flex h-full flex-col px-6 py-5">
      <header>
        <span className="bg-ok-bg text-ok-fg inline-block rounded-md px-2 py-0.5 text-[11px] font-semibold">
          v{data.version}
        </span>
        <h1 className="mt-2 mb-1 text-[22px] font-bold tracking-[-0.5px]">{t.whatsNew.title}</h1>
      </header>
      {/* Only the notes scroll; the title and button stay put. */}
      <div className="min-h-0 flex-1 overflow-y-auto leading-[1.45] select-text">{render(parseNotes(data.notes))}</div>
      <footer className="flex justify-end pt-4">
        <button
          className="rounded-box bg-primary px-4 py-2 font-semibold text-white"
          autoFocus
          onClick={() => void dismissWhatsNew()}
        >
          {t.whatsNew.ok}
        </button>
      </footer>
    </div>
  );
}
