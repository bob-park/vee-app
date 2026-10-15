/** A tiny reader for GitHub release notes: headings, bullet lists, `**bold**` and `` `code` ``. Everything else stays plain text. */

export type Inline = { kind: 'text' | 'bold' | 'code'; text: string };
export type Block =
  | { kind: 'heading'; level: 2 | 3; inlines: Inline[] }
  | { kind: 'item'; depth: number; inlines: Inline[] }
  | { kind: 'para'; inlines: Inline[] };

const INLINE = /\*\*(.+?)\*\*|`([^`]+)`/g;

function parseInlines(s: string): Inline[] {
  const out: Inline[] = [];
  let last = 0;
  for (const m of s.matchAll(INLINE)) {
    if (m.index > last) out.push({ kind: 'text', text: s.slice(last, m.index) });
    out.push(m[1] !== undefined ? { kind: 'bold', text: m[1] } : { kind: 'code', text: m[2] });
    last = m.index + m[0].length;
  }
  if (last < s.length) out.push({ kind: 'text', text: s.slice(last) });
  return out;
}

export function parseNotes(md: string): Block[] {
  const blocks: Block[] = [];
  for (const raw of md.split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (!line.trim()) continue;
    const heading = /^(#+) (.*)$/.exec(line);
    const item = /^( *)[-*] (.*)$/.exec(line);
    if (heading) {
      // One `#` or four or more read as a subheading.
      blocks.push({ kind: 'heading', level: heading[1].length === 2 ? 2 : 3, inlines: parseInlines(heading[2]) });
    } else if (item) {
      blocks.push({ kind: 'item', depth: Math.floor(item[1].length / 2), inlines: parseInlines(item[2]) });
    } else {
      blocks.push({ kind: 'para', inlines: parseInlines(line.trim()) });
    }
  }
  return blocks;
}
