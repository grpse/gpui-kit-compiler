use crate::support;
use gpui_kit::{prelude::*, *};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, StyledExt as _,
    Icon, Theme, ThemeMode, button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState}, list::ListItem,
    menu::{ContextMenuExt as _, PopupMenuItem},
    resizable::{h_resizable, resizable_panel}, tab::{Tab, TabBar},
    tree::{tree, TreeItem, TreeState}, spinner::Spinner,
    setting::{Settings, SettingPage, SettingGroup, SettingItem, SettingField},
};
use gpui_kit::assets::IconName;
use rsx_file_browser::{
    Entry, History, Preview, Appearance, Preferences, Node, Scan, ScanProgress, SearchUpdate,
    create_folder, delete_permanently, duplicate_entry, move_to_trash, node_at, preview,
    read_directory, rename_entry, search, search_tree, scan_with_progress, largest_files,
};
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};

actions!(file_explorer, [Find, ToggleHidden, GoToFolder, Back, Forward, Parent, NewTab, CloseTab, QuickLook, OpenSelection, NextItem, PreviousItem, ExpandItem, CollapseItem, Dismiss, ShowSettings, RenameItem, GetInfo, MoveToTrash, NewFolder]);

struct FolderTab {
    id: usize, generation: u64, history: History,
    entries: Vec<Entry>, expanded: HashSet<PathBuf>, children: HashMap<PathBuf, Vec<Entry>>,
    loading_children: HashSet<PathBuf>, loading: bool, unreadable: usize,
    selected: Option<Entry>, preview: Option<Result<Preview, String>>, preview_generation: u64,
    status: String,
    query: String, searching: bool, search_update: SearchUpdate, search_error: Option<String>,
    search_generation: u64, search_cancel: Arc<AtomicBool>,
}
#[derive(Clone)]
struct FileRow { entry: Entry, depth: usize }
struct Analysis {
    id: usize, path: PathBuf, cancel: Arc<AtomicBool>, scanning: bool, queued: bool,
    progress: ScanProgress, scan: Option<Arc<Scan>>, sizes: Arc<HashMap<PathBuf, Node>>,
    largest_logical: Vec<Node>, largest_allocated: Vec<Node>, error: Option<String>,
}

pub struct Browser {
    tabs: Vec<FolderTab>, active: usize, next_id: usize,
    address: Entity<InputState>, search_input: Entity<InputState>, rename_input: Entity<InputState>,
    focus: FocusHandle, list_focus: FocusHandle, list_scroll: UniformListScrollHandle,
    search_open: bool, hidden: bool,
    analyses: Vec<Analysis>, next_analysis: usize, analysis_tab: usize,
    tree: Entity<TreeState>, tree_nodes: Arc<HashMap<String, Node>>,
    settings_open: bool, inspector_open: bool, detail_tab: usize,
    rename: Option<PathBuf>, confirm_delete: Option<Entry>, dragging_tab: Option<usize>,
    appearance: Appearance, allocated: bool, preference_status: String,
    sort: usize, descending: bool, _subscriptions: Vec<Subscription>,
}
impl Drop for Browser {
    fn drop(&mut self) {
        for tab in &self.tabs { tab.search_cancel.store(true, Ordering::Relaxed); }
        for analysis in &self.analyses { analysis.cancel.store(true, Ordering::Relaxed); }
    }
}
impl Browser {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::bind_shortcuts(cx);
        let address = cx.new(|cx| InputState::new(window, cx).placeholder("Full folder path — Enter to open"));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Find in this folder and subfolders…"));
        let rename_input = cx.new(|cx| InputState::new(window, cx).placeholder("New name"));
        let tree = cx.new(|cx| TreeState::new(cx));
        let preferences = preferences_path().filter(|path|path.exists()).map(|path| Preferences::load(&path)).unwrap_or(Preferences {appearance: Appearance::System, allocated: true});
        apply_appearance(preferences.appearance, window, cx);
        let subscriptions = vec![
            cx.subscribe(&search_input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.tabs[this.active].query = input.read(cx).value().to_string();
                    this.start_search(cx);
                }
            }),
            cx.subscribe_in(&address, window, |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let value = input.read(cx).value().to_string();
                    this.open_path(&value, window, cx);
                    this.list_focus.focus(window,cx);
                }
            }),
            cx.subscribe_in(&rename_input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) { this.commit_rename(window, cx); }
            }),
            cx.observe(&tree, |this, tree, cx| {
                let node = tree.read(cx).selected_item().and_then(|item| this.tree_nodes.get(item.id.as_ref())).cloned();
                if let Some(node) = node {
                    let entry = entry_from_node(&node);
                    if this.tabs[this.active].selected.as_ref().map(|e| &e.path) != Some(&entry.path) {
                        this.tabs[this.active].selected = Some(entry);
                        this.tabs[this.active].preview = None;
                    }
                }
                cx.notify();
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance == Appearance::System { apply_appearance(this.appearance, window, cx); }
            }),
        ];
        Self::observe_theme(cx);
        let mut this = Self {
            tabs: vec![], active: 0, next_id: 0, address, search_input, rename_input,
            focus: cx.focus_handle(), list_focus: cx.focus_handle(), list_scroll: UniformListScrollHandle::default(),
            search_open: false, hidden: false,
            analyses: vec![], next_analysis: 0, analysis_tab: 0,
            tree, tree_nodes: Arc::new(HashMap::new()),
            settings_open: false, inspector_open: false, detail_tab: 1,
            rename: None, confirm_delete: None, dragging_tab: None,
            appearance: preferences.appearance, allocated: preferences.allocated, preference_status: "Changes save automatically.".into(),
            sort: 0, descending: false, _subscriptions: subscriptions,
        };
        let mut initial = support::initial_directory();
        if initial == Path::new("/") && std::env::args_os().nth(1).is_none() {
            if let Some(home) = std::env::var_os("HOME") {initial = home.into();}
        }
        this.add_tab(initial, window, cx);
        this.list_focus.focus(window,cx);
        this
    }
    fn observe_theme(cx: &mut Context<Self>) { cx.observe_global::<Theme>(|_, cx| cx.notify()).detach(); }
    fn bind_shortcuts(cx: &mut Context<Self>) {
        cx.bind_keys([
            KeyBinding::new("cmd-f", Find, Some("FileExplorer")),
            KeyBinding::new("ctrl-f", Find, Some("FileExplorer")),
            KeyBinding::new("cmd-shift-.", ToggleHidden, Some("FileExplorer")),
            KeyBinding::new("cmd->", ToggleHidden, Some("FileExplorer")),
            KeyBinding::new("cmd-shift-g", GoToFolder, Some("FileExplorer")),
            KeyBinding::new("cmd-[", Back, Some("FileExplorer")), KeyBinding::new("cmd-]", Forward, Some("FileExplorer")),
            KeyBinding::new("cmd-up", Parent, Some("FileExplorer")),
            KeyBinding::new("cmd-t", NewTab, Some("FileExplorer")), KeyBinding::new("ctrl-t", NewTab, Some("FileExplorer")),
            KeyBinding::new("cmd-w", CloseTab, Some("FileExplorer")),
            KeyBinding::new("cmd-,", ShowSettings, Some("FileExplorer")),
            KeyBinding::new("escape", Dismiss, Some("FileExplorer")),
            KeyBinding::new("space", QuickLook, Some("ExplorerList")),
            KeyBinding::new("cmd-down", OpenSelection, Some("ExplorerList")),
            KeyBinding::new("down", NextItem, Some("ExplorerList")), KeyBinding::new("up", PreviousItem, Some("ExplorerList")),
            KeyBinding::new("right", ExpandItem, Some("ExplorerList")), KeyBinding::new("left", CollapseItem, Some("ExplorerList")),
            KeyBinding::new("f2", RenameItem, Some("ExplorerList")),
            KeyBinding::new("cmd-i", GetInfo, Some("ExplorerList")),
            KeyBinding::new("cmd-backspace", MoveToTrash, Some("ExplorerList")),
            KeyBinding::new("cmd-shift-n", NewFolder, Some("ExplorerList")),
        ]);
    }
    fn empty_tab(id: usize, path: PathBuf) -> FolderTab {
        FolderTab {
            id, generation: 0, history: History::new(path), entries: vec![], expanded: HashSet::new(), children: HashMap::new(),
            loading_children: HashSet::new(), loading: false, unreadable: 0, selected: None, preview: None, preview_generation: 0, status: String::new(),
            query: String::new(), searching: false, search_update: SearchUpdate::default(), search_error: None,
            search_generation: 0, search_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    fn add_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id; self.next_id += 1;
        self.tabs.push(Self::empty_tab(id, path));
        self.active = self.tabs.len() - 1;
        self.refresh(window, cx);
    }
    fn close_tab_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() == 1 { window.remove_window(); return; }
        self.tabs[index].search_cancel.store(true, Ordering::Relaxed);
        self.tabs.remove(index);
        self.active = if index < self.active { self.active - 1 } else { self.active.min(self.tabs.len() - 1) };
        self.sync_query(window, cx); self.sync_address(window, cx); self.rebuild_storage_tree(cx); cx.notify();
    }
    fn close_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) { self.close_tab_at(self.active, window, cx); }
    fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() { return; }
        self.active = index;
        self.sync_query(window, cx); self.sync_address(window, cx); self.rebuild_storage_tree(cx); cx.notify();
    }
    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() { return; }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        if self.active == from { self.active = to; }
        else if from < self.active && to >= self.active { self.active -= 1; }
        else if from > self.active && to <= self.active { self.active += 1; }
        self.dragging_tab = Some(to);
    }
    fn sync_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.tabs[self.active].history.current().to_string_lossy().into_owned();
        self.address.update(cx, |input, cx| input.set_value(path, window, cx));
    }
    fn sync_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.tabs[self.active].query.clone();
        self.search_input.update(cx, |input, cx| input.set_value(query, window, cx));
    }
    fn open_path(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let typed = expand_home(value.trim());
        let candidate = if typed.is_absolute() { typed } else { self.tabs[self.active].history.current().join(typed) };
        match std::fs::canonicalize(&candidate) {
            Ok(path) if path.is_dir() => self.navigate(path, window, cx),
            Ok(_) => self.tabs[self.active].status = "That path is a file. Enter a folder path.".into(),
            Err(error) => self.tabs[self.active].status = format!("Could not open folder: {error}"),
        }
        cx.notify();
    }
    fn navigate(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let path = if path.is_absolute() {path} else {self.tabs[self.active].history.current().join(path)};
        self.tabs[self.active].history.navigate(path);
        self.analysis_tab = 0; self.settings_open = false;
        self.refresh(window, cx);
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_address(window, cx);
        let tab = &mut self.tabs[self.active];
        tab.generation += 1; tab.loading = true; tab.selected = None; tab.preview = None;
        tab.entries.clear(); tab.children.clear(); tab.expanded.clear(); tab.loading_children.clear();
        let generation = tab.generation; let id = tab.id; let path = tab.history.current().to_owned();
        self.start_search(cx);
        self.rebuild_storage_tree(cx);
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { read_directory(&path) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.id == id && tab.generation == generation) {
                    tab.loading = false;
                    match result { Ok((entries, unreadable)) => {tab.entries = entries; tab.unreadable = unreadable; tab.status.clear();}, Err(error) => tab.status = error }
                    cx.notify();
                }
            });
        }).detach();
        cx.notify();
    }
    fn indexed_node(&self, path: &Path) -> Option<Node> {
        let mut best: Option<&Node> = None;
        for analysis in &self.analyses {
            if let Some(scan) = &analysis.scan {
                if path.starts_with(&scan.root.path) {
                    if let Some(node) = node_at(&scan.root, path) {
                        if best.as_ref().is_none_or(|current| node.path.components().count() >= current.path.components().count()) {
                            best = Some(node);
                        }
                    }
                }
            }
        }
        best.cloned()
    }
    fn start_search(&mut self, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active];
        tab.search_cancel.store(true, Ordering::Relaxed);
        tab.search_cancel = Arc::new(AtomicBool::new(false)); tab.search_generation += 1;
        tab.search_update = SearchUpdate::default(); tab.search_error = None;
        tab.searching = !tab.query.trim().is_empty();
        if !tab.searching { cx.notify(); return; }
        self.analysis_tab = 0;
        let generation = tab.search_generation; let cancel = tab.search_cancel.clone();
        let query = tab.query.trim().to_owned(); let path = tab.history.current().to_owned();
        let id = tab.id; let hidden = self.hidden;
        let indexed = self.indexed_node(&path);
        let (sender, receiver) = async_channel::bounded(8);
        cx.spawn(async move |this, cx| {
            while let Ok(update) = receiver.recv().await {
                if this.update(cx, |this, cx| {
                    if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.id == id && tab.search_generation == generation && tab.searching) {
                        tab.search_update = update; cx.notify();
                    }
                }).is_err() {break;}
            }
        }).detach();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(180)).await;
            if cancel.load(Ordering::Relaxed) { return; }
            let result = cx.background_spawn(async move {
                if let Some(node) = indexed {
                    search_tree(&node, &query, hidden, cancel, |update| { let _ = sender.try_send(update); })
                } else {
                    search(&path, &query, true, hidden, cancel, |update| { let _ = sender.try_send(update); })
                }
            }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.id == id && tab.search_generation == generation) {
                    tab.searching = false;
                    match result {Ok(update) => tab.search_update = update, Err(error) => tab.search_error = Some(error)}
                    cx.notify();
                }
            });
        }).detach();
        cx.notify();
    }
    fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.hidden = !self.hidden;
        if !self.hidden && self.tabs[self.active].selected.as_ref().is_some_and(|entry| has_hidden_component(&entry.path, self.tabs[self.active].history.current())) {
            self.tabs[self.active].selected = None; self.tabs[self.active].preview = None;
        }
        self.rebuild_storage_tree(cx); self.start_search(cx);
    }
    fn toggle_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active];
        if tab.expanded.remove(&path) {cx.notify(); return;}
        tab.expanded.insert(path.clone());
        if tab.children.contains_key(&path) || !tab.loading_children.insert(path.clone()) { cx.notify(); return; }
        let id = tab.id; let generation = tab.generation;
        cx.spawn(async move |this, cx| {
            let load_path = path.clone();
            let result = cx.background_spawn(async move {read_directory(&load_path)}).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.id == id && tab.generation == generation) {
                    tab.loading_children.remove(&path);
                    match result {Ok((entries, unreadable)) => {tab.children.insert(path, entries); tab.unreadable += unreadable;}, Err(error) => {tab.expanded.remove(&path); tab.status = error;}}
                    cx.notify();
                }
            });
        }).detach(); cx.notify();
    }
    fn select(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.list_focus.focus(window,cx);
        let tab = &mut self.tabs[self.active];
        tab.preview_generation += 1;
        let preview_generation = tab.preview_generation; let generation = tab.generation; let id = tab.id;
        let path = entry.path.clone(); tab.selected = Some(entry.clone()); tab.preview = None;
        if entry.is_dir || !self.inspector_open || self.detail_tab != 0 {cx.notify(); return;}
        cx.spawn(async move |this, cx| {
            let selected_path = path.clone();
            let result = cx.background_spawn(async move {preview(&path)}).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| tab.id == id && tab.generation == generation && tab.preview_generation == preview_generation && tab.selected.as_ref().is_some_and(|e| e.path == selected_path)) {
                    tab.preview = Some(result); cx.notify();
                }
            });
        }).detach(); cx.notify();
    }
    fn open_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.tabs[self.active].selected.clone() {
            if entry.is_dir {self.navigate(entry.path, window, cx);} else {cx.open_with_system(&entry.path);}
        }
    }
    fn rows(&self) -> Vec<FileRow> {
        if !self.tabs[self.active].query.trim().is_empty() { return self.tabs[self.active].search_update.results.iter().cloned().map(|entry| FileRow {entry, depth: 0}).collect(); }
        fn visit(browser: &Browser, entries: &[Entry], depth: usize, output: &mut Vec<FileRow>) {
            let tab = &browser.tabs[browser.active];
            let mut entries = entries.iter().filter(|entry| browser.hidden || !entry.name.starts_with('.')).collect::<Vec<_>>();
            entries.sort_by(|a, b| {
                let order = match browser.sort {
                    1 => a.modified.cmp(&b.modified),
                    2 => browser.entry_size(a).cmp(&browser.entry_size(b)),
                    3 => a.kind().cmp(&b.kind()),
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }.then_with(|| a.path.cmp(&b.path));
                if browser.descending {order.reverse()} else {order}
            });
            for entry in entries {
                output.push(FileRow {entry: entry.clone(), depth});
                if tab.expanded.contains(&entry.path) {
                    if let Some(children) = tab.children.get(&entry.path) {visit(browser, children, depth + 1, output);}
                }
            }
        }
        let mut output = vec![]; visit(self, &self.tabs[self.active].entries, 0, &mut output); output
    }
    fn entry_size(&self, entry: &Entry) -> Option<u64> {
        for analysis in self.analyses.iter().rev() {
            if let Some(node) = analysis.sizes.get(&entry.path) { return Some(node.weight(self.allocated)); }
        }
        if entry.is_dir {None} else {Some(if self.allocated {entry.allocated} else {entry.bytes})}
    }
    fn move_selection(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(); if rows.is_empty() {return;}
        let selected = self.tabs[self.active].selected.as_ref().and_then(|entry| rows.iter().position(|row| row.entry.path == entry.path));
        let index = match selected {Some(index) if forward => (index + 1).min(rows.len()-1), Some(index) => index.saturating_sub(1), None => 0};
        self.list_scroll.scroll_to_item(index, if forward {ScrollStrategy::Bottom}else{ScrollStrategy::Top});
        self.select(rows[index].entry.clone(), window, cx);
    }
    fn enqueue_scan(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(existing) = self.analyses.iter_mut().find(|analysis| analysis.path == path) {
            if existing.scanning || existing.queued { cx.notify(); return; }
            existing.queued = true; existing.scan = None; existing.error = None; existing.progress = ScanProgress::default();
            existing.sizes = Arc::new(HashMap::new()); existing.largest_logical.clear(); existing.largest_allocated.clear();
        } else {
            self.analyses.push(Analysis {
                id: self.next_analysis, path, cancel: Arc::new(AtomicBool::new(false)), scanning: false, queued: true,
                progress: ScanProgress::default(), scan: None, sizes: Arc::new(HashMap::new()),
                largest_logical: vec![], largest_allocated: vec![], error: None,
            });
            self.next_analysis += 1;
        }
        self.pump_queue(cx); cx.notify();
    }
    fn pump_queue(&mut self, cx: &mut Context<Self>) {
        if self.analyses.iter().any(|analysis| analysis.scanning) { return; }
        let Some(index) = self.analyses.iter().position(|analysis| analysis.queued && !analysis.scanning) else { return; };
        self.start_analysis(index, cx);
    }
    fn start_analysis(&mut self, index: usize, cx: &mut Context<Self>) {
        let analysis = &mut self.analyses[index];
        analysis.cancel = Arc::new(AtomicBool::new(false));
        analysis.scanning = true; analysis.queued = false; analysis.error = None;
        let generation = analysis.id; let cancel = analysis.cancel.clone(); let path = analysis.path.clone();
        let (sender, receiver) = async_channel::bounded(1);
        cx.spawn(async move |this, cx| {
            while let Ok(progress) = receiver.recv().await {
                if this.update(cx, |this, cx| {
                    if let Some(analysis) = this.analyses.iter_mut().find(|analysis| analysis.id == generation && analysis.scanning) {
                        analysis.progress = progress; cx.notify();
                    }
                }).is_err() {break;}
            }
        }).detach();
        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                scan_with_progress(&path, cancel, |progress| {let _ = sender.try_send(progress);}).map(|scan| {
                    fn index(node: &Node, sizes: &mut HashMap<PathBuf, Node>) {sizes.insert(node.path.clone(), node.summary()); for child in &node.children {index(child, sizes);}}
                    let mut sizes = HashMap::new(); index(&scan.root, &mut sizes);
                    let logical = largest_files(&scan.root, false);
                    let allocated = largest_files(&scan.root, true);
                    (Arc::new(scan), Arc::new(sizes), logical, allocated)
                })
            }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(analysis) = this.analyses.iter_mut().find(|analysis| analysis.id == generation) {
                    analysis.scanning = false;
                    match result {Ok((scan, sizes, logical, allocated)) => {
                        analysis.progress.files = scan.root.files; analysis.progress.logical = scan.root.logical; analysis.progress.allocated = scan.root.allocated; analysis.progress.skipped = scan.skipped;
                        analysis.scan = Some(scan); analysis.sizes = sizes; analysis.largest_logical = logical; analysis.largest_allocated = allocated;
                    }, Err(error) => analysis.error = Some(error)}
                }
                this.rebuild_storage_tree(cx); this.pump_queue(cx); cx.notify();
            });
        }).detach();
    }
    fn current_analysis(&self) -> Option<&Analysis> {
        let path = self.tabs[self.active].history.current();
        self.analyses.iter().rev().find(|analysis| path.starts_with(&analysis.path) && (analysis.scan.is_some() || analysis.scanning || analysis.queued))
    }
    fn analysis_banner(&self) -> String {
        let waiting = self.analyses.iter().filter(|analysis| analysis.queued).count();
        if let Some(scan) = self.analyses.iter().find(|analysis| analysis.scanning) {
            let root_name = scan.path.file_name().unwrap_or(scan.path.as_os_str()).to_string_lossy();
            let size = if self.allocated { scan.progress.allocated } else { scan.progress.logical };
            let extra = if waiting > 0 { format!(" · {waiting} waiting") } else { String::new() };
            return format!("Analyzing {root_name}… {} files · {} so far{extra}", scan.progress.files, support::bytes(size));
        }
        if let Some(error) = self.analyses.iter().rev().find_map(|analysis| analysis.error.clone()) {
            return error;
        }
        if let Some(scan) = self.current_analysis() {
            let root_name = scan.path.file_name().unwrap_or(scan.path.as_os_str()).to_string_lossy();
            let size = if self.allocated { scan.progress.allocated } else { scan.progress.logical };
            return format!("{root_name} · {} {} · {} files · {} skipped{}", support::bytes(size), if self.allocated {"on disk"} else {"file size"}, scan.progress.files, scan.progress.skipped, if waiting > 0 { format!(" · {waiting} queued") } else { String::new() });
        }
        format!("{waiting} analyses queued")
    }
    fn report_snapshot(&self) -> (ScanProgress, Vec<String>) {
        match self.current_analysis() {
            Some(analysis) => (analysis.progress.clone(), analysis.scan.as_ref().map(|scan| scan.warnings.clone()).unwrap_or_default()),
            None => (ScanProgress::default(), vec![]),
        }
    }
    fn start_scan(&mut self, cx: &mut Context<Self>) {
        let path = self.tabs[self.active].history.current().to_owned();
        self.enqueue_scan(path, cx);
    }
    fn clear_scan(&mut self, cx: &mut Context<Self>) {
        for analysis in &self.analyses { analysis.cancel.store(true, Ordering::Relaxed); }
        self.analyses.clear(); self.analysis_tab = 0;
        self.tree_nodes = Arc::new(HashMap::new()); self.tree.update(cx, |tree, cx| tree.set_items(vec![], cx)); cx.notify();
    }
    fn rebuild_storage_tree(&mut self, cx: &mut Context<Self>) {
        if let Some(scan) = self.current_analysis().and_then(|analysis| analysis.scan.clone()) {
            let mut expanded = HashSet::new();
            let selected_id = self.tree.read(cx).selected_item().map(|item| item.id.clone());
            if let Some(root) = self.tree.read(cx).entry(0) {collect_expanded(root.item(), &mut expanded);}
            let mut nodes = HashMap::new();
            let item = storage_tree(&scan.root, "root".into(), self.hidden, self.allocated, &expanded, &mut nodes);
            self.tree_nodes = Arc::new(nodes);
            self.tree.update(cx, |tree, cx| {
                tree.set_items(vec![item], cx);
                if let Some(id) = selected_id {let index = tree.index_of(&id); tree.set_selected_index(index, cx);}
            });
        } else {
            self.tree_nodes = Arc::new(HashMap::new());
            self.tree.update(cx, |tree, cx| tree.set_items(vec![], cx));
        }
    }
    fn apply_result(&mut self, result: Result<PathBuf, String>, window: &mut Window, cx: &mut Context<Self>) {
        match result {
            Ok(path) => {
                self.tabs[self.active].status.clear();
                if let Some(parent) = path.parent() {
                    if parent == self.tabs[self.active].history.current() { self.refresh(window, cx); }
                    else { self.navigate(parent.to_owned(), window, cx); }
                } else { self.refresh(window, cx); }
            }
            Err(error) => { self.tabs[self.active].status = error; cx.notify(); }
        }
    }
    fn begin_rename(&mut self, entry: Option<Entry>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = entry.or_else(|| self.tabs[self.active].selected.clone()) else { return; };
        self.rename = Some(entry.path.clone());
        self.rename_input.update(cx, |input, cx| { input.set_value(entry.name.clone(), window, cx); });
        self.rename_input.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }
    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.rename.take() else { return; };
        let name = self.rename_input.read(cx).value().to_string();
        self.apply_result(rename_entry(&path, &name), window, cx);
    }
    fn trash_entry(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        let result = move_to_trash(&entry.path).map(|_| entry.path.clone());
        self.apply_result(result, window, cx);
    }
    fn delete_entry(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        let result = delete_permanently(&entry.path).map(|_| entry.path.clone());
        self.apply_result(result, window, cx);
    }
    fn duplicate_selected(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_result(duplicate_entry(&entry.path), window, cx);
    }
    fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = self.tabs[self.active].history.current().to_owned();
        match create_folder(&parent, "untitled folder") {
            Ok(path) => {
                self.refresh(window, cx);
                self.begin_rename(Some(Entry { path: path.clone(), name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(), is_dir: true, is_symlink: false, bytes: 0, allocated: 0, modified: None }), window, cx);
            }
            Err(error) => { self.tabs[self.active].status = error; cx.notify(); }
        }
    }
    fn show_info(&mut self, entry: Option<Entry>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = entry { self.select(entry, window, cx); }
        self.inspector_open = true; self.detail_tab = 1; cx.notify();
    }
    fn copy_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(path.to_string_lossy().into_owned()));
        self.tabs[self.active].status = "Path copied.".into(); cx.notify();
    }
    fn save_preferences(&mut self) {
        self.preference_status = match preferences_path() {
            Some(path) => match (Preferences {appearance: self.appearance, allocated: self.allocated}).save(&path) {Ok(()) => "Changes saved.".into(), Err(error) => format!("Applied this session. Could not save: {error}")},
            None => "Applied this session. No settings folder is available.".into(),
        };
    }
    fn set_metric(&mut self, value: bool, cx: &mut Context<Self>) {
        self.allocated = value;
        self.rebuild_storage_tree(cx); self.save_preferences(); cx.notify();
    }
    #[gpui]
    fn file_row(&mut self, row: FileRow, index: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let entry = row.entry;
        let selected = self.tabs[self.active].selected.as_ref().is_some_and(|selected| selected.path == entry.path);
        let expanded = self.tabs[self.active].expanded.contains(&entry.path);
        let loading = self.tabs[self.active].loading_children.contains(&entry.path);
        let is_search = !self.tabs[self.active].query.trim().is_empty();
        let size = self.entry_size(&entry).map(support::bytes).unwrap_or_else(|| "—".into());
        let name = entry.name.clone(); let kind = entry.kind();
        let modified = entry.modified.map(chrono::DateTime::<chrono::Local>::from).map(|time| time.format("%d %b %Y %H:%M").to_string()).unwrap_or_else(|| "—".into());
        let icon = if entry.is_dir {IconName::Folder} else {IconName::FileText};
        let accessible_name = format!("{}, {}, {}",name,kind,size);
        let path = entry.path.clone();
        let disclosure = if entry.is_dir && !is_search {
            <Button args={format!("expand-{index}")} ghost xsmall icon={if expanded {IconName::ChevronDown}else{IconName::ChevronRight}}
                on-click={cx.listener(move |this,_,_,cx|{cx.stop_propagation();this.toggle_folder(path.clone(),cx);})} />.into_any_element()
        } else {<div w={px(20.)} flex-shrink-0 />.into_any_element()};
        let location = entry.path.parent().unwrap_or(Path::new("/")).strip_prefix(self.tabs[self.active].history.current()).unwrap_or(entry.path.parent().unwrap_or(Path::new("/"))).to_string_lossy().into_owned();
        let compact = f32::from(window.bounds().size.width) < 1000. || self.inspector_open;
        let owner = cx.entity();
        let menu_entry = entry.clone();
        <div id={("file",index)} role={Role::Row} aria-label={accessible_name} aria-selected={selected} h={px(if is_search {48.}else{29.})} w-full flex items-center gap-3 px-3 cursor-pointer rounded-md
            bg={if selected {cx.theme().list_active}else if index%2==1 {cx.theme().muted}else{cx.theme().background}}
            hover={|style|style.bg(cx.theme().list_hover)}
            on-click={cx.listener(move |this,event:&ClickEvent,window,cx|{
            this.select(entry.clone(),window,cx);
            if event.click_count()>=2 {this.open_selection(window,cx);}
            })}
            context-menu={move |menu, window, cx| fill_file_menu(menu, owner.clone(), menu_entry.clone(), window, cx)}>
            <div flex flex-1 min-w-0 items-center gap-2 pl={px(row.depth as f32*16.)}>
                {disclosure}<Icon args={icon} size={px(15.)} text-color={if kind=="Folder" {cx.theme().link}else{cx.theme().muted_foreground}} />
                <div flex flex-col flex-1 min-w-0>
                    <div truncate>{name}</div>
                    {if is_search {<div text-xs truncate text-color={cx.theme().muted_foreground}>{if location.is_empty(){"This folder".into()}else{location}}</div>.into_any_element()}else{<div />.into_any_element()}}
                </div>
                {if loading {<Spinner small />.into_any_element()}else{<div />.into_any_element()}}
            </div>
            {if !compact {<div w={px(158.)} flex-shrink-0 text-xs text-color={cx.theme().muted_foreground}>{modified}</div>.into_any_element()}else{<div />.into_any_element()}}
            <div w={px(84.)} flex-shrink-0 text-right text-xs>{size}</div>
            <div w={px(105.)} flex-shrink-0 text-xs truncate text-color={cx.theme().muted_foreground}>{kind}</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let rows = Arc::new(self.rows()); let count = rows.len(); let owner = cx.entity();
        let compact = f32::from(window.bounds().size.width) < 1000. || self.inspector_open;
        let columns = [(0,"Name",None), (1,"Date modified",Some(158.)), (2,if self.allocated {"On disk"}else{"Size"},Some(84.)), (3,"Kind",Some(105.))]
            .into_iter().filter(|(index,_,_)|!compact || *index!=1).map(|(index,label,width)| {
                let label = if self.sort==index {format!("{label} {}",if self.descending {"↓"}else{"↑"})} else {label.into()};
                div().id(("column",index)).when_some(width,|el,width|el.w(px(width)).flex_shrink_0()).when(width.is_none(),|el|el.flex_1().min_w_0())
                    .cursor_pointer().child(label).on_click(cx.listener(move|this,_,_,cx|{if this.sort==index {this.descending=!this.descending;}else{this.sort=index;this.descending=false;}cx.notify();}))
            }).collect::<Vec<_>>();
        let content = if count == 0 {
            let tab = &self.tabs[self.active];
            let message = if let Some(error)=&tab.search_error {error.clone()} else if !tab.status.is_empty(){tab.status.clone()}else if tab.searching {"Searching this folder and subfolders…".into()}else if tab.loading {"Opening folder…".into()}else if !tab.query.trim().is_empty(){"No matching files".into()}else{"This folder is empty".into()};
            <div flex flex-col flex-1 items-center justify-center gap-3 text-color={cx.theme().muted_foreground}>
                <Icon args={if self.search_open {IconName::Search}else{IconName::FolderOpen}} size={px(32.)} /><div>{message}</div>
            </div>.into_any_element()
        } else {
            <uniform_list args={("file-list",count,move|range,window,cx|owner.update(cx,|this,cx|range.map(|i|this.file_row(rows[i].clone(),i,window,cx)).collect::<Vec<_>>()))} track-scroll={&self.list_scroll} flex-1 min-h-0 />.into_any_element()
        };
        <div id="files-pane" key-context="ExplorerList" track-focus={&self.list_focus} flex flex-col size-full min-h-0>
            <div flex items-center gap-3 px-3 py-2 border-b-1 border-color={cx.theme().border} text-xs text-color={cx.theme().muted_foreground} children={columns} />
            <div flex flex-col flex-1 min-h-0 p-2>{content}</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn storage(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(scan) = self.current_analysis().and_then(|analysis| analysis.scan.clone()) else {
            let scanning = self.current_analysis().is_some_and(|analysis| analysis.scanning || analysis.queued);
            return <div flex size-full items-center justify-center text-color={cx.theme().muted_foreground}>{if scanning {"Storage analysis is running. You can keep browsing."}else{"Run an analysis to explore folder sizes."}}</div>.into_any_element();
        };
        let nodes = self.tree_nodes.clone(); let allocated = self.allocated; let total = scan.root.weight(allocated);
        let rows = tree(&self.tree, move |index, entry, selected, _, cx| {
            let node = &nodes[entry.item().id.as_ref()];
            let share = if total==0 {0.}else{node.weight(allocated) as f32/total as f32};
            let disclosure = if entry.is_folder() {Icon::new(if entry.is_expanded(){IconName::ChevronDown}else{IconName::ChevronRight}).small().into_any_element()}else{<div w={px(16.)} />.into_any_element()};
            ListItem::new(index).selected(selected).h(px(30.)).text_sm().font_normal().px_3().child(
                <div flex items-center gap-3 w-full>
                    <div flex flex-1 min-w-0 items-center gap-2 pl={px(entry.depth() as f32*16.)}>{disclosure}<Icon args={if node.directory {IconName::Folder}else{IconName::File}} small /><div truncate>{node.name()}</div></div>
                    <div w={px(90.)} text-right>{support::bytes(node.weight(allocated))}</div>
                    <div relative w={px(70.)} h={px(4.)} bg={cx.theme().muted} rounded-full overflow-hidden><div absolute left-0 top-0 h-full w={relative(share)} bg={cx.theme().link} /></div>
                    <div w={px(50.)} text-right text-xs text-color={cx.theme().muted_foreground}>{format!("{:.1}%",share*100.)}</div>
                </div>
            )
        });
        <div flex flex-col size-full min-h-0><div px-4 py-2 text-xs text-color={cx.theme().muted_foreground}>Folder usage · percentages of the scan · totals include hidden files</div><div flex-1 min-h-0>{rows}</div></div>.into_any_element()
    }
    #[gpui]
    fn largest_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let analysis = self.current_analysis();
        let root = analysis.map(|analysis| analysis.path.as_path()).unwrap_or(Path::new("/"));
        let files = Arc::new(analysis.map(|analysis| {
            let files = if self.allocated { &analysis.largest_allocated } else { &analysis.largest_logical };
            files.iter().filter(|node|self.hidden || !has_hidden_component(&node.path, root)).cloned().collect::<Vec<_>>()
        }).unwrap_or_default());
        let count = files.len(); let owner = cx.entity(); let allocated = self.allocated;
        <uniform_list args={("largest-files",count,move|range,_,cx|owner.update(cx,|_,cx|range.map(|i|{
            let node=files[i].clone(); let name=node.name(); let path=node.path.to_string_lossy().into_owned();
            <div id={("large",i)} h={px(48.)} px-4 flex items-center gap-3 cursor-pointer hover={|style|style.bg(cx.theme().list_hover)} on-click={cx.listener(move|this,_,window,cx|this.select(entry_from_node(&node),window,cx))}>
            <Icon args={IconName::File} small /><div flex flex-col flex-1 min-w-0><div truncate>{name}</div><div text-xs truncate text-color={cx.theme().muted_foreground}>{path}</div></div><div>{support::bytes(files[i].weight(allocated))}</div>
            </div>.into_any_element()
            }).collect::<Vec<_>>()))} size-full />.into_any_element()
    }
    #[gpui]
    fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab=&self.tabs[self.active];
        let content=if let Some(entry)=&tab.selected {
            if self.detail_tab==0 && !entry.is_dir {
                match &tab.preview {
                    Some(Ok(Preview::Image(path))) => <img args={path.clone()} w={px(260.)} h={px(260.)} max-w-full object-fit={ObjectFit::Contain} />.into_any_element(),
                    Some(Ok(Preview::Text(text))) | Some(Ok(Preview::Binary(text))) => <div text-xs font-family="monospace" whitespace-normal>{text.clone()}</div>.into_any_element(),
                    Some(Err(error)) => <div text-sm>{error.clone()}</div>.into_any_element(),
                    None => <div text-sm text-color={cx.theme().muted_foreground}>Select the file in the list to load a preview.</div>.into_any_element(),
                }
            }else{
                let size=self.entry_size(entry).map(support::bytes).unwrap_or_else(||"Run storage analysis to calculate this folder.".into());
                let path=entry.path.clone();
                <div flex flex-col gap-4 whitespace-normal>
                    <Icon args={if entry.is_dir {IconName::FolderOpen}else{IconName::FileText}} size={px(36.)} text-color={cx.theme().link} />
                    <div font-semibold>{entry.name.clone()}</div><div text-xs text-color={cx.theme().muted_foreground}>{entry.path.to_string_lossy().into_owned()}</div>
                    <div text-xs text-color={cx.theme().muted_foreground}>{entry.kind()}</div>
                    <div border-t-1 border-color={cx.theme().border} pt-3><div text-xs text-color={cx.theme().muted_foreground}>{if self.allocated {"ON DISK"}else{"SIZE"}}</div><div mt-1>{size}</div></div>
                    <div text-xs>{entry.modified.map(chrono::DateTime::<chrono::Local>::from).map(|time|format!("Modified {}",time.format("%d %b %Y, %H:%M"))).unwrap_or_default()}</div>
                    <Button args={"open-file"} small label={if entry.is_dir {"Open folder"}else{"Open file"}} on-click={cx.listener(|this,_,window,cx|this.open_selection(window,cx))} />
                    <Button args={"reveal-file"} ghost small label="Show in Finder" on-click={move|_,_,cx|cx.reveal_path(&path)} />
                </div>.into_any_element()
            }
        }else{<div text-sm text-color={cx.theme().muted_foreground}>Select a file or folder to see its details.</div>.into_any_element()};
        <div flex flex-col size-full min-h-0>
            <TabBar args={"inspector-tabs"} selected-index={self.detail_tab} on-click={cx.listener(|this,index,window,cx|{this.detail_tab=*index;if *index==0 {if let Some(entry)=this.tabs[this.active].selected.clone(){this.select(entry,window,cx);}}cx.notify();})}><Tab label="Preview" /><Tab label="Info" /></TabBar>
            <div id="inspector-scroll" flex-1 min-h-0 overflow-y-scroll p-4 whitespace-normal>{content}</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn folder_tabs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let tabs = self.tabs.iter().enumerate().map(|(index, tab)| {
            let selected = index == self.active;
            let id = tab.id;
            let label = tab.history.current().file_name().unwrap_or(tab.history.current().as_os_str()).to_string_lossy().into_owned();
            let close = self.tabs.len() > 1;
            <div id={("nav-tab", id)} h={px(28.)} px-3 flex items-center gap-2 rounded-lg cursor-pointer flex-shrink-0
                bg={if selected {cx.theme().background} else {cx.theme().sidebar}}
                on-mouse-down:args={(MouseButton::Left, cx.listener(move |this, _, _, cx| { this.dragging_tab = Some(index); this.active = index; cx.notify(); }))}
                on-mouse-move={cx.listener(move |this, event: &MouseMoveEvent, window, cx| {
                    if event.dragging() {
                        if let Some(from) = this.dragging_tab { this.move_tab(from, index); this.sync_address(window, cx); cx.notify(); }
                    }
                })}
                on-mouse-up:args={(MouseButton::Left, cx.listener(move |this, _, window, cx| { this.dragging_tab = None; this.activate_tab(index, window, cx); }))}>
                <Icon args={IconName::Folder} size={px(12.)} text-color={cx.theme().link} />
                <div text-xs truncate max-w={px(140.)}>{label}</div>
                {if close {
                    <Button args={format!("close-tab-{id}")} ghost xsmall icon={IconName::Close} on-click={cx.listener(move |this, _, window, cx| { cx.stop_propagation(); this.close_tab_at(index, window, cx); })} />.into_any_element()
                } else { <div />.into_any_element() }}
            </div>.into_any_element()
        }).collect::<Vec<_>>();
        <div flex items-center gap-1 px-2 py-1 bg={cx.theme().sidebar} border-b-1 border-color={cx.theme().border}>
            <div flex items-center gap-1 flex-1 min-w-0 overflow-x-hidden children={tabs} />
            <Button args={"new-tab"} ghost xsmall icon={IconName::Plus} accessibility-label="New Tab" on-click={cx.listener(|this, _, window, cx| this.add_tab(this.tabs[this.active].history.current().to_owned(), window, cx))} />
        </div>.into_any_element()
    }
    fn place(&self, id: usize, label: &'static str, icon: IconName, path: PathBuf, cx: &mut Context<Self>) -> AnyElement {
        Button::new(("place", id)).accessibility_label(label).ghost().w_full()
            .selected(self.tabs[self.active].history.current()==path)
            .child(div().flex().items_center().gap_2().w_full().child(Icon::new(icon).small()).child(label))
            .on_click(cx.listener(move|this,_,window,cx|this.navigate(path.clone(),window,cx)))
            .into_any_element()
    }
    #[gpui]
    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let current=self.tabs[self.active].history.current();
        let home=std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(||current.to_owned());
        let places=[("Home",home.clone(),IconName::User),("Desktop",home.join("Desktop"),IconName::Frame),("Documents",home.join("Documents"),IconName::FileText),("Downloads",home.join("Downloads"),IconName::Inbox),("Applications",PathBuf::from("/Applications"),IconName::LayoutDashboard),("Project",std::env::current_dir().ok().filter(|p|p!=Path::new("/")).unwrap_or_else(||home.join("Projects")),IconName::SquareTerminal)];
        let rows=places.into_iter().enumerate().map(|(index,(label,path,icon))|self.place(index,label,icon,path,cx)).collect::<Vec<_>>();
        <div id="sidebar" flex flex-col size-full px-3 py-4 bg={cx.theme().sidebar}>
            <div px-2 pb-2 text-xs text-color={cx.theme().muted_foreground}>Favorites</div><div flex flex-col gap-1 children={rows} />
            <div px-2 pt-5 pb-2 text-xs text-color={cx.theme().muted_foreground}>Locations</div>
            {self.place(100,"Filesystem",IconName::HardDrive,"/".into(),cx)}
            <div flex-1 />
            <Button args={"show-hidden"} accessibility-label="Toggle hidden files" ghost small w-full selected={self.hidden} on-click={cx.listener(|this,_,_,cx|this.toggle_hidden(cx))}>
            <div flex items-center gap-2 w-full><Icon args={IconName::Eye} small /><div>{if self.hidden {"Hidden files visible"}else{"Show hidden files"}}</div></div>
            </Button>
            <Button args={"settings"} accessibility-label="Settings" ghost small w-full selected={self.settings_open} on-click={cx.listener(|this,_,_,cx|{this.settings_open=!this.settings_open;cx.notify();})}>
            <div flex items-center gap-2 w-full><Icon args={IconName::Settings} small /><div>Settings</div></div>
            </Button>
        </div>.into_any_element()
    }
    #[gpui]
    fn settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner=cx.entity(); let metric_owner=owner.clone(); let metric_setter=owner.clone();
        let settings=Settings::new("file-explorer-settings").sidebar_width(px(150.)).page(SettingPage::new("General").default_open(true)
            .group(SettingGroup::new().title("Appearance").item(SettingItem::new("Color mode",SettingField::render(move|_,_,cx|owner.update(cx,|this,cx|{
                let buttons=[(Appearance::System,"System",IconName::Frame),(Appearance::Light,"Light",IconName::Sun),(Appearance::Dark,"Dark",IconName::Moon)].into_iter().map(|(mode,label,icon)|{
                    <Button args={label} label={label} icon={icon} small selected={this.appearance==mode} on-click={cx.listener(move|this,_,window,cx|{this.appearance=mode;apply_appearance(mode,window,cx);this.save_preferences();cx.notify();})} />
                }).collect::<Vec<_>>();
                <div flex gap-2 children={buttons} />.into_any_element()
            }))).description("Follow your system or choose a light or dark interface.")))
            .group(SettingGroup::new().title("Storage").item(SettingItem::new("Use allocated space",SettingField::switch(move|cx|metric_owner.read(cx).allocated,move|value,cx|metric_setter.update(cx,|this,cx|this.set_metric(value,cx)))).description("Show disk blocks instead of file length. Folder totals are calculated only when you run an analysis."))));
        <div flex flex-col size-full><div px-5 py-4 text-lg font-semibold>Settings</div><div flex-1 min-h-0>{settings}</div><div px-5 py-3 text-xs text-color={cx.theme().muted_foreground}>{self.preference_status.clone()}</div></div>.into_any_element()
    }
}

impl Render for Browser {
    #[gpui]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = &self.tabs[self.active];
        let can_back = tab.history.index > 0;
        let can_forward = tab.history.index + 1 < tab.history.paths.len();
        let searching = tab.searching;
        let has_scan = !self.analyses.is_empty();
        let status = if searching { format!("Searching… {} items checked · {} matches · {} folders", tab.search_update.visited, tab.search_update.matches, tab.search_update.folders) }
            else if let Some(error) = &tab.search_error { error.clone() }
            else if !tab.query.trim().is_empty() { format!("{} matches · showing {} · {} skipped", tab.search_update.matches, tab.search_update.results.len(), tab.search_update.skipped) }
            else if tab.loading { "Opening folder…".into() }
            else if !tab.status.is_empty() { tab.status.clone() }
            else { format!("{} items{}", self.rows().len(), if tab.unreadable > 0 { format!(" · {} unreadable", tab.unreadable) } else { String::new() }) };
        let rename_open = self.rename.is_some();
        let confirm = self.confirm_delete.clone();
        let content = if self.settings_open { self.settings(cx) } else {
            let (progress, warnings) = self.report_snapshot();
            let banner = self.analysis_banner();
            let scanning = self.analyses.iter().any(|analysis| analysis.scanning);
            let analysis = match self.analysis_tab {
                1 => self.storage(cx),
                2 => self.largest_view(cx),
                3 => {
                    let warnings = warnings.into_iter().map(|warning| <div text-xs py-2>{warning}</div>).collect::<Vec<_>>();
                    <div id="report" size-full overflow-y-scroll p-4 flex flex-col gap-3 whitespace-normal><div font-semibold>Scan report</div><div text-sm>Totals include hidden items. Symbolic links and special files are skipped. On Unix, hard links count once and scanning stays on the starting filesystem.</div><div text-sm>{format!("{} files · {} skipped", progress.files, progress.skipped)}</div><div flex flex-col children={warnings} /></div>.into_any_element()
                },
                _ => self.files(window, cx),
            };
            let mut panes = h_resizable("file-content").child(resizable_panel().child(analysis));
            if self.inspector_open { panes = panes.child(resizable_panel().size(px(290.)).size_range(px(240.)..px(460.)).child(self.inspector(cx))); }
            <div flex flex-col size-full min-h-0>
                {if has_scan {
                    <div flex items-center gap-2 px-4 py-2 bg={cx.theme().muted}>
                        {if scanning { <Spinner small />.into_any_element() } else { <Icon args={IconName::HardDrive} small />.into_any_element() }}
                        <div flex-1 min-w-0 truncate text-xs>{banner}</div>
                        {if scanning { <Button args={"cancel-analysis"} label="Cancel" ghost small on-click={cx.listener(|this,_,_,cx|{ if let Some(analysis) = this.analyses.iter().find(|analysis| analysis.scanning) { analysis.cancel.store(true, Ordering::Relaxed); } cx.notify(); })} />.into_any_element() } else { <div />.into_any_element() }}
                        <Button args={"clear-analysis"} label="Clear" ghost small on-click={cx.listener(|this,_,_,cx| this.clear_scan(cx))} />
                    </div>.into_any_element()
                } else { <div />.into_any_element() }}
                {if has_scan {
                    <TabBar args={"analysis-tabs"} selected-index={self.analysis_tab} on-click={cx.listener(|this,index,_,cx|{ this.analysis_tab=*index; cx.notify(); })}><Tab label="Files" /><Tab label="Storage tree" /><Tab label="Largest files" /><Tab label="Scan report" /></TabBar>.into_any_element()
                } else { <div />.into_any_element() }}
                <div flex-1 min-h-0>{panes}</div>
                <div flex items-center gap-2 px-4 py-2 border-t-1 border-color={cx.theme().border} text-xs text-color={cx.theme().muted_foreground}>
                    {if searching { <Spinner small />.into_any_element() } else { <div />.into_any_element() }}<div flex-1 min-w-0 truncate>{status}</div>
                    <div>{if self.hidden {"Hidden files visible"} else {"Hidden files off"}}</div>
                </div>
            </div>.into_any_element()
        };
        let overlay = if rename_open {
            <div absolute inset-0 flex items-center justify-center bg={hsla(0., 0., 0., 0.35)}>
                <div w={px(360.)} p-4 rounded-lg bg={cx.theme().background} border-1 border-color={cx.theme().border} flex flex-col gap-3>
                    <div font-semibold>Rename</div>
                    <Input args={&self.rename_input} />
                    <div flex justify-end gap-2>
                        <Button args={"cancel-rename"} ghost small label="Cancel" on-click={cx.listener(|this,_,_,cx|{ this.rename=None; cx.notify(); })} />
                        <Button args={"save-rename"} small label="Save" on-click={cx.listener(|this,_,window,cx| this.commit_rename(window,cx))} />
                    </div>
                </div>
            </div>.into_any_element()
        } else if let Some(entry) = confirm {
            let name = entry.name.clone();
            <div absolute inset-0 flex items-center justify-center bg={hsla(0., 0., 0., 0.35)}>
                <div w={px(380.)} p-4 rounded-lg bg={cx.theme().background} border-1 border-color={cx.theme().border} flex flex-col gap-3>
                    <div font-semibold>Delete immediately?</div>
                    <div text-sm whitespace-normal>{format!("{name} will be removed permanently. This cannot be undone.")}</div>
                    <div flex justify-end gap-2>
                        <Button args={"cancel-delete"} ghost small label="Cancel" on-click={cx.listener(|this,_,_,cx|{ this.confirm_delete=None; cx.notify(); })} />
                        <Button args={"confirm-delete"} primary small label="Delete" on-click={cx.listener(move |this,_,window,cx| this.delete_entry(entry.clone(), window, cx))} />
                    </div>
                </div>
            </div>.into_any_element()
        } else { <div />.into_any_element() };
        <div id="file-explorer" role={Role::Group} aria-label="File Explorer" key-context="FileExplorer" track-focus={&self.focus} relative flex size-full text-sm font-normal overflow-hidden bg={cx.theme().background} text-color={cx.theme().foreground}
            on-action={cx.listener(|this,_:&Find,window,cx|{ this.settings_open=false; this.search_open=true; this.search_input.read(cx).focus_handle(cx).focus(window,cx); cx.notify(); })}
            on-action={cx.listener(|this,_:&ToggleHidden,_,cx| this.toggle_hidden(cx))}
            on-action={cx.listener(|this,_:&GoToFolder,window,cx|{ this.address.read(cx).focus_handle(cx).focus(window,cx); cx.notify(); })}
            on-action={cx.listener(|this,_:&Back,window,cx|{ this.tabs[this.active].history.back(); this.refresh(window,cx); })}
            on-action={cx.listener(|this,_:&Forward,window,cx|{ this.tabs[this.active].history.forward(); this.refresh(window,cx); })}
            on-action={cx.listener(|this,_:&Parent,window,cx|{ if let Some(parent)=this.tabs[this.active].history.current().parent() { this.navigate(parent.to_owned(), window, cx); } })}
            on-action={cx.listener(|this,_:&NewTab,window,cx| this.add_tab(this.tabs[this.active].history.current().to_owned(), window, cx))}
            on-action={cx.listener(|this,_:&CloseTab,window,cx| this.close_tab(window,cx))}
            on-action={cx.listener(|this,_:&QuickLook,window,cx|{ this.inspector_open=!this.inspector_open; this.detail_tab=0; if this.inspector_open { if let Some(entry)=this.tabs[this.active].selected.clone() { this.select(entry,window,cx); } } cx.notify(); })}
            on-action={cx.listener(|this,_:&OpenSelection,window,cx| this.open_selection(window,cx))}
            on-action={cx.listener(|this,_:&NextItem,window,cx| this.move_selection(true,window,cx))}
            on-action={cx.listener(|this,_:&PreviousItem,window,cx| this.move_selection(false,window,cx))}
            on-action={cx.listener(|this,_:&ExpandItem,_,cx|{ if let Some(entry)=this.tabs[this.active].selected.clone() { if entry.is_dir && !this.tabs[this.active].expanded.contains(&entry.path) { this.toggle_folder(entry.path,cx); } } })}
            on-action={cx.listener(|this,_:&CollapseItem,_,cx|{ if let Some(entry)=this.tabs[this.active].selected.clone() { if this.tabs[this.active].expanded.remove(&entry.path) { cx.notify(); } } })}
            on-action={cx.listener(|this,_:&RenameItem,window,cx| this.begin_rename(None, window, cx))}
            on-action={cx.listener(|this,_:&GetInfo,window,cx| this.show_info(None, window, cx))}
            on-action={cx.listener(|this,_:&MoveToTrash,window,cx|{ if let Some(entry)=this.tabs[this.active].selected.clone() { this.trash_entry(entry, window, cx); } })}
            on-action={cx.listener(|this,_:&NewFolder,window,cx| this.new_folder(window, cx))}
            on-action={cx.listener(|this,_:&Dismiss,window,cx|{
                if this.rename.take().is_some() || this.confirm_delete.take().is_some() { cx.notify(); return; }
                this.settings_open=false; this.inspector_open=false; this.search_open=false;
                this.tabs[this.active].query.clear();
                this.search_input.update(cx,|input,cx| input.set_value("", window, cx));
                this.start_search(cx); this.list_focus.focus(window,cx); cx.notify();
            })}
            on-action={cx.listener(|this,_:&ShowSettings,_,cx|{ this.settings_open=!this.settings_open; cx.notify(); })}>
            <div w={px(180.)} flex-shrink-0 border-r-1 border-color={cx.theme().border}>{self.sidebar(cx)}</div>
            <div flex flex-col flex-1 min-w-0 min-h-0>
                {self.folder_tabs(cx)}
                <div flex items-center gap-2 px-4 h={px(52.)} flex-shrink-0 border-b-1 border-color={cx.theme().border}>
                    <Button args={"back"} accessibility-label="Back" icon={IconName::ChevronLeft} ghost small disabled={!can_back} on-click={cx.listener(|this,_,window,cx|{ this.tabs[this.active].history.back(); this.refresh(window,cx); })} />
                    <Button args={"forward"} accessibility-label="Forward" icon={IconName::ChevronRight} ghost small disabled={!can_forward} on-click={cx.listener(|this,_,window,cx|{ this.tabs[this.active].history.forward(); this.refresh(window,cx); })} />
                    <div flex-1 min-w-0>{if self.settings_open { <div font-semibold>Settings</div>.into_any_element() } else { <Input args={&self.address} small />.into_any_element() }}</div>
                    <Button args={"analyze-space"} label="Analyze space" icon={IconName::HardDrive} small ghost on-click={cx.listener(|this,_,_,cx| this.start_scan(cx))} />
                    <Button args={"inspector-toggle"} accessibility-label="Toggle inspector" icon={IconName::PanelRight} small ghost selected={self.inspector_open} on-click={cx.listener(|this,_,_,cx|{ this.inspector_open=!this.inspector_open; cx.notify(); })} />
                    <Button args={"browse"} accessibility-label="Choose folder" icon={IconName::FolderOpen} small ghost on-click={cx.listener(|_,_,window,cx|{
                        let choice=cx.prompt_for_paths(PathPromptOptions{files:false,directories:true,multiple:false,prompt:Some("Open folder".into())});
                        cx.spawn_in(window,async move|this,cx|{if let Ok(Ok(Some(paths)))=choice.await{if let Some(path)=paths.into_iter().next(){let _=this.update_in(cx,|this,window,cx|this.navigate(path,window,cx));}}}).detach();
                        })} />
                    {if self.search_open { <div w={px(240.)}><Input args={&self.search_input} small /></div>.into_any_element() } else { <Button args={"find"} accessibility-label="Find files" icon={IconName::Search} small ghost on-click={cx.listener(|this,_,window,cx|{ this.search_open=true; this.search_input.read(cx).focus_handle(cx).focus(window,cx); cx.notify(); })} />.into_any_element() }}
                </div>
                <div flex-1 min-h-0>{content}</div>
            </div>
            {overlay}
        </div>
    }
}

fn apply_appearance(appearance: Appearance, window: &mut Window, cx: &mut App) {
    match appearance {Appearance::System=>Theme::sync_system_appearance(Some(window),cx),Appearance::Light=>Theme::change(ThemeMode::Light,Some(window),cx),Appearance::Dark=>Theme::change(ThemeMode::Dark,Some(window),cx)}
    Theme::update(cx, |theme| {
        let dark = theme.is_dark();
        theme.background = rgb(if dark {0x202124}else{0xffffff}).into();
        theme.foreground = rgb(if dark {0xe8eaed}else{0x25272a}).into();
        theme.sidebar = rgb(if dark {0x18191b}else{0xf3f4f6}).into();
        theme.muted = rgb(if dark {0x28292c}else{0xf6f7f9}).into();
        theme.muted_foreground = rgb(if dark {0xa0a4ab}else{0x6b7077}).into();
        theme.border = rgb(if dark {0x34363a}else{0xe2e4e7}).into();
        theme.list_hover = rgb(if dark {0x2e3136}else{0xeff3f8}).into();
        theme.list_active = rgb(if dark {0x244568}else{0xdcecff}).into();
        theme.link = rgb(if dark {0x61adfa}else{0x2276ce}).into();
    });
}
fn preferences_path() -> Option<PathBuf> {
    std::env::var_os("GPUI_EXPLORER_PREFERENCES").map(PathBuf::from).or_else(||Preferences::path_for("rsx-file-explorer"))
}
type BrowserAction = Box<dyn Fn(&mut Browser, &mut Window, &mut Context<Browser>) + 'static>;
fn call_browser<C: AppContext>(owner: &Entity<Browser>, window: &mut Window, cx: &mut C, handler: impl FnOnce(&mut Browser, &mut Window, &mut Context<Browser>)) {
    let owner = owner.clone();
    let _ = window.window_handle().update(cx, |_, window, cx| {
        owner.update(cx, |this, cx| handler(this, window, cx));
    });
}
fn fill_file_menu(menu: gpui_kit::component::menu::PopupMenu, owner: Entity<Browser>, entry: Entry, window: &mut Window, cx: &mut Context<gpui_kit::component::menu::PopupMenu>) -> gpui_kit::component::menu::PopupMenu {
    call_browser(&owner, window, cx, {
        let entry = entry.clone();
        move |this, window, cx| this.select(entry, window, cx)
    });
    let item = |label: &str, handler: BrowserAction| {
        let owner = owner.clone();
        PopupMenuItem::new(label.to_owned()).on_click(move |_, window, cx| {
            call_browser(&owner, window, cx, |this, window, cx| handler(this, window, cx));
        })
    };
    let open = entry.clone();
    let tab = entry.clone();
    let rename = entry.clone();
    let duplicate = entry.clone();
    let copy = entry.clone();
    let reveal = entry.clone();
    let info = entry.clone();
    let analyze = entry.clone();
    let trash = entry.clone();
    let delete = entry.clone();
    menu.item(item("Open", Box::new(move |this, window, cx| { this.select(open.clone(), window, cx); this.open_selection(window, cx); })))
        .item(item("Open in New Tab", Box::new(move |this, window, cx| {
            if tab.is_dir { this.add_tab(tab.path.clone(), window, cx); }
            else { this.add_tab(tab.path.parent().unwrap_or(Path::new("/")).to_owned(), window, cx); }
        })))
        .separator()
        .item(item("Get Info", Box::new(move |this, window, cx| this.show_info(Some(info.clone()), window, cx))))
        .item(item("Rename", Box::new(move |this, window, cx| this.begin_rename(Some(rename.clone()), window, cx))))
        .item(item("Duplicate", Box::new(move |this, window, cx| this.duplicate_selected(duplicate.clone(), window, cx))))
        .item(item("New Folder", Box::new(|this, window, cx| this.new_folder(window, cx))))
        .separator()
        .item(item("Copy Path", Box::new(move |this, _, cx| this.copy_path(&copy.path, cx))))
        .item(PopupMenuItem::new("Show in Finder").on_click({ let reveal = reveal.clone(); move |_,_,cx| cx.reveal_path(&reveal.path) }))
        .item(item("Analyze this Folder", Box::new(move |this, _, cx| {
            let path = if analyze.is_dir { analyze.path.clone() } else { analyze.path.parent().unwrap_or(Path::new("/")).to_owned() };
            this.enqueue_scan(path, cx);
        })))
        .separator()
        .item(item("Move to Trash", Box::new(move |this, window, cx| this.trash_entry(trash.clone(), window, cx))))
        .item(item("Delete Immediately...", Box::new(move |this, _, cx| { this.confirm_delete = Some(delete.clone()); cx.notify(); })))
}
fn expand_home(value: &str) -> PathBuf {
    if value=="~" || value.starts_with("~/") {
        if let Some(home)=std::env::var_os("HOME") {return PathBuf::from(home).join(value.strip_prefix("~/").unwrap_or(""));}
    }
    value.into()
}
fn has_hidden_component(path: &Path, root: &Path) -> bool {path.strip_prefix(root).unwrap_or(path).components().any(|part|part.as_os_str().to_string_lossy().starts_with('.'))}
fn entry_from_node(node: &Node) -> Entry {Entry{path:node.path.clone(),name:node.name(),is_dir:node.directory,is_symlink:false,bytes:node.logical,allocated:node.allocated,modified:None}}
fn collect_expanded(item: &TreeItem, expanded: &mut HashSet<String>) {if item.is_expanded(){expanded.insert(item.id.to_string());}for child in &item.children {collect_expanded(child,expanded);}}
fn storage_tree(node: &Node, id: String, hidden: bool, allocated: bool, expanded: &HashSet<String>, nodes: &mut HashMap<String, Node>) -> TreeItem {
    nodes.insert(id.clone(),node.summary());
    let mut order=node.children.iter().enumerate().filter(|(_,child)|hidden||!child.name().starts_with('.')).collect::<Vec<_>>();
    order.sort_by(|(_,a),(_,b)|b.weight(allocated).cmp(&a.weight(allocated)).then_with(||a.path.cmp(&b.path)));
    let children=order.into_iter().map(|(index,child)|storage_tree(child,format!("{id}/{index}"),hidden,allocated,expanded,nodes)).collect::<Vec<_>>();
    TreeItem::new(id.clone(),node.name()).children(children).expanded(id=="root"||expanded.contains(&id))
}

#[cfg(test)]
mod tests {
    use super::{Node, HashMap, HashSet, storage_tree, collect_expanded, has_hidden_component, node_at};
    use std::path::Path;
    #[test]
    fn storage_tree_hides_dot_subtrees_and_preserves_stable_expansion_ids() {
        let leaf = |path: &str, logical, allocated| Node {path:path.into(),directory:false,logical,allocated,files:1,children:vec![]};
        let root=Node {path:"/scan".into(),directory:true,logical:110,allocated:200,files:3,children:vec![
            Node {path:"/scan/folder".into(),directory:true,logical:90,allocated:20,files:1,children:vec![leaf("/scan/folder/a",90,20)]},
            leaf("/scan/b",10,170),leaf("/scan/.hidden",10,10),
        ]};
        let mut nodes=HashMap::new();
        let mut tree=storage_tree(&root,"root".into(),false,false,&HashSet::new(),&mut nodes);
        assert_eq!(tree.children.len(),2);
        tree.children[0]=tree.children[0].clone().expanded(true);
        let mut expanded=HashSet::new();collect_expanded(&tree,&mut expanded);
        nodes.clear();
        let reordered=storage_tree(&root,"root".into(),false,true,&expanded,&mut nodes);
        assert_eq!(reordered.children[0].id.as_ref(),"root/1");
        assert!(reordered.children[1].is_expanded());
        assert!(!nodes.values().any(|node|node.name().starts_with('.')));
        assert!(has_hidden_component(Path::new("/scan/.cache/file"),Path::new("/scan")));
        assert!(!has_hidden_component(Path::new("/scan/folder/file"),Path::new("/scan")));
    }

    #[test]
    fn node_at_finds_nested_indexed_folder() {
        let leaf = Node {path:"/scan/folder/a".into(),directory:false,logical:90,allocated:20,files:1,children:vec![]};
        let root=Node {path:"/scan".into(),directory:true,logical:90,allocated:20,files:1,children:vec![
            Node {path:"/scan/folder".into(),directory:true,logical:90,allocated:20,files:1,children:vec![leaf]},
        ]};
        assert_eq!(node_at(&root, Path::new("/scan/folder")).unwrap().name(), "folder");
        assert!(node_at(&root, Path::new("/missing")).is_none());
    }
}
