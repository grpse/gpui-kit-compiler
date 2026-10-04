# Media Playground

Run this GPUI RSX example from the repository root:

```sh
cargo run -- run examples/media-playground
```

The **Video** tab starts with an empty player. **Choose video…** opens the system file picker for a local video. Use the external **Play / Resume**, **Pause**, and **Stop** buttons below the player; Stop resets playback to the beginning. Switching to the Cat Photos tab closes the video session. Install `gst-launch-1.0` and GStreamer playback, video conversion, JPEG encoding, and codec plugins to use it. The **Cat Photos** tab loads a random cat from [CATAAS](https://cataas.com/); **Show another cat** changes the image URL so GPUI requests a new photo. Cat photos need an internet connection.

On macOS with Homebrew, install the video dependencies with `brew install gstreamer`.

The runtime installs GPUI's desktop HTTP client automatically for web images and video posters.

The footer uses [`src/native.rsx`](src/native.rsx): a `#[gpui]` function with a qualified GPUI Kit tag and an imported `Label`. These convert directly to Rust constructor and builder calls. `Definition::native` makes the function available to the stateful app as `<NativeBadge />`.

Export the direct function to a standalone Rust file:

```sh
cargo run -- convert examples/media-playground/src/native.rsx /tmp/native.rs
```

Cat photos use CATAAS's random `/cat` endpoint, with a new query value on each click to bypass GPUI's image cache.
