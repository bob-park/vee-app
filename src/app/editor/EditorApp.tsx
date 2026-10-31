import { useEffect, useRef, useState } from 'react';

import BgToolRail, { type BgTool } from '@/app/editor/_components/BgToolRail';
import BrushLayer from '@/app/editor/_components/BrushLayer';
import CompareView from '@/app/editor/_components/CompareView';
import CropBox from '@/app/editor/_components/CropBox';
import Footer from '@/app/editor/_components/Footer';
import ModelPrompt from '@/app/editor/_components/ModelPrompt';
import ResizeForm from '@/app/editor/_components/ResizeForm';
import RotatePanel from '@/app/editor/_components/RotatePanel';
import SubjectLayer from '@/app/editor/_components/SubjectLayer';
import { TOOL_BUTTON } from '@/app/editor/_components/button';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import { closeEditor, copyEdit, getEditSource, renderEdit } from '@/domain/edit/apis/edit';
import type { BrushMode, EditImage, EditOp } from '@/domain/edit/apis/edit.dto';
import Aurora from '@/shared/components/Aurora';
import Segmented from '@/shared/components/Segmented';
import { useI18n } from '@/shared/providers/i18n/I18nProvider';
import {
  type Size,
  type SubjectMarks,
  addLasso,
  addPoint,
  dropLastMark,
  emptyMarks,
  previewScale,
  sizeAfter,
  subjectOp,
  undoMark,
} from '@/utils/edit';
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
const BRUSH_MODES: BrushMode[] = ['erase', 'restore'];
/** Brush radius in CSS px: default, range and the `[` / `]` step. */
const BRUSH = { size: 16, min: 2, max: 80, step: 2 };

const isMac = navigator.userAgent.includes('Mac');

/** The neutral button beside Apply, like the footer's Cancel. */
const RESET_BUTTON =
  'rounded-box bg-subtle text-base-content px-3.5 py-1.5 text-[12px] font-semibold disabled:cursor-default disabled:opacity-50';

const sizeOf = (img: EditImage): Size => ({ w: img.width, h: img.height });

export function EditorApp() {
  // ref
  const stageRef = useRef<HTMLDivElement>(null);
  /** `pending` as of the last render, for a live render that returns after it changed. */
  const pendingRef = useRef<EditOp | null>(null);

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
  /** The render of `[...ops, pending]` for a live step, shown before it's applied; `ops` is the base it was made on. */
  const [result, setResult] = useState<{ img: EditImage; op: EditOp; ops: EditOp[] } | null>(null);
  /** The last AI step failed to render; its tab offers to try again. */
  const [aiFailed, setAiFailed] = useState(false);
  const [scale, setScale] = useState<(typeof SCALES)[number]>('2');
  const [zoom, setZoom] = useState<(typeof ZOOMS)[number]>('fit');
  const [bgTool, setBgTool] = useState<BgTool>('auto');
  const [brushMode, setBrushMode] = useState<BrushMode>('erase');
  /** Brush radius in CSS px. */
  const [brushSize, setBrushSize] = useState(BRUSH.size);
  /** A live step's render is running; the next click or stroke waits for it (latest wins). */
  const [liveRendering, setLiveRendering] = useState(false);
  /** The Subject tool's clicks and lassos; they count only while `pending` is the subject step they made. */
  const [subjectMarks, setSubjectMarks] = useState<SubjectMarks>(emptyMarks);

  // hooks
  const { t } = useI18n();

  // queries
  const { status: bgStatus } = useEngineStatus('bgremove');
  const { status: samStatus } = useEngineStatus('sam');

  const bgReady = bgStatus?.state === 'installed';
  const samReady = samStatus?.state === 'installed';
  const opsShown = preview?.ops === ops;
  /** A Subject or Brush step being built; never empty (the last item removed makes `pending` null). */
  const live = pending?.type === 'subject' || pending?.type === 'brush' ? pending : null;
  /** A step rendered before it's applied. */
  const aiPending = pending?.type === 'removeBg' || pending?.type === 'upscale' ? pending : live;
  const fresh = result?.ops === ops ? result : null;
  const shown = aiPending && fresh?.op === aiPending ? fresh.img : null;
  // A live step keeps showing its last render while the next click or stroke renders.
  const liveImg = live && fresh?.op.type === live.type ? fresh.img : null;
  // Only a slow render blocks: Auto, Upscale and a subject's first click (it embeds the image), never a stroke.
  const spinner = !!aiPending && !shown && aiPending.type !== 'brush' && !liveImg;
  /** The image shows every click or stroke of the live step (or there is none). */
  const liveSynced = !live || fresh?.op === live;
  // Any other `pending` (applied, reset, another tool) leaves no marks behind.
  const marks = pending?.type === 'subject' ? subjectMarks : emptyMarks();

  // useEffect
  useEffect(() => {
    /** Back to a fresh editor; on close this lets go of every image so the hidden window holds no memory. */
    const reset = () => {
      setSource(null);
      setPreview(null);
      setResult(null);
      setOps([]);
      setPending(null);
      setTab('crop');
      setError(null);
      setAiFailed(false);
      setScale('2');
      setZoom('fit');
      setBgTool('auto');
      setBrushMode('erase');
      setBrushSize(BRUSH.size);
      setSubjectMarks(emptyMarks());
    };
    const load = () => {
      // Reset first, so a reopen never shows the last image while the new one loads.
      reset();
      void getEditSource()
        .then(setSource)
        .catch((e) => setError(errorText(e)));
    };
    // The window may have been opened before this listener existed, so load once on mount too.
    load();
    const offOpened = listen('editor://opened', load);
    // Every close path (Cancel, Esc, the window's close button, a finished copy) goes through Rust `close`.
    const offClosed = listen('editor://closed', reset);
    return () => {
      void offOpened.then((f) => f());
      void offClosed.then((f) => f());
    };
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
    if (tab === 'removeBg' && bgTool === 'auto') setPending(bgReady ? { type: 'removeBg' } : null);
  }, [tab, bgTool, bgReady]);

  useEffect(() => {
    if (tab === 'removeBg' && bgTool === 'subject' && !samReady) setPending(null);
  }, [tab, bgTool, samReady]);

  useEffect(() => {
    pendingRef.current = pending;
  });

  // An AI step renders once the steps before it have; a failure drops only the step, never `ops`.
  useEffect(() => {
    if (!aiPending || live || !opsShown) return;
    setAiFailed(false);
    let stale = false;
    renderEdit([...ops, aiPending])
      .then((img) => {
        if (!stale) setResult({ img, op: aiPending, ops });
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
  }, [aiPending, live, ops, opsShown]);

  // A live step renders one request at a time, so quick clicks or strokes never queue full-res
  // renders: the latest step renders once the running one returns. A failure takes back only the
  // last click or stroke.
  useEffect(() => {
    if (!live || liveSynced || !opsShown || liveRendering) return;
    setAiFailed(false);
    setLiveRendering(true);
    renderEdit([...ops, live])
      .then((img) => {
        // Dropped once the step is gone (applied, reset or the editor closed).
        if (pendingRef.current?.type === live.type) setResult({ img, op: live, ops });
      })
      .catch((e) => {
        if (pendingRef.current !== live) return;
        setError(errorText(e));
        setAiFailed(true);
        if (live.type === 'subject') handleMarks(undoMark(marks));
        else setPending(dropLastMark(live));
      })
      .finally(() => setLiveRendering(false));
    // `marks` needs no dep: it changes only together with `live`.
  }, [live, liveSynced, ops, opsShown, liveRendering]);

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
      // Inside a text field, ⌘Z undoes the typing instead.
      const typing = e.target instanceof HTMLInputElement && e.target.type !== 'range';
      if (e.key === 'Escape') handleCancel();
      else if (mod && e.key === 'Enter') handleCopy();
      else if (mod && !e.shiftKey && e.code === 'KeyZ' && !typing) handleUndoKey();
      else if (brushOn && !mod && (e.code === 'BracketLeft' || e.code === 'BracketRight'))
        handleBrushSize(brushSize + (e.code === 'BracketRight' ? BRUSH.step : -BRUSH.step));
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
    on === 'upscale'
      ? upscaleOp(scale)
      : on === 'removeBg' && bgTool === 'auto' && bgReady
        ? { type: 'removeBg' }
        : null;

  const handleUndo = () => {
    if (busy) return;
    setError(null);
    // An AI tab offers its step again on the image the undo brings back.
    setPending(aiStep(tab));
    setOps((cur) => cur.slice(0, -1));
  };

  /** The Subject tool's marks and the step they make, set together. */
  const handleMarks = (next: SubjectMarks) => {
    setSubjectMarks(next);
    setPending(subjectOp(next));
  };

  /** ⌘Z: before a live step is applied, it takes back its last click, lasso or stroke. */
  const handleUndoKey = () => {
    if (!live) return handleUndo();
    if (live.type === 'subject') handleMarks(undoMark(marks));
    else setPending(dropLastMark(live));
  };

  const handleTab = (next: EditorTab) => {
    if (next === tab) return;
    // Clicks or strokes become a step on the way out; Auto's result was only a preview.
    handleApply(live);
    setPending(next === 'upscale' ? upscaleOp(scale) : null);
    if (next === 'removeBg') setBgTool('auto');
    setTab(next);
  };

  const handleBgTool = (next: BgTool) => {
    if (next === bgTool) return;
    handleApply(live);
    setPending(null);
    setBgTool(next);
  };

  const handleBrushSize = (next: number) => setBrushSize(Math.min(Math.max(next, BRUSH.min), BRUSH.max));

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
  // A slow AI step also waits for its own render, which can take seconds.
  const busy = !opsShown || spinner;
  const bgOn = tab === 'removeBg';
  const subjectOn = bgOn && bgTool === 'subject';
  const brushOn = bgOn && bgTool === 'brush';
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

  const liveApply = (
    <button className={TOOL_BUTTON} disabled={!live || busy} onClick={() => handleApply(live)}>
      {t.editor.apply}
    </button>
  );

  return (
    <main className="relative flex h-full flex-col">
      <Aurora className="inset-x-0 top-0 h-[110px]" />
      {/* macOS overlays the traffic lights on this row (`titleBarStyle: Overlay`); Windows keeps its native bar. */}
      {isMac && (
        <div
          className="relative z-1 flex h-7 shrink-0 items-center justify-center font-semibold select-none"
          data-tauri-drag-region
        >
          {t.editor.title}
        </div>
      )}
      <div className="relative z-1 flex shrink-0 justify-center pt-1 pb-3">
        {/* Disabled (natively, for every tab button) while a slow AI step renders. */}
        <fieldset className="glass rounded-xl p-0.5 disabled:opacity-60" disabled={spinner}>
          <Segmented
            value={tab}
            onChange={handleTab}
            options={TABS.map((value) => ({ value, label: t.editor.tabs[value] }))}
          />
        </fieldset>
      </div>
      <div
        ref={stageRef}
        className={cx('relative z-1 mx-5 flex min-h-0 flex-1 items-center justify-center', { 'pl-12': bgOn })}
      >
        {/* Room for it is kept free by the stage's left padding, so it never covers the image.
            Auto's render may be left mid-way (it's dropped); a live step's first render may not (it'd be lost). */}
        {bgOn && <BgToolRail value={bgTool} onChange={handleBgTool} disabled={!!live && busy} />}
        {preview && shown && !live && (
          <CompareView
            key={`${zoom}-${shown.width}x${shown.height}`}
            before={preview.img.dataUrl}
            after={shown.dataUrl}
            size={sizeOf(shown)}
            zoom={tab === 'upscale' && zoom === 'actual' ? 1 : 'fit'}
            stage={stage}
          />
        )}
        {preview && (!shown || live) && image && view && (
          <div className="relative" style={{ width: view.w, height: view.h }}>
            {/* Brush paints on the latest render; over a checkerboard so erased parts read as transparent. */}
            <img
              src={brushOn && liveImg ? liveImg.dataUrl : preview.img.dataUrl}
              alt=""
              draggable={false}
              className={cx('size-full rounded-md transition-opacity duration-200 select-none', {
                'opacity-60': busy,
                'opacity-35': subjectOn && liveImg && !busy,
                'bg-checker': brushOn,
              })}
            />
            {/* Subject: the cut on top of the dimmed original, so what's left out still shows faintly. */}
            {subjectOn && liveImg && (
              <img
                src={liveImg.dataUrl}
                alt=""
                draggable={false}
                className="absolute inset-0 size-full rounded-md select-none"
              />
            )}
            {spinner && (
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
            {subjectOn && (
              <SubjectLayer
                view={view}
                image={image}
                points={marks.points}
                lasso={marks.lassos.at(-1)}
                disabled={busy || !samReady}
                onPoint={(point) => handleMarks(addPoint(marks, point))}
                onLasso={(lasso) => handleMarks(addLasso(marks, lasso))}
              />
            )}
            {brushOn && (
              <BrushLayer
                view={view}
                image={image}
                mode={brushMode}
                viewRadius={brushSize}
                synced={liveSynced}
                disabled={busy}
                onStroke={(stroke) =>
                  setPending((cur) => ({
                    type: 'brush',
                    strokes: [...(cur?.type === 'brush' ? cur.strokes : []), stroke],
                  }))
                }
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
        {bgOn &&
          bgTool === 'auto' &&
          (bgStatus && !bgReady ? (
            <ModelPrompt />
          ) : (
            <div className="flex items-center gap-3">
              <span className="text-muted text-[12px]">{t.editor.removeBgHint}</span>
              {aiButton}
            </div>
          ))}
        {subjectOn &&
          (samStatus && !samReady ? (
            <ModelPrompt />
          ) : (
            <div className="flex items-center gap-3">
              <span className="text-muted text-[12px]">{t.editor.subjectHint(isMac)}</span>
              <button className={RESET_BUTTON} disabled={!live || busy} onClick={() => setPending(null)}>
                {t.editor.subjectReset}
              </button>
              {liveApply}
            </div>
          ))}
        {brushOn && (
          <div className="flex items-center gap-3">
            <div className="glass rounded-xl p-0.5">
              <Segmented
                value={brushMode}
                onChange={setBrushMode}
                options={BRUSH_MODES.map((value) => ({ value, label: t.editor.brushModes[value] }))}
              />
            </div>
            <label className="text-muted flex items-center gap-2 text-[12px]">
              {t.editor.brushSize}
              <input
                type="range"
                className="accent-primary w-24"
                min={BRUSH.min}
                max={BRUSH.max}
                value={brushSize}
                aria-label={t.editor.brushSize}
                onChange={(e) => handleBrushSize(Number(e.target.value))}
              />
              <span className="w-5 tabular-nums">{brushSize}</span>
            </label>
            {liveApply}
          </div>
        )}
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
