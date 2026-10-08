# Sample playback and synthesis API

The engine directly consumes the existing model. It does not require a new project schema or duplicate sound settings in a DSP-specific save file. All authored implementation remains `.rsx`.

## Choice and research

A bounded additive oscillator bank is a direct fit for 1–32 editable sine partials, phase, detune and noise. General synthesis libraries such as [FunDSP](https://docs.rs/crate/fundsp/latest/source/README.md) offer broader DSP graphs, but this implementation needs a small, explicit model with instance ownership and predictable bounded work. [CPAL](https://docs.rs/cpal/0.18.2/cpal/) handles cross-platform device I/O rather than synthesizing sounds. [rtrb](https://docs.rs/rtrb/0.4.0/rtrb/) supplies a preallocated, wait-free single-producer/single-consumer command queue.

The sound model also follows the useful sinusoids-plus-noise direction described in [Spectral Modeling Synthesis](https://mtg.upf.edu/node/251) and [Musical Sound Modeling with Sinusoids plus Noise](https://mtg.upf.edu/node/111). Recorded-sound reconstruction is a separate analysis operation: decode the reference, estimate pitch, track STFT peaks over time, fit amplitudes/phases/detunes, and estimate the residual noise spectrum. The current single static patch and ADSR cannot preserve changing phonemes, formants, unvoiced consonants or transients. Those require time-varying harmonic/noise trajectories and an extended storage contract. The main recording workflow splits into sample regions and plays the original audio, as described in [RECORDING.md](RECORDING.md). Legacy harmonic fitting remains available as an explicit API.

## Parameter mapping

| Saved/UI parameter | Rendering behavior |
| --- | --- |
| MIDI/tuning | `reference_hz × 2^((midi − reference_midi) / 12)` |
| Harmonic multiple/detune | Fundamental × multiple × `2^(detune_cents/1200)` |
| Amplitude/phase | Linear sine level; initial phase in degrees; phase edits take the shortest circular path |
| Harmonics near Nyquist | Fade from 0.45 to 0.5 × sample rate; remove partials at/above Nyquist rather than folding them into aliases |
| White noise | Seeded xorshift64 sequence mapped to bipolar samples |
| Pink noise | Eight-row Voss-style bank plus white sample, divided by nine |
| Brown noise | Leaky integrated white noise with a 20 Hz leak and bounded state |
| Noise seed | Exact nonzero seed; zero maps to a fixed nonzero seed. Seed changes affect new voices. |
| ADSR | Linear attack/decay/sustain/release. Attack/release have a minimum 1 ms click-prevention ramp; decay has a minimum one sample. Gate ends at the stored note duration. |
| Brightness | One-pole low-pass; logarithmic cutoff from 100 Hz to 18 kHz, capped at 0.45 × sample rate |
| Gain/velocity | Sound gain × note velocity × clip gain × track gain |
| Pan | Constant-power stereo pan; center is approximately 0.707 per channel |
| Headroom | Per-patch normalization by `max(1, sum(partial amplitudes) + noise level)`, master gain 0.25 by default, final clamp to ±1 |

Amplitude, phase, frequency, noise level/color, brightness, gain, normalization and sustain changes are smoothed with a 5 ms one-pole coefficient, settling/snapping after 50 ms. Attack/decay durations already in progress remain those captured at note start; changes apply to subsequent notes. Release edits affect a voice before release begins; an active release retains its duration. Timing/track mixing/tempo are captured in the playback plan, so restart playback after those edits. Sound-parameter commands affect currently active voices owned by the edited draft or clip; later scheduled notes retain the sound snapshot compiled when playback started.

Steady voices use double-precision recursive sine/cosine rotations; table lookup with linear interpolation supports smooth live edits. Initial coefficients and notes are prepared on the control thread. Updating active voices uses table-derived normalized rotations. There is no per-sample sin/cos, FFT, allocation, deallocation, lock, JSON, file I/O or UI work. Block boundaries preserve oscillator, filter, noise and envelope state. Determinism is defined for the same sample rate, configuration, seed, note ordering and control sequence; cross-platform floating-point output is not promised to be bit-identical.

## Public wrapper

`SynthApi::new(EngineConfig, Tuning)` validates configuration. `patch(&Sound)` produces an opaque fixed-size Patch. `note(&Sound, NoteRequest)` validates and compiles a voice. `note_frequency(&Sound, NoteRequest, hz)` auditions measured pitch without MIDI rounding. `timeline(&Project)` creates an owned RenderPlan. `export_wav(&Project, &Path)` writes stereo PCM16 with a bounded block buffer and temporary-file replacement.

`NoteRequest` carries `voice_id`, `owner`, `midi`, `velocity`, `gate_seconds`, `gain` and `pan`. Owner zero denotes the draft; clip IDs identify independent timeline instances. Synthesized voices do not need source files. Recorded voices require source verification and decoding during preparation.

```rust
use rsx_overtone::{engine::*, model::Project};
let project = Project::default();
let api = SynthApi::new(EngineConfig::default(), project.tuning.clone())?;
let note = api.note(&project.draft, NoteRequest {
    voice_id: 1, owner: 0, midi: 60, velocity: 0.8,
    gate_seconds: 1.0, gain: 1.0, pan: 0.0,
})?;
let mut engine = SynthEngine::new(api.config())?;
engine.command(Command::Note(note));
let mut stereo = [0.0_f32; 256]; // 128 interleaved stereo frames
engine.render(&mut stereo);
// Send only the changed owner's sound to active voices:
engine.command(Command::Update { owner: 0, patch: api.patch(&project.draft)? });
engine.command(Command::Stop); // 10 ms release; continue rendering the tail
```

For a timeline, construct `SynthEngine::from_plan(api.timeline(&project)?)` and render successive stereo blocks. `frame()`, `active_voices()`, `finished()` and `dropped_notes` expose status. EngineConfig defaults to 48 kHz, 32 voices and master gain 0.25; accepts 8–192 kHz and 1–64 voices.

The optional `AudioSession` uses 32 live voices by default and owns the device stream and queue. `start(Some(&project), tuning)` plays a prepared timeline; `start(None, tuning)` creates an audition session. `trigger`, `audition`, `update_sound` and `stop` send commands. `audition` uses a sound’s captured pitch and duration when available, otherwise C4 for two seconds. `failed`, `finished` and `dropped_notes` read callback status through atomics. The callback processes at most eight commands per buffer from a 128-command queue; a full queue returns an explicit error rather than blocking or overwriting data.

A patch has no pitch or gate until a note is supplied. Synthesized clips without explicit MIDI notes produce no sound; recorded clips play their source bounds without notes. Render length ends at the last recorded sample or synthesized release tail.

Timeline preparation validates project references, applies mute/solo, converts each track's beats with its effective BPM, and includes release tails in polyphony validation. It supports at most 65,536 playable events and 64 simultaneous voices. Programs beyond the configured polyphony fail clearly; extra live audition notes are counted in `dropped_notes`. Output is stereo with mono downmix when required; unused device channels are silent. The device wrapper supports f32, f64, i16, u16 and i32 device formats and reports unsupported formats/configurations.

## Running

The default desktop build includes microphone capture, sample/synth playback and WAV export. The `audio-output` feature is enabled by default:

```sh
cargo run --manifest-path examples/overtone/Cargo.toml --release --features audio-output --bin overtone
```

Linux CPAL builds require ALSA development headers and pkg-config. On Debian/Ubuntu, run `sudo apt-get install libasound2-dev pkg-config`, then verify `pkg-config --modversion alsa` succeeds before building. The runtime library alone does not provide `alsa.pc`; a standard package installation needs no `PKG_CONFIG_PATH` override. See the [setup instructions](README.md) for custom SDK paths. A missing/disconnected device produces a UI error; it does not prevent saving or offline synthesis. Device hardware playback has not been verified in this environment; compilation, offline audio and UI export are tested.

A lightweight consumer can disable default features to exclude GPUI and device libraries:

```sh
cargo run --manifest-path examples/overtone/Cargo.toml --no-default-features --release --bin overtone-render -- session.overtone session.wav
cargo run --manifest-path examples/overtone/Cargo.toml --no-default-features --release --bin overtone-render -- --demo demo.wav
cargo run --manifest-path examples/overtone/Cargo.toml --no-default-features --release --bin overtone-render -- --benchmark
```

The benchmark uses 64 simultaneous voices × 32 nonzero harmonics, 48 kHz and 128-frame blocks. Two quiet release runs in this environment rendered one second of audio in 0.508–0.513 seconds (about 1.96× realtime), with p99 block times of 2.01–2.36 ms against a 2.667 ms deadline. Competing compilation caused deadline overruns in another run; these measurements are not scheduling guarantees. The live wrapper defaults to 32 voices to retain more headroom. Measure on the target hardware in release mode; default debug builds and operating-system scheduling do not establish realtime guarantees. Tests cover pitch/phase/detune, Nyquist suppression, color spectra, seeded/block determinism, sample-timed tempos, release tails, ownership isolation, invalid values, WAV contents and zero allocations/deallocations during rendering/control updates.

`Command::Pause(bool)` freezes scheduled-event time, oscillator phase, envelope progression and noise generators while emitting silence. Resume continues the same engine state. `AudioSession::set_paused(bool)` queues that command; `elapsed_seconds()` reads an atomic sample-frame counter. The UI polls at 20 Hz for its playhead and completion without entering the audio callback. Stop closes the device session and resets the playhead. Timing and tempo changes still require restarting the compiled playback plan.

Partial multiples may be sparse after deletion, but must remain unique, increasing and within 1–32. Oscillator slots use each stored multiple rather than the row index, so deleting one partial cannot retune the others.

The waveform editor uses a separate bounded display API: `waveform::SourcePreview::new(Recording)` produces a cached source overview/detail, and `waveform::sine_display(&Sound, hz, start_seconds, seconds)` produces individual sine contributions and their exact sum. It uses stored sparse multiples, phase, detune and sound gain with the patch’s amplitude/noise normalization. This pedagogical harmonic plot omits ADSR, random noise, brightness filtering and engine master gain; it is not a PCM export or an exact comparison metric. Audition still routes through `SynthApi` and `AudioSession`, so waveform editing does not add work to the realtime callback.

## Recorded sample path

`Sound.recorded_sample` selects direct playback of `capture.start_seconds` / `duration_seconds` from its source. `SynthApi::recorded_sound(project, sound)` verifies the source and returns a PreparedSample for `Command::Sample`. `timeline` prepares both sample and synthesis events, sharing one decoded Arc buffer per source. Preparation applies mute/solo and validates combined polyphony. Source errors reject playback/export instead of producing an approximation or silence.

Sample voices interpolate at `source_rate / output_rate` and retain their natural pitch/duration across BPM changes. Gain is sound × clip × track; constant-power pan, 3 ms edge fades and master gain apply. Samples bypass harmonic/noise/brightness/ADSR processing. Pause freezes their cursor, Stop clears them, and WAV export uses the same renderer. Decode and hashing run before stream creation or command submission. Plans and AudioSession retain decoded buffers so a completed callback voice cannot free the final source buffer.
