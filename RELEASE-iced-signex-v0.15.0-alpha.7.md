# iced-signex-v0.15.0-alpha.7

Tag target: the `signex` merge commit of the `dev` -> `signex` release pull request. Builds on `iced-signex-v0.15.0-alpha.6`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there. From this release on, the notes are assembled from `changelog.d/` fragments (#87).

## Upstream PRs merged

- **tiny-skia raw text respects its clip bounds (#84, upstream #3445)** — text drawn with `Renderer::fill_raw` under tiny-skia ignored `Raw::clip_bounds` and could be clipped by a stale mask; it is now clipped like other text and like wgpu. Signex: no Signex code calls `fill_raw`, so nothing visible changes; tiny-skia renders (headless snapshots) stay consistent with wgpu.
- **wgpu picks the surface alpha mode on every configure, and failed images no longer panic tiny-skia (#88, upstream #3532, #3215)** — some Windows Vulkan drivers stop offering `PreMultiplied` after a resize or restore, which crashed on reconfigure (upstream #3387); the mode is now chosen from the current capabilities each time and falls back to `Auto`. An image that fails to decode no longer panics tiny-skia on its second lookup ("Image should be allocated"), and a 0x0 image is an `Empty` error on every backend. **Behaviour change:** on such drivers a transparent window can turn opaque instead of crashing; a 0x0 RGBA image measures 1x1 on wgpu. Signex: windows survive resize/restore on the affected Windows drivers; tiny-skia renders (headless snapshots) no longer crash on a broken image. Not checked on an affected Windows machine.
- **Meshes with infinite clip bounds are drawn, and `Rectangle::offset` handles rectangles outside their container (#89, upstream #3291, #3379)** — `Rectangle::INFINITE` turned into NaN under any transformation, so a mesh clipped to it vanished on wgpu; it now stays infinite and is not snapped. `Rectangle::offset` returned no offset for a rectangle that does not overlap its container. **Behaviour change:** tiny-skia no longer redraws infinitely clipped items every frame; `offset` now moves non-overlapping rectangles. Signex: no Signex code uses `Rectangle::INFINITE` clip bounds or `Rectangle::offset`, so nothing visible changes.
- **Cached text is reshaped after its font loads, and `rich_text` ellipsis works (#90, upstream #3352, #3484)** — text shaped with a fallback before its font was loaded kept the fallback shape in the text buffer cache; the cache now starts over when the font system changes (on wgpu, text inside a `canvas::Cache` still waits for that cache to be cleared). `rich_text(..).ellipsis(..)` had no effect and now truncates; the "…" takes the first span's color and size. Signex: no change; it registers every font at startup and does not use `rich_text` ellipsis.

## Added

- **Custom mouse cursors (#91, upstream #3240)** — `window::create_cursor(rgba, size, hotspot)` turns pixels into a cursor and returns its `window::cursor::Id`; a widget shows it with `mouse::Interaction::Custom(id)`. An unknown id shows the default arrow, and the cursor-grab rules of #42 still apply. **Breaking:** new variants on `mouse::Interaction` and `window::Action`, so exhaustive matches on them need a new arm. Signex: nothing changes until it uses the API; it can now draw its own tool cursors (for example a crosshair for placing parts or wires). Signex has no exhaustive match on either enum.
- **SVG `currentColor` from the text color (#92, upstream #3434)** — `svg(..).style(svg::current_color)`, or `inherit_current_color: true` in `svg::Style`, paints an icon's `currentColor` with the parent's text color, so an icon next to a label follows the theme without flattening its other colors. Translucent colors keep their alpha. **Breaking:** new public fields `svg::Style::inherit_current_color` and `core::svg::Svg::current_color`, so struct literals need them or `..Default::default()`. Signex: its 13 `svg::Style { color: .. }` literals without a `..` base need `..Default::default()`; icons drawn with `currentColor` can then follow the theme text color instead of a flat `color` tint.
- **Apps learn when the native macOS menu fails to build (#93, #37)** — `system::menu_failures()` yields the error text once per distinct menu whose build failed; before, the failure was only logged, and a failed first install left no menu bar at all with `Settings::default_menu: false`. **Breaking:** new variant `subscription::MacOS::MenuFailed(String)` on a public exhaustive enum. **Behaviour change:** a failed rebuild writes the new menu's enabled/checked flags onto the kept old menu's rows with the same id right away (before, on the next sync), and a menu taken down and given again is retried. Signex: lets the move of its macOS menu bar to `iced::menu` (signex#2255) keep the in-window menu row as the fallback when the native menu cannot be built.

## Fixed

- **Dashed canvas strokes on wgpu keep to their subpaths and corners (#85, upstream #3501)** — a dashed path no longer draws stray lines between subpaths, cuts corners or drops a shortened last dash, and on a closed shape the dashes join through the start point. **Behaviour change:** the dash pattern restarts at each subpath; a pattern with no positive length strokes solid; a path with more than a million dashes strokes solid. Signex: the footprint editor draws its guides with `[6, 4]` dashes (`library/editor/footprint/canvas/draw/background.rs`); a guide made of several subpaths, or one ending mid-dash, now dashes correctly.
- **Images and SVGs on wgpu draw in submission order (#86, upstream #3512)** — every nearest-filtered image (all SVGs) used to draw before every linear raster in a layer, so an SVG drawn over a raster image, in the same layer or in one merged into it, ended up under it. Alternating filters now cost a few more draw calls. Signex: SVG icons (icon set, dock, menu bar, tree view, active bar) can no longer disappear under a raster image drawn before them; whether a current Signex screen hit this was not checked.

## Breaking

- `mouse::Interaction::Custom` and `window::Action::CreateCursor` (#91), `subscription::MacOS::MenuFailed` (#93): new variants on public exhaustive enums.
- `svg::Style::inherit_current_color` and `core::svg::Svg::current_color` (#92): new public fields; struct literals without `..Default::default()` stop compiling. Signex needs this in its 13 `svg::Style` literals.

## Behaviour changes

- #85: dash patterns restart per subpath; zero-length or over-long patterns stroke solid.
- #88: on drivers that drop `PreMultiplied`, a transparent window can turn opaque instead of crashing; a 0x0 RGBA image measures 1x1 on wgpu.
- #89: tiny-skia no longer redraws infinitely clipped items every frame; `Rectangle::offset` moves non-overlapping rectangles.
- #93: a failed native menu rebuild updates the kept menu's flags at once; a menu taken down and given again is retried.

## Verification

Each change was merged into `dev` through its own pull request with Fork CI (`fmt`, `clippy`, `nextest --workspace --all-features`, wasm check) green. #93 had two adversarial review rounds and a regression review; its macOS-only broadcast and click-map paths are not covered by tests.

## Not verified

#88 on an affected Windows Vulkan driver; #91 custom cursors on Windows, X11 and Wayland; #92 by running an example with a `currentColor` icon; #93 with a real muda build failure on a Mac; Signex has not been built against this release yet (the `svg::Style` fix lands with the re-pin).
