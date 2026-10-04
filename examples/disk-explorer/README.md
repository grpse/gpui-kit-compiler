# Disk space explorer

A runnable RSX example that decomposes a folder's file usage. Resizable panes hold a directory list, a tabbed analysis view, and a file inspector. The Breakdown tab renders a clickable treemap whose area represents bytes; selecting a folder drills into its children. Largest files lists the top 200 files in the whole scan, and Scan report explains skipped entries. Switch between logical file length and allocated blocks. On non-Unix platforms, allocated size falls back to logical size.

From the repository root:

```sh
cargo run -- compile examples/disk-explorer
cargo run --manifest-path examples/disk-explorer/Cargo.toml --target-dir target
# Passing a folder starts its scan automatically:
cargo run --manifest-path examples/disk-explorer/Cargo.toml --target-dir target -- /path/to/folder
```

Without an argument, choose a folder and click Analyze. Scans run in the background and can be cancelled. Symbolic links and special files are skipped. Unix scans stay on the starting filesystem and deduplicate hard links. Permission errors produce a partial result and are counted in the report; the report retains the first 30 explanations. The scanner totals regular file bytes, so the total does not include directory metadata or filesystem overhead. This is a folder usage explorer, not a measurement of the volume's full used/free capacity.

The treemap displays up to 64 children and groups the rest into an Other cell; the directory list can browse every child. Zero-byte entries occupy no area. No files are deleted or modified.

[`src/ui.rsx`](src/ui.rsx) demonstrates native callbacks, tabs, virtualized lists, pane resizing, and a treemap assembled from positioned RSX elements. [`src/model.rs`](src/model.rs) contains scanning, cancellation, aggregation, and the balanced slice-and-dice layout.

```sh
cargo test --manifest-path examples/disk-explorer/Cargo.toml --target-dir target --lib
```
