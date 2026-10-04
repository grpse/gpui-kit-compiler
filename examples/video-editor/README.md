# FlowCut — RSX video editor UI

Four UI-only screens based on the supplied mocks: **Edit** (compositing), **Tracking** (the editor inside `main.png`), **Media** (the full library), and **Overview** (import and preprocessing). Switch screens in the top bar. The instructional callouts around `main.png` are design context, not extra application panels or requests to implement processing services.

The default app requires no FFmpeg or GStreamer installation. All actions go through `Editor::dispatch` → `EditorState::apply`; the last 100 calls are retained in `state.events`. Buttons that stand in for services display a dismissible notice. Docking, resizing, scrolling, selection, and playhead dragging are real local UI behavior. Media processing and service calls remain mocked. Nothing is imported, exported, uploaded, shared, played externally, or written to a project file.

## Run

From the repository root:

```sh
cargo run -- compile examples/video-editor
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target
```

Start on a specific screen by appending `-- --library`, `-- --overview`, or `-- --tracking` to the second command. The window starts at 1536 × 980 and supports sizes down to 1100 × 740. Each workspace card has a draggable tab: drag it onto another card to group tabs, or onto a highlighted edge to split the workspace. Drag dividers to resize; the title-bar expand button zooms a card. Navigation, media/presets, player, inspector, timeline, processing, details, and generated assets all use the same dock host. The Edit/Tracking workspace and Media/Overview workspace retain separate layouts during the session; **Reset layout** restores the current workspace. Content scrolls inside its card, and library columns follow its actual width. Layouts are currently kept in memory only. The preview fullscreen button expands the player within the application.

## Components

Every rendered element of the new UI lives in `.rsx`, using native `#[gpui]` markup. The normal Rust files contain startup, fixtures, and state transitions.

| Source | Responsibility |
| --- | --- |
| `src/ui.rsx` | Editor entity, input subscriptions, action dispatch, and screen composition |
| `src/components/primitives.rsx` | Palette, icons, action buttons, clipped fixture photos, thumbnails, waveforms, progress bars, section headings, metadata rows |
| `src/components/workspace.rsx` | Reusable dock panel, native DockArea/DockSkin, responsive card bounds, separate workspace layouts |
| `src/components/presets.rsx` | Text, effects, transitions, elements, and captions browsers, preset tiles and sample transcript |
| `src/components/chrome.rsx` | Top bar, screen navigation, tool sidebar, and folders |
| `src/components/library.rsx` | Compact/full media library, cards/list rows, search and filters, import area, processing queue |
| `src/components/player.rsx` | Shared preview, transport controls, seek slider, tracking overlay, filmstrip, details tabs, generated assets |
| `src/components/inspector.rsx` | Reusable property sliders/toggles, collapsible transform/crop/compositing sections, tracking, audio, effects, color |
| `src/components/timeline.rsx` | Tool strip, ruler, tracks, clips, filmstrips, waveforms, automation path, playhead, zoom |
| `src/state.rs` | Typed actions, deterministic media/jobs/tracks/clip fixtures, bounded event log, state tests |
| `src/legacy.rsx`, `src/model.rs` | Preserved FFmpeg example, available separately with `--legacy` |

Audio and Images select matching fixture previews and reset incompatible filters when opened. Every tool in the editing sidebar opens its own browser; selecting a tool also reveals the media/presets card if it is behind another dock tab. Preset clicks select a tile and show a simple mocked overlay in the preview.

Drag the purple playhead handle to seek, or click/drag the ruler. Pointer positions use the timeline’s rendered bounds, so seeking stays aligned after horizontal scrolling, zooming, and docking. The playhead clamps to the 18-second project; media-screen transport follows the selected asset’s duration. Tracks share vertical scrolling, while the ruler and clips share horizontal scrolling.

Header and library search fields use separate native input entities with synchronized text. Reusing one entity in two visible controls creates duplicate accessibility IDs in GPUI. Cards, timeline clips, icon buttons, and the import area expose accessible names.

## RSX coverage and gaps

**No UI element in these screens required a separate Rust renderer or a compiler change.** Native RSX accepts the GPUI Kit controls, builder methods, event callbacks, iterator children, and canvas used here.

There are still conveniences worth adding if this should also be expressible entirely in the higher-level HTML-style RSX runtime:

| Missing declarative facility | Current implementation | Possible extension / alternative |
| --- | --- | --- |
| Connected vector paths and automation points | A `<canvas>` with a short `PathBuilder` painting callback, inside `timeline.rsx` | Add declarative SVG/path or canvas drawing primitives; alternatively import a `Definition::native` renderer |
| A source rectangle for image fixtures | A clipped, positioned `<img>` showing photographs inside the unchanged reference image | Add a source-rectangle image component; production can supply standalone thumbnails and poster files to `<img>` |
| Editor-specific pointer gestures and entities in the HTML-style runtime | Native `#[gpui]` RSX and native slider/input entities | Add typed pointer/drag/keyboard bindings and control adapters to the HTML-style runtime; native RSX already exposes the GPUI APIs |
| CSS grid/sticky tracks | Explicit RSX rows/flex layout, horizontal timeline scrolling, and a shared vertical track container | Add richer layout primitives if needed; native DockArea/DockSkin and measured panel bounds handle docking/resizing, while bounded native scroll containers handle overflow |

Waveforms use deterministic RSX bars; filmstrips repeat a fixture still. The supplied images contain no standalone source videos/photos, so the photo component clips their existing photographic regions without modifying the reference files. Larger previews of non-skate media therefore inherit the thumbnail resolution. Real poster/thumbnail assets are the replacement point. Native window controls follow the operating system; the in-app bar follows the reference styling.

## Mock inventory

| Area | Mocked calls/data | Visible behavior |
| --- | --- | --- |
| Media catalog | 15 assets (video/audio/image fixtures); names, durations, sizes, dates, codecs, resolution/frame-rate/audio metadata, paths, generated assets | Local selection, multiselect, favorites, search, type/resolution/duration/source filters, folder subsets, and grid/list layout; Audio shows waveforms and Images shows still artwork, type-aware metadata, and generated assets |
| Sorting and folders | Fixture order / alphabetical order; Travel, Projects, House, Documentary, Ads, SFX, Music, Stock, Trash | Sort toggles labels/order; folder selection filters fixture subsets; create-folder is a notice |
| Import | File chooser, filesystem import, drag/drop ingestion | Clicking Import or the import area queues three fixture jobs; no file dialog or file access |
| Processing | Preview generation, thumbnail extraction, audio analysis/waveforms, proxy cache, processing percentages | Static job percentages; import resets the queue; cancel removes individual/all mock jobs |
| Playback | Decode/render frames, audio output, real-time playback, seeking, quality/resolution, repeat behavior | Still-image preview; play/pause, seek/step, mute, quality label, and expanded-player state; no background playback clock |
| Transform/crop | X/Y position, scale, rotation, opacity, four crop values, reset | Sliders and resets update values; X/Y buttons record calls; media pixels remain fixture pixels |
| Compositing | Stabilization, AI tracking, blend modes, masks | Toggles update; blend/mask actions show notices |
| Tracking | Point/box/AI object detection, forward/backward tracking, target, smoothing, path, stabilize, attach target | Mock overlay/path, seek position, labels, sliders, and toggle changes; no detection or analysis |
| Audio/color/effects | Volume, normalization, noise reduction, fades, LUTs, exposure/intensity, auto color, Cinematic/Soft Glow/Film Grain/Vignette | Inspector state changes or notices; no pixel/audio processing |
| Timeline | Six fixture clips on four tracks; repeated filmstrips, synthesized waveforms, fixed automation points | Clip selection, track visibility/lock state, add-track, zoom/fit, and draggable playhead/ruler seeking (including when zoomed or scrolled); no clip drag, resize, or persisted edits |
| Editing commands | Undo, redo, split, delete, ripple, speed, crop, audio, fade, marker | Calls are recorded and a notice names the action |
| Details | Cached thumbnails, audio waveform, analysis statistics, metadata, markers, file location | Tabs change content; favorite state updates; Show in Finder / Add Marker record mock calls |
| Preset browsers | Text titles/lower thirds, effects, transitions, shapes/stickers, caption styles and sample transcript | Dedicated browsers, search, local selection, and representative preview overlays; saved presets, caption generation, and applying presets to real clips remain mocked |
| Project/account | Project menu, profile, notifications, settings | Dismissible notices; no dialogs, authentication, persistence, or external navigation |
| Export/share | Export queue/encoding/destination, cloud sharing, clipboard | Notices show fixture export/link information; no export, upload, clipboard write, or external service |

## Verify

```sh
cargo run -- build examples/video-editor
cargo test --manifest-path examples/video-editor/Cargo.toml --target-dir target --lib
```

The state tests cover type-view selection and stale-filter reset, still-image fixtures, preset selection, full-project/audio seeking, combined filters/favorites, selection and transport bounds, import/cancel, track controls, zoom, and screen transitions. The preserved model validation test also runs. The real FFmpeg end-to-end test remains ignored by default.

## Preserved FFmpeg demo

For the original single-clip trimming/export example:

```sh
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- --legacy
# Optional startup import:
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- --legacy /path/to/clip.mp4
```

This separate mode requires `ffmpeg` and `ffprobe` with H.264 (`libx264`) and AAC support on `PATH`. It probes media, previews frames, trims, rotates, scales, mutes, exports MP4, and uses the system media player. Exports preserve existing destinations; cancellation removes staged output; background work uses a private temporary workspace. The new four-screen UI does not call these services.
