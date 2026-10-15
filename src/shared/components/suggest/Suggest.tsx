import type { Ref } from 'react';

import type { App } from '@/domain/source_app/apis/sourceApp.dto';
import { highlight } from '@/shared/components/highlight';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';

import cx from 'classnames';

interface SuggestProps {
  apps: App[];
  /** The highlighted row. */
  index: number;
  /** The part of each app name to bold. */
  query: string;
  listRef: Ref<HTMLDivElement>;
  /** Width and max height; they differ between the panel and settings. */
  className: string;
  onHover?: (index: number) => void;
  onPick: (app: App) => void;
}

/** `@app` suggestion popup, shared by the panel search and the settings app picker. */
export default function Suggest({ apps, index, query, listRef, className, onHover, onPick }: SuggestProps) {
  // hooks
  const { t } = useI18n();

  return (
    // The list scrolls; the hint below it always stays visible.
    <div
      className={cx(
        'rounded-box border-base-300 bg-card text-base-content absolute top-[calc(100%+6px)] left-0 z-3 flex flex-col border p-1 [box-shadow:rgba(0,0,0,0.12)_0_8px_24px]',
        className,
      )}
    >
      {apps.length === 0 ? (
        <p className="text-muted px-2 py-1.5">{t.noApps}</p>
      ) : (
        <div
          className="min-h-0 [scrollbar-width:none] overflow-y-auto [&::-webkit-scrollbar]:hidden"
          role="listbox"
          ref={listRef}
        >
          {apps.map((a, i) => (
            <div
              key={a.id}
              role="option"
              aria-selected={i === index}
              className={cx('flex items-center gap-2 rounded-lg px-2 py-1.5', { 'bg-accent-subtle': i === index })}
              onPointerEnter={onHover && (() => onHover(i))}
              onClick={() => onPick(a)}
            >
              {a.icon ? (
                <img className="size-5 flex-none rounded-[5px]" src={a.icon} alt="" />
              ) : (
                <span className="bg-subtle border-base-300 size-5 flex-none rounded-[5px] border" />
              )}
              <span className="[&_b]:text-primary min-w-0 flex-1 truncate">{highlight(a.name, query)}</span>
              <span className="text-muted text-[11px]">{a.count}</span>
            </div>
          ))}
        </div>
      )}
      <p className="border-divider text-muted mt-1 flex-none border-t px-2 pt-1.5 pb-1 text-[11px]">{t.suggestHint}</p>
    </div>
  );
}
