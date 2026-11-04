# iced-signex-v0.15.0-alpha.4 (draft)

Tag target: the `signex` merge commit of the `dev` -> `signex` release pull request (not tagged here). Builds on `iced-signex-v0.15.0-alpha.3`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full merged/skipped list.

## Upstream PRs merged
3513 (text editor selection cache), 3521 (checkbox selection offset), 3505 (pane_grid text selection), 3529 (Rectangle::chord), 3393 (SVG text via iced fontdb; `svg` feature now enables `iced_renderer/svg-text`), 3179 and 3220 (sensor), 3139 (Windows 11 blur backdrop), 3522 (Escape resets image viewer), 3494 (`markdown::Settings::table_width`). 3514 and 3450 re-checked, unchanged. 3443 skipped.

## Fork fixes
New since alpha.3: #49 (`Monitor::size()` multiplied the physical size by the scale factor; it now divides, so a 3840x2160 monitor at 2.0 reports 1920x1080; callers that compensated must drop the correction), #50 (every monitor was flagged primary). Hand-fixed markdown `CappedTable` (3494) for this fork's `Widget`/`Element` API.

Fork deviation from upstream 3522: the image viewer resets and captures Escape only when the cursor is over it or its zoom/pan is not the default; otherwise Escape stays with the application.

## Verification
`cargo check --workspace --all-targets` clean; tests pass for iced_core, iced_widget, iced_graphics, iced_tiny_skia, iced_wgpu (software Vulkan), iced_runtime.

## Not verified
No real GPU; nothing seen rendered (SVG text, Windows blur, table_width, sensor, pane_grid, image viewer).

## Breaking
- `markdown::Settings::table_width: Option<Length>` is a new public field (breaks struct-literal construction).
- The `svg` feature now also pulls in resvg text and system fonts.
- `Monitor::size()` values change on displays whose scale factor is not 1 (see #49).
- `pane_grid` (3505): a left press that publishes `on_click`, or picks a pane for `on_drag`, is no longer captured, so widgets and handlers behind it now see that press.
- `sensor` (3179, 3220): `on_show` now fires on every re-entry into view when `on_hide` is `None`.
- Image viewer (3522, with the fork deviation above): Escape resets zoom/pan, and is captured only when hovered or changed.

Otherwise as alpha.3.

## Known gaps
- SVG text (3393) clones the font database once, on the first SVG load, and never refreshes it, so fonts loaded later are not seen. A poisoned lock yields an empty database silently. This is upstream's code, left as is.
