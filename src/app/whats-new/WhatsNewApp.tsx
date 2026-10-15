import { type ReactNode, useEffect } from 'react';

import { dismissWhatsNew } from '@/domain/whats_new/apis/whatsNew';
import { useWhatsNew } from '@/domain/whats_new/queries/whatsNew';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { type Block, type Inline, parseNotes } from '@/utils/notes';

import './whatsnew.css';

function inlines(list: Inline[]) {
  return list.map((s, i) =>
    s.kind === 'bold' ? <b key={i}>{s.text}</b> : s.kind === 'code' ? <kbd key={i}>{s.text}</kbd> : s.text,
  );
}

/** Blocks as elements; runs of items share one list. */
function render(blocks: Block[]) {
  const out: ReactNode[] = [];
  let items: ReactNode[] = [];
  const flush = () => {
    if (items.length) out.push(<ul key={out.length}>{items}</ul>);
    items = [];
  };
  blocks.forEach((b, i) => {
    if (b.kind === 'item') {
      items.push(
        <li key={i} style={{ marginLeft: b.depth * 16 }}>
          {inlines(b.inlines)}
        </li>,
      );
      return;
    }
    flush();
    out.push(
      b.kind === 'heading' ? (
        b.level === 2 ? (
          <h2 key={out.length}>{inlines(b.inlines)}</h2>
        ) : (
          <h3 key={out.length}>{inlines(b.inlines)}</h3>
        )
      ) : (
        <p key={out.length}>{inlines(b.inlines)}</p>
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
    <div className="whatsnew">
      <header>
        <span className="whatsnew-version">v{data.version}</span>
        <h1>{t.whatsNew.title}</h1>
      </header>
      <div className="whatsnew-body">{render(parseNotes(data.notes))}</div>
      <footer>
        <button className="whatsnew-ok" autoFocus onClick={() => void dismissWhatsNew()}>
          {t.whatsNew.ok}
        </button>
      </footer>
    </div>
  );
}
