# Media Playground

Run this GPUI RSX example from the repository root:

```sh
cargo run -- run examples/media-playground
```

The left card plays a bundled MP4 through the `<video>` component. Install `gst-launch-1.0` and GStreamer playback, video conversion, JPEG encoding, and codec plugins to use it. The right card loads a random cat from [CATAAS](https://cataas.com/); **Show another cat** changes the image URL so GPUI requests a new photo. Cat photos need an internet connection.

The runtime installs GPUI's desktop HTTP client automatically for web images and video posters.

The footer uses [`src/native.rsx`](src/native.rsx): a `#[gpui]` function with a qualified GPUI Kit tag and an imported `Label`. These convert directly to Rust constructor and builder calls. `Definition::native` makes the function available to the stateful app as `<NativeBadge />`.

Export the direct function to a standalone Rust file:

```sh
cargo run -- convert examples/media-playground/src/native.rsx /tmp/native.rs
```

The bundled `assets/demo.mp4` is a generated moving test pattern, so video playback works without downloading a clip. Cat photos use CATAAS's random `/cat` endpoint, with a new query value on each click to bypass GPUI's image cache.
