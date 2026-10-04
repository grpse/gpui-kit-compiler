# Minimal FFmpeg video editor

A runnable RSX example for a single video clip: import, inspect media, choose trim start/end, rotate in 90-degree increments, reduce width, mute audio, preview a frame at a selected time, and export a new MP4. Resizable preview/settings panes and Preview, Clip info, and Exports tabs demonstrate a desktop editing workflow. The range bar updates with the trim fields. Preview frames apply the current rotation and size settings; Play original and Play exported video use the system's media application.

Install `ffmpeg` and `ffprobe` with the H.264 (`libx264`) and AAC encoders available on your `PATH`. The example reports a readable error if these tools are missing. It uses FFmpeg directly and does not require GStreamer. See the official [FFmpeg manual](https://ffmpeg.org/ffmpeg.html) and [FFprobe manual](https://ffmpeg.org/ffprobe.html) for the underlying APIs.

From the repository root:

```sh
cargo run -- compile examples/video-editor
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target
# Or import a clip at startup:
cargo run --manifest-path examples/video-editor/Cargo.toml --target-dir target -- /path/to/clip.mp4
```

Enter times as seconds, with `0 ≤ start < end ≤ duration`. Use −1 s / +1 s or type a preview time and click Preview at time. Set start/end to the preview time when choosing a cut. The size button cycles original / 1280px / 720px and avoids upscaling. Export MP4 asks for a new `.mp4` destination and re-encodes the selected range as H.264 / AAC. The Exports tab shows progress, cancellation, and error logs. This is a frame preview and single-clip editor; it has no multitrack timeline, transitions, or embedded realtime playback.

All FFmpeg and FFprobe work runs in the background. Arguments are passed directly to `Command`, without a shell. Exports use a temporary file beside the destination, and publish using a hard link that refuses to replace an existing file. The destination filesystem must support hard links. Cancel kills and waits for FFmpeg, then removes the incomplete file. Closing the view also cancels active work. Preview images live in a private temporary workspace, which is removed when its last task releases it.

[`src/ui.rsx`](src/ui.rsx) contains the native view and task coordination; [`src/model.rs`](src/model.rs) contains probing, filter/argument construction, frame rendering, and export lifecycle code.

```sh
cargo test --manifest-path examples/video-editor/Cargo.toml --target-dir target --lib
# Also run the real FFmpeg test (generates its own short test clip):
cargo test --manifest-path examples/video-editor/Cargo.toml --target-dir target --lib -- --include-ignored
```

The FFmpeg test checks trim duration, rotated dimensions, muted output, PNG preview generation, cancellation during encoding, temporary-file cleanup, spaced/metacharacter paths, and preservation of an existing destination.
