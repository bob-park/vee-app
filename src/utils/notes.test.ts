import assert from 'node:assert/strict';
import { test } from 'node:test';

import { parseNotes } from './notes.ts';

void test('parses headings, nested items, bold and code', () => {
  const md = '## 변경 사항\n\n### 새 기능\n- **패널 고정 단축키:** `⌘⇧P`로 켜요.\n  - 설정에서 바꿔요.';
  assert.deepEqual(parseNotes(md), [
    { kind: 'heading', level: 2, inlines: [{ kind: 'text', text: '변경 사항' }] },
    { kind: 'heading', level: 3, inlines: [{ kind: 'text', text: '새 기능' }] },
    {
      kind: 'item',
      depth: 0,
      inlines: [
        { kind: 'bold', text: '패널 고정 단축키:' },
        { kind: 'text', text: ' ' },
        { kind: 'code', text: '⌘⇧P' },
        { kind: 'text', text: '로 켜요.' },
      ],
    },
    { kind: 'item', depth: 1, inlines: [{ kind: 'text', text: '설정에서 바꿔요.' }] },
  ]);
});

void test('keeps HTML as plain text', () => {
  assert.deepEqual(parseNotes('<script>x</script>'), [
    { kind: 'para', inlines: [{ kind: 'text', text: '<script>x</script>' }] },
  ]);
});

void test('handles CRLF and an unclosed marker', () => {
  assert.deepEqual(parseNotes('- a **b\r\n'), [{ kind: 'item', depth: 0, inlines: [{ kind: 'text', text: 'a **b' }] }]);
});
