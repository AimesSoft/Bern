# Bern GPUI touch input patches

Vendored crates: `gpui-pre`, `gpui-pre-linux`, and `gpui-pre-windows`, version
**0.3.10**, from their published crates.io source archives. Their package metadata
identifies the upstream snapshot as Zed `08cbb88`. Original authorship and
`LICENSE-APACHE` files remain in each crate. Upstream: https://github.com/zed-industries/zed
(`crates/gpui`, `crates/gpui_linux`, `crates/gpui_windows`).

Changes are deliberately limited to input delivery:

- Core: dispatch raw `TouchEvent` to capture listeners before recognition; a
  listener may claim after movement exceeds its threshold. Claiming cancels the
  prior native recognizer, without manufacturing a tap or fling.
- Linux/Wayland: bind `wl_touch`, retain each contact's originating surface,
  dispatch exact logical coordinates, and cancel on seat/device loss.
- Linux/X11: select XI 2.2 touch events for direct-touch devices, track contacts,
  and suppress their emulated pointer duplicates (not touchpads).
- Windows: dispatch `WM_POINTER` touch contacts, preserve per-contact IDs,
  cancel on capture/window loss, and suppress promoted touch mouse messages.

The Bern runtime manifest and generated application root manifests both patch
these crates: Cargo does not apply a dependency's own `[patch.crates-io]` table.
No registry cache files are edited. Do not copy build outputs, Cargo lockfiles,
Cargo registry metadata, or Git metadata into this directory.
