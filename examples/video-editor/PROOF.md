# Implementation proof — 2026-10-04

The editor uses real files and native FFmpeg/CPAL playback. Verified on this macOS development machine; measurements are individual runs, not a performance guarantee.

## Real startup project

Normal startup imports four bundled files through the native importer: Murchison Falls VP9/Opus video, bell PCM, stereo cracker PCM, and CC0 piano Vorbis audio. It populates **one video track and seven independent audio channel tracks**, with actual thumbnails and measured waveforms. The running application was visually checked in Edit, including the additional sound-effect/music tracks after scrolling, and played the complete roughly 20-second timeline to its end. The fixture catalog and processing percentages were absent.

The bundled-media integration test checks native 48/44.1 kHz rates, continuous Opus/Vorbis spans, native channel tracks, audio overlay at the playhead, source byte preservation, and duplicate reimport without additional clips. Latest suite: **48 library tests passed**; one legacy CLI export test is opt-in. Build and Clippy with warnings denied passed. A pre-existing dependency (`block` 0.1.6) emits a future-compatibility notice.

## Native editor interaction checks

Verified in the running GPUI app against the real bundled project:

- Right-clicking the waterfall clip opens its actual popup menu. Rename changes the timeline title to `Opening falls` while the library keeps `murchison-falls.webm`. Double-click opens the same inline input; Escape discards a replacement title.
- Double-clicking the video track name commits `Picture` with Enter. Dragging its handle from row 1 to row 3 moves the video clip with the track and keeps its linked audio on their own rows. The track menu's **Move track to top** restores the order.
- **Edit timing…** exposes Start/Source-in/Duration inputs. Start `1`, source-in `0.5`, and duration `99` reject without moving clips. Replacing duration with `4` applies: the video and both waterfall audio clips occupy the same 1–5 second interval and preview seeks to the new source-in.
- The library right-click menu exposes insertion, source preview, favorites, and source reveal; its favorite command updates the Favorites filter.
- In the final build, favoriting the stereo cracker keeps the waterfall selected and the playhead at roughly 2 seconds. **Add to timeline** creates two new channel tracks with both clips starting at that playhead. Locking one linked waterfall audio track disables rename/timing/split/duplicate/delete in the video clip menu; source reveal remains available.
- Right-clicking the waterfall after selecting an audio source restores the decoded video frame at the existing playhead, without rewinding it.
- Timeline editing shortcuts have moved into the context menu. The header shows only zoom and Fit. Verified a single clip menu containing Ripple, Speed, Crop, Audio, Fade, Marker, Undo and Redo alongside the existing clip actions; Speed still produces its existing prototype notice. Empty track space opens selected-clip actions plus **Add track**, which creates a new row. A linked audio lock disables clip editing entries. Scrolling over a clip still reaches the lower channel tracks.

Automated interaction checks additionally compare channel/stream references, source ranges, links and playback gains before/after reordering, verify names survive split/duplicate with independent duplicate links, reject locked/invalid timing edits, and verify context targeting preserves the playhead for split and audio insertion. Drag handles and inline timing now restore trimmed media up to available source bounds. Playback speed/time stretching, undo and persisted projects remain future work.

## Clip drag verification — 2026-10-05

Verified the running native editor with the real bundled waterfall:

- Inspector exposes only Video and Audio; selecting a video/channel switches to its corresponding panel.
- Dragged the video body approximately two seconds later. Both audio channels moved by the same amount and retained their native start offsets.
- Dragged the right edge inward by approximately two seconds. Video and channel ranges shortened together; the start stayed fixed. Pulled the edge beyond the source end and it clamped to the original available ranges.
- Dragged the left edge approximately one second later. The out point stayed fixed, waveforms trimmed, and the preview decoded the new source-in. Pulled it back beyond the source start to restore the original ranges.
- Locked one linked audio track and attempted another body drag. All linked clips stayed in place and the editor reported that the linked tracks must be unlocked.

New model tests verify negative-time/source-bound clamps, restoration of both edges, native lead-in/tail preservation, recovery of hidden named channel spans after reordering, independent split/duplicate recovery ranges, audio discontinuity gaps and selected-span identity, and gesture snapshot rollback without copying decoded media. Escape cancellation is wired to the focused timeline; rollback is covered by the model test. Stretching currently means extending source ranges at original speed.

## Actual audio device delivery

The opt-in proof imports an AV file, creates linked channel tracks, splits at 2 seconds, shifts the linked right group by 1 second, and plays the resulting timeline through the native audio device and native video decoder. It monitors every imported audio stream/channel, including different sample rates. The original remains unchanged.

| Measurement | Multichannel QA source | Bundled waterfall footage |
| --- | ---: | ---: |
| Source video | 10-bit FFV1, 320 × 180 | VP9, 1920 × 1080 |
| Native audio | Stereo 24-bit PCM decoded to I32 at 48 kHz; mono AAC decoded to F32 at 44.1 kHz | Stereo Opus decoded to F32 at 48 kHz |
| Channel tracks | 3 | 2 |
| Output rate | 44100 Hz | 44100 Hz |
| Frames consumed by device callback | 309,317 | 326,341 |
| Nonzero device audio frames | 265,210 | 276,181 |
| Underrun callbacks / clipped frames | 0 / 0 | 0 / 0 |
| Video frames received | 150 | 186 |
| Video/device-clock difference, p95 | 3.26 ms | 3.39 ms |
| Maximum observed difference | 10.63 ms | 42.14 ms |
| Planned/device end | 7.013968 / 7.013991 s | 7.400000 / 7.400023 s |
| Source unchanged | Yes | Yes |

Raw snapshots: [multichannel](assets/proof/multichannel-playback.json), [real footage](assets/proof/bundled-playback.json). Runtime outputs: `target/video-editor-proof/multichannel/` and `target/video-editor-proof/bundled/`; each contains `playback.json` and `monitor-mix.wav`. The stereo 48 kHz WAVs come from the same offline mixer; both were checked to contain exact silence from 2.1 to 2.9 seconds inside the inserted gap. They are validation artifacts, not physical loopback recordings or an editor export feature.

The clock includes the output API's predicted DAC latency. Frame differences compare decoded frame timestamps received by the proof harness with this device clock; they do not measure screen presentation or speaker acoustics. No microphone/loopback recording is performed. Source integrity uses before/after FNV-1a checks; bundled asset provenance separately records SHA-256 in `assets/media/manifest.json`.

## Reproduce

From the repository root, with FFmpeg 9 libraries, pkg-config and Clang available:

```sh
cargo run -- compile examples/video-editor
cargo build --manifest-path examples/video-editor/Cargo.toml --target-dir target
cargo test --manifest-path examples/video-editor/Cargo.toml --target-dir target --lib
cargo clippy --manifest-path examples/video-editor/Cargo.toml --target-dir target --all-targets -- -D warnings
# This proof plays briefly through the default audio device at reduced gain.
target/debug/rsx-video-editor --prove-playback \
  examples/video-editor/assets/media/murchison-falls.webm \
  target/video-editor-proof/bundled
# Launch the actual default project; its bundled files import automatically.
target/debug/rsx-video-editor
```

For a longer mixed-format QA source (CLI used only to generate test media):

```sh
ffmpeg -v error -nostdin -n \
  -f lavfi -i 'testsrc2=size=320x180:rate=25:duration=6' \
  -f lavfi -i 'aevalsrc=0.25*sin(2*PI*440*t)|0.5*sin(2*PI*880*t):s=48000:d=6' \
  -f lavfi -i 'sine=frequency=220:sample_rate=44100:duration=6' \
  -map 0:v -map 1:a -map 2:a -c:v ffv1 -pix_fmt yuv420p10le \
  -c:a:0 pcm_s24le -c:a:1 aac -b:a:1 96k \
  target/video-editor-proof/proof-media.mkv

target/debug/rsx-video-editor --prove-playback \
  target/video-editor-proof/proof-media.mkv \
  target/video-editor-proof/multichannel
```

Use a fresh destination for fixture generation. Encoding versions may produce different source checksums; the proof checks that each run leaves its own source unchanged. The importer, channel preprocessing, preview, mixing, and resampling run in-process. Project save/open, effects/compositing, fades/normalization, undo/redo, and project export remain future work.


## Native compositing graph — 2026-10-05

The Compositing tab was checked in the running native application with bundled media. Its seeded graph connects waterfall video through a saturation node to Video output, and separate FL/FR source nodes through Audio mix to Audio output. The palette lists real video streams and separate audio channels. There are no fixture inputs in normal startup.

- Dragged the Color node across the canvas; connected wires followed and the rendered result stayed current because only layout changed.
- Applied saturation `0`. The old result became stale and insertion/audition disabled. Rerender produced a grayscale frame.
- Added Horizontal flip, wired Source → Flip → Color → Video output, and rerendered. The actual waterfall frame mirrored horizontally.
- Attempted an audio-to-video wire; it was rejected without changing the existing connection. Attempted Color → Flip after Flip → Color; cycle protection rejected it. Escape cleared the pending connection; Arrange placed dependency columns.
- Rendered five seconds of stereo audio, clicked Listen, observed native output start and stop at its end without a device error. This verifies the audition path on this machine; no microphone/loopback recording was used.
- Added the result and inspected Edit: the generated WAV appeared on two linked channel tracks with real waveforms. In the final build, verified the PNG stays in Media without placing an unsupported still-image clip. The integration test also verifies this behavior and audio insertion at the existing playhead.

Nine new tests cover typed connections/cycles, fan-out ordering, settings limits, real pixel flip/overlay/scale, native gain/mix/resampling, real source rendering, stereo placement for undeclared channel layouts, cache preservation, changed-source rejection without partial output, and render-result import. The complete suite has **48 passing tests and one opt-in legacy CLI test**. RSX generation, build and Clippy with warnings denied pass.

A separate reproducible render runs the bundled waterfall through Horizontal flip and zero-saturation Color, then mixes its separate audio channels. The recorded [composition proof](assets/proof/composition.json) includes actual frame dimensions, audio rate/channel/sample counts, peak, render duration, graph edges, and before/after integrity checks for both source media and channel caches. The timing is one local run, not a performance guarantee.

```sh
cargo run -- compile examples/video-editor
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- \
  --prove-composition /path/to/a/new/proof-directory
```

The new directory receives `composite-frame.png`, `composite-audio.wav`, and `composition.json`. Graph rendering uses native libavfilter and does not launch FFmpeg/ffprobe. The video artifact is one 8-bit preview frame (up to 960 × 540), and the audio artifact is a bounded stereo excerpt (48 kHz, 32-bit float). Continuous graph playback, movie encoding/muxing, graph persistence and undo remain pending. Filter semantics use the [official FFmpeg documentation](https://ffmpeg.org/ffmpeg-filters.html).

## Blender compositor expansion — 2026-10-05

The final compositor follows the official Blender 4.5 LTS catalog and Alpha Over example. The checked-in catalog was extracted from the checksum-verified official Blender 4.5.14 macOS runtime. It includes 131 compositor-compatible root types (including legacy variants and shared shader nodes), plus five native audio types. Internal Group Input/Output are evaluated within imported groups; the app does not author groups.

All 131 exported defaults were applied successfully in Blender with the same float/integer distinction used by the inspector. The full library suite passes **63 tests**, with one opt-in legacy FFmpeg CLI test ignored. RSX generation, native build, Clippy with warnings denied, whitespace checks, and both app bundle signature verifications pass. Actual Blender integration checks cover selected outputs, mute, File Output files, procedural Noise, imported exposure groups, duplicate-name socket remapping, and Glare mode changes with integer sockets.

Native checks in the separate FlowCut Review build verified the 136-type library, search/category filtering, zoom/Fit, automatic placement/reveal of tall nodes, typed multiple outputs, enum menus, pinned Apply/Remove and Viewer, one-step undo after a simple header click, a single node context menu, and upstream branch frames with their wires visible. In the final build, changed Glare from Streaks to Fog Glow, applied settings, observed Size replace the Streaks-specific sockets, wired Alpha Over → Glare → Composite, and rendered a visible waterfall result with a current Viewer and five seconds of stereo audio (48 kHz, peak 0.465). Arrange restored forward dependency order. The running FlowCut MVP project window was preserved while its bundle was updated for the next launch.

Rendering uses an isolated Blender CPU worker for Blender video graphs and native libavfilter/CPAL for audio. The starter renders a blurred waterfall background with a scaled translucent inset and a stereo excerpt. Continuous graph playback, encoded graph movie export, graph persistence, and in-app group/scene/mask/tracking authoring remain outside this workflow. Asset-dependent Blender nodes require their corresponding files and data; default-setting validation does not claim every asset-dependent effect was visually exercised.


## Timeline wheel routing — 2026-10-05

Verified in the native FlowCut Scroll QA preview with the real bundled project:

- Wheel input over a video clip scrolls vertically to its audio channel tracks without moving clips horizontally.
- Wheel input over a track mute button continues to the lower sound-effect and piano tracks; controls and clips stay vertically aligned.
- At 120% zoom, unmodified horizontal scrolling over the piano clip leaves its horizontal position unchanged.
- Moving the pointer to the Inspector and scrolling moves the Inspector while the timeline retains its position, without a focus click.
- The Tools & folders header has no expand or ellipsis buttons. Its tab still drags into the Media tab group, remains selectable, and shows only tabs when selected. Reset layout restores the separate card.

Regression tests cover unmodified diagonal/horizontal pixel input, Shift with ordinary vertical deltas and macOS-remapped horizontal deltas, both directions, and line-height conversion for mouse wheels. The automation API cannot hold Shift during a wheel gesture; that combination is verified by the event-routing tests.

Final verification: RSX generation and build pass; 63 library tests and three wheel-routing tests pass, with one opt-in legacy CLI test ignored. Clippy passes for all targets with warnings denied.
