# Vee patch

Copied from drag 2.1.1 (crates.io). The only change is in
`src/platform_impl/windows/mod.rs`, `get_drag_image`: `ptOffset` is the centre of the image
instead of `(0, 0)`, so Windows holds the drag image by its centre like macOS does.
