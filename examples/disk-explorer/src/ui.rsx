use gpui_kit::{prelude::*, *};
use gpui_kit::component::{
    ActiveTheme as _, StyledExt as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    tab::{Tab, TabBar},
    resizable::{h_resizable, resizable_panel},
    spinner::Spinner,
};
use rsx_disk_explorer::{Node, Scan, scan, treemap, largest_files};
use crate::support;
use std::{
    path::PathBuf,
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
        // Scan only after the user clicks Analyze, so opening the example is cheap.
        let mut this = Self {
            path,
            scan: None,
            focus: vec![],
            tab: 0,
            allocated: false,
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
                self.selected = None;
            } else {
                self.selected = Some(child.clone());
            }
            cx.notify();
        }
    }
    #[gpui]
    fn directory_row(
        &mut self,
        index: usize,
        name: String,
        size: u64,
        directory: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        <div id={("disk-row",index)} flex items-center gap-2 px-3 h={px(36.)} cursor-pointer
            on-click={cx.listener(move|this,_,_,cx|this.drill(index,cx))}>
            <div flex-1 min-w-0 truncate>{format!("{} {name}",if directory {"▸"}else{"·"})}</div><div text-xs>{support::bytes(size)}</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn map(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(scan) = &self.scan else {
            return <div p-6>Analyze a folder to see how its files occupy space.</div>.into_any_element();
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
                    on-click={cx.listener(move|this,_,_,cx| {if index<count {this.drill(index,cx);}else {this.status="Other entries are grouped; use the directory list to explore them.".into();cx.notify();}})}>
                    {if show_label {<div p-2>{label}</div>.into_any_element()}else{<div />.into_any_element()}}
                </div>
            })
            .collect::<Vec<_>>();
        <div id="treemap-scroll" flex flex-col p-4 gap-3 size-full overflow-y-scroll>
            <div>{format!("{} · {}",node.path.display(),support::bytes(node.weight(self.allocated)))}</div>
            <div relative w-full h={px(420.)} overflow-hidden children={content} />
            <div text-xs>Click a folder to drill down. Use Up to return. Colors separate entries; area represents bytes.</div>
        </div>.into_any_element()
    }
    #[gpui]
    fn largest(&self, cx: &mut Context<Self>) -> AnyElement {
        let files = if self.allocated {
            self.largest_allocated.clone()
        } else {
            self.largest_logical.clone()
        };
        let owner = cx.entity();
        let allocated = self.allocated;
        <uniform_list args={("largest-files",files.len(),move|range,_,cx|owner.update(cx,|_,cx|range.map(|index| {
            let node=files[index].clone();let label=format!("{}   {}",support::bytes(node.weight(allocated)),node.path.display());
            <div id={("largest",index)} h={px(36.)} px-3 min-w-0 truncate cursor-pointer on-click={cx.listener(move|this,_,_,cx|{this.selected=Some(node.clone());cx.notify();})}>{label}</div>.into_any_element()
        }).collect::<Vec<_>>()))} size-full />.into_any_element()
    }
}
impl Render for Explorer {
    #[gpui]
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let rows = self
            .scan
            .as_ref()
            .map(|scan| {
                scan.root
                    .at(&self.focus)
                    .children
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (i, n.name(), n.weight(self.allocated), n.directory))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let list = <uniform_list args={("directories",rows.len(),move|range,_,cx|owner.update(cx,|this,cx|range.map(|i|{let (index,name,size,directory)=&rows[i];this.directory_row(*index,name.clone(),*size,*directory,cx)}).collect::<Vec<_>>()))} size-full />;
        let total = self
            .scan
            .as_ref()
            .map(|scan| {
                format!(
                    "{} · {} files",
                    support::bytes(scan.root.weight(self.allocated)),
                    scan.root.files
                )
            })
            .unwrap_or_else(|| "No scan yet".into());
        let body = match self.tab {
            0 => self.map(cx),
            1 => self.largest(cx),
            _ => {
                let warnings = self
                    .scan
                    .as_ref()
                    .map(|scan| scan.warnings.join("\n"))
                    .unwrap_or_default();
                <div id="scan-report" size-full overflow-y-scroll p-4>
                    <div font-semibold>Scan report</div><div>{self.status.clone()}</div>
                    <div>Scans count regular file bytes, stay on one filesystem, and skip symbolic links and special files. Hard links count once on Unix. Permission errors produce a partial result.</div>
                    <div>{warnings}</div>
                </div>.into_any_element()
            }
        };
        let details = if let Some(node) = &self.selected {
            <div flex flex-col gap-3 p-4><div font-semibold>{node.name()}</div><div>{node.path.to_string_lossy().into_owned()}</div>
                <div>{format!("Logical: {}",support::bytes(node.logical))}</div><div>{format!("Allocated: {}",support::bytes(node.allocated))}</div>
                <Button args={"reveal"} label="Reveal file" on-click={{let path=node.path.clone();move|_,_,cx|cx.reveal_path(&path)}} />
            </div>.into_any_element()
        } else {
            <div p-4>Select a file to inspect it.</div>.into_any_element()
        };
        <div flex flex-col size-full bg={cx.theme().background} text-color={cx.theme().foreground} overflow-hidden>
            <div flex items-center gap-3 p-3><div text-xl font-semibold flex-1>Disk space explorer</div><div>{total}</div></div>
            <div flex gap-2 p-3><div flex-1><Input args={&self.path} /></div>
                <Button args={"choose"} label="Choose folder…" disabled={self.busy} on-click={cx.listener(|_,_,window,cx|{
                    let choice=cx.prompt_for_paths(PathPromptOptions{files:false,directories:true,multiple:false,prompt:Some("Analyze folder".into())});
                    cx.spawn_in(window,async move|this,cx|{if let Ok(Ok(Some(paths)))=choice.await {if let Some(path)=paths.first(){let path=path.to_string_lossy().into_owned();let _=this.update_in(cx,|this,window,cx|this.path.update(cx,|input,cx|input.set_value(path,window,cx)));}}}).detach();
                })} />
                <Button args={"scan"} label="Analyze" primary disabled={self.busy} on-click={cx.listener(|this,_,_,cx|this.analyze(cx))} />
                <Button args={"cancel"} label="Cancel scan" disabled={!self.busy} on-click={cx.listener(|this,_,_,cx|{this.cancel.store(true,Ordering::Relaxed);this.status="Cancelling scan…".into();cx.notify();})} />
            </div>
            <div flex gap-2 items-center p-3>
                <Button args={"up"} label="Up" disabled={self.focus.is_empty()} on-click={cx.listener(|this,_,_,cx|{this.focus.pop();cx.notify();})} />
                <Button args={"root"} label="Scan root" on-click={cx.listener(|this,_,_,cx|{this.focus.clear();cx.notify();})} />
                <Button args={"metric"} label={if self.allocated {"Using allocated bytes"}else{"Using logical bytes"}} on-click={cx.listener(|this,_,_,cx|{this.allocated=!this.allocated;cx.notify();})} />
                {if self.busy {<Spinner />.into_any_element()}else{<div />.into_any_element()}}
            </div>
            <div flex-1 min-h-0><h_resizable args={"disk-panes"}>
                <resizable_panel size={px(280.)} size-range={px(180.)..px(480.)}>{list}</resizable_panel>
                <resizable_panel><div flex flex-col size-full min-h-0>
                    <TabBar args={"analysis-tabs"} selected-index={self.tab} on-click={cx.listener(|this,index,_,cx|{this.tab=*index;cx.notify();})}><Tab label="Breakdown" /><Tab label="Largest files" /><Tab label="Scan report" /></TabBar>
                    <div flex-1 min-h-0>{body}</div>
                </div></resizable_panel>
                <resizable_panel size={px(230.)} size-range={px(180.)..px(400.)}>{details}</resizable_panel>
            </h_resizable></div>
            <div p-3 text-xs>{self.status.clone()}</div>
        </div>
    }
}
