# iced-signex-v0.15.0-alpha.8

**Tag target:** the `signex` merge commit of the `dev` -> `signex` release pull request.
**Builds on:** `iced-signex-v0.15.0-alpha.7`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there.

winit now comes from `alplabai/winit` branch `iced-signex` at `0570f845` (was `d18f6382`). The new commits make macOS queue an `Occluded` event at the end of a fullscreen exit (#107) and let Windows extend the client area into the title bar (#114).

## Upstream PRs merged

- **Opaque layers block wheel scrolling; scrollers stay in their track (#97, upstream #3493, #3437)**
  - `opaque` used to capture only button presses, so wheel scrolling over a modal or dim layer still reached the widgets below it. It now captures wheel scrolling too.
  - With both scrollbars visible, a scroller at its minimum length could run past its track into the corner gap. It is now clamped to the track.
  - **Signex:** scrolling over the keyboard-shortcuts modal, the about modal and the locked-window overlay no longer scrolls the schematic or the panels underneath.
- **Primary clipboard on Linux and the BSDs (#102, upstream #3325)**
  - Adds `ClipboardKind::{Standard, Primary}`, `clipboard::primary_selection_supported()`, `clipboard::{read_primary, write_primary}` and `Shell::{read,write}_clipboard_primary`.
  - `text_input` and `text_editor` use it: selecting text copies it to the primary selection, and a middle click pastes it.
  - **Signex:** text fields gain middle-click paste and select-to-copy on Linux. `crates/signex-app/src/task_driver.rs` must add the new `clipboard_kind` field to its `Action::Read` and `Action::Write` matches, and should answer `Primary` reads with an error.
- **Upstream batch (#113: upstream #3557, #3531, #3486, #3542, #3543)**
  - **#3542:** an `svg::Style::color` with alpha below 1 makes the SVG translucent on wgpu and tiny_skia.
    - Fork deviation: the mask alpha scales pixel coverage instead of replacing it, so anti-aliased edges are kept and opaque masks look as before.
  - **#3543:** the hovered `pick_list`/`combo_box` menu option no longer draws over the menu borders.
  - **#3531, #3486:** tiny_skia repaints fully on a scale factor change, and text damage grows by 1 px, so no stale pixels or glyph fringes are left.
  - **#3557:** fixes the public helper `core::widget::tree::diff_children_custom_with_search`. `keyed::Column` already reconciled by key.
  - **Signex:** translucent icon masks (chevrons, active bar) render translucent with smooth edges; bordered dropdowns no longer bleed the hover colour.

## Added

- **`VirtualList` works inside `scrollable` (#101, #32)**
  - `scrollable(virtual_list(..)).id(id)` gives a scrollbar, scroller drag, rail, smooth scrolling, anchors and `reveal`, while still building only the visible rows.
  - **Signex:** the library browser can drop its hand-rolled window and `size_sensor` once it adopts this.
- **`window::set_enabled` and centring on the parent (#104, #35 slice 2)**
  - `window::set_enabled(id, bool)` disables a window natively on Windows; on every platform a runtime filter also drops its input.
  - A disabled window that gains focus passes it to the newest enabled window it owns (its dialog).
  - `window::parent_supported()` reports whether native owned dialogs work (Windows, macOS).
  - `Position::Centered` with `parent: Some(..)` centres on the parent.
  - **Signex:** modal dialogs can disable their owner and open centred on it. On X11 and Wayland, keep the in-window lock.
- **Windows title bar and border colours (#106, #103)**
  - The Windows `PlatformSpecific` gains `title_background_color`, `title_text_color` and `border_color`.
  - `window::set_title_colors(id, background, text, border)` recolours an open window. It works on Windows 11 and does nothing elsewhere. Each call sets all three colours, and `None` means the system default.
  - **Signex:** dialogs and the main window frame can follow the theme on Windows. `Theme::mode()` must return `Dark` or `Light` for the caption to follow dark or light mode.
- **Windows: app content in the title bar, geometry (#114, #105 slice 1)**
  - The Windows `PlatformSpecific` gains `extend_content_into_title_bar: bool` (default `false`). The client area covers the caption while the window keeps its native resize borders, shadow and rounded corners.
  - It only applies with `decorations: true`, outside fullscreen. The top resize band and corners still resize. `window::drag` works, and the app draws its own caption buttons.
  - Caption and maximize regions (slice 2) and Snap Layouts (slice 3) come later.
  - **Signex:** a custom title bar on Windows with native borders.

## Changed

- **Scrollers are at least 20 px long (#99)** — a scroller no longer shrinks to an invisible 2 px with very long content. **Signex:** long lists get a visible, grabbable scroller.
- **Windows defaults to DX12 and GL, not Vulkan (#112, #110)**
  - `backend::Api::Best`, `compositor::Settings::default` and the headless renderer use `DX12 | GL` on Windows. GL is used only when DX12 offers no adapter.
  - `WGPU_BACKEND=vulkan` or an explicit `backend::Api::Vulkan` still selects Vulkan.
  - **Signex:** opens on machines whose Vulkan driver is broken. An Intel UHD 630 laptop crashed in `vkCreateDevice` before the first window was shown.

## Fixed

- **Minimum-length scrollers move over the whole scroll range (#98).** **Signex:** the scroller of very long scrollables no longer stalls early or jumps when grabbed.
- **`window::Event::ModeChanged` fires after an OS-started macOS fullscreen exit (#107)**
  - Leaving fullscreen through the green button, Ctrl+Cmd+F or the menu now reports `fullscreen: false` when the exit animation ends. It used to stay `true` until the next focus change or resize.
  - **Signex:** its fullscreen flag follows OS-started exits (signex#2256).

## Tests only

- **#100:** tiny-skia canvas text alignment is pinned by tests (upstream #3192 is not needed).

## Breaking

- **#102:** `core::Clipboard::{reads, write}` carry a `ClipboardKind`.
  - `runtime::clipboard::Action::{Read, Write}` gain a `clipboard_kind` field.
  - `iced_winit::Clipboard::{read, write}` take the `ClipboardKind` first.
  - `core::text::editor::Update` gains `CopyPrimary` and `PastePrimary`.
- **#104:** `runtime::window::Action::SetEnabled` is a new variant.
- **#106:**
  - The Windows `PlatformSpecific` loses `Eq` and gains three fields.
  - `runtime::window::Action::SetTitleColors` is a new variant.
- **#112:** on Windows, the default wgpu backends change from `Backends::all()` to `DX12 | GL`, and the headless default from `Backends::PRIMARY` to `DX12 | GL`. Apps that relied on Vulkan there must set `WGPU_BACKEND=vulkan`.
- **#114:** the Windows `PlatformSpecific` gains `extend_content_into_title_bar`, so exhaustive struct literals stop compiling.

## Behaviour changes

- **#97:** wheel events over any `opaque` widget no longer reach the layers below.
  - That includes a parent `scrollable`.
  - `event::listen()` no longer sees them either.
- **#99:** longer minimum scroller.
  - The free track gets shorter, so dragging moves the content faster.
  - Rail jump and stop offsets shift.
- **#101:** a `VirtualList` with `infinite` height limits is as tall as all its rows and no longer scrolls itself.
  - Scroll operations and `reveal_row` on its own `Id` do nothing there; they must target the scrollable's `Id`.
- **#102:** on X11 and Wayland, a pointer selection replaces the primary selection, and a middle click focuses the field and pastes.
  - Secure inputs ignore both.
  - On macOS, Windows and wasm nothing changes.
- **#104:**
  - `Position::Centered` with a parent centres on the parent.
  - Plain `Position::Centered` now really centres when `scale_factor` is not 1.0.
  - When only disabled windows are left after a close, they are enabled again with a warning.
- **#107:** other winit consumers see one extra, correct `Occluded` at the end of a macOS fullscreen exit. iced does not forward it to apps.
- **#112:** Windows apps no longer use Vulkan unless asked for. A DX12 device failure is not retried on GL.
- **#113:** a translucent `svg::Style::color` now renders translucent (upstream #3542, with coverage kept).

## Verification

Each change was merged into `dev` through its own pull request: Fork CI green (`fmt`, `clippy`, `nextest --workspace --all-features`, the wasm check) and an adversarial Opus review that ended in MERGE.

Manual tests by the maintainer:
- **#98, #99:** scroll demo on macOS.
- **#104:** native modals on Windows 11 and macOS.
- **#106:** title bar colours on Windows 11.
- **#107:** an OS-started fullscreen exit on a Mac. The late `fullscreen: false` arrived about 0.5 s after the animation began.

On a Windows 11 laptop (Intel UHD 630 + GTX 1650 Max-Q, scale 1.25), driven over SSH:
- **#112:** the `tour` example opens with DX12 and no `WGPU_BACKEND` set. Before, it crashed in the Intel Vulkan driver.
- **#114:** a probe in the interactive session checked:
  - hit-test codes on every edge and corner;
  - client area equal to the work area when maximized;
  - Win+arrow snapping;
  - mouse resizes from the top band and corners;
  - the same fullscreen `Resized` sequence as a plain window.

## Not verified

- **#102:** primary clipboard on a real X11 or Wayland desktop.
- **#104:** X11 and Wayland.
- **#106:** the runtime recolour after a theme switch inside Signex.
- **#101:** `VirtualList` inside `scrollable` in the Signex library browser.
- **#113:** the mask and damage fixes were not viewed on a GPU; CPU unit tests only.
- **#111 (open):** a brief white area and lag while resizing on Windows, and GL fullscreen flicker.
- **Signex build:** Signex has not been built against this release yet. The re-pin needs the `task_driver.rs` clipboard fix.
