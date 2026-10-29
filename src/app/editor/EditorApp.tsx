import { useEffect, useRef, useState } from 'react';

import CropBox from '@/app/editor/_components/CropBox';
import Footer from '@/app/editor/_components/Footer';
import ResizeForm from '@/app/editor/_components/ResizeForm';
import RotatePanel from '@/app/editor/_components/RotatePanel';
import { closeEditor, copyEdit, getEditSource, renderEdit } from '@/domain/edit/apis/edit';
import type { EditImage, EditOp } from '@/domain/edit/apis/edit.dto';
import Aurora from '@/shared/components/Aurora';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { type Size, sizeAfter } from '@/utils/edit';
import { errorText } from '@/utils/errorText';

import { listen } from '@tauri-apps/api/event';

/** The editor's tools. `removeBg` and `upscale` show an empty panel until their AI tools land. */
export type EditorTab = 'crop' | 'rotate' | 'resize' | 'removeBg' | 'upscale';

const TABS: EditorTab[] = ['crop', 'rotate', 'resize', 'removeBg', 'upscale'];

const isMac = navigator.userAgent.includes('Mac');

const sizeOf = (img: EditImage): Size => ({ w: img.width, h: img.height });

export function EditorApp() {
  // ref
  const stageRef = useRef<HTMLDivElement>(null);

  // state
  const [source, setSource] = useState<EditImage | null>(null);
  /** The preview of `ops`: what the tools work on. */
  const [preview, setPreview] = useState<EditImage | null>(null);
  /** Applied steps, oldest first. */
  const [ops, setOps] = useState<EditOp[]>([]);
  /** The current tab's step, not applied yet (a drawn crop, a typed size). Copy includes it. */
  const [pending, setPending] = useState<EditOp | null>(null);
  const [tab, setTab] = useState<EditorTab>('crop');
  const [stage, setStage] = useState<Size>({ w: 0, h: 0 });
  const [error, setError] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);

  // hooks
  const { t } = useI18n();

  // useEffect
  useEffect(() => {
    const load = () =>
      void getEditSource()
        .then((img) => {
          setSource(img);
          setPreview(img);
          setOps([]);
          setPending(null);
          setTab('crop');
          setError(null);
        })
        .catch((e) => setError(errorText(e)));
    // The window may have been opened before this listener existed, so load once on mount too.
    load();
    const off = listen('editor://opened', load);
    return () => void off.then((f) => f());
  }, []);

  useEffect(() => {
    if (!source) return;
    if (ops.length === 0) {
      setPreview(source);
      return;
    }
    let stale = false;
    renderEdit(ops)
      .then((img) => {
        if (stale) return;
        setPreview(img);
        setError(null);
      })
      .catch((e) => {
        if (!stale) setError(errorText(e));
      });
    return () => {
      stale = true;
    };
  }, [ops, source]);

  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) =>
      setStage({ w: entry.contentRect.width, h: entry.contentRect.height }),
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Re-subscribed every render so the keys always see the current ops.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = isMac ? e.metaKey : e.ctrlKey;
      if (e.key === 'Escape') handleCancel();
      else if (mod && e.key === 'Enter') handleCopy();
      // Inside an input, ⌘Z undoes the typing instead.
      else if (mod && e.code === 'KeyZ' && !(e.target instanceof HTMLInputElement)) handleUndo();
      else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  // handle
  const all = pending ? [...ops, pending] : ops;
  // An unchanged image would only bump the original clip (same hash), so there is nothing to copy.
  const canCopy = !!source && all.length > 0 && !copying;

  const handleCancel = () => void closeEditor();

  const handleCopy = () => {
    if (!canCopy) return;
    setCopying(true);
    void copyEdit(all)
      .catch((e) => setError(errorText(e)))
      .finally(() => setCopying(false));
  };

  const handleUndo = () => {
    setPending(null);
    setOps((cur) => cur.slice(0, -1));
  };

  const handleTab = (next: EditorTab) => {
    setPending(null);
    setTab(next);
  };

  const apply = (op: EditOp | null) => {
    if (!op) return;
    setPending(null);
    setOps((cur) => [...cur, op]);
  };

  const image = preview && sizeOf(preview);
  // Fit the full-res aspect into the stage, never above 1 CSS px per image px.
  const fit = image && Math.min(stage.w / image.w, stage.h / image.h, 1);
  const view = image && fit ? { w: image.w * fit, h: image.h * fit } : null;
  const crop = pending?.type === 'crop' ? pending : null;

  return (
    <main className="relative flex h-full flex-col">
      <Aurora className="inset-x-0 top-0 h-[110px]" />
      {/* macOS overlays the traffic lights on this row (`titleBarStyle: Overlay`); Windows keeps its native bar. */}
      {isMac && <div className="relative z-1 h-7 shrink-0" data-tauri-drag-region />}
      <div className="relative z-1 flex shrink-0 justify-center pt-1 pb-3">
        <div className="glass rounded-xl p-0.5">
          <Segmented
            value={tab}
            onChange={handleTab}
            options={TABS.map((value) => ({ value, label: t.editor.tabs[value] }))}
          />
        </div>
      </div>
      <div ref={stageRef} className="relative z-1 mx-5 flex min-h-0 flex-1 items-center justify-center">
        {preview && image && view && (
          <div className="relative" style={{ width: view.w, height: view.h }}>
            <img src={preview.dataUrl} alt="" draggable={false} className="size-full rounded-md select-none" />
            {tab === 'crop' && (
              <CropBox
                view={view}
                image={image}
                rect={crop}
                onChange={(rect) => setPending(rect && { type: 'crop', ...rect })}
              />
            )}
          </div>
        )}
      </div>
      <div className="relative z-1 flex h-16 shrink-0 items-center justify-center px-5">
        {tab === 'crop' && (
          <div className="flex items-center gap-3">
            <span className="text-muted text-[12px]">{t.editor.cropHint}</span>
            <button
              className="rounded-box bg-accent-subtle text-primary px-3.5 py-1.5 text-[12px] font-semibold disabled:cursor-default disabled:opacity-50"
              disabled={!crop}
              onClick={() => apply(crop)}
            >
              {t.editor.apply}
            </button>
          </div>
        )}
        {tab === 'rotate' && <RotatePanel onRotate={(quarter) => apply({ type: 'rotate', quarter })} />}
        {tab === 'resize' && image && (
          <ResizeForm
            key={`${image.w}x${image.h}`}
            size={image}
            onPending={setPending}
            onApply={() => apply(pending)}
          />
        )}
        {/* removeBg / upscale: filled in with the AI tools. */}
      </div>
      <Footer
        from={source ? sizeOf(source) : { w: 0, h: 0 }}
        to={source ? sizeAfter(sizeOf(source), all) : { w: 0, h: 0 }}
        error={error}
        canCopy={canCopy}
        onCancel={handleCancel}
        onCopy={handleCopy}
      />
    </main>
  );
}
