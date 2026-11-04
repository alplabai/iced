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
- [#3513](https://github.com/iced-rs/iced/pull/3513): text editor clears the cached selection when highlighting re-shapes lines.
- [#3521](https://github.com/iced-rs/iced/pull/3521): text selection offset in the checkbox label (adds `widget/tests/checkbox.rs`).
- [#3505](https://github.com/iced-rs/iced/pull/3505): text selection inside `pane_grid` (only captures the press when a split resize starts).
- [#3529](https://github.com/iced-rs/iced/pull/3529): `Rectangle::chord` endpoint distance for diagonal angles.
- [#3393](https://github.com/iced-rs/iced/pull/3393): SVG text drawn with the iced fontdb (squashed; the `svg` feature now also enables `iced_renderer/svg-text`).
- [#3179](https://github.com/iced-rs/iced/pull/3179), [#3220](https://github.com/iced-rs/iced/pull/3220): sensor distance is 0 when visible; `on_show` fires every time it enters view.
- [#3139](https://github.com/iced-rs/iced/pull/3139): window blur (acrylic backdrop) on Windows 11.
- [#3522](https://github.com/iced-rs/iced/pull/3522): Escape resets the image viewer.
- [#3494](https://github.com/iced-rs/iced/pull/3494): `markdown::Settings::table_width` (new public field, default `None`).
- #3514 and #3450 were re-checked against their current upstream heads: unchanged since merged.

## Skipped

- [#3315](https://github.com/iced-rs/iced/pull/3315): selectable text; conflicts with the different text-selection design already in the tree (13 files).
- [#3227](https://github.com/iced-rs/iced/pull/3227): text_editor drag on double/triple click; behavior already present in `core/src/text/editor.rs`.
- [#1542](https://github.com/iced-rs/iced/pull/1542): window drag-resize; the touched files no longer exist after the winit restructure.
- [#2483](https://github.com/iced-rs/iced/pull/2483): animations for default widgets; based on a 2024 tree, conflicts in about 40 files including files deleted since (winit program/system, text_input cursor), 139 files changed. Superseded in practice by the smaller #2770/#2779 ports.
- [#3443](https://github.com/iced-rs/iced/pull/3443): `blank` field in `window::Settings`; it refactors winit window/surface ownership into a new `Interface` type and conflicts with the frame clock, surface-error handling and #3450 work (9 hunks).
- [#3021](https://github.com/iced-rs/iced/pull/3021): architectural port (winit run/shell plumbing rewritten); about 39 conflict hunks.

## Fork fixes

Numbers refer to the ranks in `TRIAGE.md`, which has the status of all 20.

- #1, #2: zero-size image and surface guards (wgpu).
- #8, #9: `keyed::Column` reconciles children by key; `from_vecs` asserts one key per child. Regression tests in `widget/tests/keyed_column.rs`.
- #12: `window::Direction::at`/`at_with_corner` hit-testing and `Direction::interaction()` / `From<Direction> for mouse::Interaction` (core).
- #15: pending events, including the final `Closed`, are flushed to subscriptions before exit (winit).
- #16: pane_grid title bar padded width sign bug.
- #18: canvas `Frame` contents are split into ordered sections so shapes, images and text keep draw order, and pasted/clipped child frames keep their own clip in tiny_skia (graphics, wgpu, tiny_skia).
- #20: progress bar fill radius clamp.
- #7 was not reproducible (a negative-sweep arc test was added); #13 was already handled in this tree. Other ranks were skipped.

## Verification

- `cargo check --workspace --all-targets`: passes with no warnings in the iced crates.
- `cargo test -p iced_widget`: passes, including 4 new `keyed_column` tests (reorder keeps state by key, insert/remove, unchanged keys, `from_vecs` length-mismatch panic). By reasoning, the reorder test fails on the old positional diff, because key `c` would keep the tree created for `a`.
- `cargo test -p iced_core`, `-p iced_graphics --features geometry`, `-p iced_tiny_skia --features geometry,image`: pass.
- `cargo test -p iced_wgpu --features image,svg,geometry` (under xvfb with lavapipe): passes (3 nudge tests).
- Signex builds, passes its tests and launches against alpha.1 using software Vulkan only.

## Known gaps

- Nothing was run or seen on a real GPU; only software Vulkan (lavapipe).
- The wgpu side of #18 has no automated test.
- #1, #2 and #15 have no automated tests.
- The `animations` feature (radio and toggler) and per-side borders were never viewed rendered.
- Not run: iced_winit tests.
