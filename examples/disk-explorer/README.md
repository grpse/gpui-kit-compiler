# Disk space explorer

For the combined Finder-style file browser with on-demand storage analysis and background fuzzy search, run [File Explorer](../file-browser/). This example remains a standalone storage view and provides the shared scanner, progress reports, and preferences used by File Explorer.

A runnable RSX example that decomposes a folder's file usage. The default **Tree view** uses GPUI Kit's virtualized Tree component, built on GPUI Base, to browse the complete folder hierarchy. Siblings are sorted by descending size; each row shows bytes and a usage bar with its percentage of the whole scan. Expand or collapse a folder with a click, or use the arrow keys and Enter. Selecting a file or folder updates a resizable inspector with its location, file size, size on disk, file count, and share of the scan.

The **Treemap** tab offers a clickable area breakdown, **Largest files** lists the top 200 files, and **Scan report** explains skipped entries. Summary counters and the status bar keep scan totals visible. Scans run in the background and can be cancelled; the Settings page can be opened while a scan continues.

Open **Settings → General → Appearance** to choose **System**, **Light**, or **Dark**. System mode follows live operating-system appearance changes. Settings also switches between logical file length and allocated blocks; tree order, percentages, inspector, treemap, and largest files follow that choice. Expanded folders and tree selection survive metric changes. On non-Unix platforms, allocated size falls back to logical size.

Preferences save automatically in `~/Library/Application Support/rsx-disk-explorer/preferences` on macOS, `%APPDATA%/rsx-disk-explorer/preferences` on Windows, or `$XDG_CONFIG_HOME/rsx-disk-explorer/preferences` (default `~/.config`) on Linux. If saving fails, the change still applies for the session and Settings displays the error.

From the repository root:

```sh
cargo run -- compile examples/disk-explorer
cargo run --manifest-path examples/disk-explorer/Cargo.toml --target-dir target
# Passing a folder starts its scan automatically:
cargo run --manifest-path examples/disk-explorer/Cargo.toml --target-dir target -- /path/to/folder
```

Without an argument, choose a folder and click Analyze. Scans run in the background and can be cancelled. Symbolic links and special files are skipped. Unix scans stay on the starting filesystem and deduplicate hard links. Permission errors produce a partial result and are counted in the report; the report retains the first 30 explanations. The scanner totals regular file bytes, so the total does not include directory metadata or filesystem overhead. This is a folder usage explorer, not a measurement of the volume's full used/free capacity.

The treemap displays up to 64 children and groups the rest into an Other cell; the tree can browse every child. Zero-byte entries appear in the tree but occupy no treemap area. No scanned files are deleted or modified.

[`src/ui.rsx`](src/ui.rsx) demonstrates GPUI Kit Tree, Settings, themed controls, native callbacks, virtualized lists, resizable panes, and a treemap assembled from positioned RSX elements. [`src/model.rs`](src/model.rs) contains scanning, cancellation, aggregation, and the balanced slice-and-dice layout; [`src/preferences.rs`](src/preferences.rs) handles saved preferences.

Framework references: [GPUI Kit documentation](https://gpui-kit.com/docs/), [Tree component](https://gpui-kit.com/component/tree/), [Settings component](https://gpui-kit.com/component/settings/), [Theme](https://gpui-kit.com/component/theme/), and [GPUI Base Tree](https://gpui-kit.com/base/primitives/tree/).

```sh
cargo test --manifest-path examples/disk-explorer/Cargo.toml --target-dir target
```
