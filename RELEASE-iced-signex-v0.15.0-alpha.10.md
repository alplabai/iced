# iced-signex-v0.15.0-alpha.10

**Tag target:** the `signex` merge commit of the `dev` -> `signex` release pull request.
**Builds on:** `iced-signex-v0.15.0-alpha.9`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there.

No upstream sync since alpha.9 (still `98bd48fe0`). winit moves to `alplabai/winit` branch `iced-signex` at `cb4f2acd` (was `0570f845`). New: the whole wgpu family is patched to `alplabai/wgpu` branch `iced-signex` at `e738655e` (#132).

## Pinning this release in Signex

Besides the iced crates, copy the workspace `[patch.crates-io]` entries for `wgpu`, `wgpu-core`, `wgpu-core-deps-*`, `wgpu-hal`, `wgpu-types`, `wgpu-naga-bridge`, `naga` and `naga-types` (all `alplabai/wgpu` at `e738655e`). `[patch]` only applies to the root workspace; without them the DX12 stretch fix of #132 is silently lost. Check that `cargo tree -i wgpu-hal` shows the `alplabai/wgpu` git source.

## Added

- **Windows 11: native caption and maximize regions in an app-drawn title bar (#105, #126)** — with `extend_content_into_title_bar`, `widget::title_bar_region(window::TitleBarRegion::Caption, ..)` makes part of the strip the native caption (drag, Aero Snap, double-click maximize, system menu on right click), and `TitleBarRegion::Maximize` gives the app's maximize button the Snap Layouts flyout. Regions are suspended while an overlay is open. Windows 10 is not supported. **Signex:** the main window can move from `decorations: false` plus `window::drag` to the native frame with caption regions; no Signex code path uses it yet.
- **Title bar caption regions on Linux and macOS (#129, #130)** — a press in a `Caption` region drags the window, a double click toggles maximization and a right click opens the window menu; a `Maximize` region keeps its button's clicks. **Signex:** one title strip code path for every desktop platform.
- **`iced_test::Snapshot::renderer` (#134)** — the renderer name used in snapshot file names; `iced_wgpu` tests create devices on the compositor's default backends (no Vulkan on Windows). **Signex:** no code path uses this.

## Changed

- **vello_cpu renders on several threads by default (#123, #125)** — outside `wasm32`, the software renderer draws on `available_parallelism() - 1` threads (at most 8). **Signex:** the vello_cpu fallback and headless tests use several cores; the explicit `vello-cpu-multithreading` opt-in is no longer needed.

## Fixed

- **vello_cpu repaints moved text groups and caches (#121, #122)** — damage covers the old and new positions. **Signex:** scrolled and panned canvas text on the software fallback redraws in place.
- **No stretched "zoom" while a window maximizes, restores or resizes on macOS and Windows (#132, #133)** — the Metal layer keeps the last frame unscaled at the top-left, and DX12 swapchains use `DXGI_SCALING_NONE` through the `alplabai/wgpu` fork. **Signex:** copy the `[patch.crates-io]` entries above; this addresses the frame stretch of Signex #2848.
- **One `Resized` per fullscreen change on Windows (#111, #135)** — no intermediate client size while the style changes. **Signex:** toggling fullscreen draws no frame at an intermediate size.
- **Linux title bar follow-ups (#136, #137, #138, #139)** — a 6-pixel resize band along the edges of an undecorated window that reports caption regions (with the resize cursor), the X11 window menu on a right click, and no duplicate `Resized` at the end of a Wayland resize. **Signex:** the custom title bar resizes from its edges on Linux without its own strips.
- **Linux and renderer fixes from the pop-os comparison (#140–#145)** — SVGs reuse cosmic-text's font database (no second system font scan; fonts loaded later reach SVGs parsed afterwards); the wgpu surface is clamped to the device limit and suboptimal frames are presented; X11 resizes are paced with `_NET_WM_SYNC_REQUEST` and a wheel notch scrolls once; on Wayland a finger can drag and resize a window with its own title bar. **Signex:** faster first SVG on Linux, smoother X11 resizes.

## Breaking

- **#126:** `runtime::user_interface::State::Updated` gains a `title_bar` field; exhaustive matches without `..` stop compiling.
- **#123:** vello_cpu always enables vello_cpu's `multithreading` outside wasm (the feature still exists and changes nothing there).

## Behaviour changes

- **#130:** apps that already wrapped a strip in a caption region for Windows now get dragging, double-click maximize and the window menu on Linux and macOS too, whatever their decorations.
- **#139 (Linux, BSDs):** the outer 6 pixels of an undecorated, resizable, restored window that reports caption regions no longer pass a left press to the widgets; the band wins. An app's own edge strips keep working.
- **#145:**
  - a poisoned font-system lock panics in SVG loading (wgpu used to fall back to an empty database);
  - an SVG parsed before a font load keeps the old database until it leaves the cache;
  - suboptimal frames are shown instead of dropped;
  - X11 windows advertise `_NET_WM_SYNC_REQUEST`;
  - X11 scroll button releases no longer produce a second wheel step.

## Verification

Each change was merged into `dev` through its own pull request with Fork CI green (`fmt`, `clippy`, `nextest --workspace --all-features`, wasm check) and an independent review.

- **Windows 11 laptop (scale 1.25, DX12):** #126 caption and maximize regions with a probe, #133 and #135 by hand.
- **macOS:** #133 by hand.
- **Fedora 45, KDE Plasma 6 and GNOME 51, Wayland and XWayland, driven with `ydotool`:**
  - #139: resize bands, X11 window menu, one `Resized` at the end of a resize;
  - #145: `_NET_WM_SYNC_REQUEST` advertised, no stretched frame mid-drag, one wheel step per notch, SVGs render;
  - decorated windows on all four sessions: drag, maximize and restore by double click and button, minimize and restore, close, corner resize, wheel (results on #145).

## Not verified

- **#146:** the IME fixes with a real input method.
- **#147:** Windows DX12 resizing after the suboptimal-frame change.
- **#148:** a window larger than the GPU texture limit.
- **#128:** DPI changes and a second monitor with the Windows 11 title bar.
- Touch on Wayland (#143), and macOS drag, double click and right click on caption regions.
- **#111 (open):** the white strip while dragging an edge on Windows.
- **Signex build:** Signex moves to this release in its own pull request, which runs the full Signex gate set.
