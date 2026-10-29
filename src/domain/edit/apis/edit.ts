import type { EditImage, EditOp } from '@/domain/edit/apis/edit.dto';
import call from '@/shared/api';

/** The image open in the editor; rejects with `INVALID_INPUT` when none is. */
export async function getEditSource() {
  return call<EditImage>('get_edit_source');
}

/** The source with `ops` applied in order. */
export async function renderEdit(ops: EditOp[]) {
  return call<EditImage>('render_edit', { ops });
}

/** Saves the result as a new clip, copies it and hides the editor. */
export async function copyEdit(ops: EditOp[]) {
  return call<void>('copy_edit', { ops });
}

export async function closeEditor() {
  return call<void>('close_editor');
}
