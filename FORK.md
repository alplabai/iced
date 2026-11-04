# Fork notes

This branch carries upstream iced-rs/iced pull requests merged on top of the base.

## Merged upstream PRs

- [#3391](https://github.com/iced-rs/iced/pull/3391): per-side border widths and colors (hand-merged with the gradient shadow work).
- [#3377](https://github.com/iced-rs/iced/pull/3377): trackpad pinch gestures as `mouse::Event::PinchGesture`.
- [#3283](https://github.com/iced-rs/iced/pull/3283): gesture events (Pan/Rotate/DoubleTap kept; its Pinch arm dropped in favor of #3377).
- [#3290](https://github.com/iced-rs/iced/pull/3290): fix IME preedit slicing.
- [#3514](https://github.com/iced-rs/iced/pull/3514): enable wgpu texture formats and external memory features.
- [#3316](https://github.com/iced-rs/iced/pull/3316): COPY_SRC usage on the compositor surface (cherry-picked, applied in `configure_surface`).
- [#3450](https://github.com/iced-rs/iced/pull/3450): multi-monitor window management via `MonitorIndex`.
- [#3360](https://github.com/iced-rs/iced/pull/3360): clip text groups in tiny_skia.
- [#3359](https://github.com/iced-rs/iced/pull/3359): fix culling of shadowed quads in tiny_skia (clean merge).
- [#2770](https://github.com/iced-rs/iced/pull/2770): radio dot animation (hand-ported to the current `Widget` API; animated only with the `animations` feature).
- [#2779](https://github.com/iced-rs/iced/pull/2779): toggler animation (hand-ported; `Status`/`Style` public API left unchanged, background colors are mixed and the knob position interpolated inside the widget; animated only with the `animations` feature; its `animations` example was not added). Also adds `Animation::force`/`force_mut` in core.

## Skipped

- [#3315](https://github.com/iced-rs/iced/pull/3315): selectable text; conflicts with the different text-selection design already in the tree (13 files).
- [#3227](https://github.com/iced-rs/iced/pull/3227): text_editor drag on double/triple click; behavior already present in `core/src/text/editor.rs`.
- [#1542](https://github.com/iced-rs/iced/pull/1542): window drag-resize; the touched files no longer exist after the winit restructure.
- [#2483](https://github.com/iced-rs/iced/pull/2483): animations for default widgets; based on a 2024 tree, conflicts in about 40 files including files deleted since (winit program/system, text_input cursor), 139 files changed. Superseded in practice by the smaller #2770/#2779 ports.
- [#3021](https://github.com/iced-rs/iced/pull/3021): architectural port (winit run/shell plumbing rewritten); about 39 conflict hunks.

## Fork fixes

- #12: `window::Direction::at`/`at_with_corner` hit-testing and `Direction::interaction()` / `From<Direction> for mouse::Interaction` (core).
## Verification

- One `cargo check --workspace --all-targets` for the batch of #3359, #2770 and #2779 (second run after adding `Animation::force`): passes.
- `cargo test -p iced_core`: passed (9 + 14 tests).
- Not run: iced_widget/iced_winit tests, any GPU rendering, and the animations were never executed visually (the `animations` feature was not built or exercised).
