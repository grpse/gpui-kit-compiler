# File Explorer

The combined file browser and disk explorer: a Finder-style file list, Favorites sidebar, navigation toolbar, optional inspector, and on-demand background storage analysis. It uses GPUI Kit's controls, Settings, virtualized lists, resizable panes, and storage Tree, with matching light and dark palettes. **System appearance is the default** and follows live operating-system changes; Settings offers optional Light and Dark overrides. Allocated size is the default metric.

From the repository root:

```sh
cargo run -- compile examples/file-browser
cargo run --manifest-path examples/file-browser/Cargo.toml --target-dir target
# Start in a particular folder:
cargo run --manifest-path examples/file-browser/Cargo.toml --target-dir target -- /path/to/folder
```

Click once to select; double-click a folder to open it, or a file to open its default application. The toolbar path field shows the complete folder path; press Enter to open that folder if it exists. Disclosure arrows load and expand folders in the list without entering them. Click a column heading to sort. Right-click a row for Open, Open in New Tab, Get Info, Rename, Duplicate, New Folder, Copy Path, Show in Finder, Analyze this Folder, Move to Trash, and Delete Immediately. The inspector is hidden initially; its toolbar button or Get Info shows Info and Preview. Text previews stop at 32 KiB, binary files show a short hex view, and images use GPUI's image element. Directory reads, lazy expansion, and previews run in the background.

**Find files** searches the current folder and every nested subfolder. There is no separate scope toggle. Matches are ranked by filename and relative path, with items in the current folder preferred over deeper matches. Results stream into the active tab while the search is still running; the status bar reports checked items, folders, matches, and skipped entries. If that folder (or a parent) was already analyzed, search uses the indexed tree instead of walking the disk again. Search debounces typing, cancels superseded work, and retains the best 500 results. Escape returns to browsing. Dot files and dot folders are hidden by default; search skips hidden subtrees until you show them. Live search never recurses through symbolic links.

**Analyze space** queues a cancellable background scan of the current folder. You can enqueue another folder while one scan is running; the banner shows progress and how many jobs are waiting. Keep browsing, searching, opening tabs, or inspecting files while scans run. Completion fills folder sizes in the ordinary file list and provides **Storage tree**, **Largest files**, and **Scan report** tabs for the analyzed tree. **Clear** cancels the queue and discards results. Analysis includes hidden files in its totals even when their rows are hidden. Unix scans stay on the starting filesystem and count hard links once; links and special files are skipped. Permission errors produce partial results. Allocated size uses disk blocks on Unix and file length elsewhere. Folder totals describe regular files, excluding directory metadata and filesystem overhead.

Finder-compatible shortcuts:

| Shortcut | Action |
| --- | --- |
| ⌘F / Ctrl+F | Find files |
| ⌘⇧. | Show/hide dot files and folders |
| ⌘⇧G | Focus the path field; supports `~` and relative paths, Enter opens the folder |
| ⌘[ / ⌘] | Back / Forward |
| ⌘↑ | Open parent folder |
| ⌘↓ | Open selected file or folder |
| ↑ / ↓ | Select previous / next visible item |
| → / ← | Expand / collapse selected folder |
| Space | Toggle Quick Look preview |
| F2 | Rename the selected file or folder |
| ⌘I | Get Info |
| ⌘⌫ | Move the selection to the Trash |
| ⌘⇧N | New folder |
| ⌘T / Ctrl+T / ⌘W | New tab / close tab (last tab closes the window). Drag tabs to reorder. |
| ⌘, | Settings |
| Escape | Close search, rename, delete confirmation, inspector, or Settings |

Shortcuts apply within this explorer. Rename, duplicate, new folder, Trash, and permanent delete change files on disk. The key positions follow [Apple's Finder shortcuts](https://support.apple.com/en-us/102650); shifted punctuation is normalized for GPUI's macOS event handling.

Preferences save in `~/Library/Application Support/rsx-file-explorer/preferences` on macOS, `%APPDATA%/rsx-file-explorer/preferences` on Windows, or `$XDG_CONFIG_HOME/rsx-file-explorer/preferences` (default `~/.config`) on Linux. `GPUI_EXPLORER_PREFERENCES` can supply a separate file for isolated previews. Hidden-file visibility resets to off at launch. Failed preference writes leave the selected appearance active for the session and show an error in Settings.

[`src/ui.rsx`](src/ui.rsx) owns browser state, generation guards, progressive background jobs, keyboard bindings, and the interface. [`src/model.rs`](src/model.rs) provides directory reading, navigation, and previews; [`src/search.rs`](src/search.rs) provides fuzzy ranking and cancellable traversal. The scan, progress counters, aggregation, and preferences come from the [disk explorer library](../disk-explorer/), which also remains runnable as a standalone storage example.

```sh
cargo test --manifest-path examples/file-browser/Cargo.toml --target-dir target
cargo test --manifest-path examples/disk-explorer/Cargo.toml --target-dir target --lib
```

GPUI's normal desktop graphics and windowing dependencies are required, as with the other examples.
