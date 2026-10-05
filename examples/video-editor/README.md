# FlowCut — RSX video editor

Five main screens: **Edit** (the timeline), **Compositing** (a real video/audio node graph), **Tracking** (the editor inside `main.png`), **Media** (the full library), and **Overview** (import and preprocessing). The four original workspaces remain dockable. Switch screens in the top bar. The instructional callouts around `main.png` are design context, not extra application panels or requests to implement processing services.

The app opens with **real bundled media** from [`assets/media`](assets/media/README.md): CC0 Murchison Falls footage with stereo sound, CC0 piano music, and mono/stereo public-domain sound effects. Startup preprocesses these files and automatically adds video and every audio channel to the project timeline. Native file/folder import uses the same pipeline; videos append on the primary video track and audio-only files overlay at the current playhead. Imports list both video and audio, deduplicate canonical paths, and retain real metadata, source thumbnails, and measured waveforms. Failed files stay in the catalog with errors and are not automatically placed.

FFmpeg runs **in-process**. Import preserves encoded video and indexes keyframes, then decodes every audio stream into disk-backed mono PCM caches at its native sample rate and decoded precision. Every channel gets a separate track, linked to its source video. **Split** and **Delete** keep linked ranges aligned; a locked linked track blocks the edit. **Add to timeline** can insert an existing library item again.

The timeline header keeps only zoom and Fit controls. Right-click a **clip** to rename it, edit timing, split at the playhead, duplicate/delete its linked group, or reveal its source. Ripple, Speed, Crop, Audio, Fade, Marker, Undo, and Redo have moved from the header to this menu and retain their current prototype behavior. Right-click **empty timeline space**, the ruler, or the zoom bar for selected-clip actions and **Add track**; an empty project offers Marker, Undo, Redo, and Add track. Right-click a **track** to rename, mute/hide, lock, or move it up/down/to an edge. Right-click **library media** to insert, preview, favorite, or reveal it; the full library's ellipsis opens the same menu. Audio insertion keeps the current playhead, and favorite/reveal commands keep the current selection and transport.

Double-click a clip title or track name to rename inline: **Enter** or clicking away applies a valid name, and **Escape** cancels. Names belong to the project and never rename source files. **Edit timing…** opens inline start, source-in, and duration fields in seconds; **Apply** or **Enter** commits, **Cancel** or **Escape** cancels. Moving preserves linked offsets/gaps; trimming intersects all linked channel ranges with the chosen interval. Clip ranges can shorten or extend within the original source bounds; trimmed channel spans are retained for recovery. Invalid ranges and locked linked tracks block edits.

With the timeline focused, **Delete / Backspace** removes the selected clip and its linked channels; **⌘X / Ctrl+X**, **⌘C / Ctrl+C**, and **⌘V / Ctrl+V** cut, copy, and paste linked clips. The clipboard is project-local. Paste anchors the earliest copied clip at the playhead, preserves timing offsets, source ranges and names, and creates an independent link group with recoverable trims. Clipboard destinations follow track reordering; deleted destinations are recreated on paste and reused for subsequent pastes. Locked linked tracks block Cut/Delete; locked destination tracks block Paste. Inline name/timing inputs retain native text editing shortcuts. Cut, Copy, and Paste are also in the timeline context menus.

Right-click a track and choose **Delete track** to remove that track and its own clips. Other linked tracks retain their clips; deleted channels are also removed from recoverable trim spans. Remaining track references are remapped, and the last track can be deleted. Unlock a locked track before deleting it.

Drag the **body of a clip** to move its linked group in time. Drag either **edge handle** inward to shrink, or outward to restore available source media. The start handle keeps the out point fixed; the end handle keeps the start fixed. Edges clamp to source bounds and keep a positive duration; moving clamps at the project start. **Escape** during a drag restores the original ranges. Playback pauses during dragging and refreshes when released. This changes source ranges at the original playback speed; speed changes remain a prototype.

Drag a track's **⠿ handle** onto another track to reorder; clips, names, gain, mute, visibility, and lock travel with their track, while source ranges and links stay intact.

**Play** in Edit plays the entire timeline: visible video cuts and gaps, and all unmuted audio channels with per-track gain. CPAL feeds a bounded audio queue to the output device; the device clock schedules native video frames. Different audio sample rates are converted only for stereo monitoring, leaving the caches untouched. Global mute, per-track mute/gain, seek/step, and looping work. Media previews the selected source. See the [implementation proof](PROOF.md) and [MVP architecture](MVP.md).

The Inspector currently exposes only **Video** and **Audio** tabs.

**Compositing** follows the [Blender 4.5 LTS compositor](https://docs.blender.org/UATEST/manual/en/4.5/compositing/index.html): a categorized node library, a central graph, an inspector and a persistent Viewer. The catalog contains **131 compositor-compatible Blender node types, including legacy variants**, plus five native audio types. It covers inputs/outputs, color, blur and other filters, keying, masks/Cryptomatte, tracking, nine procedural textures, transforms, utilities, vector operations, imported groups, frames and reroutes. It includes the shared shader node types that Blender permits in compositor trees. Definitions, socket defaults and enum choices are extracted from the official **4.5.14 LTS** runtime into [`assets/blender-nodes.json`](assets/blender-nodes.json); [`scripts/blender_bridge.py`](scripts/blender_bridge.py) regenerates and validates them. Obsolete RNA properties replaced by sockets are omitted from the inspector.

Search by node name or description, or filter by category. Imported footage and separate audio channels appear as media sources. The starter graph adapts [Blender’s Alpha Over example](https://docs.blender.org/UATEST/manual/en/4.5/compositing/types/color/mix/alpha_over.html): blurred background, scaled translucent foreground, Translate, Alpha Over, Composite, and a separate stereo audio mix. Socket colors identify image/color, value, vector, boolean and audio data. Nodes have their actual named inputs and multiple outputs; unlinked Blender sockets use their editable defaults. [Blender’s implicit conversions](https://docs.blender.org/manual/en/4.5/compositing/compositor_system.html) apply between compositor data types; audio remains separate.

Drag headers, use zoom/Fit or Arrange, and click an output then an input to wire nodes. Tall nodes are measured individually. Right-click for categorized node creation, duplication, muting, disconnection, removal or **Frame upstream branch**; dragging a frame moves its contained nodes. **Delete**, **M**, **Shift+D**, and **Cmd/Ctrl+Z / Shift+Z** work while the canvas has focus. Undo/Redo retain up to 100 graph snapshots. Escape cancels a wire or restores a drag. The inspector provides enum dropdowns, booleans, numerical/vector/color fields, live color swatches, ramp stops, curve points and native asset browsing. **Apply settings** (or Cmd/Ctrl+Enter) refreshes sockets when a mode, group or output-slot list changes. Group, mask, texture and scene assets can be loaded from `.blend` files; tracking clips can be loaded from movies or `.blend` files with tracking data. Internal Group Input/Output nodes are evaluated inside imported groups; in-app group construction/editing is not implemented.

**Render preview** evaluates video through an isolated Blender CPU worker and audio through native libavfilter. The worker starts with factory settings, disables automatic script execution and accepts JSON data. It renders one frame at the selected source time (24 fps) and an optional 0.1–10 second audio excerpt. Decoded timeline sources are bounded to 960 × 540; standalone Blender inputs can use their asset dimensions. Preview display/import uses an 8-bit PNG; Blender computes its compositor in scene-linear space with Standard display conversion. File Output retains its generated files under the session render’s `files` folder; its slot names and image format can be edited. Blender’s File Output behavior for dimensionless constant inputs is preserved. A selected connected Composite/Viewer/File Output determines the preview; otherwise Viewer takes precedence over Composite. **Listen to audio** auditions stereo through CPAL; **Add result to project** imports the PNG into Media and the float stereo WAV onto separate channel tracks. Graph changes mark the preview stale. Sources and lossless audio caches remain unchanged. Graph playback, encoded movie export and in-app creation of 3D scenes/tracking/masks remain outside this preview workflow.

Install the compositor runtime once on macOS:

```sh
python3 examples/video-editor/scripts/setup_blender.py
```

This downloads the pinned vendor build, verifies the official SHA-256 checksum, and installs it under ignored `target/blender-runtime/`. A verified runtime is already present in this checkout. Alternatively set `FLOWCUT_BLENDER` to a Blender 4.5 LTS executable; `/Applications/Blender.app` and `PATH` are also searched. Native audio and legacy FFmpeg-only graphs remain usable without Blender. Nodes requiring render passes, packed masks or tracking data require appropriate assets. The bundled application is a development build that uses the repository bridge script and runtime paths.

The original screenshot project data now lives in [`assets/mocks/project.json`](assets/mocks/project.json) and is available through **`--mock`**. Normal startup contains no fictitious media or processing percentages. Projects remain in memory; timeline transform/effect/tracking controls, timeline undo/redo, project export, and sharing remain interface prototypes. Filmstrips currently repeat the actual source poster rather than decoding every thumbnail.
All UI actions go through `Editor::dispatch` → `EditorState::apply`; the last 100 calls are retained in `state.events`. Docking, resizing, scrolling, selection, and playhead dragging are real local UI behavior. Controls that still stand in for services display a dismissible notice.

## Run

Install FFmpeg 9 development libraries, `pkg-config`, and Clang/libclang for the Rust bindings. On macOS with Homebrew:

```sh
brew install ffmpeg pkg-config
```

The default editor links `libavformat`, `libavcodec`, `libavutil`, `libavfilter`, `libswscale`, and `libswresample`; it does not launch `ffmpeg` or `ffprobe` for import/preview/playback. The separate legacy mode still requires those executables.

From the repository root:

```sh
cargo run -- compile examples/video-editor
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target
```

Run with no arguments to load the bundled files. Use `--import /path/to/media` to load your own file or folder instead; imported video and audio appear in the timeline automatically. Use `--empty` to start with no media, or `--mock` for the original visual reference project.

```sh
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- --import /path/to/media
```
Start on a specific screen by appending `-- --library`, `-- --overview`, `-- --tracking`, or `-- --compositing` to the second command. The window starts at 1536 × 980 and supports sizes down to 1100 × 740. Each workspace card has a draggable tab: drag it onto another card to group tabs, or onto a highlighted edge to split the workspace. The gaps between cards are nine-pixel resize targets with visible grips. Drag either vertical or horizontal divisions to resize, down to the native 100-pixel minimum per dock slot; the title-bar expand button zooms a card. Navigation, media/presets, player, inspector, timeline, processing, details, and generated assets all use the same dock host. The Edit/Tracking workspace and Media/Overview workspace retain separate layouts during the session; **Reset layout** restores the current workspace. Content scrolls inside its card, and library columns follow its actual width. Layouts are currently kept in memory only. The preview fullscreen button expands the player within the application.

## Components

The default application starts through the free `entry(window, cx)` function in `src/ui.rsx`. Views are free `#[gpui]` functions composed from smaller `.rsx` components. Native state and docking contracts stay in Rust; their required `Render` implementations simply delegate to the view functions. This supports incremental adoption: replace one view at a time while keeping existing entities, events, and lifecycle code. The separate legacy demo retains its original implementation.

| Source | Responsibility |
| --- | --- |
| `src/ui.rsx` | Function entry point and root screen composition |
| `src/editor.rs` | Editor entity, input subscriptions, action dispatch, thin Render bridge |
| `src/workspace.rs`, `src/dock_skin.rs` | Native docking contracts, workspace construction, renderer adapters |
| `src/components/primitives.rsx` | Palette, icons, action buttons, clipped fixture photos, thumbnails, waveforms, progress bars, section headings, metadata rows |
| `src/components/workspace.rsx`, `src/components/docking.rsx` | Card content routing, measured bounds, island frames, headers, and resize grips |
| `src/components/presets.rsx` | Text, effects, transitions, elements, and captions browsers, preset tiles and sample transcript |
| `src/components/chrome.rsx`, `src/components/navigation.rsx` | Top bar, screen navigation, tool sidebar, and folders |
| `src/components/library.rsx`, `src/components/processing.rsx` | Media library, cards/list rows, search and filters; separate import area and processing queue |
| `src/components/player.rsx`, `src/components/details.rsx` | Preview, transport, seek slider, overlay, filmstrip; separate details and generated assets |
| `src/components/inspector.rsx`, `src/components/properties.rsx`, `src/components/tracking.rsx` | Audio/Video Inspector, reusable property controls, retained tracking reference prototype |
| `src/components/timeline.rsx` | Tool strip, ruler, tracks, clips, filmstrips, waveforms, automation path, playhead, zoom |
| `src/components/tracks.rsx` | Channel controls, inline names, track drag handles/preview, Add Track, and width divider |
| `src/interactions.rs` | Native timeline/media/graph popup menus and scoped track drag payload |
| `src/components/compositing.rsx`, `src/graph_editor.rs` | Node canvas, wires, palette, parameter editing, background rendering and audition |
| `src/composition.rs`, `src/compositing.rs` | Typed graph validation and native libavfilter video/audio preview evaluation |
| `src/state.rs` | Project media references, timeline source ranges, typed actions, explicit asset-backed mock loader, bounded event log, state tests |
| `src/catalog.rs` | Recursive local discovery, source identity, background import, metadata/thumbnail inspection |
| `src/playback.rs` | Device-clock transport, bounded audio output queue, native resampling, timeline mixing |
| `src/proof.rs` | Opt-in native video/audio device proof and offline mix artifacts |
| `src/media.rs` | In-process FFmpeg inspection, scaled BGRA decoding, persistent preview worker, bounded frame queue |
| `src/preprocess.rs` | Native stream analysis, disk keyframe indexes, lossless PCM channel caches, bounded waveform pyramids and random reads |
| `src/legacy.rsx`, `src/model.rs` | Preserved FFmpeg example, available separately with `--legacy` |

Audio and Images select matching fixture previews and reset incompatible filters when opened. Every tool in the editing sidebar opens its own browser; selecting a tool also reveals the media/presets card if it is behind another dock tab. Preset clicks select a tile and show a simple mocked overlay in the preview.

Drag the purple playhead handle to seek, or click/drag the ruler. Pointer positions use the timeline’s rendered bounds, so seeking stays aligned after horizontal scrolling, zooming, and docking. The playhead clamps to the project duration, which grows as clips are added; media-screen transport follows the selected asset’s duration. Tracks share vertical scrolling, while the ruler and clips share horizontal scrolling. Hover the timeline and use the wheel to scroll vertically; hold **Shift** to scroll horizontally. Sideways wheel or trackpad gestures also scroll horizontally without Shift. Diagonal gestures follow their dominant axis so minor sideways motion does not move the timeline during vertical scrolling. Scrolling targets the panel under the pointer without requiring a click. The **Tools & folders** header keeps its draggable tab and omits the expand and ellipsis buttons. Drag the nine-pixel divider beside the channel controls to change their width (100–420 pixels, while preserving at least 160 pixels for the timeline). Zoom uses the draggable slider or −/+ buttons, from 25% to 800%; it preserves the playhead’s viewport position where scrolling bounds allow. Fit sizes the current project to the available timeline width (the demo starts at 18 seconds).

Header and library search fields use separate native input entities with synchronized text. Reusing one entity in two visible controls creates duplicate accessibility IDs in GPUI. Cards, timeline clips, icon buttons, and the import area expose accessible names.

## RSX coverage and gaps

**Visual markup stays in RSX; no compiler change was required.** Rust adapters implement the docking library’s renderer traits and call RSX functions for presentation. The docking library currently exposes a fixed 100-pixel split minimum rather than per-card minimum settings; configurable card limits would require a native docking API extension. Native RSX accepts the GPUI Kit controls, builder methods, event callbacks, iterator children, and canvas used here.

There are still conveniences worth adding if this should also be expressible entirely in the higher-level HTML-style RSX runtime:

| Missing declarative facility | Current implementation | Possible extension / alternative |
| --- | --- | --- |
| Connected vector paths and automation points | A `<canvas>` with a short `PathBuilder` painting callback, inside `timeline.rsx` | Add declarative SVG/path or canvas drawing primitives; alternatively import a `Definition::native` renderer |
| A source rectangle for image fixtures | A clipped, positioned `<img>` showing photographs inside the unchanged reference image | Add a source-rectangle image component; production can supply standalone thumbnails and poster files to `<img>` |
| Editor-specific pointer gestures and entities in the HTML-style runtime | Native `#[gpui]` RSX and native slider/input entities | Add typed pointer/drag/keyboard bindings and control adapters to the HTML-style runtime; native RSX already exposes the GPUI APIs |
| CSS grid/sticky tracks | Explicit RSX rows/flex layout, horizontal timeline scrolling, and a shared vertical track container | Add richer layout primitives if needed; native DockArea/DockSkin and measured panel bounds handle docking/resizing, while bounded native scroll containers handle overflow |

Fixture waveforms use deterministic RSX bars; filmstrips repeat a poster still. Imported videos have FFmpeg-generated thumbnails and real decoded video preview; imported images display the original image. Imported audio waveforms use bounded min/max pyramids from the decoded samples. The supplied images contain no standalone source videos/photos, so the photo component clips their existing photographic regions without modifying the reference files. Larger previews of non-skate media therefore inherit the thumbnail resolution. Imported assets replace those fixture crops with real thumbnails. Native window controls follow the operating system; the in-app bar follows the reference styling.

## Implemented and remaining

| Area | Current behavior |
| --- | --- |
| Compositing | 131 Blender types + five audio types, typed multi-output graph, grouped searchable library, settings/ramps/curves/assets, zoom/Fit/Arrange, frames, undo/redo, Blender preview/File Output, audio audition and result import; continuous graph playback/movie export pending |
| Media and import | Real bundled or local files; recursive folders, metadata, source thumbnails, duplicate detection, cancellation, search/filter/favorites, grid/list |
| Preprocessing | Original encoded video, disk keyframe indexes, separate native-rate/precision PCM channels, measured waveform pyramids |
| Timeline | Automatic video/channel insertion, linked split/delete/duplicate, inline names and linked timing edits, clip movement and reversible edge trims, drag/menu track reordering and deletion, Cut/Copy/Paste/Delete shortcuts, visibility/lock/mute/gain, zoom, playhead seeking |
| Playback | Native decoded video cuts and gaps; all channel tracks mixed for stereo output; device-clock synchronization, seek, loop, global mute |
| Visual reference data | Original catalog, folders, jobs, tracks and clips moved to `assets/mocks/project.json`; loaded only by `--mock` |
| Remaining prototypes | Project save/open, undo/redo, playback speed/time stretching, effects, transforms, tracking, normalization/noise reduction/fades, export/share |

## Verify

```sh
cargo run -- build examples/video-editor
cargo test --manifest-path examples/video-editor/Cargo.toml --target-dir target --lib
cargo clippy --manifest-path examples/video-editor/Cargo.toml --target-dir target --all-targets -- -D warnings
```

Tests import the actual bundled files, verify all separate channel tracks and overlays, skip repeated imports without duplicating clips, and confirm sources stay unchanged. They also cover native 10-bit video/multiple sample rates, byte-exact packed/planar PCM separation, bounded waveforms/random reads, native resampling/pitch, overlapping mixing/gap silence, callback seek generations, mute/gain, cache cleanup, linked edits, real decode/seek, and preview backpressure. The legacy CLI export test is opt-in. Hardware playback proof is described in [PROOF.md](PROOF.md).

Interaction regression tests also verify track remapping/playback properties, project-local names, separate duplicate links, linked timing and lock/range validation, and context targeting without losing the playhead. Native UI checks are recorded in [PROOF.md](PROOF.md).

## Preserved FFmpeg demo

For the original single-clip trimming/export example:

```sh
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- --legacy
# Optional startup import:
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- --legacy /path/to/clip.mp4
```

This separate mode requires `ffmpeg` and `ffprobe` with H.264 (`libx264`) and AAC support on `PATH`. It probes media, previews frames, trims, rotates, scales, mutes, exports MP4, and uses the system media player. Exports preserve existing destinations; cancellation removes staged output; background work uses a private temporary workspace. The four-screen editor uses the FFmpeg libraries for import/preview and keeps this legacy CLI export flow separate.
