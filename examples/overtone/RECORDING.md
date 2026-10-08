# Recording and sound clips

The default build includes microphone capture and device playback. **Record microphone** opens the operating system's default input and displays elapsed time, device name and peak level. **Stop & split** finalizes a WAV, detects separate sounds and places individual clips on the selected track at Insert. **Discard take** deletes the current take. Takes stop at 120 seconds. Saving or replacing/closing the session during capture first finalizes and splits the take.

## Split detection

`analysis::split_file(path, settings)` decodes a recording and calls `split_recording(&Recording, settings)`. It computes RMS in 10 ms windows and compares levels to `silence_db` (default −40 dBFS, range −80 to −10). A continuous quiet gap of `split_gap_ms` (default 150, range 50–1000) separates events. Audible regions shorter than `min_sound_ms` (default 100, range 50–2000) are discarded. Shorter internal pauses remain inside a sound. Leading/trailing silence is trimmed, and the optional API trim bounds refer to the original source.

Each result has `recorded_sample: true`, a source ID assigned at ingestion, and `capture.start_seconds` / `duration_seconds` identifying its samples. It plays the recording directly, without harmonic approximation or pitch conversion. Silence produces no sounds and the source remains available to retry with different settings. Repeating a split adds independent library sounds and clips. All ingestion is one undo step. Generation guards prevent stale background work from entering another project.

A manual timeline cut uses `Project::split_selected_recording(absolute_seconds)`. The cut must be at least 50 ms from both clip edges. It updates the left clip's source bounds and creates a right clip, preserving library entries and the original asset. Each piece inherits level, mute and track. Undo/redo and project persistence preserve these bounds.

## Capture and assets

`capture::CaptureSession::start()` negotiates CPAL's default input. `status()` reports elapsed seconds, peak, failure, dropped frames and automatic completion. `finish()` stops input, joins the worker and returns the finalized WAV path; `discard()` removes it. Capture supports f32/f64 and signed/unsigned 8/16/32-bit formats, downmixing channels to mono. The callback converts samples, pushes a bounded two-second rtrb queue and updates atomics; it performs no decoding, FFT, file I/O, allocation or locking. A worker writes PCM16 WAV. Overruns/device failures report a partial take rather than silently accepting missing data.

Unsaved takes live in the user's Overtone recordings directory. Project saves copy them into `<project-stem>.assets/sources/<sha256>.<extension>`. Source fingerprints are verified before playback and reuse. Changed files require reimporting; missing files produce an error. Undo and cuts do not delete original recordings.

Symphonia decodes supported WAV/AIFF PCM, FLAC, MP3, AAC/ALAC in MP4/M4A, Vorbis in OGG and supported WebM/MKV codecs. Opus is unsupported. Inputs are limited to 8–192 kHz, 1–32 channels and 120 seconds, downmixed to mono. Changing sample rates, invalid samples and oversized recordings fail before ingestion.

## Playback and waveforms

Library Listen and timeline playback use the same source bounds. Source decoding and verification happen off the device thread. Timeline clips from the same source share a decoded buffer. The engine resamples with linear interpolation at the output device rate, applies 3 ms edge fades, gain, constant-power pan and master headroom. Clip pitch and length stay unchanged across tempo edits; BPM affects the placement grid. Play/Pause/Stop, mute/solo and WAV export support recorded and legacy synthesized clips together.

Library cards, timeline clips and the inspector show the real waveform from a transient background cache. Source audio never enters project JSON. Existing synthesized sounds continue using their saved harmonics, noise and envelope. The older `analysis::analyze` / `analyze_file` APIs still fit a whole-take static harmonic patch for advanced consumers; the main recording workflow uses the split APIs.

Tests cover pause boundaries, minimum lengths, trimming, independent cuts, sample identity, resampling, mute, pause/resume, unchanged duration across BPM edits, portable saves, exported WAV audio and allocation-free rendering. Native tests cover split controls and undo. Automated tests do not open the user's microphone or verify physical device capture/playback.
