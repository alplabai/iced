# iced-signex-v0.15.0-alpha.5

Tag target: the `signex` merge commit of the `dev` -> `signex` release pull request. Builds on `iced-signex-v0.15.0-alpha.4`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there.

## Upstream PRs merged
3282 (`text_input` `on_focus` / `on_blur`, hand-ported; fork #34).

## Fork fixes and features
New since alpha.4:
- Text and canvas: #38 steps 1 and 2 (rotated and other outline-drawn canvas text reuses a shared swash cache and a per-thread glyph mesh cache: tessellated once per glyph and tolerance bucket, vertices transformed per frame), #40 (`canvas::Frame::draw_primitive` draws a custom wgpu primitive between geometry in draw order), #43 (OpenType font features on `Font`), #6 (opt-in `font-cache` feature: on-disk system font index).
- Widgets: #33 stage 1 (button keyboard focus and focus ring; a pointer press never focuses a button; no built-in Tab traversal), #32 (`VirtualList`, `scrollable::reveal`), #44 (`push_boxed` / `extend_boxed`), #74 (`combo_box` non-ASCII search, keyboard menu scrolling), #52 (image `Viewer` captures Escape only when its view changed).
- Windows and platform: #35 slice 1 (`window::Settings::parent`, owned windows on Windows and macOS), #36 (`window::Event::ModeChanged`), #37 (native macOS menu bar and cancellable quit requests; off unless the program returns a menu / sets `exit_on_quit_request: false`), #42 (cursor grab and `RawMotion`).
- Rendering robustness: #48 phase 1 part (device loss and uncaptured wgpu errors are logged instead of panicking in release; validation errors still panic in debug or with `ICED_WGPU_PANIC_ON_ERROR`), #47 (richer adapter errors, `ICED_SOFTWARE_FALLBACK`; default `Allow` keeps today's adapter selection), #39 (keyed shader primitive pipelines; default key `0` keeps one pipeline per type), #10 (tiny-skia clears only the previous clip region).
- Testing: #45 part (tolerant snapshot comparison, configurable scale factor).
- CI: #77 (checkout v7).

## Verification
Each change was merged into `dev` through its own pull request with Fork CI (`fmt`, `clippy`, `nextest --workspace --all-features`, doc tests, wasm check) green. #38 step 2 was measured in Signex's schematic render bench (alplabai/iced#79). Signex trunk builds against `dev` with one source fix (below).

## Not verified
Platform-specific window work (#35, #36, #37, #42) beyond what each pull request states; nothing was run on Windows, X11 or Wayland for this release as a whole.

## Breaking
- `button::Style` gains a public field `focus_ring: Option<Border>` (#33): struct literals without `..Default` stop compiling.
- `Font` gains a public field `features` (#43): struct literals need `..Font::default()`.
- `window::Settings` gains a public field `parent` (#35).
- New variants on public exhaustive enums: `section::Kind::Primitive` (#40), `window::Event::ModeChanged` (#36).
- New public API in `iced_graphics::geometry`: `Text::visit_glyphs`, `Glyph`, `GlyphKey`, `GlyphOutline` (#38).
- Behaviour: Tab cycling includes enabled buttons for programs that call `focus_next` / `focus_previous` (#33); Enter or Space presses a focused button.
- Behaviour: in release builds an uncaptured wgpu error no longer panics; it is logged (#48).

Otherwise as alpha.4.

## Signex impact
- `vendor/iced_aw/src/widget/card.rs` builds a `button::Style` literal and needs `focus_ring: None`; that is the only compile error when Signex trunk builds against this release.
- Signex never calls `focus_next` / `focus_previous` and never sets a menu or `exit_on_quit_request`, so #33's Tab cycling and #37 do not change its behaviour.
- Rotated schematic text is 2-4x faster on cache-miss frames (zoom, fractional pan, first draw) on both the canvas and the GPU mount (#38).
- A wgpu validation error at runtime now logs instead of crashing in release builds (#48); it should reach Signex's Messages panel.

## Known gaps
- #38: each frame still lays out (shapes) outline-drawn text; only tessellation is cached. Step 3 (atlas rotation) is not done.
- #48: a device loss inside a blocking `device.poll` can still panic inside wgpu.
