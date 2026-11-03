# Upstream iced issue triage for the Signex fork

Target: large-scene wgpu canvas EDA app (Signex). Priority order: crashes, CPU/memory, rendering correctness, text/input, window management.
Based on issue titles and prior notes only; most issue bodies were not readable. Verify each in-tree before changing code. No cargo was run.

## Top 20 (ranked)

| # | Issue | Area | Effort | Why it matters to Signex | Fix hint | Batch |
|---|-------|------|--------|--------------------------|----------|-------|
| 1 | [3342](https://github.com/iced-rs/iced/issues/3342) | wgpu/image/atlas.rs, raster.rs | small | 0x0 image handle can crash on GPU upload; cheap guard | Skip upload/draw (return None) when width==0 or height==0 | B |
| 2 | [2648](https://github.com/iced-rs/iced/issues/2648) | wgpu/window/compositor.rs | small | Zero-size surface panics in screenshot (minimized) | Return early/empty when physical size is zero | B |
| 3 | [2659](https://github.com/iced-rs/iced/issues/2659) | winit/lib.rs, compositor present | small | Memory growth while minimized in a long-running app | Skip redraw/present when size is 0 or minimized | A |
| 4 | [3331](https://github.com/iced-rs/iced/issues/3331) | winit/lib.rs redraw scheduling | medium | Idle GPU/battery with heavy canvas | Only request_redraw when RedrawRequest pending; audit continuous redraw sources and present mode | A |
| 5 | [2616](https://github.com/iced-rs/iced/issues/2616) | winit/lib.rs event loop | medium | High CPU on mouse move hurts canvas pan | Profile CursorMoved; avoid redraw/interface rebuild per event when no widget requests it (also 2119, 2266) | A |
| 6 | [2455](https://github.com/iced-rs/iced/issues/2455) | graphics/src/text.rs | medium | Slow Linux startup from scanning all system fonts | Build fontdb manually (bundled + needed dirs), avoid full scan | C |
| 7 | [2669](https://github.com/iced-rs/iced/issues/2669) | graphics/geometry/path/builder.rs | small | Wrong arc sweep gives wrong pad/symbol geometry | Check Arc to Ellipse conversion for negative sweep_angle | C |
| 8 | [3488](https://github.com/iced-rs/iced/issues/3488) | core/widget/tree.rs, widget/keyed/column.rs | medium | Keyed lists map wrong state to children after reorder | Reconcile by full key lookup instead of first/last heuristic | D |
| 9 | [3461](https://github.com/iced-rs/iced/issues/3461) | widget/keyed/column.rs | small | from_vecs length mismatch panics in diff | assert_eq!(keys.len(), children.len()) | D |
| 10 | [2603](https://github.com/iced-rs/iced/issues/2603) | widget/scrollable.rs | medium | Scroll perf in many large panels | Check redundant layout/operate/clipping per scroll frame | D |
| 11 | [3320](https://github.com/iced-rs/iced/issues/3320) | winit/lib.rs multi-window | medium | Detached panels may not present after state change on Windows | request_redraw for all windows with dirty UI after update | A |
| 12 | [3409](https://github.com/iced-rs/iced/issues/3409) | winit/lib.rs | medium | Custom chrome needs borderless resize | Edge hit-test calling drag_resize_window, or expose helper | A |
| 13 | [3387](https://github.com/iced-rs/iced/issues/3387) | wgpu/window/compositor.rs | small | Panic if PreMultiplied alpha mode lost (transparent chrome) | Re-query capabilities, fall back to Auto instead of unwrap | B |
| 14 | [3467](https://github.com/iced-rs/iced/issues/3467) | widget/text_editor.rs | small | Plain keys also fire app shortcuts while typing | capture_event for text-producing KeyPressed when focused (see 2462 for panics) | D |
| 15 | [3229](https://github.com/iced-rs/iced/issues/3229) | winit/lib.rs | small | Last Closed event lost; breaks save-on-exit | Deliver pending Closed to subscriptions before exit | A |
| 16 | [2101](https://github.com/iced-rs/iced/issues/2101) | widget/pane_grid/title_bar.rs | small | padded_width sign bug (`- left + right`) and Fill title swallows drags | Fix to `- left - right`; fix is_over_pick_area | D |
| 17 | [2765](https://github.com/iced-rs/iced/issues/2765) / [2567](https://github.com/iced-rs/iced/issues/2567) / [2638](https://github.com/iced-rs/iced/issues/2638) | wgpu/geometry.rs | medium | Thin/degenerate lines, sharp-angle spikes, end artifacts on wires | Handle zero-length segments, set miter limit / bevel fallback | E |
| 18 | [3017](https://github.com/iced-rs/iced/issues/3017) / [1947](https://github.com/iced-rs/iced/issues/1947) | wgpu/geometry.rs, tiny_skia geometry | medium | Images always above shapes; with_clip ignores text/images | Split layers on image/mesh transitions; clip text/images in paste | E |
| 19 | [3173](https://github.com/iced-rs/iced/issues/3173) | graphics/cache.rs, wgpu/geometry.rs | medium | Canvas cache perf regression (cause unconfirmed) | Profile Cache clear/draw for per-frame rebuilds | E |
| 20 | [3526](https://github.com/iced-rs/iced/issues/3526) / [3440](https://github.com/iced-rs/iced/issues/3440) | widget/progress_bar.rs; Cargo.lock | small | Cosmetic fill overflow; lru security advisory | Clamp radius to min(radius, fill_w/2); cargo update -p lru | F |

## Batches (implement together, build once)

- **A: winit/src/lib.rs** — 2659, 3331, 2616, 3320, 3409, 3229 (later: 2144, 2119, 3156/2652 resize relayout)
- **B: wgpu image + compositor** — 3342, 2648, 3387 (later: 3102, 3265, 1746 texture size clamp)
- **C: graphics text + path** — 2455, 2669
- **D: widgets** — 3488, 3461 (keyed + core tree); 2603 (scrollable; later 3345, 3403, 3181); 3467 (text_editor); 2101 (pane_grid)
- **E: wgpu/tiny_skia geometry** — 2765, 2567, 2638, 3017, 1947, 3173
- **F: deps/cosmetic** — 3440, 3526

## Maybe (verify only if Signex hits them)

- Windows/Wayland windowing: 3496, 3241, 3126, 3108, 2926, 2525, 2258, 2256, 2657, 2268, 1941 (likely works with exit_on_close_request(false)), 1870, 1312, 1618, 2572, 3265, 3418
- Text/input: 3431, 3214, 3202, 3189, 3266, 2674, 2542, 1544, 2462, 2328, 2318, 2131, 2721, 2847, 3380, 3150, 1666
- Widgets/layout: 3201, 1983, 1906, 2935, 2088, 1727, 1397, 1632, 1488, 2398, 2546
- Rendering/memory: 3512, 3040, 2246, 2254, 2212, 2079, 2064, 786, 549, 2166, 897, 439, 1720 (virtualize long lists in Signex, no upstream fix), 3233, 2687, 2314
- Examples only: 2336, 2323, 2042, 1442
