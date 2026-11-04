# iced-signex-v0.15.0-alpha.1

Tag target: `108a717b18c49c91d780d6e03fd0d2f2ec8b40ae`

alplabai's fork of iced for Signex: upstream iced master (0.15.0-dev) plus cherry-picked upstream PRs and fork fixes. See `FORK.md` for the full merged/skipped list and `TRIAGE.md` for the issue triage.

## Upstream PRs merged
3391 (per-side border widths/colors), 3377 (pinch gesture), 3283 (gesture events), 3290 (IME preedit fix), 3514 (wgpu texture formats), 3316 (COPY_SRC), 3450 (multi-monitor), 3359 and 3360 (tiny_skia clipping), plus hand-ported radio (2770) and toggler (2779) animations (behind the `animations` feature).

## Fork fixes
#1 zero-size image guard, #2 zero-size surface guard, #8 keyed Column reorder state, #9 keyed from_vecs length assert, #15 final Closed event before exit, #16 pane_grid title-bar padding, #20 progress_bar radius clamp.

## Verification
cargo check --workspace --all-targets; tests for iced_core, iced_widget, iced_graphics. alplabai/signex builds, passes its tests and launches against this commit (software Vulkan only).

## Not verified
No real-GPU run; animations and per-side borders never seen rendered; no regression test for the keyed reorder fix.

## Breaking
Based on iced master, not 0.14: `Row`/`Column` are generic over the child type and `push` takes the child directly (use `.boxed()`), wgpu 30, `Theme::palette()` replaces `extended_palette()`.
