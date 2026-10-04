use gpui_kit::{prelude::*, *};
use gpui_kit::component::{
    ActiveTheme as _, StyledExt as _, Disableable as _, Sizable as _, Selectable as _,
    Icon, IconName, Theme, ThemeMode,
    list::ListItem,
    tree::{tree, TreeState, TreeItem},
    setting::{Settings, SettingPage, SettingGroup, SettingItem, SettingField},
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    tab::{Tab, TabBar},
    resizable::{h_resizable, resizable_panel},
    spinner::Spinner,
};
use rsx_disk_explorer::{Appearance, Preferences, Node, Scan, scan, treemap, largest_files};
use crate::support;
use std::{
    path::PathBuf,
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Explorer {
    path: Entity<InputState>,
    scan: Option<Arc<Scan>>,
    focus: Vec<usize>,
    tab: usize,
    allocated: bool,
    appearance: Appearance,
    settings_open: bool,
    preference_status: String,
    tree: Entity<TreeState>,
    tree_nodes: Arc<HashMap<String, (Vec<usize>, Node)>>,
    selected_id: Option<String>,
    busy: bool,
    generation: u64,
    cancel: Arc<AtomicBool>,
    status: String,
    largest_logical: Vec<Node>,
    largest_allocated: Vec<Node>,
    selected: Option<Node>,
}
impl Drop for Explorer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Explorer {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial = support::initial_directory();
        let path = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(initial.to_string_lossy().into_owned())
                .placeholder("Folder to analyze")
        });
        let preferences = Preferences::path().map(|path| Preferences::load(&path)).unwrap_or_default();
        apply_appearance(preferences.appearance, window, cx);
        let tree = cx.new(|cx| TreeState::new(cx));
        cx.observe(&tree, |this, tree, cx| {
            let id = tree.read(cx).selected_entry().map(|entry| entry.item().id.to_string());
            if this.selected_id != id {
                this.selected_id = id.clone();
                if let Some((indices, node)) = id.as_ref().and_then(|id| this.tree_nodes.get(id)) {
                    this.selected = Some(node.summary());
                    this.focus = indices.clone();
                    if !node.directory { this.focus.pop(); }
                }
            }
            cx.notify();
        }).detach();
        cx.observe_window_appearance(window, |this, window, cx| {
            if this.appearance == Appearance::System {
                apply_appearance(this.appearance, window, cx);
            }
        }).detach();
        Self::observe_theme(cx);
        // Scan only after Analyze or an explicit command-line folder.
        let mut this = Self {
            path,
            scan: None,
            focus: vec![],
            tab: 0,
            allocated: preferences.allocated,
            appearance: preferences.appearance,
            settings_open: false,
            preference_status: "Changes are saved automatically.".into(),
            tree,
            tree_nodes: Arc::new(HashMap::new()),
            selected_id: None,
            busy: false,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            status: "Choose a folder, then Analyze.".into(),
            largest_logical: vec![],
            largest_allocated: vec![],
            selected: None,
        };
        if std::env::args_os().nth(1).is_some() {
            this.analyze(cx);
        }
        this
    }
    fn observe_theme(cx: &mut Context<Self>) {
        cx.observe_global::<Theme>(|_, cx| cx.notify()).detach();
    }

    fn analyze(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let path = PathBuf::from(self.path.read(cx).value().as_ref());
        self.generation += 1;
        let generation = self.generation;
        self.busy = true;
        self.status = format!("Scanning {}…", path.display());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    scan(&path, cancel).map(|scan| {
                        let logical = largest_files(&scan.root, false);
                        let allocated = largest_files(&scan.root, true);
                        (Arc::new(scan), logical, allocated)
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                match result {
                    Ok((scan, logical, allocated)) => {
                        this.status = format!(
                            "{} unique files · {} skipped entries",
                            scan.root.files, scan.skipped
                        );
                        let (item, nodes) = build_tree(&scan.root, this.allocated);
                        this.tree_nodes = Arc::new(nodes);
                        this.selected_id = None;
                        this.tree.update(cx, |tree, cx| tree.set_items(vec![item], cx));
                        this.scan = Some(scan);
                        this.focus.clear();
                        this.selected = None;
                        this.largest_logical = logical;
                        this.largest_allocated = allocated;
                    }
                    Err(error) => this.status = error,
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn drill(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(scan) = &self.scan {
            let child = &scan.root.at(&self.focus).children[index];
            if child.directory {
                self.focus.push(index);
                self.selected = Some(child.summary());
            } else {
                self.selected = Some(child.summary());
            }
            cx.notify();
        }
    }
    fn save_preferences(&mut self) {
        let preferences = Preferences { appearance: self.appearance, allocated: self.allocated };
        self.preference_status = match Preferences::path() {
            Some(path) => match preferences.save(&path) {
                Ok(()) => "Changes saved.".into(),
                Err(error) => format!("Applied for this session. Could not save settings: {error}"),
            },
            None => "Applied for this session. A settings folder is unavailable.".into(),
        };
    }

    fn set_metric(&mut self, allocated: bool, cx: &mut Context<Self>) {
        self.allocated = allocated;
        // Retain shared expansion state and selection while reordering siblings.
        let nodes = self.tree_nodes.clone();
        self.tree.update(cx, |tree, cx| {
            if let Some(entry) = tree.entry(0) {
                let mut root = entry.item().clone();
                let selected = tree.selected_item().map(|item| TreeItem::new(item.id.clone(), item.label.clone()));
                sort_tree(&mut root, allocated, &nodes);
                tree.set_items(vec![root], cx);
                tree.set_selected_item(selected.as_ref(), cx);
            }
        });
        self.save_preferences();
        cx.notify();
    }

    #[gpui]
    fn tree_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.scan.is_none() { return self.empty_state(cx); }
        let nodes = self.tree_nodes.clone();
        let allocated = self.allocated;
        let total = self.scan.as_ref().unwrap().root.weight(allocated);
        let rows = tree(&self.tree, move |ix, entry, selected, _, cx| {
            let node = &nodes[entry.item().id.as_ref()].1;
            let fraction = if total == 0 { 0. } else { node.weight(allocated) as f32 / total as f32 };
            let disclosure = if entry.is_folder() {
                Icon::new(if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight }).small().into_any_element()
            } else { <div w={px(16.)} />.into_any_element() };
            let icon = if node.directory { if entry.is_expanded() { IconName::FolderOpen } else { IconName::Folder } } else { IconName::File };
            ListItem::new(ix).selected(selected).h(px(38.)).px_3().text_sm().font_normal()
                .accessibility_label(format!("{}, {}, {:.1}% of scan", node.name(), support::bytes(node.weight(allocated)), fraction * 100.))
                .child(
                    <div flex items-center gap-3 w-full>
                        <div flex flex-1 min-w-0 items-center gap-2 pl={px(entry.depth() as f32 * 18.)}>
                            {disclosure}<Icon args={icon} small text-color={cx.theme().muted_foreground} />
                            <div min-w-0 truncate>{entry.item().label.clone()}</div>
                        </div>
                        <div w={px(84.)} text-right text-sm>{support::bytes(node.weight(allocated))}</div>
                        <div w={px(108.)} flex items-center gap-2>
                            <div relative h={px(4.)} w={px(48.)} rounded-full overflow-hidden bg={cx.theme().muted}>
                                <div absolute left-0 top-0 h-full w={relative(fraction)} bg={cx.theme().primary} />
                            </div>
                            <div flex-1 text-right text-xs text-color={cx.theme().muted_foreground}>{format!("{:.1}%",fraction*100.)}</div>
                        </div>
                    </div>
                )
        });
        <div flex flex-col size-full min-h-0>
            <div flex items-center gap-3 px-3 py-2 border-b-1 border-color={cx.theme().border} text-xs text-color={cx.theme().muted_foreground}>
                <div flex-1>NAME</div><div w={px(84.)} text-right>SIZE</div><div w={px(108.)} text-right>% OF SCAN</div>
            </div>
            <div flex-1 min-h-0>{rows}</div>
            <div px-4 py-2 text-xs text-color={cx.theme().muted_foreground}>Expand folders to compare usage. Navigate with the arrow keys.</div>
        </div>.into_any_element()
    }

    #[gpui]
    fn empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        <div flex flex-col size-full items-center justify-center gap-3 p-6>
            <Icon args={IconName::FolderOpen} size={px(36.)} text-color={cx.theme().muted_foreground} />
            <div text-lg font-semibold>{if self.busy {"Analyzing your folder"}else{"See where your space goes"}}</div>
            <div text-sm text-color={cx.theme().muted_foreground}>{if self.busy {"Your folder tree will appear when the scan completes."}else{"Choose a folder above, then click Analyze."}}</div>
        </div>.into_any_element()
    }

    #[gpui]
    fn appearance_controls(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let buttons = [(Appearance::System, "System", assets::IconName::Monitor), (Appearance::Light, "Light", assets::IconName::Sun), (Appearance::Dark, "Dark", assets::IconName::Moon)]
            .into_iter().map(|(mode, label, icon)| {
                <Button args={label} label={label} icon={icon} selected={self.appearance==mode}
                    on-click={cx.listener(move |this, _, window, cx| {
                        this.appearance = mode;
                        apply_appearance(mode, window, cx);
                        this.save_preferences();
                        cx.notify();
                    })} />
            }).collect::<Vec<_>>();
        <div flex gap-2 children={buttons} />.into_any_element()
    }

    #[gpui]
    fn settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.entity();
        let metric_owner = owner.clone();
        let metric_setter = owner.clone();
        let settings = Settings::new("explorer-settings").sidebar_width(px(180.))
            .page(SettingPage::new("General").default_open(true)
                .group(SettingGroup::new().title("Appearance").item(
                    SettingItem::new("Color mode", SettingField::render(move |_, _, cx| {
                        owner.update(cx, |this, cx| this.appearance_controls(cx))
                    })).description("Choose a light or dark interface, or follow your system appearance.")
                ))
                .group(SettingGroup::new().title("Storage analysis").item(
                    SettingItem::new("Use allocated size", SettingField::switch(
                        move |cx| metric_owner.read(cx).allocated,
                        move |value, cx| metric_setter.update(cx, |this, cx| {
                            this.set_metric(value, cx);
                        }),
                    )).description("Count disk blocks instead of file length. On non-Unix systems, this uses file length.")
                )));
        <div flex flex-col size-full>
            <div px-6 pt-5 pb-3><div text-xl font-semibold>Settings</div><div mt-1 text-sm text-color={cx.theme().muted_foreground}>Make Disk Explorer feel at home.</div></div>
            <div flex-1 min-h-0>{settings}</div>
            <div px-6 py-3 text-xs text-color={cx.theme().muted_foreground}>{self.preference_status.clone()}</div>
        </div>.into_any_element()
    }

    #[gpui]
    fn map(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(scan) = &self.scan else {
            return self.empty_state(cx);
        };
        let node = scan.root.at(&self.focus);
        let count = node.children.len().min(64);
        let mut cells = node
            .children
            .iter()
            .take(count)
            .map(|n| Node {
                path: n.path.clone(),
                directory: n.directory,
                logical: n.logical,
                allocated: n.allocated,
                files: n.files,
                children: vec![],
            })
            .collect::<Vec<_>>();
        if node.children.len() > count {
            cells.push(Node {
                path: format!("Other {} entries", node.children.len() - count).into(),
                directory: false,
                logical: node.children[count..].iter().map(|n| n.logical).sum(),
                allocated: node.children[count..].iter().map(|n| n.allocated).sum(),
                files: 0,
                children: vec![],
            });
        }
        let colors = [0x2563eb, 0x0d9488, 0x7c3aed, 0xd97706, 0xdb2777, 0x4f46e5];
        let rectangles = treemap(&cells, self.allocated);
        let content = rectangles
            .into_iter()
            .map(|rect| {
                let node = &cells[rect.index];
                let label = format!(
                    "{} · {}",
                    node.name(),
                    support::bytes(node.weight(self.allocated))
                );
                let index = rect.index;
                let show_label = rect.w > 0.13 && rect.h > 0.09;
                <div id={("map-cell",index)} absolute left={relative(rect.x)} top={relative(rect.y)} w={relative(rect.w)} h={relative(rect.h)}
                    bg={rgb(colors[index%colors.len()])} text-color={rgb(0xffffff)} border-1 border-color={cx.theme().background} overflow-hidden cursor-pointer
                    on-click={cx.listener(move|this,_,_,cx| {if index<count {this.drill(index,cx);}else {this.status="Other entries are grouped; use the tree view to explore them.".into();cx.notify();}})}>
                    {if show_label {<div p-2>{label}</div>.into_any_element()}else{<div />.into_any_element()}}
                </div>
            })
            .collect::<Vec<_>>();
        <div id="treemap-scroll" flex flex-col p-4 gap-3 size-full overflow-y-scroll>
            <div flex items-center gap-2>
                <Button args={"up"} icon={IconName::ArrowUp} label="Up" small disabled={self.focus.is_empty()} on-click={cx.listener(|this,_,_,cx|{this.focus.pop();cx.notify();})} />
                <Button args={"root"} label="Scan root" small on-click={cx.listener(|this,_,_,cx|{this.focus.clear();cx.notify();})} />
                <div flex-1 min-w-0 truncate text-sm>{format!("{} · {}",node.path.display(),support::bytes(node.weight(self.allocated)))}</div>
            </div>
            <div relative w-full h={px(420.)} rounded-lg overflow-hidden children={content} />
            <div text-xs>Click a folder to drill down. Each tile’s area represents its share of storage.</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn largest(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.scan.is_none() { return self.empty_state(cx); }
        let files = if self.allocated {
            self.largest_allocated.clone()
        } else {
            self.largest_logical.clone()
        };
        let owner = cx.entity();
        let allocated = self.allocated;
        <uniform_list args={("largest-files",files.len(),move|range,_,cx|owner.update(cx,|_,cx|range.map(|index| {
            let node=files[index].clone();let label=format!("{}   {}",support::bytes(node.weight(allocated)),node.path.display());
            <div id={("largest",index)} h={px(36.)} px-3 min-w-0 truncate cursor-pointer on-click={cx.listener(move|this,_,_,cx|{this.selected=Some(node.summary());cx.notify();})}>{label}</div>.into_any_element()
        }).collect::<Vec<_>>()))} size-full />.into_any_element()
    }
}
fn apply_appearance(appearance: Appearance, window: &mut Window, cx: &mut App) {
    match appearance {
        Appearance::System => Theme::sync_system_appearance(Some(window), cx),
        Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
}

// Stable index ids avoid lossy path collisions and keep metadata out of the tree state.
fn build_tree(root: &Node, allocated: bool) -> (TreeItem, HashMap<String, (Vec<usize>, Node)>) {
    fn visit(node: &Node, indices: Vec<usize>, allocated: bool, nodes: &mut HashMap<String, (Vec<usize>, Node)>) -> TreeItem {
        let id = format!("root{}", indices.iter().map(|i| format!("/{i}")).collect::<String>());
        let expanded = indices.is_empty();
        nodes.insert(id.clone(), (indices.clone(), node.summary()));
        let mut order = (0..node.children.len()).collect::<Vec<_>>();
        order.sort_by(|&a, &b| node.children[b].weight(allocated).cmp(&node.children[a].weight(allocated)).then_with(|| node.children[a].path.cmp(&node.children[b].path)));
        let children = order.into_iter().map(|i| {
            let mut child_indices = indices.clone();
            child_indices.push(i);
            visit(&node.children[i], child_indices, allocated, nodes)
        }).collect::<Vec<_>>();
        TreeItem::new(id, node.name()).children(children).expanded(expanded)
    }
    let mut nodes = HashMap::new();
    let item = visit(root, vec![], allocated, &mut nodes);
    (item, nodes)
}

fn sort_tree(item: &mut TreeItem, allocated: bool, nodes: &HashMap<String, (Vec<usize>, Node)>) {
    item.children.sort_by(|a, b| {
        let left = &nodes[a.id.as_ref()].1;
        let right = &nodes[b.id.as_ref()].1;
        right.weight(allocated).cmp(&left.weight(allocated)).then_with(|| left.path.cmp(&right.path))
    });
    for child in &mut item.children { sort_tree(child, allocated, nodes); }
}

impl Render for Explorer {
    #[gpui]
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scan_data = self.scan.clone();
        let scan = scan_data.as_ref();
        let total = scan.map(|s| support::bytes(s.root.weight(self.allocated))).unwrap_or_else(|| "—".into());
        let files = scan.map(|s| s.root.files.to_string()).unwrap_or_else(|| "—".into());
        let skipped = scan.map(|s| s.skipped.to_string()).unwrap_or_else(|| "—".into());
        let body = if self.settings_open { self.settings(cx) } else {
            let analysis = match self.tab {
                0 => self.tree_view(cx),
                1 => self.map(cx),
                2 => self.largest(cx),
                _ if scan.is_none() => self.empty_state(cx),
                _ => {
                    let warnings = scan.map(|s| s.warnings.clone()).unwrap_or_default();
                    let rows = warnings.into_iter().map(|warning| <div py-2 border-b-1 border-color={cx.theme().border} text-sm>{warning}</div>).collect::<Vec<_>>();
                    <div id="scan-report" size-full overflow-y-scroll p-5 flex flex-col gap-3>
                        <div text-lg font-semibold>Scan report</div><div text-sm>{self.status.clone()}</div>
                        <div text-sm text-color={cx.theme().muted_foreground}>Regular files are counted on the starting filesystem. Symbolic links and special files are skipped; hard links count once on Unix. Permission errors produce a partial result.</div>
                        <div flex flex-col children={rows} />
                    </div>.into_any_element()
                }
            };
            let details = if let Some(node) = &self.selected {
                let fraction = scan.map(|s| { let total = s.root.weight(self.allocated); if total == 0 {0.} else {node.weight(self.allocated) as f64 / total as f64 * 100.} }).unwrap_or(0.);
                <div id="inspector" flex flex-col gap-4 p-4 size-full overflow-y-scroll whitespace-normal>
                    <div text-xs text-color={cx.theme().muted_foreground}>SELECTION</div>
                    <Icon args={if node.directory {IconName::FolderOpen}else{IconName::File}} size={px(28.)} text-color={cx.theme().primary} />
                    <div font-semibold>{node.name()}</div>
                    <div min-w-0 w-full text-xs whitespace-normal text-color={cx.theme().muted_foreground}>{node.path.to_string_lossy().into_owned()}</div>
                    <div flex flex-col gap-3 border-t-1 border-color={cx.theme().border} pt-4>
                        <div text-xs text-color={cx.theme().muted_foreground}>FILE SIZE</div><div>{support::bytes(node.logical)}</div>
                        <div text-xs text-color={cx.theme().muted_foreground}>SIZE ON DISK</div><div>{support::bytes(node.allocated)}</div>
                        <div text-xs text-color={cx.theme().muted_foreground}>SHARE OF SCAN</div><div>{format!("{fraction:.1}%")}</div>
                        {if node.directory {<div text-sm>{format!("{} files", node.files)}</div>.into_any_element()}else{<div />.into_any_element()}}
                    </div>
                    <Button args={"reveal"} label="Show in folder" icon={IconName::FolderOpen} small on-click={{let path=node.path.clone();move|_,_,cx|cx.reveal_path(&path)}} />
                </div>.into_any_element()
            } else {
                <div flex flex-col gap-3 p-4 w-full whitespace-normal>
                    <div text-xs text-color={cx.theme().muted_foreground}>SELECTION</div>
                    <Icon args={IconName::File} size={px(28.)} text-color={cx.theme().muted_foreground} />
                    <div text-sm font-semibold>Look a little closer</div>
                    <div text-sm text-color={cx.theme().muted_foreground}>Select a file or folder to inspect its size and location.</div>
                </div>.into_any_element()
            };
            <div flex flex-col size-full min-h-0>
                <div flex gap-6 px-5 py-4 border-b-1 border-color={cx.theme().border}>
                    <div flex-1><div text-xs text-color={cx.theme().muted_foreground}>{if self.allocated {"SIZE ON DISK"}else{"TOTAL FILE SIZE"}}</div><div mt-1 text-2xl font-semibold>{total}</div></div>
                    <div flex-1><div text-xs text-color={cx.theme().muted_foreground}>FILES</div><div mt-1 text-2xl font-semibold>{files}</div></div>
                    <div flex-1><div text-xs text-color={cx.theme().muted_foreground}>SKIPPED</div><div mt-1 text-2xl font-semibold>{skipped}</div></div>
                </div>
                <div flex-1 min-h-0><h_resizable args={"analysis-panes"}>
                    <resizable_panel><div flex flex-col size-full min-h-0>
                        <TabBar args={"analysis-tabs"} selected-index={self.tab} on-click={cx.listener(|this,index,_,cx|{this.tab=*index;cx.notify();})}>
                            <Tab label="Tree view" /><Tab label="Treemap" /><Tab label="Largest files" /><Tab label="Scan report" />
                        </TabBar>
                        <div flex-1 min-h-0>{analysis}</div>
                    </div></resizable_panel>
                    <resizable_panel size={px(240.)} size-range={px(190.)..px(380.)}>{details}</resizable_panel>
                </h_resizable></div>
            </div>.into_any_element()
        };
        <div flex flex-col size-full text-sm font-normal bg={cx.theme().background} text-color={cx.theme().foreground} overflow-hidden>
            <div flex items-center gap-3 px-5 py-4 border-b-1 border-color={cx.theme().border}>
                <Icon args={IconName::FolderOpen} text-color={cx.theme().primary} />
                <div font-semibold>Disk Explorer</div><div flex-1 />
                <Button args={"analysis-page"} label="Analysis" ghost selected={!self.settings_open} on-click={cx.listener(|this,_,_,cx|{this.settings_open=false;cx.notify();})} />
                <Button args={"settings-page"} label="Settings" icon={IconName::Settings} ghost selected={self.settings_open} on-click={cx.listener(|this,_,_,cx|{this.settings_open=true;cx.notify();})} />
            </div>
            {if !self.settings_open {
                <div flex items-center gap-2 px-5 py-3 border-b-1 border-color={cx.theme().border}>
                    <div flex-1 min-w-0><Input args={&self.path} /></div>
                    <Button args={"choose"} label="Browse…" disabled={self.busy} on-click={cx.listener(|_,_,window,cx|{
                        let choice=cx.prompt_for_paths(PathPromptOptions{files:false,directories:true,multiple:false,prompt:Some("Analyze folder".into())});
                        cx.spawn_in(window,async move|this,cx|{if let Ok(Ok(Some(paths)))=choice.await {if let Some(path)=paths.first(){let path=path.to_string_lossy().into_owned();let _=this.update_in(cx,|this,window,cx|this.path.update(cx,|input,cx|input.set_value(path,window,cx)));}}}).detach();
                    })} />
                    <Button args={"scan"} label="Analyze" primary disabled={self.busy} on-click={cx.listener(|this,_,_,cx|this.analyze(cx))} />
                    {if self.busy {<Button args={"cancel"} label="Cancel" on-click={cx.listener(|this,_,_,cx|{this.cancel.store(true,Ordering::Relaxed);this.status="Cancelling scan…".into();cx.notify();})} />.into_any_element()}else{<div />.into_any_element()}}
                </div>.into_any_element()
            } else {<div />.into_any_element()}}
            <div flex-1 min-h-0>{body}</div>
            <div flex items-center gap-2 px-5 py-2 border-t-1 border-color={cx.theme().border} text-xs text-color={cx.theme().muted_foreground}>
                {if self.busy {<Spinner small />.into_any_element()}else{<div />.into_any_element()}}
                <div flex-1 min-w-0 truncate>{self.status.clone()}</div>
                <div>{if self.allocated {"Allocated size"}else{"Logical size"}}</div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{Node, build_tree, sort_tree};

    #[test]
    fn tree_reorders_by_metric_without_losing_identity_or_expansion() {
        let root = Node { path: "/scan".into(), directory: true, logical: 100, allocated: 200, files: 2,
            children: vec![
                Node { path: "/scan/a".into(), directory: true, logical: 80, allocated: 20, files: 1,
                    children: vec![Node {path: "/scan/a/file".into(), directory: false, logical: 80, allocated: 20, files: 1, children: vec![]}] },
                Node { path: "/scan/b".into(), directory: false, logical: 20, allocated: 180, files: 1, children: vec![] },
            ] };
        let (mut tree, nodes) = build_tree(&root, false);
        assert_eq!(tree.children[0].id.as_ref(), "root/0");
        tree.children[0] = tree.children[0].clone().expanded(true);
        sort_tree(&mut tree, true, &nodes);
        assert_eq!(tree.children[0].id.as_ref(), "root/1");
        assert!(tree.children[1].is_expanded());
        for (indices, node) in nodes.values() {
            assert_eq!(root.at(indices).path, node.path);
            assert!(node.children.is_empty());
        }
    }
}
