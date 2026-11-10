# iced-signex-v0.15.0-alpha.8

**Tag target:** the `signex` merge commit of the `dev` -> `signex` release pull request.
**Builds on:** `iced-signex-v0.15.0-alpha.7`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there.

winit now comes from `alplabai/winit` branch `iced-signex` at `a2cc4a0e` (was `d18f6382`). The new commit makes macOS queue an `Occluded` event at the end of a fullscreen exit (#107).

## Upstream PRs merged

- **Opaque layers block wheel scrolling; scrollers stay in their track (#97, upstream #3493, #3437)**
  - `opaque` used to capture only button presses, so wheel scrolling over a modal or dim layer still reached the widgets below it. It now captures wheel scrolling too.
  - With both scrollbars visible, a scroller at its minimum length could run past its track into the corner gap. It is now clamped to the track.
  - **Signex:** scrolling over the keyboard-shortcuts modal, the about modal and the locked-window overlay no longer scrolls the schematic or the panels underneath.
- **Primary clipboard on Linux and the BSDs (#102, upstream #3325)**
  - Adds `ClipboardKind::{Standard, Primary}`, `clipboard::primary_selection_supported()`, `clipboard::{read_primary, write_primary}` and `Shell::{read,write}_clipboard_primary`.
  - `text_input` and `text_editor` use it: selecting text copies it to the primary selection, and a middle click pastes it.
  - **Signex:** text fields gain middle-click paste and select-to-copy on Linux. `crates/signex-app/src/task_driver.rs` must add the new `clipboard_kind` field to its `Action::Read` and `Action::Write` matches, and should answer `Primary` reads with an error.

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

## Changed

- **Scrollers are at least 20 px long (#99)** — a scroller no longer shrinks to an invisible 2 px with very long content. **Signex:** long lists get a visible, grabbable scroller.

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

## Verification

Each change was merged into `dev` through its own pull request: Fork CI green (`fmt`, `clippy`, `nextest --workspace --all-features`, the wasm check) and an adversarial Opus review that ended in MERGE.

Manual tests by the maintainer:
- **#98, #99:** scroll demo on macOS.
- **#104:** native modals on Windows 11 and macOS.
- **#106:** title bar colours on Windows 11.
- **#107:** an OS-started fullscreen exit on a Mac. The late `fullscreen: false` arrived about 0.5 s after the animation began.

## Not verified

- **#102:** primary clipboard on a real X11 or Wayland desktop.
- **#104:** X11 and Wayland.
- **#106:** the runtime recolour after a theme switch inside Signex.
- **#101:** `VirtualList` inside `scrollable` in the Signex library browser.
- **Signex build:** Signex has not been built against this release yet. The re-pin needs the `task_driver.rs` clipboard fix.
