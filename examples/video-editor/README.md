# FlowCut — RSX video editor

FlowCut provides a complete local **import → edit → save/open → export MP4** workflow. Four dockable screens cover Edit, Media, Overview and an optional Compositing preview workspace. The editor starts empty; use **Import files** or **Import folder**. `--demo` loads the real licensed media in `assets/media`; runtime screenshot fixtures and simulated processing are removed.

Imports discover supported video, audio and still images, inspect streams, index video keyframes, decode each audio channel into lossless native-rate PCM, measure waveforms and generate a source thumbnail. The processing panel shows actual per-file stages, packet-position progress and errors. Cancellation stops remaining work and retains files already prepared successfully. Failed sources stay in the library with their error and are excluded from automatic timeline insertion. Sources are never rewritten.

Import automatically adds video and separate linked audio tracks. **Add to timeline** inserts library media again; still images start with a five-second duration. Drag a clip to move its linked group, or either edge to trim within the source range. Double-click clip/track names to rename. **Edit timing** sets start, source-in and duration in seconds. Right-click clips for split, cut/copy/paste, duplicate, delete and source reveal. Track menus support rename, reorder, delete, lock, visibility and mute; audio gain is available in the Inspector. Locked linked tracks block edits.

With the timeline focused, Delete/Backspace removes linked clips; Cmd/Ctrl+C, X and V copy, cut and paste. **Undo/Redo** buttons and Cmd/Ctrl+Z / Shift+Z restore up to 100 timeline edit snapshots. Pointer drags create one edit on release; Escape restores the original range. The media library remains available after undoing an import's timeline insertion.

**Play** and paused timeline seeking show the highest visible video/still track at the playhead, with black visual gaps and mixed unmuted audio channels. Media previews the selected source. Preview mute and monitor volume affect listening only; per-track mute/gain affect both playback and export. Different sample rates are resampled for stereo output without changing source caches.

**Save project** writes versioned `.flowcut` metadata containing media references, timeline ranges, linked recovery spans, track properties and the compositor graph. **Open project** regenerates media analysis before replacing the current project; invalid projects or missing sources show an error and preserve the current editor state. Generated session media is copied into a sibling `.flowcut.media-…` folder on save so it survives application shutdown. Keep this folder beside the project. Ordinary imported files remain external references and must stay available. Dock layouts, transport position and edit history remain session-local.

**Export MP4** renders a snapshot of the complete timeline through native FFmpeg libraries: H.264 (CRF 20), 30 fps, source-shaped canvas capped at 1920 × 1080, and 48 kHz stereo AAC at 192 kbps. It respects source-in ranges, cuts, gaps, still images, track visibility, mute and gain. Other aspect ratios are centered with black borders. The processing panel reports rendering and finalization progress and offers cancellation and source-file reveal. Export uses staged output, checks sources for changes, preserves existing destination files and removes partial output on failure/cancellation. The finished MP4 can be imported and played in the editor.

Unfinished tracking, sharing, account, preset, transform/mask, normalization and fade controls have been removed from the editing interface. The Inspector exposes working timing, naming, visibility and audio controls. All action rows, including the top bar, library, transport, processing and timeline controls, start at the left.

Hover the timeline to scroll vertically. Hold **Shift** for horizontal scrolling; sideways wheel/trackpad gestures also scroll horizontally. Diagonal gestures follow their dominant axis. Scrolling follows the hovered panel without clicking. The Tools header retains its draggable tab without expand/ellipsis buttons. Tracks share vertical scrolling; clips and ruler share horizontal scrolling. Zoom (25–800%) preserves the playhead's viewport position where bounds allow, and Fit shows the complete timeline.

Drag panel tabs to group or split the workspace and drag the nine-pixel dividers to resize. Edit and Media/Overview retain separate layouts during the session. **Reset layout** restores the current workspace. Preview fullscreen expands the player within the app. Filmstrips repeat the measured source poster rather than generating a thumbnail sequence.

**Compositing** follows the [Blender 4.5 LTS compositor](https://docs.blender.org/UATEST/manual/en/4.5/compositing/index.html): a categorized node library, a central graph, an inspector and a persistent Viewer. The catalog contains **131 compositor-compatible Blender node types, including legacy variants**, plus five native audio types. It covers inputs/outputs, color, blur and other filters, keying, masks/Cryptomatte, tracking, nine procedural textures, transforms, utilities, vector operations, imported groups, frames and reroutes. It includes the shared shader node types that Blender permits in compositor trees. Definitions, socket defaults and enum choices are extracted from the official **4.5.14 LTS** runtime into [`assets/blender-nodes.json`](assets/blender-nodes.json); [`scripts/blender_bridge.py`](scripts/blender_bridge.py) regenerates and validates them. Obsolete RNA properties replaced by sockets are omitted from the inspector.

Search by node name or description, or filter by category. Imported footage and separate audio channels appear as media sources. The starter graph adapts [Blender’s Alpha Over example](https://docs.blender.org/UATEST/manual/en/4.5/compositing/types/color/mix/alpha_over.html): blurred background, scaled translucent foreground, Translate, Alpha Over, Composite, and a separate stereo audio mix. Socket colors identify image/color, value, vector, boolean and audio data. Nodes have their actual named inputs and multiple outputs; unlinked Blender sockets use their editable defaults. [Blender’s implicit conversions](https://docs.blender.org/manual/en/4.5/compositing/compositor_system.html) apply between compositor data types; audio remains separate.

Drag headers, use zoom/Fit or Arrange, and click an output then an input to wire nodes. Tall nodes are measured individually. Right-click for categorized node creation, duplication, muting, disconnection, removal or **Frame upstream branch**; dragging a frame moves its contained nodes. **Delete**, **M**, **Shift+D**, and **Cmd/Ctrl+Z / Shift+Z** work while the canvas has focus. Undo/Redo retain up to 100 graph snapshots. Escape cancels a wire or restores a drag. The inspector provides enum dropdowns, booleans, numerical/vector/color fields, live color swatches, ramp stops, curve points and native asset browsing. **Apply settings** (or Cmd/Ctrl+Enter) refreshes sockets when a mode, group or output-slot list changes. Group, mask, texture and scene assets can be loaded from `.blend` files; tracking clips can be loaded from movies or `.blend` files with tracking data. Internal Group Input/Output nodes are evaluated inside imported groups; in-app group construction/editing is not implemented.

**Render preview** evaluates video through an isolated Blender CPU worker and audio through native libavfilter. The worker starts with factory settings, disables automatic script execution and accepts JSON data. It renders one frame at the selected source time (24 fps) and an optional 0.1–10 second audio excerpt. Decoded timeline sources are bounded to 960 × 540; standalone Blender inputs can use their asset dimensions. Preview display/import uses an 8-bit PNG; Blender computes its compositor in scene-linear space with Standard display conversion. File Output retains its generated files under the session render’s `files` folder; its slot names and image format can be edited. Blender’s File Output behavior for dimensionless constant inputs is preserved. A selected connected Composite/Viewer/File Output determines the preview; otherwise Viewer takes precedence over Composite. **Listen to audio** auditions stereo through CPAL; **Add result to project** imports the PNG into Media and the float stereo WAV onto separate channel tracks. Graph changes mark the preview stale. Sources and lossless audio caches remain unchanged. Graph playback, encoded movie export and in-app creation of 3D scenes/tracking/masks remain outside this preview workflow.

Install the compositor runtime once on macOS:

```sh
python3 examples/video-editor/scripts/setup_blender.py
```

This downloads the pinned vendor build, verifies the official SHA-256 checksum, and installs it under ignored `target/blender-runtime/`. A verified runtime is already present in this checkout. Alternatively set `FLOWCUT_BLENDER` to a Blender 4.5 LTS executable; `/Applications/Blender.app` and `PATH` are also searched. Native audio and legacy FFmpeg-only graphs remain usable without Blender. Nodes requiring render passes, packed masks or tracking data require appropriate assets. The bundled application is a development build that uses the repository bridge script and runtime paths.

## Run and verify

The native editor links FFmpeg libraries for import, preview, playback and export. The separately preserved `--legacy` demonstration uses the FFmpeg CLI. On macOS install the native build dependencies with `brew install ffmpeg pkg-config` if they are absent.

From the repository root:

```sh
cargo run -- compile examples/video-editor
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target
```

Use `-- --demo` for bundled real media, `-- --import /path/to/media` for your own file/folder, or `-- --library`, `-- --overview`, or `-- --compositing` to choose the initial screen. The window starts at 1536 × 980 and supports 1100 × 740 and larger.

```sh
cargo test --manifest-path examples/video-editor/Cargo.toml --target-dir target
cargo clippy --manifest-path examples/video-editor/Cargo.toml --target-dir target --all-targets -- -D warnings
```

Tests cover native import/processing stages, corruption and cancellation, byte-exact channel caches, measured waveforms, real decode/seek, mixing and audio timing, linked editing, track remapping, project round trips including temporary-media survival, and playable H.264/AAC output with trims, gaps, still images, visibility and cancellation. Historical JSON editing fixtures are test-only. The legacy CLI test is opt-in. Native UI evidence is recorded in [PROOF.md](PROOF.md).

## Sources

RSX contains the visual markup. Rust adapters connect docking and native workers to free `#[gpui]` view functions. `editor.rs` manages interaction/transport; `editor_operations.rs` connects file dialogs, history and background work; `state.rs` stores editing decisions; `catalog.rs`, `processing.rs`, `preprocess.rs` and `media.rs` handle real sources; `playback.rs` mixes and presents them; `project.rs` persists references; `export.rs` encodes the timeline. The independent compositor uses `composition.rs`, `compositing.rs`, `blender_backend.rs` and `graph_editor.rs`.

See [MVP.md](MVP.md) for resource bounds and current export/compositor limits.
