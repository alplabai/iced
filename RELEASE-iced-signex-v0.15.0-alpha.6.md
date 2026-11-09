# iced-signex-v0.15.0-alpha.6

Tag target: the `signex` merge commit of the `dev` -> `signex` release pull request. Builds on `iced-signex-v0.15.0-alpha.5`.

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full list; every fork change below has its own entry there.

## Upstream PRs merged
3157 (open upstream; relayout after `window::resize`, ported in fork #75 as item 3 of #46).

## Fork fixes and features
New since alpha.5:
- Windows and platform: #46 item 3 (when `request_inner_size` applies a new size at once, the runner sends itself a `Resized` event so the layout updates in the same frame; only Wayland, iOS and Android take this path).

## Verification
Merged into `dev` through alplabai/iced#75 with Fork CI green, and checked by hand on Fedora GNOME (Wayland) with `examples/resizing`. Signex trunk builds against this release with the same source fix as alpha.5.

## Not verified
Nothing was run on iOS or Android.

## Breaking
None since alpha.5.

## Behaviour changes
On Wayland, iOS and Android, a `window::Event::Resized` now follows a `window::resize` that changed the size. No change on X11, Windows, macOS or web.
