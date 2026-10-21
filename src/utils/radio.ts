/**
 * The radio index a key moves to inside a `radiogroup` (WAI-ARIA radio pattern):
 * arrows step with wrap-around, Home/End jump to the ends. `null` for any other key.
 */
export function arrowStep(key: string, index: number, length: number): number | null {
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
      return (index + 1) % length;
    case 'ArrowLeft':
    case 'ArrowUp':
      return (index - 1 + length) % length;
    case 'Home':
      return 0;
    case 'End':
      return length - 1;
    default:
      return null;
  }
}
