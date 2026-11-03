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

## Skipped

- [#3315](https://github.com/iced-rs/iced/pull/3315): selectable text; conflicts with the different text-selection design already in the tree (13 files).
- [#3227](https://github.com/iced-rs/iced/pull/3227): text_editor drag on double/triple click; behavior already present in `core/src/text/editor.rs`.
- [#1542](https://github.com/iced-rs/iced/pull/1542): window drag-resize; the touched files no longer exist after the winit restructure.
- [#3359](https://github.com/iced-rs/iced/pull/3359): fix culling of shadowed quads in tiny_skia; merged cleanly but was reverted because its build check was blocked; can be re-merged.
- [#3021](https://github.com/iced-rs/iced/pull/3021): architectural port (winit run/shell plumbing rewritten); about 39 conflict hunks.

## Verification

- `cargo check --workspace --all-targets`: passes, no fixes needed.
- `cargo test -p iced_core -p iced_widget -p iced_winit`: all pass.
- Nothing was rendered on a GPU (shader changes from #3391 validated with naga only).
