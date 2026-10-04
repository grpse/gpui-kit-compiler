# GPUI Kit RSX compatibility

[`src/components.rsx`](src/components.rsx) contains a native RSX recipe for every component page linked from the [GPUI Kit component index](https://gpui-kit.com/component/) on October 4, 2026. [`pages.txt`](pages.txt) records all 77 reviewed URLs. Each recipe links to its source page. The examples adapt the documentation's Apache-2.0 code to RSX; no documentation prose is copied.

`cargo test` in the repository checks page coverage and both conversion entry points. This standalone crate also type-checks the generated functions against the real GPUI Kit 0.7.0 API, without opening a window:

```sh
cargo check --manifest-path tests/fixtures/gpui-kit/Cargo.toml --target-dir target
```

The build script uses `compile_file` and includes its output from Cargo's build directory. No generated Rust files need to be checked in. To inspect the conversion directly:

```sh
cargo run -- convert tests/fixtures/gpui-kit/src/components.rsx /tmp/gpui-kit-components.rs
```

Copy a recipe and its imports into your project's `.rsx` module. Keep `#[gpui]` when using the project compiler so the function retains its Rust signature. Supply state entities from your view and implement the normal Rust delegates for lists and data tables. Functions here are API examples: they receive state owned by an application, and do not create a running gallery. The existing runtime can host a native renderer through `Definition::native`, as described in the main README.

Constructor tuples expand through `args={(id, state)}`. Builder methods use `method:args={(a, b)}` when they take multiple arguments. An ordinary attribute still passes one value, so `id={("row", index)}` keeps its tuple. Repeated attributes call the same method again. Static factory tags such as `<c::tag::Tag::primary>` call that factory; `ctor={builder}` extends an existing builder in callbacks.

The fixture follows the installed API where contextual documentation differs. In 0.7.0, `Sidebar::new` requires an ID, separators live in `component::separator`, and `GroupBox` variants require `GroupBoxVariants`. Notifications, dock layouts, and plot scales are Rust descriptors or data rather than rendered elements, so their recipe signatures return those types. The other public recipe functions return `impl IntoElement`.
