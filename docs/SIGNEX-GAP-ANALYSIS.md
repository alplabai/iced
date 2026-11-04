# iced fork gap analysis for Signex

Date: 2026-11-04 · Fork commit: c1d63c8 · Signex: PR #2028 head

## 1. Purpose and method

Purpose: find what is missing in the iced fork (this repository) that Signex, an EDA application built on it, needs or would benefit from, and describe each gap precisely enough to open an issue.

Method:

- Eight area surveys read the fork and Signex statically: widget library; windowing, docking and chrome; canvas and rendering at scale; input and events; text, fonts and text editing; accessibility, i18n and platform integration; performance, testing and developer experience; upstream landscape mapping.
- Each survey produced candidate gaps. 30 candidates were then checked by adversarial verification: a skeptic tried to refute the claim against the code, and could correct the proposal and priority.
- Verdicts used:
  - **confirmed**: the gap exists as described.
  - **partly**: the gap exists but the evidence, scope, effort or priority was wrong. The report uses the corrected proposal and corrected priority.
  - **refuted**: the capability already exists, so it is not a gap.
- Result: 2 confirmed, 27 partly, 1 refuted. Priorities below are the corrected ones, not the surveyors' originals.
- Further candidates that were never verified are listed in section 7 and are not claims.

## 2. Executive summary

- 29 verified gaps; 1 refuted candidate (section 6).
- After correction no verified gap is P0 or P1. 22 are P2 and 7 are P3. Nearly every original P1 was lowered because Signex works today with a workaround; the gaps are about deleting workaround code, robustness and polish rather than unblocking features.
- Several candidates overlap and should be built together: windowing core (table-sort-resize-virtual, virtual-list-scrollable-viewport), focus (widget-disabled-and-focusable, keyboard-focus-ring-tab-order), focus events (text-input-focus-events, text-input-focus-blur-and-validation, textinput-focus-blur-events), rotated text (canvas-rotated-text-gpu, text-rotated-canvas-text).
- Top five by value for effort:
  1. text_input `on_focus`/`on_blur` (adopt upstream PR #3282). Small, real, removes a documented workaround.
  2. Layout-independent shortcuts. Signex-only fix using the existing `Key::to_latin`; no fork work.
  3. Window `ModeChanged` events. Small, deletes about 70 lines and a 900 ms heuristic.
  4. Fixed-height virtual list with reveal operation. Replaces about 500 lines of non-test code in Signex.
  5. Keyboard focus for button, checkbox, radio, toggler and pick_list. The real half of the disabled/focus proposal.
- Recommended order: quick wins (S), then M items, then L/XL items only on concrete demand. See section 8.

## 3. Summary table

Effort and priority are as corrected by verification. Status is the verification verdict.

| id | title | area | kind | priority | effort | status |
|---|---|---|---|---|---|---|
| table-sort-resize-virtual | Virtualised table rows and resizable column handle | Widget library | fork-feature | P2 | M (+S) | partly |
| virtual-list-scrollable-viewport | Virtual list and scroll-into-view operation | Widget library | fork-feature | P2 | M | partly |
| widget-disabled-and-focusable | Focusable built-in widgets, disabled gaps | Widget library | fork-feature | P2 | M | partly |
| text-input-focus-events | on_focus/on_blur, number_input deferred | Widget library | adopt-upstream-pr | P2 (P3 number_input) | S-M | partly |
| window-parent-owner-modal | Window parent/owner and set_enabled | Windowing | fork-feature | P2 | M | partly |
| window-state-events | Window mode change events | Windowing | fork-feature | P2 | S | confirmed |
| macos-native-menu-hooks | macOS default-menu opt-out and quit request | Windowing | fork-feature | P2 | M | partly |
| canvas-rotated-text-gpu | Rotated text through the glyph atlas | Canvas/rendering | fork-feature | P2 | L | partly |
| primitive-instance-keying | Keyed Primitive pipeline storage | Canvas/rendering | fork-feature | P2 | M | partly |
| primitive-in-canvas-order | Primitive inside canvas Frame order | Canvas/rendering | fork-feature | P2 | M | partly |
| layout-independent-shortcuts | Layout-independent keymap matching | Input/events | signex-side | P2 | S | partly |
| cursor-grab-and-raw-motion | Cursor grab and raw motion | Input/events | adopt-upstream-pr | P2 | M | partly |
| text-rotated-canvas-text | Cached rotated canvas text, wrapping/ellipsis | Text | fork-feature | P2 | S then M | partly |
| text-font-features | OpenType font features | Text | fork-feature | P2 | M | partly |
| text-input-focus-blur-and-validation | text_input focus events and filter | Text | adopt-upstream-pr | P2 | M | partly |
| a11y-accesskit-tree | AccessKit accessibility tree | Accessibility | fork-feature | P3 | XL | partly |
| keyboard-focus-ring-tab-order | Tab traversal, focus ring | Accessibility | fork-feature | P2 | M | partly |
| heterogeneous-children-ergonomics | push_boxed / extend_boxed | Dev experience | fork-feature | P2 | S | partly |
| headless-deterministic-goldens | Deterministic headless goldens | Dev experience | fork-feature | P2 | M | partly |
| textinput-focus-blur-events | Adopt #3282 for commit-on-blur | Upstream mapping | adopt-upstream-pr | P2 | S | confirmed |
| redraw-scheduling-audit-multiwindow | Redraw and idle CPU audit | Upstream mapping | fix-upstream-issue | P2 | M | partly |
| gpu-adapter-selection-and-failure-ux | Software adapter fallback and GPU failure UX | Upstream mapping | adopt-upstream-pr | P2 | M | partly |
| menu-context-menu-core | Overlay fit-within helper, menus | Widget library | fork-feature | P3 | S (helper) / L | partly |
| combo-box-search-keyboard | combo_box unicode matching, keyboard scroll | Widget library | adopt-upstream-pr | P3 | S | partly |
| window-resize-relayout | window::resize applies size immediately | Windowing | fix-upstream-issue | P3 | S | partly |
| file-drop-position-wayland | File drop grouping and position | Windowing | adopt-upstream-pr | P3 | S / M | partly |
| primitive-prepare-encoder-and-multipass | prepare_encoder hook | Canvas/rendering | fork-feature | P3 | M | partly |
| headless-renderer-config | HeadlessSettings | Canvas/rendering | fork-feature | P3 | S-M | partly |
| wgpu-device-lost-and-errors | Device-lost and uncaptured error handling | Canvas/rendering | fork-feature | P2 | M then L | partly |
| multiwindow-redraw-starvation | (refuted) | Windowing | n/a | n/a | n/a | already-present |

## 4. Detailed sections

Paths beginning `crates/` are in Signex (PR #2028). Other paths are in this fork. Upstream links use https://github.com/iced-rs/iced/issues/N (pull requests resolve through the same URL).

### 4.1 table-sort-resize-virtual: virtualised rows and resizable column handle

**Priority P2 · Effort M, plus S · fork-feature · partly**

What Signex needs today:
- The library browser hand-rolls a table stack: fixed-height row windowing with spacers (`crates/signex-app/src/library/browser/window/mod.rs`, 139 lines plus 658 lines of tests, OVERSCAN_ROWS=12), `size_sensor.rs` (199 lines), `keyed.rs` (149 lines), plus `columns.rs` (529), `header.rs`, `sort.rs` (208). `grid.rs:448` uses `on_scroll`.
- `size_sensor.rs` exists because scrollable reports a viewport only when content overflows. `keyed.rs` makes inline-editor focus follow a row key rather than slot position.
- Target scale is 10k+ rows. Parameter tables, net classes and ERC lists repeat the pattern.

What iced provides: `widget/src/table.rs` (666 lines) is a static layout helper with width, padding and separator only. No windowing, no resize or sort callbacks. `keyed::Column` reconciles by key but does not window. `mouse::Interaction::ResizingColumn` exists (`core/src/mouse/interaction.rs:28`, mapped at `winit/src/conversion.rs:556`). `widget/src/sticky.rs` and `widget/src/sensor.rs` exist.

Upstream: [#160](https://github.com/iced-rs/iced/issues/160) InfiniteList (open, 11 comments, 17 reactions); [#3159](https://github.com/iced-rs/iced/issues/3159) table styling only.

Proposal (corrected, split):
1. Primary, M: fixed-row-height virtualised list widget (see 4.2; share the windowing core). Takes row count, row height and a build-row closure keyed by index or key. Owns scrolling and reads viewport height from layout every frame. Builds visible range plus overscan, and reconciles tree state by user key so `text_input` focus follows the key.
2. Secondary, S: a reusable column-separator drag handle helper with the `ResizingColumn` cursor and an `on_resize` callback, as its own widget rather than inside `table`.
3. Sort and header styling stay app-side. Do not file sticky header as a separate item; `sticky` exists.

Risk: tree-state reconciliation under windowing is the subtle part. Variable row heights out of scope.

Acceptance: 50k rows built with constant widget count (test counts built rows); key-follows-state tests modelled on `widget/tests/keyed_column.rs`; drag handle emits clamped `on_resize` with resize cursor.

Verification notes: resize and sort are app-side in Signex (`columns.rs`, `sort.rs` would not be deleted by a callback layer). The windowing and key reconciliation part is what earns its place; it removes about 500 lines of non-test code, mostly two small wrappers.

### 4.2 virtual-list-scrollable-viewport: virtual list and scroll-into-view

**Priority P2 · Effort M · fork-feature · partly. Shares the windowing core with 4.1.**

What Signex needs: hand-computed scroll-to-reveal for keyboard row navigation (`library/browser/window/mod.rs:81` `offset_to_reveal`, `:98` `reveal_row_offset`). `signex-widgets/src/tree_view.rs` (793 lines) has no windowing; every expanded node builds widgets. History and ERC/message lists would also benefit.

What iced provides: `core/src/widget/operation/scrollable.rs` has `snap_to` (38), `scroll_to` (77), `scroll_by` (116) only. No reveal or scroll-into-view. No list module in `widget/src`. `widget/src/sensor.rs` already provides `on_resize(Size)` (`sensor.rs:19,55-59`), comparing laid-out bounds with `last_size` every `RedrawRequested` (`:196-203`) independent of overflow.

Upstream: [#160](https://github.com/iced-rs/iced/issues/160).

Proposal (corrected):
1. Core `Operation` `scrollable::reveal(Id, Rectangle or range)` (scroll-into-view) that adjusts the offset minimally, optional animation like `scroll_to`.
2. `widget::list::virtual_list(len, row_height, |i| Element)` wrapping a scrollable on the shared windowing core, with optional key function and `scroll_to_index`.
3. Drop the proposed `scrollable::on_viewport_change`. Document and test wrapping a scrollable in `Sensor::on_resize` as the supported way to learn viewport size when content fits. If level-triggered re-sync is wanted, add a small Sensor option (for example `on_resize_where_differs(known)`).

Risk: variable heights deferred; `scroll_to_index` needs stable Id and known row height.

Acceptance: 100k-item list builds O(viewport) widgets; reveal brings an off-screen row into view from an Operation; `tree_view` can adopt it without behaviour change.

Verification notes: the claim "no size callback independent of overflow" was refuted by `Sensor`. Signex's `SizeSensor` (`size_sensor.rs:1-30`) is level-triggered re-sync against the model's known height, a nicety that does not justify a core change.

### 4.3 widget-disabled-and-focusable (merged with 4.17)

**Priority P2 · Effort M · fork-feature · partly**

What Signex needs: disabled controls via `on_press_maybe` (18 call sites) and per-button disabled styling; Tab navigation in dialogs is unproven as a requirement (no dialog flow shown that depends on it).

What iced provides: the disabled half is mostly already present. `Status::Disabled` exists in button (`widget/src/button.rs:343,461`), checkbox (`checkbox.rs:347-351,530`), toggler (`toggler.rs:363,559`, plus `NotAllowed` cursor at `:398`), pick_list (`pick_list.rs:530`), text_input (`text_input.rs:371,532`), combo_box (`combo_box.rs:660`). Disabled is implicit via callback `None`. Not confirmed: slider and radio. Focus is real: `Focusable` is implemented only for text_input (`widget/src/text_input.rs:475`) and core text input/editor; `focus_next`/`focus_previous` exist (`core/src/widget/operation/focusable.rs:104,142`) so traversal skips buttons, checkboxes, togglers and pick_lists.

Upstream: [#1066](https://github.com/iced-rs/iced/issues/1066) disabled state; [#489](https://github.com/iced-rs/iced/issues/489) focus/tab (32 reactions); [#750](https://github.com/iced-rs/iced/issues/750).

Proposal (corrected): implement `operation::Focusable` for button, checkbox, toggler, radio, pick_list (slider optional); Space/Enter activate; widgets with a `None` callback report unfocusable so `focus_next` skips them; add an additive focus-ring field to their Styles. Audit slider and radio for a Disabled status. Skip the generic disabled wrapper (it cannot restyle children). See 4.17 for the focus-visible design.

Acceptance: Tab/Shift+Tab cycles through the widgets in tree order; Space/Enter activate; disabled widgets skipped; ring visible.

Verification notes: do not add `Status` variants where they already exist. Signex must first show a concrete dialog flow needing Tab.

### 4.4 text-input-focus-events: on_focus/on_blur (merged with 4.15 and 4.20)

**Priority P2 for focus events, P3 for number_input · Effort S-M · adopt-upstream-pr · partly**

What Signex needs: `crates/signex-app/src/panels/numeric_draft.rs` (217 lines) documents that text_input has no blur event; it is referenced from 8 files. Signex also uses `iced_aw::NumberInput` (`library/editor/footprint/body3d.rs:71,84`, `panels/properties_parameters/form_rows.rs:420`, `panels/mod.rs:7`) from `vendor/iced_aw` (path dependency, `Cargo.toml:62`).

What iced provides: `widget/src/text_input.rs` has no `on_focus` or `on_blur`; grep of `widget/` and `core/` finds nothing. FORK.md does not mention #3282.

Upstream: [#819](https://github.com/iced-rs/iced/issues/819) (focus/blur messages), PR [#3282](https://github.com/iced-rs/iced/issues/3282), [#3197](https://github.com/iced-rs/iced/issues/3197) (arrow up/down), [#3239](https://github.com/iced-rs/iced/issues/3239), [#209](https://github.com/iced-rs/iced/issues/209) (spinner).

Proposal (corrected): add `TextInput::on_focus` and `on_blur`, based on #3282, emitted from `State::focus` transitions: click, click-away, Tab, programmatic focus operations, window focus loss. Keep the app-owned draft/owner/stored-value logic, since it handles undo and external changes. Defer a first-party `number_input` until the vendored iced_aw `NumberInput` becomes a burden; it would need to take the value from the app each frame and reconcile it.

Risk: #3282 touches text_input update ordering and conflicts with fork text-selection and IME fixes are likely (the PR is on a 2026-03 base). Blur must fire exactly once.

Acceptance: blur fires on click-away, Tab and window focus loss; focus fires on click and focus operation; a regression test shows a draft clears on blur.

Verification notes: "drop numeric_draft.rs" is unrealistic; at most it becomes a simpler shim. See 4.15 and 4.20 for the overlapping findings.

### 4.5 window-parent-owner-modal

**Priority P2 · Effort M (corrected from L) · fork-feature · partly**

What Signex needs: detached dialogs as OS windows (`docs/DETACHED_MODALS.md`; `crates/signex-app/src/app/handlers/erc/modals.rs:550-576`). iced has no owner relationship, so position is `Position::Centered` on screen, dialogs can fall behind the main window (the first-run tour is kept in-window for this reason, DETACHED_MODALS.md:31), and the lock policy is an in-window dim layer plus re-raise (DETACHED_MODALS.md:75).

What iced provides: no parent, owner, `set_enabled` or `CenteredOn` in `core/src/window`, `winit/src/conversion.rs:57-128` or `runtime/src/window.rs`.

Upstream: no dedicated issue; related [#254](https://github.com/iced-rs/iced/issues/254), [#3108](https://github.com/iced-rs/iced/issues/3108), [#3443](https://github.com/iced-rs/iced/issues/3443) (skipped in fork, FORK.md:50).

Proposal (corrected): (1) `window::Settings::parent: Option<window::Id>`, resolved in the Open path against the window manager and mapped to winit's unsafe `with_parent_window` (Windows owner, X11 transient_for, macOS child window); unknown id ignored with a warning; closing the owner closes children. (2) `window::set_enabled(id, bool)`: `EnableWindow` on Windows, event dropping elsewhere, always still delivering `CloseRequested` and `Focused`. Defer `Position::CenteredOn` (Signex can compute a `Specific` position). Drop skip_taskbar/hidesOnDeactivate work; Signex wants dialogs in the taskbar (modals.rs:553-557).

Risk: platform differences (Wayland xdg_toplevel parent, macOS child windows move with parent); winit parent API is unsafe; untested on a real compositor/GPU.

Acceptance: dialog with parent stays above its owner; `set_enabled(owner,false)` blocks input but still delivers CloseRequested/Focused; closing the owner behaviour documented and tested; Signex can delete the lock layer for locking confirms.

Verification notes: grep confirmed absence. Signex works today.

### 4.6 window-state-events

**Priority P2 · Effort S · fork-feature · confirmed**

What Signex needs: `crates/signex-app/src/app/handlers/window_controls.rs:15-80` polls `window::mode`, monitor and size, with `deferred_requery`, `FULLSCREEN_EXIT_SETTLE` = 900 ms and 3 retries; `is_effectively_fullscreen` at `chrome/controls.rs:210`, used at `app/state/ui.rs:601`. `ToggleMaximizeMainWindow` (`dispatch/mod.rs:500`) has no feedback.

What iced provides: `core/src/window/event.rs` has Opened, Closed, Moved, Resized, Rescaled, RedrawRequested, CloseRequested, Focused, Unfocused and file events. `runtime/src/window.rs:338-349` offers one-shot `is_maximized`/`is_minimized` Tasks. `Occluded` is used only internally (`winit/src/lib.rs:1055`).

Upstream: related bugs [#3181](https://github.com/iced-rs/iced/issues/3181), [#3156](https://github.com/iced-rs/iced/issues/3156).

Proposal: emit `window::Event::ModeChanged { maximized, minimized, fullscreen }` from iced_winit after Resized, Moved, Occluded and after the Maximize/SetMode/Minimize actions, by diffing the window's own flags against a cached value in `winit/src/window/state.rs`. Put the diff in a pure function for testing. Drop the optional Occluded event or file separately.

Risk: macOS fullscreen animation (flag flips at completion, which is the point); Wayland maximize arrives asynchronously.

Acceptance: one event per maximize/unmaximize/minimize/restore/fullscreen enter/exit; fires for programmatic changes; no event on plain resize; unit test of the diff; Signex can delete the polling chain.

Verification notes: workaround works, so this is polish: about 70 lines plus a 900 ms heuristic.

### 4.7 macos-native-menu-hooks

**Priority P2 (P1 only if Dock Quit data loss is unacceptable) · Effort M · fork-feature · partly**

What Signex needs: `docs/MENU_BAR.md` builds an NSMenu with muda. winit installs a default menu, so Signex installs after `WindowMsg::MainWindowOpened`. Dock Quit and logout send `terminate:` and bypass the unsaved-changes guard (MENU_BAR.md:117-120).

What iced provides: `winit/src/lib.rs:85` builds the loop with bare `EventLoop::with_user_event()`; no EventLoopBuilder or platform ext. Only per-window `CloseRequested` (`conversion.rs:177`); `Control::Exit` (`lib.rs:409`) is outbound.

Upstream: [#114](https://github.com/iced-rs/iced/issues/114), [#124](https://github.com/iced-rs/iced/issues/124), PR [#3021](https://github.com/iced-rs/iced/issues/3021) (skipped), PR [#3415](https://github.com/iced-rs/iced/issues/3415) (winit 0.31 draft).

Proposal (corrected): (1) macOS-only application setting building the loop via `EventLoopBuilder` with `with_default_menu(false)`; first check the pinned winit fork (rev 4811646, `Cargo.toml:251`) exposes it, patching it if not. (2) A cancellable app-level quit request for Cmd+Q, Dock Quit and logout via an `applicationShouldTerminate:` hook in the winit fork. Drop `RunOnMainThread` (`update` already runs on the main thread). Tray is out of scope.

Risk: cannot be tested without a Mac; no CI for it; logout veto needs Apple's reply API.

Acceptance: no winit default menu items with the flag off; quit requests surface and can be vetoed, or limitations documented per OS; Signex removes the install-timing workaround.

### 4.8 canvas-rotated-text-gpu (merged with 4.13)

**Priority P2 · Effort L · fork-feature · partly**

What Signex needs: rotated schematic and PCB labels. `crates/signex-gfx/src/pipeline/text.rs:59-66` filters atlas items by `is_axis_aligned()` because a cryoglyph `TextArea` cannot rotate; `SchematicStaticTextLayer` (`gpu_mount.rs:12,35`) is a CPU layer for rotated text.

What iced provides: `wgpu/src/geometry.rs:359-410` `fill_text` uses `Text::Cached` only for uniform scale plus translation; otherwise `text.draw_with` tessellates glyphs (line 409). `wgpu/src/text.rs:~628-640` builds `cryoglyph::TextArea` without rotation. FORK.md has no rotated-text entry.

Upstream: none found; cryoglyph is a separate repo (iced-rs/cryoglyph).

Proposal (corrected, split): (A) cryoglyph fork: per-`TextArea` 2x2 rotation/mirror in the glyph vertex shader with raster size quantised to effective scale. Do this first; it lets signex-gfx drop the `is_axis_aligned` filter and `SchematicStaticTextLayer` with no iced API change. Treat exact 90/180/270 separately (swizzled quads, no blur). (B) Optional: rotation in core `Text`/`Text::Cached` for generic iced canvas users, falling back to outlines for skew. Drop the tiny_skia claim.

Risk: non-axis rotation of bitmap glyphs gets blurry; keeping a cryoglyph fork in sync.

Acceptance: gfx scene draws 10k rotated labels through the atlas; CPU rotated-text layer removed; no regression in unrotated hinting.

Verification notes: Signex GPU scene uses its own pipeline, not `Frame::fill_text`, so fixing only the iced Frame does not help it. See 4.13 for the cheaper outline-path fixes.

### 4.9 primitive-instance-keying

**Priority P2 · Effort M · fork-feature · partly**

What Signex needs: `scene_shader.rs:77-94,122-173` markers `PcbSurface`, `SchematicSurface`, `SchematicDragSurface`; the drag overlay is a second type because iced prepares every primitive of one type before drawing.

What iced provides: `wgpu/src/primitive.rs:104-135,190-222`: storage is `FxHashMap<TypeId, Box<dyn Pipeline>>`, one pipeline per type; `Pipeline::new` takes no key; `Pipeline::trim` never evicts a pipeline.

Upstream: [#766](https://github.com/iced-rs/iced/issues/766), [#903](https://github.com/iced-rs/iced/issues/903); no PR for keyed storage.

Proposal (corrected): `fn pipeline_key(&self) -> u64 { 0 }` on `Primitive`; key storage by `(TypeId, u64)`; keep `Pipeline::new` unchanged; add a defaulted `Pipeline::new_keyed(device, queue, format, key)` delegating to `new`; evict pipelines unprepared for N frames using a frame counter in `Storage`, with a Drop-counter test. Do not promise removal of the `SceneSurface` generic, which also carries style policy (TEXT_SIZE, MIN_RADIUS_PX).

Risk: unbounded keys; eviction rule is required. Benefit: removes `SchematicDragSurface` and avoids duplicate setup. PCB and schematic share a slot on purpose (different tabs). Mirrors in other windows already have separate Renderers.

Alternative (cheaper): one pipeline per type holding per-instance slots indexed by an id carried in the primitive, possible today.

Acceptance: two same-type widgets with different keys render different data; default key unchanged; unused pipelines dropped; `SchematicDragSurface` removable.

### 4.10 primitive-in-canvas-order

**Priority P2 · Effort M · fork-feature · partly**

What Signex needs: `gpu_mount.rs:28-43` five-layer `stack!` (canvas backdrop, shader content, canvas static text, shader drag, canvas overlay); `docs/architecture/schematic-gpu-mount.md` documents the single-update-layer constraint and the `PublishedDrag` side channel.

What iced provides: `graphics/src/geometry/section.rs:20-30` has `Kind` Shape, Image, Text only; `wgpu/src/layer.rs:348` `Layer::draw_primitive` and `:421-447` draw primitives from a fixed slot before images and text.

Upstream: none; precedent #18/#3017 in the fork; [#3512](https://github.com/iced-rs/iced/issues/3512).

Proposal (corrected): add `Kind::Primitive` between Shape and Image; an opt-in extension (for example `PrimitiveFrame`, or `Frame::draw_primitive` with a no-op default) pushing into the section's primitive batch via the existing `Layer::draw_primitive`; `seal`/`paste` carry the batch; tiny_skia no-op. No fallback closure in the API. Drop "mirror shader::Program State": it already exists (`widget/src/shader.rs:83,147,150`).

Honest benefit: Signex could drop the backdrop and static-text layers (two of five); the drag shader stays a separate primitive type since primitives are prepared per type.

Acceptance: geometry, primitive, geometry rendered in that order (screenshot test); tiny_skia no panic.

### 4.11 layout-independent-shortcuts

**Priority P2 · Effort S · Signex-side · partly**

What Signex needs: `crates/signex-app/src/keymap/binding.rs:100` `KeyStroke::from_iced(key, modifiers)` lowercases `Key::Character` and never sees `physical_key`. Callers in `app/dispatch/input.rs` (201, 354, 517) and `app/dispatch/keymap.rs` (331-408). 24 `Key::Character` matches in canvas handlers. Synthetic test events use an unidentified physical key (`app/dispatch/input.rs:603`).

What iced provides: `KeyPressed` carries `physical_key` (`core/src/keyboard/event.rs:22`); `Key::to_latin(Physical)` exists (`core/src/keyboard/key.rs:60`, Cyrillic doctests); `text_editor` already uses it (`core/src/text/editor.rs:865`).

Upstream: none needed; related [#489](https://github.com/iced-rs/iced/issues/489).

Proposal (corrected): Signex-only. `from_iced` takes key, physical_key, modifiers; for `Key::Character` use `key.to_latin(physical_key)` lowercased with fallback to the logical char; route all callers and the 24 canvas matches through one helper; add Cyrillic ("с" + `Code::KeyC`, "я" + `Code::KeyZ`) and Greek fixtures; use real `Physical::Code` in synthetic events. Optional follow-up: a `KeyToken::Code` form with a profile version bump. No fork `Event::shortcut_key`; at most a doc note and a Cyrillic/Greek unit test for `to_latin`.

Risk: changing matching can break profiles on non-US layouts; keep logical match first.

Verification notes: Dvorak behaviour is unchanged (`to_latin` keeps the logical Latin char); the original Dvorak criterion was wrong. Greek works only via the physical mapping.

### 4.12 cursor-grab-and-raw-motion

**Priority P2 · Effort M · adopt-upstream-pr · partly**

What Signex needs: pan uses absolute positions (`canvas/input/pointer.rs:321-331`), 3D orbit planned for v2.3.1 (`docs/ROADMAP.md:361`), footprint canvas hides the cursor (`library/editor/footprint/canvas/mod.rs:498-509`).

What iced provides: only `set_cursor_visible` (`winit/src/window.rs:353,356`) and `drag_window` (`lib.rs:1414`); no `CursorGrab`, `DeviceEvent` or `MouseMotion`.

Upstream: PR [#2614](https://github.com/iced-rs/iced/issues/2614) (draft, 2024), [#3417](https://github.com/iced-rs/iced/issues/3417).

Proposal (corrected): `window::Action::SetCursorGrab(Id, CursorGrab::{None, Confined, Locked})` with Locked-to-Confined fallback; opt-in `mouse::Event::RawMotion { delta }` from `DeviceEvent::MouseMotion`, forwarded only while grabbed and for the focused window; always release on focus loss, Escape and close. Treat as an enabler for 3D orbit; prototype 2D pan first. Optional: `window::set_cursor_position`.

Verification notes: the stated symptom "pan stops when the cursor leaves the window" is overstated; on Windows, macOS and X11 button capture keeps events flowing, and only the screen edge is real (Wayland may differ). Orbit is not built yet.

Acceptance: lock hides and locks (Confined fallback); deltas emitted while locked; release on blur/Escape/close; pan continues past the screen edge.

### 4.13 text-rotated-canvas-text

**Priority P2 · Effort S then M · fork-feature · partly. See also 4.8.**

What Signex needs: `renderer_scene_canvas.rs:507-585` draws non-axis-aligned items with `with_save(translate+rotate+fill_text)`; `schematic_runtime/mod.rs:490-503` replays them through the live transform. `is_axis_aligned` (`signex-gfx/src/primitive/text.rs:212`) is `rotation.abs() < EPSILON`, so exact 90/180/270 also takes the slow path.

What iced provides: `graphics/src/geometry/text.rs:56-83` `draw_with` builds a new `Paragraph`, a new `cosmic_text::SwashCache` (line 82), takes the font system write lock, and builds a lyon path per glyph, every call, with `Wrapping::default()`, `Ellipsis::default()` and `hint_factor: None`. `wgpu/src/text.rs:651` creates a fresh `cryoglyph::SwashCache` on every prepare.

Upstream: related fork #18; none specific.

Proposal (corrected): Step 1 (S): pass `self.wrapping` and `self.ellipsis` through in `draw_with`; reuse one `SwashCache` (thread_local or static) here and at `wgpu/src/text.rs:651`. Step 2 (M): cache outline paths per (font id, glyph id, size bucket) in glyph-local coordinates and emit through the frame transform; optionally cache the shaped paragraph. Add an exact quarter-turn fast path in Signex. Drop the "rotated cryoglyph section" idea here (see 4.8 part A). Keep the CPU fallback layer until a bench with a few thousand rotated labels shows parity.

Acceptance: 2000 rotated labels draw without per-frame SwashCache/Paragraph construction; wrapping/ellipsis take effect when rotated; advance and baseline within 0.5 px (anti-aliasing and hinting may differ).

### 4.14 text-font-features

**Priority P2 · Effort M · fork-feature · partly**

What Signex needs: aligned numeric columns and ligatures off. `dock/view.rs:437` `estimate_text_width` is used at `:313` for rotated rail tab labels, not numeric columns, so it is not a tnum case; `glyph_w = 6.5` in the sketch preview is likewise unrelated. Signex can use `Font::MONOSPACE` today.

What iced provides: `core/src/font.rs:5-15` `Font` has family, weight, stretch, style only (derives Copy, Eq, Hash). `graphics/src/text.rs:295-300` `to_attributes` sets no features. cosmic-text at rev 6adc236 exposes `FontFeatures` and `Attrs::font_features` (`src/attrs.rs:168,375,456,557`). The paragraph cache key includes `Font` (`graphics/src/text/cache.rs:56`).

Upstream: none open; class [#156](https://github.com/iced-rs/iced/issues/156).

Proposal (corrected): a Copy, const-friendly `font::Features` bitflags (TNUM, ZERO, LIGA_OFF, CALT_OFF, SMCP, SS01) and `Font::features`; map in `to_attributes` (verify the cosmic-text API at the pinned rev). The cache key picks it up through `Font`; add a test that fonts differing only in features do not collide. Adding a pub field is a breaking change for struct-literal users; note it in FORK.md. Verify editor and rich_text re-apply attrs (`editor.rs:629,796`). Test paragraph `min_width` with and without TNUM using bundled Fira Sans. A font only honours tnum or zero if the face has them.

### 4.15 text-input-focus-blur-and-validation

**Priority P2 · Effort M · adopt-upstream-pr · partly. Overlaps 4.4 and 4.20.**

What Signex needs: `numeric_draft.rs:20-23` blur workaround; 79 files reference `text_input`.

What iced provides: no on_focus/on_blur; builder methods are only `secure` (145), `on_input` (154), `on_submit` (170), `on_paste` (184). Cursor and selection operations already exist: `core/src/widget/operation/text_input.rs` has `move_cursor_to_front/end` (32, 57), `move_cursor_to` (82), `select_all` (107), `select_range` (131).

Upstream: [#819](https://github.com/iced-rs/iced/issues/819), PR [#3282](https://github.com/iced-rs/iced/issues/3282), [#3374](https://github.com/iced-rs/iced/issues/3374), [#3308](https://github.com/iced-rs/iced/issues/3308), [#3239](https://github.com/iced-rs/iced/issues/3239).

Proposal (corrected): cherry-pick #3282; optionally add `TextInput::filter(Fn(&str)->bool)` applied to typing and paste before `on_input`. Do not add new cursor/selection operations. Select-on-focus: Signex returns `operation::text_input::select_all(id)` from its on_focus handler. Keep #3308 separate.

Verification notes: the draft layer does more than emulate blur (it prevents re-formatting from snapping `-` and `1.` mid-typing and tracks owner and undo/redo invalidation), so it is simplified rather than deleted.

### 4.16 a11y-accesskit-tree

**Priority P3 · Effort XL · fork-feature · partly**

What Signex needs: no accessibility support or plan (no accesskit, aria or screen-reader in `crates/`; none in ROADMAP). Value is mostly VPAT and procurement; drawn canvases get little from a screen reader.

What iced provides: nothing. Grep matches only incidental "accessible" in `core/src/clipboard.rs`, `core/src/image.rs`, `graphics/src/image.rs`. No Widget hook, no accesskit_winit, FORK.md has no entry.

Upstream: PR [#1849](https://github.com/iced-rs/iced/issues/1849) (draft), PR [#3111](https://github.com/iced-rs/iced/issues/3111) (draft), issues [#552](https://github.com/iced-rs/iced/issues/552), [#282](https://github.com/iced-rs/iced/issues/282). Numbers were not verified from the repo.

Proposal (corrected): design spike first, study #1849 and #3111 and decide whether to rebase. Phase 1: AccessKit in iced_winit behind a cargo feature, tree built from the existing Operation traversal and only while assistive technology is connected; standard widgets first (button, checkbox, text_input, slider, pick_list); one live-region node for canvas state. Dock and tree-row labels are a Signex follow-up. Drop NVDA/VoiceOver/Orca acceptance runs until a spike proves the API.

Risk: upstream may shape the API differently; per-frame tree cost.

### 4.17 keyboard-focus-ring-tab-order (see 4.3)

**Priority P2 · Effort M (corrected from L) · fork-feature · partly**

What Signex needs: dialogs are mouse-driven; focus code is limited to the command palette and field edit (`app/dispatch/command_palette.rs:56`, `app/handlers/canvas/field_edit.rs:60`, `app/handlers/detached_modals.rs:58`); Tab is hand-handled in the footprint canvas (`library/editor/footprint/canvas/input/keys.rs:196`).

What iced provides: only text_input implements Focusable (`text_input.rs:475`; text_editor has `Status::Focused` at `:462/:745`). Nothing binds Tab to `focus_next`; examples wire it per app (modal, todos, toast, markdown, websocket).

Upstream: [#489](https://github.com/iced-rs/iced/issues/489), [#552](https://github.com/iced-rs/iced/issues/552).

Proposal (corrected), staged: Stage 1 (M): Focusable for button, checkbox, radio, toggler, pick_list; non-breaking focus indication via an optional `Style::focus_ring: Option<Border>` (default None) plus a keyboard-modality flag in widget state, not a new `Status` variant; Space/Enter activate the focused button. Stage 2 (S, in app): a Tab/Shift+Tab subscription issuing `focus_next`/`focus_previous` only while a dialog is open, following `examples/modal`, so the canvas keeps Tab. Stage 3 optional: `focus_scope` or `Settings::tab_navigation` in the fork.

Risk: a new `Status::Focused` variant would break custom Catalog closures (including vendored iced_aw). Focus-visible needs modality tracking in widget state.

Acceptance: Tab moves focus through dialog buttons and checkboxes and draws a ring only after keyboard focus; mouse clicks show no ring; Space activates the focused button.

### 4.18 heterogeneous-children-ergonomics

**Priority P2 · Effort S · fork-feature · partly**

What Signex needs: 1556 `boxed()` occurrences in `crates/`; only about 198 are single-line `push(...boxed())`. Examples: `signex-widgets/src/tree_view.rs:459,479,501,506,523`, `passive_calculator/rkm_encoder.rs:215`. 47 `Vec<Element>` sites.

What iced provides: `push(child: W) where W: Meta` at `widget/src/row.rs:124`, `extend` at `:136`; same in `column.rs:118/130`, `stack.rs:73/92`, `grid.rs:93/114`, `keyed/column.rs:131/156`. No `push_boxed`, no `children!`. `Option<T>` and `Void` implement `Meta` (`core/src/widget.rs:266,424`); Grid and keyed::Column already have `push_maybe`.

Upstream: the upstream change removing `Into` generics in push introduced the problem; no counterpart found.

Proposal (corrected): add `push_boxed` and `extend_boxed` to the Element-typed impls of Row, Column, Stack, Grid, and `keyed::Column::push_boxed(key, child)`, as thin wrappers over `push(child._boxed())`. Option works via the existing impl. `children![]` only if cheap. State the benefit as about 200 single-line sites; most of the 1556 calls are outside `push`. Drop `boxed_if`/`Element::none`.

Acceptance: methods with doc examples and tests; a loop building a Row from three widget types compiles without `.boxed()`; `None` children add nothing; `tree_view.rs` compiles after migration.

### 4.19 headless-deterministic-goldens

**Priority P2 · Effort M · fork-feature · partly**

What Signex needs: CI forces lavapipe (`.github/workflows/ci.yml:183-211`, `release.yml:113`); `crates/signex-gfx/src/debug_pass/mod.rs:75,314,475` and `composite.rs:10,155` build their own instances. The Signex golden harness was not confirmed to use iced's Simulator.

What iced provides: `wgpu/src/lib.rs:922,930` backend from env only and `force_fallback_adapter: false`; `compositor.rs:94` same; `test/src/simulator.rs:265` scale factor 2.0 fixed; `matches_image` at `:306` is exact compare, no diff or update mode; `ICED_TEST_BACKEND` only selects wgpu vs tiny_skia (`simulator.rs:67`, `emulator.rs:101`); no way to disable the system font scan (`graphics/src/text.rs:119`).

Upstream: none for tolerance/diff; [#2455](https://github.com/iced-rs/iced/issues/2455) related to font scan; fork issue #6.

Proposal (corrected, split): (A) `Snapshot::matches_image_with(Tolerance)`, write `*.actual.png` and `*.diff.png` on mismatch, `ICED_UPDATE_SNAPSHOTS=1`; (B) `Simulator::with_scale_factor`; (C) snapshot file names including renderer and adapter/driver; (D) `Settings::system_fonts` (default true) skipping the system scan and using bundled fonts, shared with fork #6; (E) optional fallback-adapter option via a new Headless constructor or `ICED_TEST_FALLBACK_ADAPTER`. Defer exposing `Renderer::device()/queue()`. Do not change the existing `Headless::new` signature.

Verification notes: CI already selects lavapipe through the environment, so (E) is convenience.

### 4.20 textinput-focus-blur-events

**Priority P2 · Effort S · adopt-upstream-pr · confirmed. See 4.4 and 4.15.**

What Signex needs: `library/messages/library_message.rs:430-432` documents `BrowserCellCommit` as "Enter / blur"; `library/browser/grid.rs:380-392` wires only `.on_input` and `.on_submit`; handler at `app/dispatch/library/mod.rs:424`, covered by `tests/regression/library_browser_cell_commit.rs`. 29 `on_submit` uses.

What iced provides: nothing: only `on_submit` (`text_input.rs:170`), `on_submit_maybe` (177), Focusable (475-477), `Status::Focused`.

Upstream: [#819](https://github.com/iced-rs/iced/issues/819), PR [#3282](https://github.com/iced-rs/iced/issues/3282), [#3239](https://github.com/iced-rs/iced/issues/3239), [#3335](https://github.com/iced-rs/iced/issues/3335).

Proposal (corrected): adopt #3282 (emitted from the focus transition in `update`). Motivation reworded: the cell buffer (`BrowserCellEdit`) is retained but uncommitted, so blur is needed to flush it, as the doc comments promise. Add acceptance items: blur also fires when the widget is removed or the window loses focus; a Signex regression test (edit cell, focus another cell, assert commit). Audit the 29 `on_submit` sites and wire `on_blur` only where intended. `#3239` and text_editor are optional follow-ups.

### 4.21 redraw-scheduling-audit-multiwindow

**Priority P2 · Effort M · fix-upstream-issue · partly**

What Signex needs: docked and undocked windows (296 `window::Id`/`window::open` references) with expensive canvases. No Signex measurement of an idle-CPU problem exists.

What iced provides: `winit/src/lib.rs:1226-1228` already calls `request_redraw` on every window after each update batch. Zero-size (minimized) windows are skipped (`lib.rs:781`, `window.rs:286`); Occluded handled at `:982` and `:1055`. TRIAGE.md ranks 3, 4, 5, 11 are "skipped / not investigated" (a stale audit status, not evidence of a bug).

Upstream: [#3320](https://github.com/iced-rs/iced/issues/3320), [#3331](https://github.com/iced-rs/iced/issues/3331), [#2616](https://github.com/iced-rs/iced/issues/2616), [#3496](https://github.com/iced-rs/iced/issues/3496), [#3156](https://github.com/iced-rs/iced/issues/3156), PR [#3157](https://github.com/iced-rs/iced/issues/3157), [#2659](https://github.com/iced-rs/iced/issues/2659), [#2926](https://github.com/iced-rs/iced/issues/2926).

Proposal (corrected): drop "request_redraw for all windows after update"; it exists. Replace with: (1) measurement: per-window present counter, measure Signex idle CPU and redraws per cursor move on a large sheet with one undocked window; (2) if confirmed, make the post-update redraw conditional on a changed view or pending `RedrawRequest` (small commit, conflict-prone with upstream); (3) take #3157 for same-frame relayout after `window::resize` (see 4.25); reproduce #2659. Verify #3320 on Windows manually before changing anything. #3496 (Wayland freeze) is separate.

Acceptance: idle app with a visible canvas performs no redraws once nothing is pending (measured); minimized windows do not present and memory stays flat over 10 minutes.

### 4.22 gpu-adapter-selection-and-failure-ux

**Priority P2 · Effort M · adopt-upstream-pr · partly**

What Signex needs: only the wgpu feature (`Cargo.toml:61`); no CPU fallback; no handling of `GraphicsAdapterNotFound`, `WGPU_BACKEND` or a GPU-failure dialog. `request_adapter` calls in `signex-gfx/src/debug_pass/` are for tests.

What iced provides: `wgpu/src/window/compositor.rs:91-101` one request with `force_fallback_adapter: false`, error `NoAdapterFound(String)`; typed errors exist (`:21-34`, `:36-43`); adapter enumeration logged (`:70-78`) and selected adapter (`:103`); `WGPU_BACKEND` already honoured (`:296`) and `PowerPreference::from_env()` (`:92`). `WGPU_ADAPTER_NAME` handling not found in iced.

Upstream: PRs [#3302](https://github.com/iced-rs/iced/issues/3302), [#3365](https://github.com/iced-rs/iced/issues/3365), [#3011](https://github.com/iced-rs/iced/issues/3011), [#3246](https://github.com/iced-rs/iced/issues/3246), [#3523](https://github.com/iced-rs/iced/issues/3523); issues [#3265](https://github.com/iced-rs/iced/issues/3265), [#2572](https://github.com/iced-rs/iced/issues/2572), [#3138](https://github.com/iced-rs/iced/issues/3138); #3306 vello_cpu not ported. Upstream numbers not verified offline.

Proposal (corrected): retry once with `force_fallback_adapter: true` after `request_adapter` fails, controllable via wgpu Settings or env var; enrich the error with the adapter list already computed; Signex catches the startup error and shows the adapter report with a relaunch hint for software rendering. Do not depend on #3302/#3011; check #3365. #3246 and #3265 only if signex-gfx hits them. Software Vulkan must also support the features and limits the signex-gfx pipelines need; verify on a no-GPU VM.

Risk: software Vulkan is slow for large PCBs; label it degraded; nothing here has run on a real GPU.

### 4.23 menu-context-menu-core

**Priority P3 (the S helper P2-ish) · fork-feature · partly**

What Signex needs: `app/view/context_menu/placement.rs` (84 lines; `clamp_menu_origin`, `menu_height`, hard-coded `LINE_HEIGHT` 1.3) because card size is unknown before layout; vendored iced_aw menu about 3.2k lines (`menu_bar.rs` 941, `menu_tree.rs` 1132, `menu_bar_overlay.rs` 911), which works today. `vendor/iced_aw` stays regardless (NumberInput, ColorPicker, Wrap).

What iced provides: no menu or context_menu widget; `overlay/menu.rs` only for pick_list/combo_box; `core/src/overlay` has no fit-within helper; `tooltip.rs` clamps privately (`~:342-351`).

Upstream: [#114](https://github.com/iced-rs/iced/issues/114).

Proposal (corrected, split): (1) S/M: an iced_core helper `overlay::Position::fit_within(anchor, size, bounds, margin)` or overlay-reported measured size, reused in tooltip and pick_list, removing `menu_height` and the line-height guess. (2) P3: first-party popup/context menu (icon, label, shortcut, disabled, separator, check mark, submenu with hover delay, keyboard) with a Catalog. (3) P3, defer: menu bar with hover-to-switch and mnemonics only if dropping the iced_aw menu subtree. Mnemonics are speculative.

### 4.24 combo-box-search-keyboard

**Priority P3 · Effort S · adopt-upstream-pr · partly**

What Signex needs: no `combo_box` use in `crates/`; no manufacturer or package pick_list was found; existing pick_lists are small fixed sets (`unit_picker`, `prefix_picker`, `rkm_encoder`). The large-list need is not demonstrated.

What iced provides: `State::push` exists (`widget/src/combo_box.rs:352-355`), so #3252 is largely present. Real bug: `search()` splits on `!is_ascii_alphanumeric` (`:780`) and `build_matcher` keeps only ASCII alphanumerics (`:809`), dropping non-ASCII symbols such as Omega and mu. No scroll handling in combo_box.rs; menu.rs unchecked.

Upstream: PRs [#3252](https://github.com/iced-rs/iced/issues/3252), [#3422](https://github.com/iced-rs/iced/issues/3422), [#3428](https://github.com/iced-rs/iced/issues/3428), [#3284](https://github.com/iced-rs/iced/issues/3284). Numbers unverified.

Proposal (corrected): use `char::is_alphanumeric` at `:780` and `:809`; check and fix keyboard scroll in a 1000-option list; optionally a positional insert or `replace_options`. Drop `allow_custom` and `pick_list::searchable` until a concrete large-list picker exists.

### 4.25 window-resize-relayout

**Priority P3 · Effort S · fix-upstream-issue · partly**

What Signex needs: `app/handlers/modal_sizing.rs:112` pushes `window::resize` only when a dialog's wanted size changes after opening (guard at 111); rare, low impact.

What iced provides: `winit/src/lib.rs:1424-1434` `let _ = window.raw.request_inner_size(...)` discards the returned `Option<PhysicalSize>`. The viewport and surface update only in the `WindowEvent::Resized` branch (`winit/src/window/state.rs:136-147`); the redraw is requested at `lib.rs:1054`. TRIAGE.md:60 lists it as "later".

Upstream: [#3156](https://github.com/iced-rs/iced/issues/3156), [#2652](https://github.com/iced-rs/iced/issues/2652), [#3126](https://github.com/iced-rs/iced/issues/3126), PR [#3157](https://github.com/iced-rs/iced/issues/3157).

Proposal (corrected): keep the return value; on `Some(size)` apply it through a shared helper on `window::State` that rebuilds the Viewport, bumps `surface_version` only if the physical size differs, and requests a redraw (making the echoed event a no-op); on `None` just request a redraw. Unit-test the pure helper. The real failure is the Some-return/no-echo path, not a delayed echo.

### 4.26 file-drop-position-wayland

**Priority P3 · Effort S now, M deferred · adopt-upstream-pr · partly**

What Signex needs: no `FileDropped`/`FileHovered` handling in `crates/signex-app`.

What iced provides: `core/src/window/event.rs:50-78` per-file events, no position, "Wayland: Not implemented". `winit/src/conversion.rs:339-345` passes them through. winit is pinned to the iced-rs/winit git fork (`Cargo.toml:251`, rev 48116469); whether that rev has drag position was not checked.

Upstream: PR [#3415](https://github.com/iced-rs/iced/issues/3415) (winit 0.31 draft), [#2538](https://github.com/iced-rs/iced/issues/2538), [#1642](https://github.com/iced-rs/iced/issues/1642), [#2597](https://github.com/iced-rs/iced/issues/2597).

Proposal (corrected): (1) Signex-only: a window-event subscription grouping `FileDropped` events from one event-loop turn into `OpenFiles(Vec<PathBuf>)`, routed by active context with a window-wide `FileHovered` overlay; works on Windows, macOS, X11. (2) Fork, deferred as a P3 watch item: positioned drag events once the fork's winit rev exposes them; check the current rev first. Do not adopt the draft winit bump just for this. Document that Wayland drops do not work. Drop the `drop_target` widget until per-panel hit-testing is needed.

### 4.27 primitive-prepare-encoder-and-multipass

**Priority P3 · Effort M · fork-feature · partly**

What Signex needs: render-to-texture layer caching, compute culling, 3D board view. No depth use was found in `signex-renderer/src/pcb3d` (`glb.rs`, `mod.rs`, `projection.rs`). `debug_pass/` creates its own encoder for tests only.

What iced provides: `wgpu/src/primitive.rs:25-63` `prepare` has no encoder, but `render(&self, &Pipeline, &mut CommandEncoder, &TextureView, clip_bounds)` already receives one; `wgpu/src/lib.rs:513-600` shows the pass-split cost (draw for all primitives first, then render after the pass is dropped and reopened with Load). Surface pass is `sample_count: 1`, `resolve_target: None`.

Upstream: [#766](https://github.com/iced-rs/iced/issues/766), [#343](https://github.com/iced-rs/iced/issues/343), [#903](https://github.com/iced-rs/iced/issues/903).

Proposal (corrected): one defaulted `prepare_encoder(&self, pipeline, device, queue, encoder, bounds, viewport)` called after all `prepare()` calls and before the main pass opens, plumbed via `Stored::prepare` and the engine prepare loop (`lib.rs ~341-358`). Drop the depth-attachment request (a primitive can own a depth texture in `render()`) and `FrameContext.sample_count` (always 1). Document the pass-split cost. Before filing, Signex should prototype caching via a primitive-owned encoder and submit in `prepare()` and file only if that measurably fails.

### 4.28 headless-renderer-config

**Priority P3 · Effort S-M · fork-feature · partly**

What Signex needs: about 30 lines of test-only setup in two places (`crates/signex-app/src/canvas/gpu_mount_tests.rs:40-110`, `tests/guide_screenshots.rs:52-62`). Signex chooses antialiasing `None` for deterministic tests. `signex-output` exports use tiny-skia on a separate scene walk; no 8k tiled export exists (no consumer).

What iced provides: `wgpu/src/lib.rs:925-975` hardcodes HighPerformance, `force_fallback_adapter: false`, fixed features (936-942), Rgba8Unorm(Srgb) (963-967) and MSAAx4 (970); `Renderer::screenshot` (`:179`) renders one viewport-sized texture. `Engine::new` is public.

Upstream: [#721](https://github.com/iced-rs/iced/issues/721), PR [#3524](https://github.com/iced-rs/iced/issues/3524), [#3307](https://github.com/iced-rs/iced/issues/3307), [#3011](https://github.com/iced-rs/iced/issues/3011), [#3365](https://github.com/iced-rs/iced/issues/3365).

Proposal (corrected): `iced_wgpu::HeadlessSettings { power_preference, force_fallback, format, antialiasing }` with a `Renderer::headless_with(settings)` helper; optionally `screenshot_into(&wgpu::Texture)`. Drop `screenshot_tiles` until a concrete large-format export exists; tile rendering needs per-tile viewport offsets, scissor and atlas handling.

### 4.29 wgpu-device-lost-and-errors

**Priority P2 · Effort M then L · fork-feature · partly**

What Signex needs: large GPU-resident scenes and its own pipelines created from iced's device (`signex-gfx/src/context.rs`, scene upload tracker). No device-lost or uncaptured-error handling in Signex either. A driver reset in a long session loses resident buffers.

What iced provides: `winit/src/lib.rs:992-1018` handles `SurfaceError::Lost` by recreating the surface (rate limited to once a second); `compositor.rs:270` maps `Lost`; device and queue come from `request_device` (`:182-196`) into a fixed `Engine`. Grep for `device_lost`, `uncaptured`, `error_scope` finds nothing.

Upstream: #3011 and #3365 are startup fallback only; none for device loss.

Proposal (corrected): Phase 1 (M): install `device.set_device_lost_callback` and `on_uncaptured_error` in `Compositor::request`, log and set an atomic flag; OOM and validation errors skip the frame instead of panicking; on a lost device call a runtime hook so the app can autosave and exit cleanly or offer a restart; add a documented `RendererGeneration` counter or event so Signex can invalidate resident buffers. Phase 2 (L, optional): rebuild Adapter/Device/Queue/Engine and recreate surfaces for all windows, resetting every device-keyed cache (text atlas, images, primitive Storage). Drop the mid-session tiny_skia fallback or make it a stretch item.

Risk: many GPU handles to reset; hard to test without a driver that loses devices (needs a test hook).

## 5. Dependency notes between gaps

- 4.1 and 4.2 share one windowing core; build the virtual list first.
- 4.3 and 4.17 are one focus design; 4.4, 4.15, 4.20 are one focus-events change.
- 4.8 part A (cryoglyph) and 4.13 step 1 are independent and can ship separately.
- 4.19 (D) system_fonts is shared with fork issue #6.
- 4.21 step 3 and 4.25 are the same relayout fix.
- 4.22 and 4.19 (E) and 4.28 all touch headless/adapter selection; do them together.

## 6. Checked and NOT gaps

**multiwindow-redraw-starvation** (upstream #3320: unfocused or secondary windows do not present after state changes). Verdict: already present. In `winit/src/lib.rs` `about_to_wait` (about lines 1190-1230), when messages are pending or UIs are stale the fork runs `update()`, rebuilds UIs for every window and calls `window.raw.request_redraw()` on every window (about line 1226). A `RedrawRequested` on one window redraws the others after an update (about 890-900). Resized and Occluded(false) request redraws (1055-1056); Occluded and Outdated branches request redraws (982-1017); `NewEvents(Init)` redraws all; `ResumeTimeReached` drains per-window `redraw_at` (713-723). TRIAGE.md:20 lists it as "skipped / not investigated", which is only a stale status. A Windows-specific present stall cannot be excluded without a Windows machine. Action: update the TRIAGE.md rank 11 note and verify on Windows only; do not open a fix issue. See 4.21 for the narrower measurement-first item.

Several claims inside other candidates were also refuted and are recorded in their sections: "no size callback independent of overflow" (`Sensor` exists), "no consistent disabled look" for button, checkbox, toggler, pick_list, text_input (they have `Status::Disabled`), "need new cursor/selection operations" for text_input (they exist), "RunOnMainThread is needed" (update runs on the main thread), "shader::Program lacks State/update" (it has them), and "WGPU_BACKEND is not honoured" (it is).

## 7. Unverified candidates (not checked; not claims)

Priority is the surveyor's, unverified.

P2: canvas-cache-region; pen-tablet-pressure-events; wheel-phase-momentum; custom-cursor-icons; file-drop-position-and-drag-out; text-editor-code-editing; text-field-context-menu; text-small-size-hinting-and-lcd; text-measure-without-widget; system-appearance-accent-contrast-motion; native-dialog-parenting; signex-follow-system-theme-and-drop; task-test-driver-and-is-empty; emulator-multiwindow; profiling-spans-startup-and-export; large-view-cost-and-lazy-ergonomics; mouse-area-position-messages; tray-icon-background-tasks; keyboard-focus-traversal-widgets (likely duplicate of 4.17).

P3: tooltip-hover-card; signex-replace-iced-aw-vendor; win-chrome-native-attrs; win-chrome-hittest-regions; pane-grid-vs-custom-dock; clipboard-custom-formats; signex-use-core-click-and-modifier-hygiene; taskbar-progress-badge-tray-notify; rtl-bidi-locale-hooks; selector-roles-and-widget-kinds; pointer-tablet-winit-031.

Note: file-drop-position-and-drag-out overlaps 4.26, and signex-follow-system-theme-and-drop suggests that the fork may already have system theme and file-drop APIs that Signex does not use; this should be checked before any fork work.

## 8. Suggested roadmap

Phase 1, quick wins (S), mostly Signex-side or small fork patches:
- 4.11 layout-independent shortcuts (Signex only).
- 4.4/4.15/4.20 adopt #3282 (on_focus/on_blur); then Signex wires `BrowserCellCommit` and simplifies `numeric_draft.rs`. Upstream-bound: the patch should stay close to #3282.
- 4.6 window ModeChanged events. Fork-only.
- 4.13 step 1 (pass wrapping/ellipsis, shared SwashCache). Small and upstreamable.
- 4.18 push_boxed/extend_boxed. Upstreamable but name-clash risk; land in the fork first.
- 4.25 resize relayout helper; 4.24 combo_box unicode matcher (cheap fixes).
- 4.19 (A)(B)(C) snapshot tolerance, diff PNGs, scale factor, naming.
- 4.26 phase 1 (Signex-only drop grouping).
- 4.21 step 1 measurement.

Phase 2, medium (M):
- 4.1/4.2 virtual list plus reveal operation, then the column handle helper. Depends on nothing; unblocks Signex deleting `window/`, `size_sensor.rs`, `keyed.rs` and the tree view.
- 4.17/4.3 focus for built-in widgets, focus ring, Signex Tab subscription.
- 4.5 window parent and `set_enabled`.
- 4.9 pipeline keys; 4.10 primitive in canvas order; 4.12 cursor grab (before 3D orbit work).
- 4.7 default-menu opt-out (check the winit fork first), then quit request.
- 4.14 font features; 4.22 adapter fallback with 4.28 HeadlessSettings; 4.29 phase 1.
- 4.13 step 2 outline path cache.

Phase 3, large (L/XL), only on concrete demand:
- 4.8 part A cryoglyph rotation (a separate fork of iced-rs/cryoglyph).
- 4.29 phase 2 renderer rebuild.
- 4.23 menu widgets; 4.16 AccessKit (spike first); 4.27 prepare_encoder after a Signex prototype.

Where work lives:
- Upstream-first or tracking upstream PRs: #3282 (focus events), #3157 (relayout), #2614 (cursor grab), #3302/#3365 (adapter), #160 (virtual list), #114 (menus), AccessKit PRs #1849/#3111. Prefer adopting upstream code and rebasing rather than diverging.
- Fork-only: window parent/set_enabled, ModeChanged, macOS hooks, pipeline keys, canvas primitive order, cryoglyph rotation, device-lost handling, headless options.
- Signex-only: 4.11, 4.26 phase 1, 4.21 measurement, the removal of workarounds after each fork change.

Tag and release process: each fork change should be a small, separate commit with a FORK.md entry (and a TRIAGE.md status update where relevant), so rebases onto upstream stay tractable. Signex bumps its pin to a fork tag after the change merges, then lands the workaround-removal in a separate Signex PR. Keep additive and default-off changes so existing Signex code keeps compiling between tags. Redraw changes in `winit/src/lib.rs` conflict with upstream merges, so keep them minimal.

## 9. Limitations

- Static, read-only analysis. Nothing was built, run or profiled; no performance numbers were measured.
- No real GPU, compositor, Windows or macOS machine was available. Platform behaviour (Wayland, macOS fullscreen, Windows owner windows, device loss) is untested.
- Counts come from text search and may be incomplete (for example `.boxed()` totals, `on_submit` sites, `text_input` references).
- Upstream issue and PR numbers and states were taken from the survey inputs and were not re-verified offline; some verifiers flagged this explicitly (a11y, combo_box, GPU adapter PRs). Confirm before linking in public issues.
- Signex was read at PR #2028 head only; its code may have changed. Some Signex claims (for example the golden harness using iced's Simulator, dialog Tab flows, large-list pickers) were not confirmed.
- The pinned winit fork rev (4811646 / 48116469) capabilities were not inspected.
- Unverified candidates in section 7 carry no evidence.
