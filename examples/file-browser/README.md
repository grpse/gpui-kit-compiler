# File browser

A runnable native RSX example with folder tabs, Back/Forward history, an editable address bar, Places shortcuts, dotfile filtering, virtualized file rows, and resizable navigation / file / preview panes. The inspector has Preview and Details tabs. Text previews stop at 32 KiB; binary files show a short hex view, and images use GPUI's image element. Directory reads and previews run on the background executor. Results carry a tab ID and generation so old work cannot replace a newer folder or selection.

From the repository root:

```sh
cargo run -- compile examples/file-browser
cargo run --manifest-path examples/file-browser/Cargo.toml --target-dir target
# Or start in a particular folder:
cargo run --manifest-path examples/file-browser/Cargo.toml --target-dir target -- /path/to/folder
```

Click a folder to enter it, or a file to preview it. Add a tab to keep another folder open. The Details tab can open a file with its default application. The browser does not rename or delete files.

[`src/ui.rsx`](src/ui.rsx) demonstrates `#[gpui]` methods, TabBar callbacks, resizable panels, uniform lists, input subscriptions, and background tasks. [`src/model.rs`](src/model.rs) contains filesystem operations and navigation history. [`../support/mod.rs`](../support/mod.rs) provides shared window startup and byte formatting.

```sh
cargo test --manifest-path examples/file-browser/Cargo.toml --target-dir target --lib
```

GPUI's normal desktop graphics and windowing dependencies are required, as with the other examples.
