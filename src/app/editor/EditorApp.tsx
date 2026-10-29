import { useEffect, useRef, useState } from 'react';

import CompareView from '@/app/editor/_components/CompareView';
import CropBox from '@/app/editor/_components/CropBox';
import Footer from '@/app/editor/_components/Footer';
import ModelPrompt from '@/app/editor/_components/ModelPrompt';
import ResizeForm from '@/app/editor/_components/ResizeForm';
import RotatePanel from '@/app/editor/_components/RotatePanel';
import { TOOL_BUTTON } from '@/app/editor/_components/button';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import { closeEditor, copyEdit, getEditSource, renderEdit } from '@/domain/edit/apis/edit';
import type { EditImage, EditOp } from '@/domain/edit/apis/edit.dto';
import Aurora from '@/shared/components/Aurora';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import { type Size, previewScale, sizeAfter } from '@/utils/edit';
import { errorText } from '@/utils/errorText';

import { listen } from '@tauri-apps/api/event';
import cx from 'classnames';

/** The editor's tools. `removeBg` and `upscale` show their result next to the original before it's applied. */
export type EditorTab = 'crop' | 'rotate' | 'resize' | 'removeBg' | 'upscale';

const TABS: EditorTab[] = ['crop', 'rotate', 'resize', 'removeBg', 'upscale'];

const SCALES = ['2', '4'] as const;
// A fresh object each time: a shown AI result is matched to its step by identity.
const upscaleOp = (k: (typeof SCALES)[number]): EditOp => ({ type: 'upscale', scale: k === '4' ? 4 : 2 });
const ZOOMS = ['fit', 'actual'] as const;

const isMac = navigator.userAgent.includes('Mac');

const sizeOf = (img: EditImage): Size => ({ w: img.width, h: img.height });

export function EditorApp() {
  // ref
  const stageRef = useRef<HTMLDivElement>(null);

  // state
  const [source, setSource] = useState<EditImage | null>(null);
  /** The last finished render and the `ops` it shows; the tools work on it. */
  const [preview, setPreview] = useState<{ img: EditImage; ops: EditOp[] } | null>(null);
  /** Applied steps, oldest first. */
  const [ops, setOps] = useState<EditOp[]>([]);
  /** The current tab's step, not applied yet (a drawn crop, a typed size). Copy includes it. */
  const [pending, setPending] = useState<EditOp | null>(null);
  const [tab, setTab] = useState<EditorTab>('crop');
  const [stage, setStage] = useState<Size>({ w: 0, h: 0 });
  const [error, setError] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);
  /** The render of `[...ops, pending]` for an AI step, shown beside the preview before it's applied. */
  const [result, setResult] = useState<{ img: EditImage; op: EditOp } | null>(null);
  /** The last AI step failed to render; its tab offers to try again. */
  const [aiFailed, setAiFailed] = useState(false);
  const [scale, setScale] = useState<(typeof SCALES)[number]>('2');
  const [zoom, setZoom] = useState<(typeof ZOOMS)[number]>('fit');

  // hooks
  const { t } = useI18n();

  // queries
  const { status: bgStatus } = useEngineStatus('bgremove');

  const bgReady = bgStatus?.state === 'installed';
  const opsShown = preview?.ops === ops;
  const aiPending = pending?.type === 'removeBg' || pending?.type === 'upscale' ? pending : null;
  const shown = aiPending && result?.op === aiPending ? result.img : null;

  // useEffect
  useEffect(() => {
    const load = () =>
      void getEditSource()
        .then((img) => {
          setSource(img);
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
      setPreview({ img: source, ops });
      return;
    }
    let stale = false;
    renderEdit(ops)
      .then((img) => {
        if (!stale) setPreview({ img, ops });
      })
      .catch((e) => {
        if (stale) return;
        // Roll the failed step back so `ops` never runs ahead of the preview.
        setError(errorText(e));
        setOps(ops.slice(0, -1));
      });
    return () => {
      stale = true;
    };
  }, [ops, source]);

  // The model may be downloaded or removed in Settings while this tab is open.
  useEffect(() => {
    if (tab === 'removeBg') setPending(bgReady ? { type: 'removeBg' } : null);
  }, [tab, bgReady]);

  // An AI step renders once the steps before it have; a failure drops only the step, never `ops`.
  useEffect(() => {
    if (!aiPending || !opsShown) return;
    setAiFailed(false);
    let stale = false;
    renderEdit([...ops, aiPending])
      .then((img) => {
        if (!stale) setResult({ img, op: aiPending });
      })
      .catch((e) => {
        if (stale) return;
        setError(errorText(e));
        setAiFailed(true);
        setPending(null);
      });
    return () => {
      stale = true;
    };
  }, [aiPending, ops, opsShown]);

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
      else if (mod && !e.shiftKey && e.code === 'KeyZ' && !(e.target instanceof HTMLInputElement)) handleUndo();
      else return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  // handle
  const handleCancel = () => void closeEditor();

  const handleCopy = () => {
    if (!canCopy) return;
    setCopying(true);
    void copyEdit(all)
      .catch((e) => setError(errorText(e)))
      .finally(() => setCopying(false));
  };

  /** The step an AI tab offers on its own; `null` on the other tabs or without the model. */
  const aiStep = (on: EditorTab): EditOp | null =>
    on === 'upscale' ? upscaleOp(scale) : on === 'removeBg' && bgReady ? { type: 'removeBg' } : null;

  const handleUndo = () => {
    setError(null);
    // An AI tab offers its step again on the image the undo brings back.
    setPending(aiStep(tab));
    setOps((cur) => cur.slice(0, -1));
  };

  const handleTab = (next: EditorTab) => {
    setPending(next === 'upscale' ? upscaleOp(scale) : null);
    setTab(next);
  };

  const handleRetry = () => {
    setError(null);
    setPending(aiStep(tab));
  };

  const handleScale = (next: (typeof SCALES)[number]) => {
    setScale(next);
    setPending(upscaleOp(next));
  };

  const handleApply = (op: EditOp | null) => {
    if (!op || busy) return;
    setError(null);
    setPending(null);
    setOps((cur) => [...cur, op]);
  };

  // A step waits for the render before it: until then the preview still has the old size.
  // An AI step also waits for its own render, which can take seconds.
  const busy = !opsShown || (!!aiPending && !shown);
  const all = pending ? [...ops, pending] : ops;
  // An unchanged image would only bump the original clip (same hash), so there is nothing to copy.
  const canCopy = !!source && all.length > 0 && !copying && !busy;
  const image = preview && sizeOf(preview.img);
  // Fit the full-res aspect into the stage, never above 1 CSS px per image px.
  const fit = image && Math.min(stage.w / image.w, stage.h / image.h, 1);
  const view = image && fit ? { w: image.w * fit, h: image.h * fit } : null;
  const crop = pending?.type === 'crop' ? pending : null;
  const finalSize = source ? sizeAfter(sizeOf(source), all) : null;
  // "100%" is the preview's own px; past the preview cap that's less than the full-res image.
  const actualPct = finalSize ? Math.round(previewScale(finalSize) * 100) : 100;
  // After a failed render the step is gone; the button offers it again.
  const aiButton =
    !aiPending && aiFailed ? (
      <button className={TOOL_BUTTON} disabled={busy} onClick={handleRetry}>
        {t.editor.retry}
      </button>
    ) : (
      <button className={TOOL_BUTTON} disabled={!aiPending || busy} onClick={() => handleApply(aiPending)}>
        {t.editor.apply}
      </button>
    );

  return (
    <main className="relative flex h-full flex-col">
      <Aurora className="inset-x-0 top-0 h-[110px]" />
      {/* macOS overlays the traffic lights on this row (`titleBarStyle: Overlay`); Windows keeps its native bar. */}
      {isMac && <div className="relative z-1 h-7 shrink-0" data-tauri-drag-region />}
      <div className="relative z-1 flex shrink-0 justify-center pt-1 pb-3">
        {/* Disabled (natively, for every tab button) while an AI step renders. */}
        <fieldset className="glass rounded-xl p-0.5 disabled:opacity-60" disabled={!!aiPending && busy}>
          <Segmented
            value={tab}
            onChange={handleTab}
            options={TABS.map((value) => ({ value, label: t.editor.tabs[value] }))}
          />
        </fieldset>
      </div>
      <div ref={stageRef} className="relative z-1 mx-5 flex min-h-0 flex-1 items-center justify-center">
        {preview && shown && (
          <CompareView
            key={`${zoom}-${shown.width}x${shown.height}`}
            before={preview.img.dataUrl}
            after={shown.dataUrl}
            size={sizeOf(shown)}
            zoom={tab === 'upscale' && zoom === 'actual' ? 1 : 'fit'}
            stage={stage}
          />
        )}
        {preview && !shown && image && view && (
          <div className="relative" style={{ width: view.w, height: view.h }}>
            <img
              src={preview.img.dataUrl}
              alt=""
              draggable={false}
              className={cx('size-full rounded-md transition-opacity duration-200 select-none', { 'opacity-60': busy })}
            />
            {aiPending && busy && (
              <div
                className="absolute inset-0 flex items-center justify-center"
                role="status"
                aria-label={t.editor.working}
              >
                <span className="glass rounded-full p-2">
                  <svg
                    className="text-primary size-5 animate-spin motion-reduce:animate-none"
                    viewBox="0 0 24 24"
                    fill="none"
                    aria-hidden
                  >
                    <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.25" strokeWidth="3" />
                    <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
                  </svg>
                </span>
              </div>
            )}
            {tab === 'crop' && (
              <CropBox
                disabled={busy}
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
            <button className={TOOL_BUTTON} disabled={!crop || busy} onClick={() => handleApply(crop)}>
              {t.editor.apply}
            </button>
          </div>
        )}
        {tab === 'rotate' && (
          <RotatePanel disabled={busy} onRotate={(quarter) => handleApply({ type: 'rotate', quarter })} />
        )}
        {tab === 'resize' && image && (
          <ResizeForm
            key={`${image.w}x${image.h}`}
            size={image}
            disabled={busy}
            onPending={setPending}
            onApply={() => handleApply(pending)}
          />
        )}
        {tab === 'removeBg' &&
          (bgStatus && !bgReady ? (
            <ModelPrompt />
          ) : (
            <div className="flex items-center gap-3">
              <span className="text-muted text-[12px]">{t.editor.removeBgHint}</span>
              {aiButton}
            </div>
          ))}
        {tab === 'upscale' && (
          <div className="flex items-center gap-2.5">
            <div className="glass rounded-xl p-0.5">
              <Segmented
                value={scale}
                onChange={handleScale}
                options={SCALES.map((value) => ({ value, label: t.editor.scales[value] }))}
              />
            </div>
            <div className="glass rounded-xl p-0.5">
              <Segmented
                value={zoom}
                onChange={setZoom}
                options={ZOOMS.map((value) => ({
                  value,
                  label: value === 'fit' ? t.editor.zoom.fit : t.editor.zoom.actual(actualPct),
                }))}
              />
            </div>
            {aiButton}
          </div>
        )}
      </div>
      <Footer
        from={source ? sizeOf(source) : { w: 0, h: 0 }}
        to={finalSize ?? { w: 0, h: 0 }}
        error={error}
        canCopy={canCopy}
        onCancel={handleCancel}
        onCopy={handleCopy}
      />
    </main>
  );
}
