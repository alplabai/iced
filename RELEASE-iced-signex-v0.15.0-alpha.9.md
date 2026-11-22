# iced-signex-v0.15.0-alpha.9

**Tag target:** the `signex` merge commit of the `dev` -> `signex` release pull request.
**Builds on:** `iced-signex-v0.15.0-alpha.8`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there.

This release follows upstream master to `98bd48fe0` (102 commits, #116). Upstream's history is merged, not squashed, so later syncs only bring new upstream commits. The software renderer is now `vello_cpu`, as upstream; `tiny_skia` is gone. winit is unchanged: `alplabai/winit` branch `iced-signex` at `0570f845`.

## Upstream PRs merged

- **vello_cpu replaces tiny_skia (#116, upstream #3306)**
  - The software renderer is `iced_vello_cpu`. The `tiny_skia` crate and `iced_tiny_skia` are deleted, the `tiny-skia` feature is renamed `vello-cpu` (plus `vello-cpu-multithreading`), and headless snapshots are named `*-vello_cpu`.
  - The fork re-ports its tiny_skia fixes to vello_cpu: failed image loads stay errors (#3215), the SVG mask alpha scales coverage (#3542), text damage grows 1 px (#3486), a scale factor change redraws fully (#3531), raw text clips, and a headless draw without `reset()` is clipped to the viewport.
  - **Signex:** the `iced_tiny_skia` dev-dependencies (`layout_probe`, `tab_pill` tests) move to `iced_vello_cpu`, whose `Headless::screenshot` returns RGBA. The three symbol-canvas tests that parsed tiny_skia's debug output are rewritten.
- **Tooltip API and popover widget (#116, upstream #3544)**
  - Adds `popover` and `overlay::Position` (default `Position::Auto`). `tooltip(content, tip)` drops its `position` argument (use `.position(..)`) and its `padding`, `style` and `class` (style the tip widget). `Tooltip<W, V>` loses its lifetime and theme parameters.
  - **Signex:** the house `tooltip` wrapper calls `.position()`; the 24 `tooltip(..).style(s)` calls wrap the tip in `container(tip).style(s)`, so the look is unchanged.
- **`opaque` follows upstream (#116, upstream #3544)**
  - `opaque` returns `Opaque<W>` and captures mouse presses only; wheel scrolling passes through unless `.capture_scroll(true)`. This reverts the always-capture behaviour of #97.
  - **Signex:** the keyboard-shortcuts and About modals and the locked-window veil add `.capture_scroll(true)`.
- **`Daemon::font` sets the default font (#116, upstream b2fbd3cb6)** — font bytes now go through `Daemon::fonts(iter)`. **Signex:** `main.rs` replaces its `.font(bytes)` chain with one `.fonts([...])` in the same order.
- **Small upstream fixes (#116)** — text selection while scrolling (e5c350f27), `Rectangle::snap` for negative coordinates (a46f0a93b), the frame clock rounds up (c3d2cfd0a), `Component` overlay layout invalidation (bbd4c2601), `scrollable` translation takes a `Size` (8836286bc), bounded `debug` messages (edb068eeb), real screen damage for softbuffer (797c8cbcf). **Signex:** no code change; selection in scrolled panels and cursor blink redraws benefit.

## Fixed

- **vello_cpu catches up with the fork's tiny_skia (#118, #117)**
  - Per-side border widths and colours (#3391), SVG `currentColor` with its own cache key (#3434), and canvas shapes, text and images drawn in call order with `Frame::with_clip` child clips on shapes (#18).
  - Uniform borders follow the outer curve and stroke with a miter join, so square corners are sharp and wide rounded borders match the background, as on tiny_skia and wgpu.
  - Canvas text past the old glyph atlas limit needs nothing on vello_cpu (#22, test only).
  - **Signex:** on the software path, tab pills with one-sided borders, recoloured SVG icons and symbol canvases now match the wgpu output.

## Breaking

- **#116:**
  - `iced_tiny_skia` and the `tiny-skia` feature are gone; use `iced_vello_cpu` and `vello-cpu`. `ICED_TEST_BACKEND=tiny-skia` becomes `vello_cpu` (`vello-cpu` and `software` also work).
  - `tooltip(content, tip, position)` becomes `tooltip(content, tip).position(position)`; `Tooltip` has no `padding`, `style` or `class`, and no lifetime or theme parameters.
  - `opaque` lives in its own module and returns `Opaque<W>`.
  - `Daemon::font` takes a `Font`; load bytes with `Daemon::fonts`.
  - `scrollable` translation takes a `Size`.

## Behaviour changes

- **#116:** wheel events over `opaque` reach the layers below again unless `.capture_scroll(true)` is set.
- **#118 (vello_cpu only):** canvas shapes take the layer transformation like text and images.
  - Shapes of a canvas partly clipped by its layer (a scrollable or pane scrolled past the canvas top or left) are drawn in place instead of offset by the clipped amount.
  - Shapes and stroke widths follow a scaled layer; a root frame with a non-zero origin (`Frame::with_bounds`) no longer shifts its shapes.
  - Child clips used to be dropped. Each section after the first (shape, then text, then shape again) is a new layer.
- **#118 (vello_cpu only):** square uniform borders get sharp corners instead of round ones; pixel tests that recorded the old corners change.

## Verification

Each change was merged into `dev` through its own pull request: Fork CI green (`fmt`, `clippy`, `nextest --workspace --all-features` with `ICED_TEST_BACKEND=vello_cpu`, the wasm check) and an adversarial Opus review that ended in MERGE. #116 was also tried against Signex on a trial branch before the switch to vello_cpu was decided: 17297 Signex tests passed, and the 3 failures were the symbol-canvas tests that parsed tiny_skia output.

## Not verified

- **#118:** not viewed on a real Signex screen; vello_cpu unit and pixel tests only.
- **#116:** the new tooltip and `opaque` behaviour was not clicked through in Signex.
- **#111 (open):** a brief white area and lag while resizing on Windows, and GL fullscreen flicker.
- **Signex build:** Signex moves to this release in its own pull request, which runs the full Signex gate set.
