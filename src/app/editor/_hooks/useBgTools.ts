import { type Dispatch, type SetStateAction, useState } from 'react';

import type { BgTool } from '@/app/editor/_components/BgToolRail';
import { useEngineStatus } from '@/domain/convert/queries/engines';
import type { BrushMode, EditOp, Stroke } from '@/domain/edit/apis/edit.dto';
import { type SubjectMarks, dropLastMark, emptyMarks, subjectOp, undoMark } from '@/utils/edit';

/** Brush radius in CSS px: default, range and the `[` / `]` step. */
export const BRUSH = { size: 16, min: 2, max: 80, step: 2 };

/** The Remove background tab's tools (Auto, Subject, Brush): their state, models and live step. */
export function useBgTools(pending: EditOp | null, setPending: Dispatch<SetStateAction<EditOp | null>>) {
  // state
  const [bgTool, setBgTool] = useState<BgTool>('auto');
  const [brushMode, setBrushMode] = useState<BrushMode>('erase');
  /** Brush radius in CSS px. */
  const [brushSize, setBrushSize] = useState(BRUSH.size);
  /** The Subject tool's clicks and lassos; they count only while `pending` is the subject step they made. */
  const [subjectMarks, setSubjectMarks] = useState<SubjectMarks>(emptyMarks);

  // queries
  const { status: bgStatus } = useEngineStatus('bgremove');
  const { status: samStatus } = useEngineStatus('sam');

  const bgReady = bgStatus?.state === 'installed';
  const samReady = samStatus?.state === 'installed';
  /** A Subject or Brush step being built; never empty (the last item removed makes `pending` null). */
  const live = pending?.type === 'subject' || pending?.type === 'brush' ? pending : null;
  // Any other `pending` (applied, reset, another tool) leaves no marks behind.
  const marks = pending?.type === 'subject' ? subjectMarks : emptyMarks();

  // handle
  /** The Subject tool's marks and the step they make, set together. */
  const handleMarks = (next: SubjectMarks) => {
    setSubjectMarks(next);
    setPending(subjectOp(next));
  };

  /** Takes back the live step's last click, lasso or stroke. */
  const undoLiveMark = () => {
    if (live?.type === 'subject') handleMarks(undoMark(marks));
    else if (live) setPending(dropLastMark(live));
  };

  const handleBrushSize = (next: number) => setBrushSize(Math.min(Math.max(next, BRUSH.min), BRUSH.max));

  const handleStroke = (stroke: Stroke) =>
    setPending((cur) => ({
      type: 'brush',
      strokes: [...(cur?.type === 'brush' ? cur.strokes : []), stroke],
    }));

  /** Back to the defaults; uses only setters, so a closure from any render works. */
  const reset = () => {
    setBgTool('auto');
    setBrushMode('erase');
    setBrushSize(BRUSH.size);
    setSubjectMarks(emptyMarks());
  };

  return {
    bgTool,
    setBgTool,
    brushMode,
    setBrushMode,
    brushSize,
    bgStatus,
    samStatus,
    bgReady,
    samReady,
    live,
    marks,
    handleMarks,
    undoLiveMark,
    handleBrushSize,
    handleStroke,
    reset,
  };
}
