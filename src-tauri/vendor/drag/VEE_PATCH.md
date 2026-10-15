# Vee patch

Copied from drag 2.1.1 (crates.io), with two changes:

- `src/platform_impl/windows/mod.rs`, `get_drag_image`: `ptOffset` is the centre of the image
  instead of `(0, 0)`, so Windows holds the drag image by its centre like macOS does.
- `DragItem::Text(String)`: plain text dropped where it lands (macOS: `NSPasteboardTypeString`;
  Windows: `CF_UNICODETEXT` on a shell data object; gtk: empty drag). On Windows the drag loop
  is shared by all items in `do_drag`.
