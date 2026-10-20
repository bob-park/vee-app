import type { ConvertNote } from '@/domain/convert/apis/convert.dto';

export type Kind = 'text' | 'link' | 'image' | 'files';
export type Filter = Kind | 'all';

export interface Clip {
  id: number;
  kind: Kind;
  textPreview: string | null;
  charCount: number;
  thumb: string | null;
  meta: string | null;
  appName: string | null;
  appIcon: string | null;
  lastUsedAt: number;
  missing: boolean;
  isDir: boolean;
  pinned: boolean;
  /** Preview layers of a files clip: data URL per image file, null for other files. */
  stack: (string | null)[];
}

export interface Stats {
  count: number;
  pinned: number;
  imageBytes: number;
}

export interface ToastPayload {
  ok: boolean;
  /** What was copied, for the toast's icon. */
  kind: Kind;
  text: string | null;
  files: number;
  /** Copied as plain text, without formatting. */
  plain: boolean;
  /** Set when the toast reports a conversion. */
  convert: ConvertNote | null;
}
