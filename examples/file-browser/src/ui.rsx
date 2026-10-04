use crate::support;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, StyledExt as _,
    button::Button,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    tab::{Tab, TabBar},
};
use gpui_kit::{prelude::*, *};
use rsx_file_browser::{Entry, History, Preview, preview, read_directory};
use std::path::PathBuf;

struct FolderTab {
    id: usize,
    generation: u64,
    history: History,
    entries: Vec<Entry>,
    selected: Option<Entry>,
    preview: Option<Result<Preview, String>>,
    loading: bool,
    status: String,
}
pub struct Browser {
    tabs: Vec<FolderTab>,
    active: usize,
    next_id: usize,
    detail_tab: usize,
    address: Entity<InputState>,
    search: Entity<InputState>,
    query: String,
    hidden: bool,
    _subscriptions: Vec<Subscription>,
}
impl Browser {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let address =
            cx.new(|cx| InputState::new(window, cx).placeholder("Folder path — press Enter"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter files…"));
        let subscriptions = vec![
            cx.subscribe(&search, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = input.read(cx).value().to_lowercase();
                    cx.notify();
                }
            }),
            cx.subscribe_in(
                &address,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        let path = PathBuf::from(input.read(cx).value().as_ref());
                        this.navigate(path, window, cx);
                    }
                },
            ),
        ];
        let mut this = Self {
            tabs: vec![],
            active: 0,
            next_id: 0,
            detail_tab: 0,
            address,
            search,
            query: String::new(),
            hidden: false,
            _subscriptions: subscriptions,
        };
        this.add_tab(support::initial_directory(), window, cx);
        this
    }
    fn add_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(FolderTab {
            id,
            generation: 0,
            history: History::new(path),
            entries: vec![],
            selected: None,
            preview: None,
            loading: false,
            status: String::new(),
        });
        self.active = self.tabs.len() - 1;
        self.refresh(window, cx);
    }
    fn navigate(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let path = if path.is_absolute() {
            path
        } else {
            self.tabs[self.active].history.current().join(path)
        };
        self.tabs[self.active].history.navigate(path);
        self.refresh(window, cx);
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active];
        tab.generation += 1;
        tab.loading = true;
        tab.selected = None;
        tab.preview = None;
        let generation = tab.generation;
        let id = tab.id;
        let path = tab.history.current().to_owned();
        self.address.update(cx, |state, cx| {
            state.set_value(path.to_string_lossy().into_owned(), window, cx)
        });
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { read_directory(&path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.id == id && tab.generation == generation)
                {
                    tab.loading = false;
                    match result {
                        Ok((entries, unreadable)) => {
                            tab.status = format!(
                                "{} items · {unreadable} unreadable entries",
                                entries.len()
                            );
                            tab.entries = entries;
                        }
                        Err(error) => {
                            tab.entries.clear();
                            tab.status = error;
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    fn select(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        if entry.is_dir {
            self.navigate(entry.path, window, cx);
            return;
        }
        let tab = &mut self.tabs[self.active];
        let id = tab.id;
        let path = entry.path.clone();
        let generation = tab.generation;
        tab.selected = Some(entry);
        tab.preview = None;
        cx.spawn(async move |this, cx| {
            let selected_path = path.clone();
            let result = cx.background_spawn(async move { preview(&path) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.tabs.iter_mut().find(|tab| {
                    tab.id == id
                        && tab.generation == generation
                        && tab
                            .selected
                            .as_ref()
                            .is_some_and(|e| e.path == selected_path)
                }) {
                    tab.preview = Some(result);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }
    #[gpui]
    fn row(&mut self, entry: Entry, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.tabs[self.active]
            .selected
            .as_ref()
            .is_some_and(|e| e.path == entry.path);
        let icon = if entry.is_dir {
            IconName::Folder
        } else {
            IconName::FileText
        };
        let name = entry.name.clone();
        let size = if entry.is_dir {
            "Folder".into()
        } else {
            support::bytes(entry.bytes)
        };
        <div id={("file", index)} h={px(36.)} w-full flex items-center gap-3 px-3 cursor-pointer
            bg={if selected { cx.theme().accent } else { cx.theme().background }}
            on-click={cx.listener(move |this, _, window, cx| this.select(entry.clone(), window, cx))}>
            <Icon args={icon} size-4 />
            <div flex-1 min-w-0 truncate>{name}</div><div text-xs>{size}</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn details(&self, cx: &mut Context<Self>) -> AnyElement {
        let tab = &self.tabs[self.active];
        let content = if let Some(entry) = &tab.selected {
            if self.detail_tab == 1 {
                <div flex flex-col gap-3>
                    <div font-semibold>{entry.name.clone()}</div>
                    <div>{entry.path.to_string_lossy().into_owned()}</div>
                    <div>{format!("Size: {}", support::bytes(entry.bytes))}</div>
                    <div>{format!("Symbolic link: {}", entry.is_symlink)}</div>
                    <div>{format!("Modified: {}", entry.modified.map(chrono::DateTime::<chrono::Local>::from).map(|time| time.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "Unavailable".into()))}</div>
                    <Button args={"open-system"} label="Open with default app" on-click={
                        { let path = entry.path.clone(); move |_, _, cx| cx.open_with_system(&path) }
                    } />
                </div>.into_any_element()
            } else {
                match &tab.preview {
                    Some(Ok(Preview::Image(path))) => <img args={path.clone()} w={px(300.)} h={px(300.)} min-w-0 min-h-0 max-w-full object-fit={ObjectFit::Contain} />.into_any_element(),
                    Some(Ok(Preview::Text(text))) | Some(Ok(Preview::Binary(text))) => {
                        <div text-sm font-family="monospace">{text.clone()}</div>.into_any_element()
                    }
                    Some(Err(error)) => <div>{error.clone()}</div>.into_any_element(),
                    None => <div>Loading preview…</div>.into_any_element(),
                }
            }
        } else {
            <div text-color={cx.theme().muted_foreground}>Choose a file to preview. Click a folder to browse it.</div>.into_any_element()
        };
        <div flex flex-col size-full min-h-0>
            <TabBar args={"preview-tabs"} selected-index={self.detail_tab} on-click={cx.listener(|this, index, _, cx| { this.detail_tab = *index; cx.notify(); })}>
                <Tab label="Preview" /><Tab label="Details" />
            </TabBar>
            <div id="preview-scroll" flex-1 min-h-0 overflow-y-scroll p-4>{content}</div>
        </div>.into_any_element()
    }
}
impl Render for Browser {
    #[gpui]
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = &self.tabs[self.active];
        let path = tab.history.current().to_owned();
        let can_back = tab.history.index > 0;
        let can_forward = tab.history.index + 1 < tab.history.paths.len();
        let status = if tab.loading {
            "Reading folder…".into()
        } else {
            tab.status.clone()
        };
        let entries = tab
            .entries
            .iter()
            .filter(|e| {
                (self.hidden || !e.name.starts_with('.'))
                    && e.name.to_lowercase().contains(&self.query)
            })
            .cloned()
            .collect::<Vec<_>>();
        let count = entries.len();
        let owner = cx.entity();
        let rows = <uniform_list args={("files", count, move |range, _, cx| owner.update(cx, |this, cx| range.map(|index| this.row(entries[index].clone(), index, cx)).collect::<Vec<_>>()))} flex-1 min-h-0 />;
        let tabs = self
            .tabs
            .iter()
            .map(|tab| <Tab label={tab.history.current().file_name().unwrap_or(tab.history.current().as_os_str()).to_string_lossy().into_owned()} />)
            .collect::<Vec<_>>();
        let shortcuts = [
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| path.clone()),
            std::env::current_dir().unwrap_or_else(|_| path.clone()),
            PathBuf::from("/"),
        ];
        <div flex flex-col size-full overflow-hidden bg={cx.theme().background} text-color={cx.theme().foreground}>
            <div flex items-center gap-2 p-3>
                <div text-xl font-semibold flex-1>File browser</div>
                <Button args={"new-tab"} label="New tab" on-click={cx.listener(move |this, _, window, cx| this.add_tab(path.clone(), window, cx))} />
                <Button args={"close-tab"} label="Close tab" disabled={self.tabs.len() == 1} on-click={cx.listener(|this, _, window, cx| { this.tabs.remove(this.active); this.active = this.active.min(this.tabs.len() - 1); this.refresh(window, cx); })} />
                <Button args={"folder-picker"} label="Choose folder…" on-click={cx.listener(|_, _, window, cx| {
                    let choice = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Browse folder".into()) });
                    cx.spawn_in(window, async move |this, cx| { if let Ok(Ok(Some(paths))) = choice.await { if let Some(path) = paths.into_iter().next() { let _ = this.update_in(cx, |this, window, cx| this.navigate(path, window, cx)); } } }).detach();
                })} />
            </div>
            <TabBar args={"folders"} selected-index={self.active} children={tabs}
                on-click={cx.listener(|this, index, window, cx| { this.active = *index; this.refresh(window, cx); })} />
            <div flex items-center gap-2 p-3>
                <Button args={"back"} label="Back" disabled={!can_back} on-click={cx.listener(|this, _, window, cx| { this.tabs[this.active].history.back(); this.refresh(window, cx); })} />
                <Button args={"forward"} label="Forward" disabled={!can_forward} on-click={cx.listener(|this, _, window, cx| { this.tabs[this.active].history.forward(); this.refresh(window, cx); })} />
                <Button args={"up"} label="Up" on-click={cx.listener(|this, _, window, cx| { if let Some(parent) = this.tabs[this.active].history.current().parent() { this.navigate(parent.to_owned(), window, cx); } })} />
                <div flex-1><Input args={&self.address} /></div>
                <Button args={"refresh"} label="Refresh" on-click={cx.listener(|this, _, window, cx| this.refresh(window, cx))} />
            </div>
            <div flex-1 min-h-0><h_resizable args={"browser-panes"}>
                <resizable_panel size={px(180.)} size-range={px(140.)..px(340.)}>
                    <div flex flex-col gap-2 p-3>
                        <div font-semibold>Places</div>
                        {shortcuts.into_iter().enumerate().map(|(index, path)| <Button args={format!("place-{index}")} label={["Home", "Project", "Filesystem"][index]}
                            on-click={cx.listener(move |this, _, window, cx| this.navigate(path.clone(), window, cx))} />).collect::<Vec<_>>().into_iter().fold(div().flex().flex_col().gap_2(), |parent, child| parent.child(child))}
                        <Button args={"hidden"} label={if self.hidden { "Hide dotfiles" } else { "Show dotfiles" }} on-click={cx.listener(|this, _, _, cx| { this.hidden = !this.hidden; cx.notify(); })} />
                    </div>
                </resizable_panel>
                <resizable_panel>
                    <div flex flex-col size-full min-h-0 p-3 gap-2><Input args={&self.search} />{rows}</div>
                </resizable_panel>
                <resizable_panel size={px(360.)} size-range={px(220.)..px(600.)}>{self.details(cx)}</resizable_panel>
            </h_resizable></div>
            <div p-3 text-xs>{format!("{status} · {count} visible")}</div>
        </div>
    }
}
