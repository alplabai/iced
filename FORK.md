# Fork notes

Branch workflow (`master` mirror, `dev` integration, `signex` release): see `.github/CONTRIBUTING.md`.

This branch carries upstream iced-rs/iced pull requests merged on top of the base.

## signex-v0.15.0-alpha.4 (unreleased, not tagged)

Merged/ported/refreshed on top of alpha.3 (which has fork fix #22, listed under Fork fixes). Fork fixes #49 and #50 also land in this release.

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
- [#3443](https://github.com/iced-rs/iced/pull/3443): skipped (see Skipped): 336-line winit refactor, 9 conflict hunks, unsafe without compiling.

Fork deviation from upstream #3522: the image viewer resets and captures Escape only when the cursor is over it or its zoom/pan differs from the default. Upstream captures every Escape release, which breaks app-level Escape handlers. Tests in `widget/tests/image_viewer.rs`.

Behaviour change kept as upstream (#3505): a `pane_grid` left press that publishes `on_click`, or picks a pane for `on_drag`, is no longer captured. Likewise (#3179, #3220) `sensor` `on_show` fires on every re-entry when `on_hide` is `None`.

Also: hand-fixed `markdown::CappedTable` (#3494) for this fork's `Widget`/`Element` API (added `Meta` impl, boxed `Element`, `.boxed()` instead of a `From` impl).

Verification (this batch):

- `cargo check --workspace --all-targets`: 0 errors, 0 warnings (after the #3494 fix).
- Tests pass: `iced_core` (18 unit, 16 doc), `iced_widget` (all suites incl. new `checkbox`, `markdown` 43, `pane_grid`, `scrollable`, `sticky`), `iced_graphics` (geometry), `iced_tiny_skia` (geometry,image), `iced_wgpu` (image,svg,geometry; xvfb + lavapipe), `iced_runtime`.
- #3139 (Windows blur) was only type-checked.
- Not verified: no real GPU, nothing seen rendered. SVG text (#3393), Windows blur backdrop (#3139), markdown `table_width` wrapping (no test beyond compiling), sensor, pane_grid and image viewer changes were not viewed.

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
- [#3443](https://github.com/iced-rs/iced/pull/3443): `blank` field in `window::Settings`; it refactors winit window/surface ownership into a new `Interface` type and conflicts with the frame clock, surface-error handling and #3450 work (9 hunks).
- [#3021](https://github.com/iced-rs/iced/pull/3021): architectural port (winit run/shell plumbing rewritten); about 39 conflict hunks.

## Fork fixes

Numbers refer to the ranks in `TRIAGE.md`, which has the status of all 20.

- #1, #2: zero-size image and surface guards (wgpu).
- #8, #9: `keyed::Column` reconciles children by key; `from_vecs` asserts one key per child. Regression tests in `widget/tests/keyed_column.rs`.
- #36: `window::Event::ModeChanged { maximized, minimized, fullscreen }`, emitted by `iced_winit` once per maximize, minimize or fullscreen change (pure flag diff in `winit/src/window/mode_flags.rs`), polled once per event-loop pass after `Resized`, `Occluded`, `Focused` or a mode action; `minimized` is an `Option<bool>` (`None` on Wayland/web). `window::Event` is not `#[non_exhaustive]`, so the new variant breaks exhaustive `match`es over it in downstream crates. Not run on a real macOS/Windows/X11/Wayland window yet.
- #12: `window::Direction::at`/`at_with_corner` hit-testing and `Direction::interaction()` / `From<Direction> for mouse::Interaction` (core).
- #15: pending events, including the final `Closed`, are flushed to subscriptions before exit (winit).
- #16: pane_grid title bar padded width sign bug.
- #18: canvas `Frame` contents are split into ordered sections so shapes, images and text keep draw order, and pasted/clipped child frames keep their own clip in tiny_skia (graphics, wgpu, tiny_skia).
- #20: progress bar fill radius clamp.
- Issue #22: canvas text past the glyph atlas limit (`MAX_ATLAS_TEXT_SIZE`, `Text::fits_atlas`) is drawn as lyon-filled vector outlines through `Text::draw_with` instead of failing in the atlas, in wgpu and tiny_skia; the vector path now keeps the text's wrapping and ellipsis and skips huge outline-less bitmap glyphs (graphics, wgpu, tiny_skia).
- #49: `MonitorData::size()` (and `Monitor::size()`) returned physical size multiplied by the scale factor; it now divides, so a 3840x2160 monitor at 2.0 reports 1920x1080. A non-positive or non-finite scale factor is treated as 1.0. Unit tests in `core/src/window/monitor.rs`. Signex reads `size()` as logical (trunk `handlers/window_controls.rs:22`; signex#2052 `window_geometry_events.rs`), so it needs no correction after this fix; any caller that compensated for the old bug must drop it.
- #50: `winit::conversion::monitor_list` compared the primary monitor handle with itself, so every monitor was flagged primary; it now compares each handle with the primary one (`is_primary_monitor`). Unit tests in `winit/src/conversion.rs` go through the list-building function.
- #37: native macOS menu bar and cancellable quit requests.
  - `iced::menu` describes the menu bar as data built from the state (`application(...).menu(f)`, also on `daemon`); after every update iced_winit syncs it to an NSMenu through `muda` 0.20 (same rows: enabled/checked flipped in place; different rows: rebuilt). Row ids are `<submenu path>/<label>` with the first free `#n` suffix, so a click never sends another row's message (tests in `winit/src/menu.rs`). Only macOS draws it; elsewhere it is ignored.
  - `Settings::exit_on_quit_request` (default `true`): when `false`, Command+Q, Dock ▸ Quit, logout and `Predefined::Quit` reach `system::quit_requests()`, answered with `iced::exit()` or `system::cancel_quit()`. While unanswered macOS waits; at logout only for a limited time. A request no running `quit_requests` subscription would see is answered "quit" at once, and every exit path (exit, crash, instance end) answers a pending one, so no request is left hanging (state machine and tests in `winit/src/quit.rs`). `Settings::default_menu` turns off winit's default menu.
  - No predefined Edit rows (Undo/Cut/Copy/Paste/Select All): AppKit sends them to native text views, which iced widgets are not, so they did nothing and their key equivalents took Command+C/V/X/A/Z from `text_input`. Only changed flags are written to the NSMenu; a failed rebuild keeps the old menu; non-macOS targets never build the `Menu`. iced takes over `muda`'s process-wide event handler.
  - winit now comes from `alplabai/winit` branch `iced-signex` (rev `d18f6382`, on iced-rs/winit `48116469`): opt-in `applicationShouldTerminate:` returning `NSTerminateLater`, queued when the handler is busy (a direct call re-enters and panics), the reply deferred to the main queue (a synchronous reply re-enters the event handler and panics). The same patch forwards `received_url` through the `&mut A`/`Box<A>` handler impls; it was dropped before, so `event::listen_url` never fired on macOS.
  - Seen on a real Mac (`examples/native_menu`): the menu replaces winit's default one; with unsaved changes Command+Q, Dock ▸ Quit and the window close button (routed through `window::close_requests` in the example) all show the in-app confirm, Cancel keeps the app running, Quit exits. Not verified: logout, Windows/Linux (no-op), the Window/Help menus are not registered with AppKit (no automatic window list), a second quit request in the instant after Cancel (the reply is still queued).
- #74: `combo_box` search matches non-ASCII letters and symbols (`10kΩ`, `4.7µF`, `Ärger`): the query and option matchers keep `char::is_alphanumeric` instead of ASCII only (smaller than upstream #3428, which also moves the matchers into `State`). Moving the hovered option with ArrowUp/ArrowDown/Tab scrolls the menu by the least amount that shows it, through the new `overlay::menu::State::reveal` (replaces upstream #3422's previous-frame heuristic). Tests: unit tests in `widget/src/combo_box.rs` and `widget/src/overlay/menu.rs`, simulator test in `widget/tests/combo_box.rs`.
- Issue #39: `shader::Primitive::pipeline_key` (default `0`) and `Pipeline::new_keyed` give each key of one primitive type its own pipeline; keyed pipelines are dropped only after being unprepared for both 120 frames and 10 s (frames are counted across all windows and screenshots of an engine, so a static window keeps its keyed pipelines for at least 10 s and they are recreated on its next redraw after that), key `0` never is; the key is read once per primitive value and must be unique per pipeline, `0` reserved. `Storage` gains `*_keyed` accessors (wgpu). Unit tests in `wgpu/src/primitive.rs`; the real-device `prepare` test is `#[ignore]`d, run it with `--ignored` where an adapter exists.
- #7 was not reproducible (a negative-sweep arc test was added); #13 was already handled in this tree. Other ranks were skipped.

## Verification

- `cargo check --workspace --all-targets`: passes with no warnings in the iced crates.
- `cargo test -p iced_widget`: passes, including 4 new `keyed_column` tests (reorder keeps state by key, insert/remove, unchanged keys, `from_vecs` length-mismatch panic). By reasoning, the reorder test fails on the old positional diff, because key `c` would keep the tree created for `a`.
- `cargo test -p iced_core`, `-p iced_graphics --features geometry`, `-p iced_tiny_skia --features geometry,image`: pass.
- `cargo test -p iced_wgpu --features image,svg,geometry` (under xvfb with lavapipe): passes (3 nudge tests).
- Signex builds, passes its tests and launches against alpha.1 using software Vulkan only.

## Known gaps

- SVG text (#3393): the font database is cloned once on the first SVG load and never refreshed; a poisoned lock yields an empty database silently. Upstream's code, not fixed here.
- Nothing was run or seen on a real GPU; only software Vulkan (lavapipe).
- The wgpu side of #18 has no automated test.
- #1, #2 and #15 have no automated tests.
- The `animations` feature (radio and toggler) and per-side borders were never viewed rendered.
