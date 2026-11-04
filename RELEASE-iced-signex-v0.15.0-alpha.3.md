# iced-signex-v0.15.0-alpha.3 (draft)

Tag target: to be tagged by the maintainer at the final pushed HEAD of branch `claude/adoring-goldberg-47425s` (sha recorded in the push report; not tagged here).

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full merged/skipped list.

## Upstream PRs merged
3513 (text editor selection cache), 3521 (checkbox selection offset), 3505 (pane_grid text selection), 3529 (Rectangle::chord), 3393 (SVG text via iced fontdb; `svg` feature now enables `iced_renderer/svg-text`), 3179 and 3220 (sensor), 3139 (Windows 11 blur backdrop), 3522 (Escape resets image viewer), 3494 (`markdown::Settings::table_width`). 3514 and 3450 re-checked, unchanged. 3443 skipped.

## Fork fixes
Hand-fixed markdown `CappedTable` (3494) for this fork's `Widget`/`Element` API.

## Verification
`cargo check --workspace --all-targets` clean; tests pass for iced_core, iced_widget, iced_graphics, iced_tiny_skia, iced_wgpu (software Vulkan), iced_runtime.

## Not verified
iced_winit tests not run (wayland libs missing); no real GPU; nothing seen rendered (SVG text, Windows blur, table_width, sensor, pane_grid, image viewer).

## Breaking
New public field `markdown::Settings::table_width: Option<Length>` (breaks struct-literal construction). The `svg` feature now also pulls in resvg text and system fonts. Otherwise as alpha.1.
