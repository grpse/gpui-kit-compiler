use gpui_kit::{prelude::*, *};
use gpui_kit::component::{
    ActiveTheme as _, StyledExt as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState, InputEvent},
    tab::{Tab, TabBar},
    resizable::{h_resizable, resizable_panel},
    progress::Progress,
};
use rsx_video_editor::{Media, Workspace, Edit, ExportEvent, probe, frame, export};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Editor {
    media: Option<Media>,
    workspace: Option<Arc<Workspace>>,
    image: Option<PathBuf>,
    start: Entity<InputState>,
    end: Entity<InputState>,
    position: Entity<InputState>,
    rotation: u16,
    width: Option<u32>,
    mute: bool,
    tab: usize,
    generation: u64,
    frame_generation: u64,
    probing: bool,
    previewing: bool,
    exporting: bool,
    progress: f32,
    status: String,
    log: String,
    output: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
    preview_cancel: Arc<AtomicBool>,
    export_cancel: Arc<AtomicBool>,
}
impl Drop for Editor {
    fn drop(&mut self) {
        self.preview_cancel.store(true, Ordering::Relaxed);
        self.export_cancel.store(true, Ordering::Relaxed);
    }
}
impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let start = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        let end = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        let position = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        let subscriptions = vec![
            cx.subscribe(&start, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&end, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        let workspace = Workspace::new();
        let status = workspace
            .as_ref()
            .map(|_| "Import a video to begin.".to_owned())
            .unwrap_or_else(|error| error.clone());
        let mut this = Self {
            media: None,
            workspace: workspace.ok().map(Arc::new),
            image: None,
            start,
            end,
            position,
            rotation: 0,
            width: None,
            mute: false,
            tab: 0,
            generation: 0,
            frame_generation: 0,
            probing: false,
            previewing: false,
            exporting: false,
            progress: 0.,
            status,
            log: String::new(),
            output: None,
            _subscriptions: subscriptions,
            preview_cancel: Arc::new(AtomicBool::new(false)),
            export_cancel: Arc::new(AtomicBool::new(false)),
        };
        if let Some(path) = std::env::args_os().nth(1) {
            this.import(path.into(), window, cx);
        }
        this
    }
    fn import(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.probing = true;
        self.media = None;
        self.image = None;
        self.preview_cancel.store(true, Ordering::Relaxed);
        self.frame_generation += 1;
        self.previewing = false;
        self.output = None;
        self.status = "Inspecting media with ffprobe…".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { probe(&path) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.generation != generation {
                    return;
                }
                this.probing = false;
                match result {
                    Ok(media) => {
                        let duration = format!("{:.3}", media.duration);
                        this.media = Some(media);
                        this.rotation = 0;
                        this.width = None;
                        this.mute = false;
                        this.start
                            .update(cx, |input, cx| input.set_value("0", window, cx));
                        this.end
                            .update(cx, |input, cx| input.set_value(duration, window, cx));
                        this.position
                            .update(cx, |input, cx| input.set_value("0", window, cx));
                        this.refresh_frame(cx);
                    }
                    Err(error) => {
                        this.status = error.clone();
                        this.log = error;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn refresh_frame(&mut self, cx: &mut Context<Self>) {
        if self.exporting {
            return;
        }
        let Some(media) = self.media.clone() else {
            return;
        };
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let time = match self.position.read(cx).value().parse::<f64>() {
            Ok(time) => time,
            Err(_) => {
                self.status = "Enter a preview time in seconds.".into();
                cx.notify();
                return;
            }
        };
        self.preview_cancel.store(true, Ordering::Relaxed);
        self.preview_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.preview_cancel.clone();
        self.frame_generation += 1;
        let generation = self.frame_generation;
        let rotation = self.rotation;
        let width = self.width;
        let output = workspace
            .path
            .join(format!("frame-{}-{generation}.png", self.generation));
        self.previewing = true;
        self.status = "Rendering preview frame…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let _workspace = workspace;
                    frame(&media, time, rotation, width, &output, cancel)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.frame_generation != generation {
                    if let Ok(path) = result {
                        let _ = std::fs::remove_file(path);
                    }
                    return;
                }
                this.previewing = false;
                match result {
                    Ok(path) => {
                        if let Some(old) = this.image.replace(path) {
                            let _ = std::fs::remove_file(old);
                        }
                        this.status = "Preview ready.".into();
                    }
                    Err(error) => {
                        this.status = error.clone();
                        this.log = error;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn seek(&mut self, delta: f64, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(media) = &self.media {
            let time = self.position.read(cx).value().parse::<f64>().unwrap_or(0.);
            let value = format!(
                "{:.3}",
                (time + delta).clamp(0., (media.duration - 0.001).max(0.))
            );
            self.position
                .update(cx, |input, cx| input.set_value(value, window, cx));
            self.refresh_frame(cx);
        }
    }
    fn edit(&self, output: PathBuf, cx: &App) -> Result<Edit, String> {
        let media = self.media.clone().ok_or("Import a video first")?;
        let start = self
            .start
            .read(cx)
            .value()
            .parse()
            .map_err(|_| "Start must be a number of seconds")?;
        let end = self
            .end
            .read(cx)
            .value()
            .parse()
            .map_err(|_| "End must be a number of seconds")?;
        let edit = Edit {
            media,
            output,
            start,
            end,
            rotation: self.rotation,
            width: self.width,
            mute: self.mute,
        };
        edit.validate()?;
        Ok(edit)
    }
    fn start_export(&mut self, output: PathBuf, cx: &mut Context<Self>) {
        let edit = match self.edit(output, cx) {
            Ok(edit) => edit,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        };
        self.preview_cancel.store(true, Ordering::Relaxed);
        self.frame_generation += 1;
        self.previewing = false;
        self.exporting = true;
        self.progress = 0.;
        self.log.clear();
        self.tab = 2;
        self.status = "Exporting H.264 / AAC MP4…".into();
        self.export_cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.export_cancel.clone();
        let (tx, rx) = async_channel::bounded(32);
        cx.spawn(async move |this, cx| {
            let work = cx.background_spawn(async move {
                let result = export(&edit, cancel, tx.clone());
                let _ = tx.send_blocking(ExportEvent::Finished(result));
            });
            while let Ok(event) = rx.recv().await {
                let finished = matches!(event, ExportEvent::Finished(_));
                if this
                    .update(cx, |this, cx| {
                        match event {
                            ExportEvent::Progress(value) => this.progress = value,
                            ExportEvent::Finished(result) => {
                                this.exporting = false;
                                match result {
                                    Ok(path) => {
                                        this.progress = 1.;
                                        this.status = format!("Exported {}", path.display());
                                        this.output = Some(path);
                                    }
                                    Err(error) => {
                                        this.status = error.clone();
                                        this.log = error;
                                    }
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if finished {
                    break;
                }
            }
            work.await;
        })
        .detach();
        cx.notify();
    }
    #[gpui]
    fn preview(&self, height: f32, cx: &mut Context<Self>) -> AnyElement {
        let aspect = self
            .media
            .as_ref()
            .map(|media| {
                if matches!(self.rotation, 90 | 270) {
                    media.height as f32 / media.width as f32
                } else {
                    media.width as f32 / media.height as f32
                }
            })
            .unwrap_or(16. / 9.);
        let image = if let Some(path) = &self.image {
            <img args={path.clone()} w={px(height * aspect)} h={px(height)} min-w-0 min-h-0 max-w-full object-fit={ObjectFit::Contain} />.into_any_element()
        } else {
            <div flex items-center justify-center size-full>Import a clip to render a preview frame.</div>.into_any_element()
        };
        <div id="video-preview-scroll" flex flex-col size-full gap-3 p-4 min-h-0 overflow-y-scroll>
            <div flex items-center justify-center h={px(height)} flex-shrink={0.} min-h-0 overflow-hidden bg={rgb(0x111827)} rounded-lg>{image}</div>
            <div text-sm>{if self.previewing{"Rendering frame…"}else{"Frame preview with current rotation and size settings"}}</div>
            <div flex flex-wrap items-center gap-2>
                <Button args={"previous-frame"} label="−1 s" disabled={self.media.is_none()} on-click={cx.listener(|this,_,window,cx|this.seek(-1.,window,cx))} />
                <div w={px(140.)}><Input args={&self.position} /></div>
                <Button args={"next-frame"} label="+1 s" disabled={self.media.is_none()} on-click={cx.listener(|this,_,window,cx|this.seek(1.,window,cx))} />
                <Button args={"refresh-frame"} label="Preview at time" disabled={self.media.is_none()} on-click={cx.listener(|this,_,_,cx|this.refresh_frame(cx))} />
                <Button args={"play-original"} label="Play original" disabled={self.media.is_none()} on-click={{let media=self.media.clone();move|_,_,cx|{if let Some(media)=&media{cx.open_with_system(&media.path);}}}} />
            </div>
        </div>.into_any_element()
    }
}
impl Render for Editor {
    #[gpui]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.tab {
            0 => self.preview(
                (f32::from(window.bounds().size.height) - 330.).max(160.),
                cx,
            ),
            1 => {
                let details=self.media.as_ref().map(|m|format!("{}\n\nDuration: {:.3} seconds\nResolution: {} × {}\nVideo: {}\nAudio: {}",m.path.display(),m.duration,m.width,m.height,m.video,m.audio.as_deref().unwrap_or("none"))).unwrap_or_else(||"No media imported.".into());
                <div p-4>{details}</div>.into_any_element()
            }
            _ => <div flex flex-col p-4 gap-3><div font-semibold>Export queue</div><div>{self.status.clone()}</div>
                <Progress args={"export-progress"} value={self.progress*100.} />
                <div>{format!("{:.0}%",self.progress*100.)}</div>
                <Button args={"cancel-export"} label="Cancel export" disabled={!self.exporting} on-click={cx.listener(|this,_,_,cx|{this.export_cancel.store(true,Ordering::Relaxed);this.status="Cancelling export…".into();cx.notify();})} />
                <Button args={"play-export"} label="Play exported video" disabled={self.output.is_none()||self.exporting} on-click={{let output=self.output.clone();move|_,_,cx|{if let Some(path)=&output{cx.open_with_system(path);}}}} />
                <div id="export-log" overflow-y-scroll max-h={px(320.)} text-sm>{self.log.clone()}</div>
            </div>.into_any_element(),
        };
        let duration = self.media.as_ref().map(|m| m.duration).unwrap_or(0.);
        let trim_start = self.start.read(cx).value().parse::<f64>().unwrap_or(0.);
        let trim_end = self.end.read(cx).value().parse::<f64>().unwrap_or(0.);
        let left = if duration > 0. {
            (trim_start / duration).clamp(0., 1.) as f32
        } else {
            0.
        };
        let width = if duration > 0. {
            ((trim_end - trim_start) / duration).clamp(0., 1.) as f32
        } else {
            0.
        };
        <div flex flex-col size-full overflow-hidden bg={cx.theme().background} text-color={cx.theme().foreground}>
            <div flex items-center gap-3 p-3><div text-xl font-semibold flex-1>Minimal video editor</div>
                <Button args={"import"} primary label="Import video…" disabled={self.probing||self.exporting} on-click={cx.listener(|_,_,window,cx|{
                    let choice=cx.prompt_for_paths(PathPromptOptions{files:true,directories:false,multiple:false,prompt:Some("Import video".into())});
                    cx.spawn_in(window,async move|this,cx|{if let Ok(Ok(Some(paths)))=choice.await{if let Some(path)=paths.into_iter().next(){let _=this.update_in(cx,|this,window,cx|this.import(path,window,cx));}}}).detach();
                })} />
                <Button args={"export"} label="Export MP4…" disabled={self.media.is_none()||self.exporting||self.probing} on-click={cx.listener(|this,_,window,cx|{
                    let Some(media)=&this.media else{return;};let directory=media.path.parent().unwrap_or(std::path::Path::new("."));
                    let choice=cx.prompt_for_new_path(directory,Some("edited.mp4"));
                    cx.spawn_in(window,async move|this,cx|{if let Ok(Ok(Some(output)))=choice.await{let _=this.update(cx,|this,cx|this.start_export(output,cx));}}).detach();
                })} />
            </div>
            <div flex-1 min-h-0><h_resizable args={"editor-panes"}>
                <resizable_panel><div flex flex-col size-full min-h-0>
                    <TabBar args={"editor-tabs"} selected-index={self.tab} on-click={cx.listener(|this,index,_,cx|{this.tab=*index;cx.notify();})}><Tab label="Preview" /><Tab label="Clip info" /><Tab label="Exports" /></TabBar>
                    <div flex-1 min-h-0>{body}</div>
                </div></resizable_panel>
                <resizable_panel size={px(300.)} size-range={px(260.)..px(500.)}>
                    <div id="edit-settings" flex flex-col gap-3 p-4 size-full overflow-y-scroll>
                        <div font-semibold>Trim clip</div><div text-xs>Start (seconds)</div><Input args={&self.start} disabled={self.exporting} />
                        <div text-xs>End (seconds)</div><Input args={&self.end} disabled={self.exporting} />
                        <Button args={"mark-start"} label="Set start to preview time" disabled={self.media.is_none()||self.exporting} on-click={cx.listener(|this,_,window,cx|{let value=this.position.read(cx).value();this.start.update(cx,|input,cx|input.set_value(value,window,cx));cx.notify();})} />
                        <Button args={"mark-end"} label="Set end to preview time" disabled={self.media.is_none()||self.exporting} on-click={cx.listener(|this,_,window,cx|{let value=this.position.read(cx).value();this.end.update(cx,|input,cx|input.set_value(value,window,cx));cx.notify();})} />
                        <div font-semibold>Transform</div>
                        <Button args={"rotate"} label={format!("Rotate: {}°",self.rotation)} disabled={self.exporting} on-click={cx.listener(|this,_,_,cx|{this.rotation=(this.rotation+90)%360;this.refresh_frame(cx);})} />
                        <Button args={"resize"} label={match self.width{None=>"Size: original".into(),Some(width)=>format!("Width: {width}px")}} disabled={self.exporting} on-click={cx.listener(|this,_,_,cx|{this.width=match this.width{None=>Some(1280),Some(1280)=>Some(720),_=>None};this.refresh_frame(cx);})} />
                        <Button args={"mute"} label={if self.mute{"Audio: muted"}else{"Audio: keep"}} disabled={self.exporting} on-click={cx.listener(|this,_,_,cx|{this.mute=!this.mute;cx.notify();})} />
                        <div text-xs>Exports re-encode video as H.264 and audio as AAC. Existing files are preserved.</div>
                    </div>
                </resizable_panel>
            </h_resizable></div>
            <div p-4 flex flex-col gap-2>
                <div>{format!("Selected range: {trim_start:.3}s → {trim_end:.3}s · clip {duration:.3}s")}</div>
                <div relative w-full h={px(28.)} rounded-md bg={cx.theme().muted}>
                    <div absolute left={relative(left)} w={relative(width)} h-full rounded-md bg={rgb(0x2563eb)} />
                </div>
                <div text-xs>{self.status.clone()}</div>
            </div>
        </div>
    }
}
