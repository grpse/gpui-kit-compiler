# Overtone project and workspace formats

The project is UTF-8 JSON, saved as `.overtone` or `.json`. Version 2 combines readable named objects with compact arrays for repeated harmonic and note data. Identical patches are interned into one table entry; this avoids repeating up to 32 harmonics for every instance and its reset snapshot. Values retain their stored numeric precision. File whitespace is omitted.

The serializer and reader are implemented in `src/storage.rsx`; validation lives in `src/model.rsx`, and filesystem/asset handling lives in `src/persistence.rsx`. This is a UI and persistence contract for a future engine. No sound analysis or rendering takes place yet.

## Project structure

| Field | Meaning |
| --- | --- |
| `format` | `"overtone"` |
| `schema_version` | `2`; controls document migration |
| `synthesis_model` | `"additive-sine-v1"`; identifies the intended synthesis parameter model separately from document structure |
| `title`, `next_id` | Project name and next monotonically allocated object ID |
| `session_bpm` | Session tempo, 20–300 BPM |
| `tuning` | `{ "reference_midi": 69, "reference_hz": 440.0 }` by default |
| `patches` | File-local table of harmonic/noise/envelope/tone definitions |
| `draft` | `{ "id": 0, "name": "…", "patch": 0 }` |
| `library` | Saved sound identities/names, each with a patch index |
| `sources` | Imported voice references, asset locations, fingerprints, and future analysis inputs |
| `tracks` | Track identities/names, BPM, session-tempo linkage, gain/pan, mute/solo |
| `clips` | Timeline instances, their current and original sound references, timing, notes, gain/mute |
| `workspace` | Profiles, active profile, theme, selected clip, target track, insertion time |

Objects with persistent IDs are tracks, library sounds, sources, clips, and profiles. IDs are positive and unique across those objects; `next_id` exceeds them. A patch index is a zero-based position in `patches`, local to that save file. Names and IDs are kept outside patches so renamed copies can reuse the same sound parameters.

The following fragment illustrates the sound and timeline fields. A complete document additionally contains the other fields in the table, valid tracks, and workspace profiles.

```json
{
  "format": "overtone",
  "schema_version": 2,
  "synthesis_model": "additive-sine-v1",
  "tuning": { "reference_midi": 69, "reference_hz": 440.0 },
  "patches": [
    {
      "partials": [[1, 0.8], [2, 0.25, 30, -3], [3, 0.12]],
      "noise": { "level": 0.08, "color": "Pink", "seed": 42 },
      "envelope": {
        "attack_ms": 20, "decay_ms": 180,
        "sustain": 0.7, "release_ms": 400
      },
      "brightness": 0.5,
      "gain": 0.8
    }
  ],
  "draft": { "id": 0, "name": "Vocal shimmer", "patch": 0 },
  "library": [{ "id": 10, "name": "Vocal shimmer", "patch": 0 }],
  "clips": [
    {
      "id": 11, "track": 1, "name": "Vocal shimmer",
      "start_seconds": 2, "duration_beats": 4,
      "sound": { "id": 10, "name": "Vocal shimmer", "patch": 0 },
      "original_sound": { "id": 10, "name": "Vocal shimmer", "patch": 0 },
      "library_sound": 10,
      "notes": [[60, 0, 1, 0.8], [64, 1, 1, 0.7]],
      "gain": 1, "muted": false
    }
  ]
}
```

## Harmonics and future synthesis inputs

Each patch stores 1–32 sine partials. Rows are `[multiple, amplitude, phase_degrees?, detune_cents?]`, ordered with consecutive multiples starting at 1. Amplitude is linear, 0–1; phase is −180° to +180°; detune is −100 to +100 cents. Omitted phase/detune means zero. A detuned row with zero phase must include the zero placeholder, e.g. `[2, 0.25, 0, -3]`. Silent partials are retained so the editing configuration survives.

Given note `m`, the intended pitch is `reference_hz × 2^((m − reference_midi) / 12)`. A partial's frequency is that pitch multiplied by `multiple × 2^(detune_cents / 1200)`. Amplitude and phase define its sine component; the stored ADSR describes its amplitude envelope. No waveform samples or sample rate are needed to store this parameter model.

Noise stores `color` (`White`, `Pink`, `Brown`), linear `level` (0–1), and an unsigned 64-bit `seed` reserved for reproducible future noise generation; zero is a valid seed. The engine's generator and color-filter algorithms still need to be specified before promising identical rendered samples. Attack, decay, and release use milliseconds (0–10,000), while sustain, sound gain, and brightness use 0–1. Brightness is a saved tone-control input whose processing is deferred to the engine. A patch optionally includes `source`, referring to a voice-reference ID.

Project tuning, harmonic rows, phase/detune, noise, envelope, notes, and gain therefore preserve the inputs a future additive synthesizer needs. Voice-to-harmonic fitting is a separate future operation; importing a reference does not generate a patch automatically.

## Timeline and independent instances

Clip starts use absolute `start_seconds` (0–86,400). Clip `duration_beats` and note positions/durations use beats. Each note row is `[midi, start_beat, duration_beats, velocity]`, with MIDI 0–127 and velocity 0–1. Notes are relative to their clip and must fit within its duration.

A track stores `bpm` (20–300), `follow_session`, `gain` (0–1), `pan` (−1 to +1), `muted`, and `solo`. Its effective tempo is the project BPM when `follow_session` is true, otherwise its own BPM. Duration in seconds is `duration_beats × 60 / effective_bpm`. Tempo changes preserve clip starts.

Each clip carries `sound` and `original_sound`. The original is the reset baseline; `library_sound`, when present, identifies the library source. Loading expands patch references into independent sound values. Tweaking an instance leaves its original, library source, and other instances intact. On save, identical values share a table entry and changed values get a separate entry. Patch sharing compresses storage without linking mutable editing state.

## Layout and settings

`workspace` contains `profiles`, `active_profile`, `dark`, `selected_clip`, `target_track`, and `insert_seconds`. Each profile stores:

| Field | Meaning and allowed range |
| --- | --- |
| `id`, `name` | Persistent identity and profile name |
| `zoom` | Workspace scale, 0.5–2.0; defaults to 1.0 when absent |
| `left_width`, `right_width` | Dock widths, 200–420 px |
| `lower_height` | Preferred lower dock height, 180–700 px; capped to leave room for the upper workspace in smaller windows |
| `panel_gap` | Gap between panel rows and between paired panels, 4–40 px |
| `dock_gap` | Dock divider/gap width, 6–32 px |
| `panel_padding` | Interior padding of every panel, 4–32 px |
| `panels` | Ordered placements for all seven modules, including hidden ones |

A placement stores `module`, `dock`, `visible`, `half`, `share`, and `height`, plus optional `gap_after` and `pair_gap`. Modules are `Source`, `Library`, `Harmonics`, `Controls`, `Keyboard`, `Timeline`, and `Inspector`; docks are `Left`, `Main`, `Right`, and `Lower`. Consecutive visible `half` panels in one dock form a pair. The first panel's `share` (0.15–0.85) controls its fraction; the other receives the remainder, subject to each module's minimum usable width. A single half panel occupies the full row. `height: null` means content-driven height; otherwise it is 100–1,800 px with a scrolling body. Layout values use logical pixels and are stored separately per profile.

`gap_after` overrides the vertical separation following a row; `pair_gap` overrides the horizontal separation within a pair. Both use 4–40 px and inherit `panel_gap` when absent. The first placement owns a paired row's gaps. Normal divider dragging resizes docks, paired proportions, or panel heights; dragging a row gap adjusts just that separation. Alt-dragging a panel divider changes its individual gap in the divider's direction, and Alt-dragging a dock divider changes `dock_gap`. Configure workspace exposes default gap/padding controls, height reset, and reset of individual gap overrides. All layout edits mark the project dirty.

Standalone workspace settings use their own schema version:

Each placement additionally stores `zoom` (0.5–2.0, default 1.0). Its effective scale is profile zoom multiplied by placement zoom. Layout dimensions are measured at 100% zoom; scaling changes displayed dimensions and fonts without rewriting stored sizes. Zoom is retained in both projects and standalone settings. Drag previews, scroll offsets, and Undo/Redo history are transient and are not serialized. Panel visibility is edited only in the dedicated Workspace settings window.

```json
{
  "schema_version": 1,
  "profiles": ["same complete profile objects as workspace.profiles"],
  "active_profile": 3,
  "dark": true
}
```

The array above is schematic; files contain profile objects. **Save settings** exports all profiles and theme. **Load settings** validates them, replaces the current layouts/theme, and remaps profile IDs to avoid collisions in the destination project. Its music and voice references are preserved. Settings import marks the project dirty; exporting settings does not mark unsaved composition edits as saved.

## Assets, migration, and validation

Voice files remain opaque references. Metadata stores filename, byte count, SHA-256, relative asset path, and analysis settings (`max_harmonics`, optional `fundamental_hz`, trim start/end seconds). Asset copies live in `<project-stem>.assets/sources/<sha256>.<extension>`. Move that directory with the project. The harmonic definition can be edited without decoding its reference file.

Saves validate values and references before atomically replacing JSON. Existing saves survive write failures. Source fingerprints are checked before reuse; modified files require reimport. Missing assets are reported on Open. A project can still save edits beside its existing missing-asset path; Save As requires restoring missing references.

Project loading is capped at 64 MiB and standalone settings at 1 MiB. Unknown project/settings versions or synthesis models, malformed JSON, dangling references, out-of-range values, and invalid layouts are rejected. Flat version 1 projects migrate to version 2, supplying default tuning, noise seed, panel sizes, proportions, and spacing. New saves always use version 2.
