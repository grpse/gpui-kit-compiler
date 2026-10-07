# Overtone

Native GPUI composition studio authored entirely in `.rsx`. The three workspace profiles share one project: **Studio Desk**, **Voice Lab**, and **Clip Composer**. There is no audio or synthesis engine.

From the repository root:

```sh
cargo run --manifest-path examples/overtone/Cargo.toml
```

Open a saved project at launch:

```sh
cargo run --manifest-path examples/overtone/Cargo.toml -- /absolute/path/session.overtone
```

Cargo runs `src/build.rsx` to compile RSX markup into its `OUT_DIR`. The application entry point, core data, persistence, components, and tests are all authored in `.rsx`; generated `.rs` files are build artifacts only. Use Cargo directly for this example, since the repository CLI currently expects `src/main.rs`.

## Working in the studio

The workspace begins directly below the menu bar. Select a profile through **View**; use **File → Rename project** to change its name. The title bar identifies the project and active profile, with an asterisk for unsaved changes. Closing an edited session shows a modal **Save / Don’t save / Cancel** popup. Canceling a save dialog or a failed save keeps the session open. **Help → About Overtone** shows session feedback.

- The top title bar moves the window and provides close/minimize/maximize controls supported by the desktop. Drag window edges or corners to resize; double-click the title bar to maximize/restore. Closing protects unsaved project changes with Save/Discard/Cancel.
- Edit the draft's name, up to 32 harmonic amplitudes, phase and detune, envelope, brightness, gain, and white/pink/brown noise. Drag knobs vertically. Added harmonics start at zero amplitude; removing them discards their values.
- **Save to library** creates a separate saved sound. Search the library, edit a copy, or add a sound to the timeline. **+ Timeline** places the current draft on the selected target track at the insertion time.
- The keyboard writes sequential MIDI notes into the selected instance. When there is no selection, the first key creates an instance from the draft. Each click stores a one-beat note; the instance expands to fit. No audio plays.
- Timeline instances own a snapshot of their sound. Select an instance and use Clip Composer's inspector to edit harmonics/noise/envelope, mute, change level, move to another track, change start/duration, edit note positions, duplicate, or reset its sound. These edits preserve the library and other instances.
- Drag instances between track lanes or along the timeline; pointer placement snaps to 0.25 seconds. Starts are stored in seconds and lengths/notes in beats. Tracks can follow the session BPM or use their own BPM (20–300); changing tempo adjusts lengths while preserving starts. Mute/solo are saved composition settings.
- Import a voice file as an opaque reference and attach it to the draft. Files are fingerprinted and stored; they are **not decoded or analyzed**. Record, Analyze, and Play remain disabled.
- Drag panel headers between the left, center, right, and lower docks. Drag dock dividers for sidebar widths and lower dock height, dividers between paired panels for their proportions, and each panel's bottom grip for its height. Drag a gap between rows, or hold **Alt** while dragging a divider, to adjust spacing. Individual panel gaps can override the profile default. Each profile owns these dimensions, panel visibility/order, and spacing/padding. Paired panels retain minimum usable widths and scroll when necessary; reset a fixed height to **Auto height** or restore **Default gaps** in Configure workspace.
- While dragging, a translucent panel previews its future position. Releasing commits the move; dropping outside the workspace cancels it.
- Inner edge shadows indicate more content in each scroll direction, disappear at the corresponding boundary, and remain independent of scrollbar visibility. They scale with the workspace and panel zoom.
- Open **View → Workspace settings** (Ctrl/Cmd+,) for the dedicated Settings window. This is the only place to disable panels. Copy profiles, create empty profiles, and configure module placement, spacing, padding, and height. **Save settings** exports all profiles and the theme to standalone JSON; **Load settings** applies them while preserving the composition.
- **View** provides workspace zoom (50–200%); each panel also has its own zoom controls (50–200%). Both scales affect dimensions and fonts, combine with each other, and are saved per profile. Ctrl/Cmd+Plus/Minus changes workspace zoom; Ctrl/Cmd+0 resets it. Scrollbars appear wherever content overflows.
- **File → New session / Open project / Save / Save as** manages `.overtone` JSON projects. Shortcuts: Ctrl/Cmd+S, Ctrl/Cmd+Shift+S, Ctrl/Cmd+O, Ctrl/Cmd+N. Saving includes composition and workspace configuration. New/Open/window close protect unsaved edits with Save/Discard/Cancel.
- **Edit → Undo / Redo** restores project changes, including layout edits. Ctrl/Cmd+Z undoes; Ctrl/Cmd+Shift+Z redoes (Ctrl+Y also works). History retains up to 50 changes in memory; continuous knob and resize gestures form one change.

## Storage

Schema version **2** uses compact JSON with a deduplicated patch table. Drafts, library sounds, and timeline instances refer to patches by index. Harmonics use `[multiple, amplitude, phase?, detune?]` and notes use `[midi, start_beat, duration_beats, velocity]`. Identical sound definitions occupy one table entry; instance edits create distinct definitions on save and remain independent when reopened. Noise color/level/seed, ADSR, tone/gain, tuning, voice references, all track settings, clip resets, and the complete workspace are preserved without quantizing parameters. Version 1 projects migrate on Open.

See [PROJECT_FORMAT.md](PROJECT_FORMAT.md) for the schema, units, limits, reference rules, layout behavior, and standalone settings format. `.overtone` and `.json` are both accepted project filenames. Project and settings saves use atomic replacement.

Saving copies references into `session.assets/sources/<sha256>.<extension>` beside `session.overtone`. The JSON uses relative asset paths: move the project and its companion `.assets` directory together. Saved references are verified before reuse; changed source files must be imported again. Saving uses a temporary file and atomic rename so a failed write preserves the previous project. Missing stored assets are reported on Open; an existing project can still save parameter edits, while Save As requires restoring missing references.

This implementation stores the intended audio operations as data. It does not contain microphone capture, waveform decoding, FFT, harmonic matching, synthesis, transport playback, or audio export.

## Verification

```sh
cargo test --manifest-path examples/overtone/Cargo.toml
cargo test --manifest-path examples/overtone/Cargo.toml --features ui-tests
cargo run --manifest-path examples/overtone/Cargo.toml -- --validate
```

The optional UI suite exercises native clicks and dragging. To generate native screenshots with a GPU renderer:

```sh
OVERTONE_PREVIEW_DIR=/tmp/overtone-preview cargo test --manifest-path examples/overtone/Cargo.toml --features ui-tests render_profiles_for_visual_review -- --ignored
```
