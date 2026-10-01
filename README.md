# GPUI RSC and Coffee / Lab

`gpui-rsc` compiles single-file Rust components into a GPUI Kit desktop app. The coffee example lives in [`examples/coffee`](examples/coffee). Its recipe calculations and output formatting live in the `<script>` section of `app.rsc`; the extraction gallery has a separate component for V60, French press, AeroPress, and espresso.

## Build and run

```sh
cargo run -- compile examples/coffee     # generate the app under target/rsc-build/coffee
cargo run -- build examples/coffee       # generate, Cargo build, and validate bindings
cargo run -- run examples/coffee         # build and launch
cargo run -- dev examples/coffee         # watch sources, rebuild, relaunch
cargo run --features debug-fps -- run examples/coffee
```

The component directory argument is required and may point anywhere on the filesystem. It must contain `app.rsc`, the root component, and may contain child `.rsc` files. The CLI generates the Cargo manifest, `main.rs`, module glue, and `*.inter.rs` files under `target/rsc-build/<directory-name>/`. The generated app has exactly two direct dependencies: `gpui` and `gpui-kit`. The compiler depends on `scraper` and emits GPUI code without linking GPUI itself.

On Linux, GPUI needs Wayland or X11 plus its native graphics and windowing libraries; see the [GPUI Kit installation guide](https://gpui-kit.com/docs/installation/).

## Component format

Each `.rsc` starts with a `<script>` block containing ordinary Rust code, followed by HTML and an optional component-local `<style>` block. The closing `</script>` goes on its own line. Rust in the script is copied into the generated `.inter.rs` file and compiled by `rustc`.

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

A component script declares `definition() -> Definition`. The root `app.rsc` supplies a `calculate` callback, which the generated app runs on a worker thread when values change. It can also supply an `on_change` callback to adjust inputs before recalculation. Other Rust functions, types, methods, and imports can live in any component script. The [coffee calculation and recipe solver](examples/coffee/app.rsc) are examples.

A parent imports a child in its script and instantiates it with `<component name="coffee-profile" acidity="[acidity]" ... />`. `data-in` means UI read-only, `data-out` invokes or writes to Rust, and `data-in-out` is two-way. `in_param!`, `out_param!`, and `in_out_param!` declare a child component's public parameters. Root bindings can point to application keys or Rust getter and setter closures. The compiler's build command validates names, parameters, and binding directions before launch.

The compiler uses `scraper` to read the HTML and match CSS selectors. The generated `.inter.rs` files define component structs and GPUI render functions. Each matched CSS declaration becomes a GPUI builder call in its element's render function, including conditional calls for `@media (max-width: ...px)`. Ordinary child elements and text are composed directly in those generated functions. The app does not parse HTML or CSS. Component-local `<style>` blocks support flex layout, spacing, colors, fonts, borders, dimensions, positioning, alignment, and scrolling. `@keyframes` with `from` and `to` stops can animate `top`, `bottom`, and `opacity` through `animation: name 1200ms infinite`. Inline `style` declarations take precedence over stylesheet and dynamic rules. Responsive GPUI styles receive the live viewport width, so resizing the desktop window previews the narrow layout.

Templates support bound text such as `{data.label}` and conditional branches with simple string equality, such as `{if method == "French press" { ... } else { ... }}`. The compiler translates these into GPUI bindings and render branches. The same conditional can be assigned in `<script>` as a `TemplateElement`, then supplied with `Definition::with_template`. Select options can be assigned in `<script>` or mapped inline in the select, for example `<select id="method">{methods.iter().map(|&method| => <option value={method}>{method}</option>)}</select>`. The compiler translates each option into a value and label and connects the choices by select id. `Definition::with_select_options` is also available for direct Rust configuration. The [extraction gallery](examples/coffee/extraction-previews.rsc) uses a bound method value to render only the selected brewing-method component. Its select event updates the value and triggers a rerender. Each scene is HTML and CSS, and its script computes coffee tint and liquid height from recipe values. A component script can provide an output formatter with `Definition::with_output_formatter`; without one, outputs use `Value::text()`.

For state dependent styles, bind a GPUI `StyleRefinement` to the element with `class={styles.cup_fill}`. The component script can define `fn styles(context: &StyleContext<'_>) -> Styles`, where `Styles` contains `Style` fields. `Style` is GPUI's style refinement type; the compiler lowers the existing `Style::new()` builder expressions to GPUI `div()` style builders while compiling the generated Rust. The generated render function evaluates the bound expression against the current snapshot and refines the element's GPUI style directly. A direct expression also works, such as `class={Style::new().background_color(color_for(context))}`. Static `class="..."` and a bound `class={...}` may appear on the same element. The [method previews](examples/coffee/extraction-previews.rsc) use this pattern to change coffee tint and liquid height from recipe values. There is no runtime CSS selector or style-operation resolver.

The seven flavor sliders show the predicted scores and can also be dragged. Moving one runs the solver in `app.rsc`, which searches recipe quantities and brewing choices for a closer score. The recipe controls and flavor sliders update the four method scenes; the coffee tint follows predicted intensity, bitterness, and body, while the liquid height follows the recipe water amount. Some scores cannot be reached exactly. Calculation changes are sent from the worker as events and update the GPUI view. The optional `debug-fps` build feature adds a small FPS overlay and requests continuous frames while measuring. The desktop window uses a GPUI Kit title bar with standard minimize, maximize, and close controls.

## Library layout

- [`src/lib.rs`](src/lib.rs): `.rsc` parser and GPUI code generator.
- [`src/template.rs`](src/template.rs): generated template types.
- [`src/main.rs`](src/main.rs): compiler CLI and development watcher.
- [`src/runtime`](src/runtime): GPUI Kit controls, scoped bindings, and worker state copied into generated apps.
- [`examples/coffee/app.rsc`](examples/coffee/app.rsc): coffee recipe bindings and heuristic sensory calculations.
- [`examples/coffee/extraction-previews.rsc`](examples/coffee/extraction-previews.rsc): full-width gallery composing the four method previews.
- [`examples/coffee/v60-preview.rsc`](examples/coffee/v60-preview.rsc), [`examples/coffee/french-press-preview.rsc`](examples/coffee/french-press-preview.rsc), [`examples/coffee/aeropress-preview.rsc`](examples/coffee/aeropress-preview.rsc), [`examples/coffee/espresso-preview.rsc`](examples/coffee/espresso-preview.rsc): method-specific coffee containers and dynamic fill styles.

The coffee scores are relative recipe estimates; coffee origin, roast, water chemistry, and tasting feedback are not modeled.
