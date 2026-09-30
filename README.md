# GPUI RSC and Coffee / Lab

`gpui-rsc` compiles single-file Rust components into a GPUI Kit desktop app. The coffee example is exactly three source files in [`examples/coffee`](examples/coffee): `app.rsc`, `coffee-variables-form.rsc`, and `coffee-profile.rsc`. Its recipe calculations live in the `<script>` section of `app.rsc`.

## Build and run

```sh
cargo run                         # build and launch the coffee example
cargo run -p gpui-rsc -- compile   # generate the app under target/rsc-build/coffee
cargo run -p gpui-rsc -- build     # generate, Cargo build, and validate bindings
cargo run -p gpui-rsc -- run       # build and launch
cargo run -p gpui-rsc -- dev       # watch .rsc and compiler sources, rebuild, relaunch
cargo run --features debug-fps     # launch with a small FPS overlay
```

The optional second CLI argument is another example directory. It must contain `app.rsc`, which is the root component, and may contain any number of child `.rsc` files. The CLI generates the Cargo manifest, `main.rs`, module glue, and `*.inter.rs` files under `target/rsc-build/<example-name>/`. No handwritten Rust crate is needed in the example directory.

On Linux, GPUI needs Wayland or X11 plus its native graphics and windowing libraries; see the [GPUI Kit installation guide](https://gpui-kit.com/docs/installation/).

## Component format

Each `.rsc` starts with a `<script>` block containing ordinary Rust code, followed by HTML with inline styles. The closing `</script>` goes on its own line. Rust in the script is copied into the generated `.inter.rs` file and compiled by `rustc`.

```html
<script>
use gpui_rsc::{component, in_out_param, out_param};
use gpui_rsc::runtime::Definition;

pub fn definition() -> Definition {
    component! {
        name: "coffee-variables-form",
        imports: [],
        bindings: [in_out_param!("dose"), out_param!("reset")],
    }
}
</script>
<!doctype html>
<html><body>
  <output data-in="dose"></output>
  <input type="range" min="8" max="40" step="1" value="20" data-in-out="dose">
  <button data-out="reset">Reset</button>
</body></html>
```

A component script declares `definition() -> Definition`. The root `app.rsc` supplies a `calculate` callback, which the generic library runs on a worker thread when values change. It can also supply an `on_change` callback to adjust inputs before recalculation. Other Rust functions, types, methods, and imports can live in any component script. The [coffee calculation and recipe solver](examples/coffee/app.rsc) are examples.

A parent imports a child in its script and instantiates it with `<component name="coffee-profile" acidity="[acidity]" ... />`. `data-in` means UI read-only, `data-out` invokes or writes to Rust, and `data-in-out` is two-way. `in_param!`, `out_param!`, and `in_out_param!` declare a child component's public parameters. Root bindings can point to application keys or Rust getter and setter closures. The compiler's build command validates names, parameters, and binding directions before launch.

The generated `.inter.rs` files define component structs with `definition()` and `template()` methods. HTML is compiled into Rust constructors for an element tree, so the running app does not parse HTML. The library's GPUI Kit renderer interprets that tree and supported CSS. Component-local `<style>` blocks support ordinary selectors and `@media (max-width: ...px)` rules over the same CSS subset: flex layout, spacing, colors, fonts, borders, dimensions, alignment, and scrolling. Inline `style` declarations take precedence over matching stylesheet rules. Responsive rules are evaluated against the live GPUI viewport, so resizing the desktop window previews the narrow layout.

Rust can add styles that depend on app state or viewport width with `Definition::with_style_sheet`. A style resolver receives `StyleContext { viewport_width, snapshot }` and returns `StyleRule::new(".card", InlineStyle::new().gap(16.0).max_width(560.0))` values. These rules are recomputed when the app state or viewport changes and can set one or several supported style properties. Dynamic selectors currently target a tag, class, or id.

The seven flavor sliders show the predicted scores, use the matching output color, and can also be dragged. Moving one runs the solver in `app.rsc`, which searches recipe quantities and brewing choices for a closer score. The recipe controls, flavor sliders, and coffee preview then reflect the new prediction. The preview animates the pour and steam, while coffee tint and cup fill respond to recipe values. Some scores cannot be reached exactly. Calculation changes are sent from the worker as events; only changed controls, outputs, and the preview receive GPUI notifications. The optional `debug-fps` build feature adds a small FPS overlay and requests continuous frames while measuring. The desktop window uses a GPUI Kit title bar with standard minimize, maximize, and close controls.

## Library layout

- [`src/lib.rs`](src/lib.rs): `.rsc` compiler and generated template types.
- [`src/main.rs`](src/main.rs): compiler CLI and development watcher.
- [`src/runtime`](src/runtime): GPUI Kit renderer, scoped bindings, and generic worker state.
- [`examples/coffee/app.rsc`](examples/coffee/app.rsc): coffee recipe bindings and heuristic sensory calculations.

The coffee scores are relative recipe estimates; coffee origin, roast, water chemistry, and tasting feedback are not modeled.
