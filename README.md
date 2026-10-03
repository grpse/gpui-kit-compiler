# GPUI RSX and Coffee / Lab

`gpui-rsc` compiles Rust component functions with embedded markup into a GPUI Kit desktop app. The coffee app and its recipe calculations live under [`examples/coffee/src/app`](examples/coffee/src/app/app.rsx), and its Rust entry point is [`examples/coffee/src/main.rs`](examples/coffee/src/main.rs). The [Media Playground](examples/media-playground/README.md) demonstrates `<video>` and web images with a next-cat button.

## Build and run

```sh
cargo run -- compile examples/coffee
cargo run --manifest-path examples/coffee/Cargo.toml # after compiling .rsx files
cargo run -- build examples/coffee --features debug-fps
cargo run -- run examples/coffee
cargo run -- dev examples/coffee         # watch sources, rebuild, relaunch
```

An app is a regular Cargo package with `Cargo.toml` and `Cargo.lock` at its root, plus `src/main.rs` and component sources under `src`. The `gpui-rsc` CLI recursively compiles `.rsx` files into matching paths under `target/rsc-build/generated` before Cargo builds the app. `main.rs` includes the generated module glue, selects the root component, and launches it; no separate app build script is needed. The app owns its dependencies, and the CLI leaves its manifest and Rust entry point in place.

The app package depends on `gpui`, `gpui-kit`, and the `gpui-rsc` runtime. The coffee example's dependencies are in [`examples/coffee/Cargo.toml`](examples/coffee/Cargo.toml).

On Linux, GPUI needs Wayland or X11 plus its native graphics and windowing libraries; see the [GPUI Kit installation guide](https://gpui-kit.com/docs/installation/).

## Component functions

An `.rsx` file is Rust source. Each exported `pub fn` whose body contains markup declares a component, and one file can export multiple components. Rust imports, helper functions, constants, and types remain ordinary Rust. The compiler extracts each component’s markup and emits its GPUI renderer and component definition into that module’s `.inter.rs` file. Components need no `<html>`, `<body>`, `<script>`, or `<template>` wrappers.

```rust
pub fn CoffeeCup(water: f32) -> gpui::AnyElement {
    let myStyles = styles({
        cup: { display: "flex", flexDirection: "column", width: gpui::Length::Percent(1.0) },
        coffee: { backgroundColor: rgba(0.42, 0.25, 0.14), height: gpui::Length::Percent(0.6) }
    });
    <div class={myStyles.cup}>
        <div class={myStyles.coffee}></div>
        <div>{water}</div>
    </div>
}
```

Function parameters are the component’s inputs. Immutable parameters are read-only; `&mut` parameters are two-way inputs; `#[out] action: ()` declares an output action. Matching markup attributes pass values into those named parameters, and generated render code exposes each readable value under the same parameter name. Interpolate readable values directly as `{name}` and bind a range input or select with `value={name}`. For local state, declare a `signal(initial_value)` in the component body; the compiler registers it with that component, and writes through `.set(...)` redraw the declaring view and any child views that receive or render the signal. Existing `Definition::with_initial_values(...)` state and binding macros remain available for calculated models and existing apps. Use `<output>` when a value needs a custom formatter or bar rendering. Declare component styles in a Rust `styles({...})` bundle and bind each element with `class={myStyles.name}`.

```rust
use gpui_rsc::runtime::signal;

pub fn GrindSlider() -> gpui::AnyElement {
    let grind = signal(5);
    let myStyles = styles({ fieldValue: { fontSize: 14.0, fontWeight: 600 } });
    <div>
        <div class={myStyles.fieldValue}>{grind}/10</div>
        <input id="grind" type="range" min="1" max="10" step="1" value={grind} />
        <button on-click={grind.set(6)}>Set to six</button>
    </div>
}
```

The compiler keeps the signal alive for the component definition. A control bound with `value={grind}` updates it, and `on-click={grind.set(6)}` runs a Rust action that can update it; both changes redraw the declaring view and any child view that receives or renders `grind`. The same signal can back multiple controls or child components; give each control a unique `id` so updates stay synchronized across them. A signal-only root can omit a calculation callback. Use component parameters when a child should receive a value from its parent without owning that state.

For a calculated app, declare `pub fn calculate(values: &HashMap<String, Value>, reset_epoch: u64) -> Snapshot` in the `.rsx` file. The compiler wires it into the root component automatically, along with optional `pub fn on_change(values: &mut HashMap<String, Value>, changed_key: &str, value: &Value)` and `pub fn output_format(name: &str, value: &Value) -> String` functions. Simple child properties such as `<CoffeeProfile acidity={acidity} />` are inferred as readable calculated values; local signals with those names are passed through directly. This removes the need for a handwritten `definition()` and binding list.

Compose components by importing their exported function names with an ordinary Rust `use`, then writing the uppercase name as a self-closing markup tag:

```rust
use crate::generated::coffee_cup::CoffeeCup;

pub fn Recipe(water: f32) -> gpui::AnyElement {
    let myStyles = styles({ recipe: { display: "flex", flexDirection: "column", gap: 12.0 } });
    <div class={myStyles.recipe}>
        <CoffeeCup water={water} />
    </div>
}
```

The compiler resolves each uppercase tag to its imported function, validates the supplied properties against the function parameters, and adds the child definition to the parent. Component properties must use braced Rust expressions, for example `<CoffeeCup water={recipe.water} />` or `<CoffeeVariablesForm section={"recipe"} />`; the older square-bracket property form is rejected. Import the component under the same name used by its markup tag.

### Native Rust components

#### Direct RSX to Rust

Use `#[gpui]` on a function in an `.rsx` project file to keep its Rust signature and lower markup directly to constructors and builder methods. This accepts GPUI and GPUI Kit APIs without adding tags to the compiler's HTML component list. Import the usual traits and types; Rust checks constructors, methods, argument types, and lifetimes.

```rust
use gpui_kit::{prelude::*, *};
use gpui_kit::component::label::Label;

#[gpui]
pub fn badge(message: &str) -> impl IntoElement {
    <gpui_kit::div flex gap-2>
        <Label args={message.to_owned()} />
    </gpui_kit::div>
}
```

This becomes a regular Rust function:

```rust
pub fn badge(message: &str) -> impl IntoElement {
    gpui_kit::div().flex().gap_2().child(Label::new(message.to_owned()))
}
```

| RSX syntax | Rust call |
| --- | --- |
| `<gpui_kit::div />` or imported `<div />` | `gpui_kit::div()` or `div()` |
| `<Button args={"save"} />` | `Button::new("save")` |
| `<Custom args={(id, window, cx)} />` | `Custom::new(id, window, cx)` |
| `<Custom ctor={Custom::with_state(state)} />` | `Custom::with_state(state)` |
| `flex`, `gap-2`, `p={px(8.0)}` | `.flex()`, `.gap_2()`, `.p(px(8.0))` |
| `disabled={true}` | `.disabled(true)` |
| `{expression}` or nested tags | `.child(expression)` |
| `children={items.map(\|item\| <Label args={item} />)}` | `.children(items.map(\|item\| Label::new(item)))` |

Lowercase tags call functions; names ending in an uppercase type call `::new`. `args` supplies one constructor argument, or expands a tuple into multiple arguments; use `args={(tuple_value,)}` to pass a tuple as one argument. `ctor` overrides the constructor and supports generic types, alternate factories, existing entities, or complete Rust builder expressions. Builder attributes follow source order, before child nodes. A bare attribute calls a method with no arguments, so flags requiring a boolean need `={true}`. Use the actual GPUI builder methods in direct mode, for example `<img args={url} />` rather than the HTML-style `src` attribute.

Rust blocks, closures, iteration, conditional branches, private functions, generic signatures, and `impl` methods are supported. If conditional branches return different element types, use `.into_any_element()` on each branch. Plain text becomes a string child with whitespace collapsed; quoted text or `{...}` preserves exact text. The compiler consumes `#[gpui]`; it is not a Rust macro. Unmarked exported markup functions continue to use the stateful component runtime described above.

To convert a standalone file to ordinary Rust without generating runtime definitions:

```sh
cargo run -- convert examples/media-playground/src/native.rsx /tmp/native.rs
```

The converter also exposes `gpui_rsc::convert_source(&str)` and `gpui_rsc::convert_file(input, output)` with the `compiler` feature. It formats the output; ordinary comments are omitted, while Rust documentation attributes remain. Conversion preserves function signatures rather than the HTML runtime's signal and property semantics. Use `Definition::native` to display a converted function inside a stateful component, as demonstrated by [`native.rsx`](examples/media-playground/src/native.rsx).

#### Register a native renderer

An imported uppercase tag may also point to an ordinary Rust function that returns `runtime::Definition`. Use `Definition::native(name, bindings, view_inputs, renderer)` when the component needs GPUI APIs that RSX does not expose, such as `with_animation`, `with_spring`, custom painting, or an entity. The `name` is the lowercase, hyphenated form of the tag (`ExtractionIllustration` becomes `extraction-illustration`). Bindings describe the accepted properties; `view_inputs` lists readable properties to deliver to the renderer in `ComponentProps`. The renderer receives the same `Window` and `Context<HtmlView>` as generated components and returns `gpui::AnyElement`.

```rust
// src/native_preview.rs, declared with `mod native_preview;` in main.rs
#[allow(non_snake_case)]
pub fn ExtractionIllustration() -> gpui_rsc::runtime::Definition {
    gpui_rsc::runtime::Definition::native(
        "extraction-illustration",
        vec![gpui_rsc::in_param!("water")],
        &["water"],
        render_illustration,
    )
}

// In an .rsx component:
// use crate::native_preview::ExtractionIllustration;
// <ExtractionIllustration water={water} />
```

The [coffee native renderer](examples/coffee/src/native_preview.rs) uses GPUI's animation and spring APIs, following the approach in [GPUI's animation example](https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/animation.rs). Its four original vector drawings follow equipment layouts shown by [Hario's V60 guide](https://www.hario-usa.com/blogs/recipes-and-more-from-friends/james-hoffmann-uitimate-v60-technique), [Bodum's French press guide](https://www.bodum.com/us/en/1928-16us4-chambord), [AeroPress's instructions](https://aeropress.com/pages/how-to-use), and [Breville's portafilter explanation](https://www.breville.com/us/en/blog/coffee-and-espresso/types-of-portafilters.html).

Templates support Rust interpolations such as `{water}` and conditional branches such as `{if method == "French press" { <FrenchPress /> } else { <V60 /> }}`. Select options can be generated by a Rust iterator directly inside markup:

```rust
const METHODS: [&str; 2] = ["V60", "French press"];

pub fn MethodSelect(method: &mut String) -> gpui::AnyElement {
    let methods = ["V60", "French press"];
    let options = methods.iter().map(|&value| => <option value={value}>{value}</option>);
    <select id="method" value={method}>
        {options}
    </select>
}
```

`<div>` is the generic GPUI element. `<button>`, `<input>`, `<textarea>`, `<select>` with `<option>` values, `<progress>`, `<img>`, `<video>`, formatted `<output>`, and imported uppercase component functions lower to GPUI Kit or GPUI elements. A plain value can be displayed inline with `{name}` and ordinary text, such as `{grind}/10`. Unsupported markup elements and style declarations are reported during compilation. Component styles support flex layout, spacing, colors, fonts, borders, dimensions, positioning, alignment, opacity, overflow, and scrolling. Responsive values can use `context.viewport_width` inside the component’s `styles({...})` bundle.

### Basic HTML-style elements

`<img src="path/to/photo.png" alt="Photo" width="240" height="160" object-fit="cover" />` loads a local file. HTTP and HTTPS sources are also accepted. The `src` can read a component parameter or signal with `src={photo}`. Image width and height attributes are pixel values; a `class={...}` style can override them. `alt` is shown if loading fails. Supported `object-fit` values are `contain` (default), `cover`, `fill`, `scale-down`, and `none`.

Inputs use the same two-way binding syntax as the existing range slider. Supported types are `range`, `text`, `email`, `password`, `search`, `url`, `tel`, `date`, and `checkbox`; text-like types use GPUI Kit’s text field. Date values are ISO `YYYY-MM-DD` strings and use GPUI Kit’s calendar picker. A checkbox binds a boolean signal through `checked={enabled}`. `<textarea value={notes}></textarea>` provides multiline text. Each input or textarea needs a unique `id` in its component. `placeholder` works on text, textarea, and date controls; `disabled` works on all except the range slider and select. Text inputs and textarea also accept `readonly`; `aria-label` is passed to text fields, textarea, checkboxes, images, and progress indicators.

`<progress value={fraction} max="1"></progress>` displays a read-only progress indicator; `value` may be a number or a readable binding.

`<video id="demo" src="movie.mp4" controls width="640" height="360" poster="poster.png"></video>` plays local or HTTP(S) media through GStreamer and draws decoded frames in GPUI. `src={clip}` accepts a readable binding; a nested `<source src="movie.mp4" />` is also accepted. `autoplay`, `loop`, and `muted` are supported, along with the same `object-fit` values as `<img>`. `controls` adds a play/pause button; seeking and volume sliders are not implemented. On non-Unix platforms, resuming after pause restarts the video. Playback requires `gst-launch-1.0` and GStreamer playback, video conversion, JPEG encoding, and codec plugins on the user's `PATH`.

Build scoped styles with the Rust `styles({ ... })` expression. Each named style is a struct field, and properties use camelCase names that lower to GPUI style refinements. Colors accept `rgba(r, g, b)` or `rgba(r, g, b, a)` with normalized channels. Style values can use component parameters directly or pass them to Rust helpers:

```rust
fn liquid_level(water: f32) -> f32 {
    (0.24 + ((water - 100.0) / 500.0).clamp(0.0, 1.0) * 0.62).clamp(0.2, 0.88)
}

pub fn Preview(water: f32) -> gpui::AnyElement {
    let myStyles = styles({
        liquid: {
            backgroundColor: rgba(1.0, 0.35, 0.12),
            height: gpui::Length::Percent(liquid_level(water))
        }
    });
    <div class={myStyles.liquid}></div>
}
```

The compiler evaluates the style bundle in generated element render functions, with named locals sourced from the component’s attributes. This pattern is useful for simple parameter-driven graphics; native components can handle more involved drawings and motion.

Style bundle properties cover the compiler’s supported GPUI style set: display and flex layout, gaps, padding, colors, font size and weight, borders and corner radius, width and height constraints, automatic margins, justification and alignment, position and offsets, overflow, and opacity. Use Rust tuples for shorthands such as `padding: (top, right, bottom, left)` and `border: (width, color)`. Style values are Rust expressions, so responsive values and helper calls can be written directly in the component function.

The coffee app shows a recipe, cup prediction, and action tabs alongside method-specific coffee drawings. Changing recipe inputs updates the owning views through events. The scores are relative estimates; coffee origin, roast, water chemistry, and tasting feedback are not modeled. The optional `debug-fps` build feature adds an FPS overlay and requests continuous frames while measuring.

The Cursor extension associates `.rsx` with Rust-aware markup highlighting, completions for GPUI elements, component tags, bindings, and style declarations, and nested markup indentation formatting. Run **Format Document** to align markup tags with the document's indentation settings.

## Application startup

Declare the generated module in `main.rs`, then call the component that should be the app entry point. Startup and window settings are regular Rust values:

```rust
mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/rsc-build/generated.rs"
    ));
}

use gpui_rsc::runtime::{self, StartupConfig};

fn main() {
    runtime::run_with_config(
        generated::app::App(),
        StartupConfig {
            width: 1440.0,
            height: 960.0,
            min_width: Some(900.0),
            min_height: Some(640.0),
            ..StartupConfig::default()
        },
    );
}
```

The generated module name comes from each `.rsx` filename (`app.rsx` becomes `generated::app`), and the component function is the Rust entry point argument. `runtime::run` uses the default 1240×870 window; `run_with_config` accepts the full `StartupConfig`, including window state, decorations, and interaction options.

## Library layout

- [`src/lib.rs`](src/lib.rs): `.rsx` parser, GPUI code generator, and feature-gated runtime exports.
- [`src/template.rs`](src/template.rs): generated template types.
- [`src/main.rs`](src/main.rs): compiler CLI and development watcher.
- [`examples/coffee/Cargo.toml`](examples/coffee/Cargo.toml), [`examples/coffee/Cargo.lock`](examples/coffee/Cargo.lock): the coffee app's independent Cargo package and dependency lockfile.
- [`src/runtime`](src/runtime): GPUI Kit controls, scoped bindings, and worker state exported by the runtime feature.
- [`examples/coffee/src/app/app.rsx`](examples/coffee/src/app/app.rsx): coffee recipe bindings and heuristic sensory calculations.
- [`examples/coffee/src/app/extraction-previews.rsx`](examples/coffee/src/app/extraction-previews.rsx): embeds the native illustration through an uppercase component tag.
- [`examples/coffee/src/native_preview.rs`](examples/coffee/src/native_preview.rs), [`examples/coffee/src/art`](examples/coffee/src/art): native GPUI recipe visualization, animation, and four method drawings.

The coffee scores are relative recipe estimates; coffee origin, roast, water chemistry, and tasting feedback are not modeled.
