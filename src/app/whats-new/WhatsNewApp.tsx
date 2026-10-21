import { type ReactNode, useEffect } from 'react';

import { dismissWhatsNew } from '@/domain/whats_new/apis/whatsNew';
import { useWhatsNew } from '@/domain/whats_new/queries/whatsNew';
import Aurora from '@/shared/components/Aurora';
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

/** Blocks as elements; each top-level item and its nested items form one glass card. */
function render(blocks: Block[]) {
  const out: ReactNode[] = [];
  let card: ReactNode[] = [];
  let cards = 0;
  const flush = () => {
    if (card.length)
      out.push(
        <ul
          key={out.length}
          className="glass rounded-box animate-spring-in my-2 list-none px-3.5 py-3"
          style={{ animationDelay: `${160 + cards++ * 60}ms` }}
        >
          {card}
        </ul>,
      );
    card = [];
  };
  blocks.forEach((b, i) => {
    if (b.kind === 'item') {
      if (b.depth === 0) flush();
      card.push(
        b.depth === 0 ? (
          <li key={i}>{inlines(b.inlines)}</li>
        ) : (
          <li key={i} className="mt-1 list-disc" style={{ marginLeft: b.depth * 16 }}>
            {inlines(b.inlines)}
          </li>
        ),
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
    <div className="relative flex h-full flex-col px-6 py-5">
      <Aurora className="inset-x-0 top-0 h-[110px]" />
      <header className="relative z-1">
        <span className="animate-pop bg-ok-bg text-ok-fg inline-block rounded-md px-2 py-0.5 text-[11px] font-semibold">
          v{data.version}
        </span>
        <h1 className="animate-spring-in mt-2 mb-1 text-[22px] font-bold tracking-[-0.5px] [animation-delay:80ms]">
          {t.whatsNew.title}
        </h1>
      </header>
      {/* Only the notes scroll; the title and button stay put. */}
      <div className="relative z-1 min-h-0 flex-1 overflow-y-auto leading-[1.45] select-text">
        {render(parseNotes(data.notes))}
      </div>
      <footer className="relative z-1 flex justify-end pt-4">
        <button
          className="rounded-box from-primary to-sky bg-linear-to-br px-4 py-2 font-semibold text-white transition-transform duration-150 active:scale-96 motion-reduce:transition-none"
          autoFocus
          onClick={() => void dismissWhatsNew()}
        >
          {t.whatsNew.ok}
        </button>
      </footer>
    </div>
  );
}
