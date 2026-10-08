# Overtone

A native recording and composition workspace, authored in `.rsx`.

```sh
cargo run --manifest-path examples/overtone/Cargo.toml --release --bin overtone
```

Microphone capture and speaker playback are enabled by default. On Debian/Ubuntu, install `libasound2-dev` and `pkg-config` before building. For offline use without device dependencies, use `--no-default-features --features desktop`; WAV export still works.

## Record, split, compose

1. Click **Record microphone** in **Record & split**. Record your voice, taps, instruments, or other sounds, leaving pauses between them. The input meter shows the default microphone's level. Takes stop automatically at 120 seconds.
2. Click **Stop & split**. Overtone splits at pauses and adds every detected sound to **Sounds** and the selected timeline track. Each clip has its own waveform and plays the original recorded audio. Internal gaps retain their original spacing; the first sound begins at **Insert**.
3. Click **Listen** to audition a sound. Drag sounds onto tracks, use **+ Timeline** to reuse them, or drag existing clips to rearrange them. Select a clip to adjust its level, mute it, change tracks, duplicate, or delete it.
4. For a manual cut, click inside a recorded clip on the timeline to set **Insert**, then click **Split at Insert** in **Selected clip**. Turn snapping off for a precise cut. Each resulting piece moves independently; the library sound and original recording remain intact. Undo restores the cut.
5. Use **Play / Pause / Stop** to hear your composition, and **Export WAV** to save a stereo mix.

The default desk shows only recording, sounds, timeline, and selected-clip controls. Harmonic synthesis, keyboard, and reconstruction panels remain available through **View → Workspace settings** for older projects and advanced use. Saved projects retain their own layouts.

**Import audio** also accepts a recording. Choose **Split → timeline** on its source card. Repeating a split adds new independent clips and leaves previous edits intact. Adjust **Silence**, **Split after … ms silence**, and **Minimum sound** when sounds merge or quiet sounds are missed. This detects separate events in time; overlapping sounds remain together.

Recorded clips keep their original pitch and length when BPM changes. Tempo controls the placement grid. Sample playback uses small edge fades to avoid clicks. Audio is downmixed to mono at import/capture and panned into stereo for playback/export.

## Projects and workspace

**File → New / Open / Save / Save as** manages `.overtone` JSON projects. Ctrl/Cmd+S saves, Ctrl/Cmd+O opens, Ctrl/Cmd+N creates a session. Open a project at launch by adding its absolute path after `--`. Closing or replacing an edited session protects unsaved work with Save / Discard / Cancel. Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z undo and redo.

Saving copies recordings into `<project-stem>.assets/sources/` beside the project, using relative paths and SHA-256 verification. Move the project and its companion `.assets` directory together. Cuts store source bounds without rewriting or duplicating the recording. Library removal preserves placed clips.

Workspace settings retain panel visibility, layout, resizing and zoom, and can be exported/imported independently of the composition. See [RECORDING.md](RECORDING.md), [PROJECT_FORMAT.md](PROJECT_FORMAT.md), [ARCHITECTURE.md](ARCHITECTURE.md), and [ENGINE.md](ENGINE.md) for details.

## Verification

```sh
cargo test --manifest-path examples/overtone/Cargo.toml
cargo test --manifest-path examples/overtone/Cargo.toml --features ui-tests
cargo run --manifest-path examples/overtone/Cargo.toml --bin overtone -- --validate
```

The native UI tests use generated audio without opening the user's microphone. Physical microphone and speaker behavior must be checked on the target device.
