use gpui_kit::{prelude::*, *};
use gpui_kit::component::{input::{InputState,InputEvent},slider::{SliderState,SliderEvent}};
use crate::{state::{Action,EditorState,Screen},generated::primitives::*};

pub struct Editor {
    pub state: EditorState,
    pub search: Entity<InputState>,
    pub header_search: Entity<InputState>,
    pub sliders: Vec<Entity<SliderState>>,
    pub seek: Entity<SliderState>,
    pub _subscriptions: Vec<Subscription>,
    pub edit_workspace: crate::generated::workspace::Workspace,
    pub library_workspace: crate::generated::workspace::Workspace,
    pub dragging_playhead: bool,
    pub timeline_origin: f32,
    pub timeline_scale: f32,
    pub media_scroll: ScrollHandle,
    pub preset_scroll: ScrollHandle,
}
impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state=EditorState::default();
        if let Some(screen)=std::env::args().find_map(|arg|match arg.as_str(){"--library"=>Some(Screen::Library),"--overview"=>Some(Screen::Overview),"--tracking"=>Some(Screen::Tracking),_=>None}) {state.apply(Action::Screen(screen));}
        let search=cx.new(|cx|InputState::new(window,cx).placeholder("Search media…"));
        let header_search=cx.new(|cx|InputState::new(window,cx).placeholder("Search media, folders, or tags…"));
        let mut subscriptions=vec![
            cx.subscribe_in(&search,window,|this,input,event:&InputEvent,window,cx| {
                if matches!(event,InputEvent::Change) {
                    let value=input.read(cx).value().to_string();
                    this.state.apply(Action::Search(value.clone()));
                    this.header_search.update(cx,|input,cx|input.set_value(value,window,cx));
                    cx.notify();
                }
            }),
            cx.subscribe_in(&header_search,window,|this,input,event:&InputEvent,window,cx| {
                if matches!(event,InputEvent::Change) {
                    let value=input.read(cx).value().to_string();
                    this.state.apply(Action::Search(value.clone()));
                    this.search.update(cx,|input,cx|input.set_value(value,window,cx));
                    cx.notify();
                }
            }),
        ];
        let sliders=state.controls.iter().enumerate().map(|(i,&value)| {
            let (min,max)=match i {0=>(10.,200.),1=>(-180.,180.),_ =>(0.,100.)};
            let slider=cx.new(|_|SliderState::new().min(min).max(max).step(1.).default_value(value));
            subscriptions.push(cx.subscribe(&slider,move |this,_,event:&SliderEvent,cx|{
                if let SliderEvent::Change(value)=event {this.state.apply(Action::Control(i,value.end()));cx.notify();}
            }));slider
        }).collect();
        let seek=cx.new(|_|SliderState::new().min(0.).max(1.).step(0.001).default_value(state.position/state.seek_limit()));
        subscriptions.push(cx.subscribe_in(&seek,window,|this,_,event:&SliderEvent,window,cx| {
            if let SliderEvent::Change(value)=event {
                this.state.apply(Action::Seek(value.end()*this.state.seek_limit()));
                this.seek.update(cx,|slider,cx|slider.set_value(this.state.position/this.state.seek_limit(),window,cx));
                cx.notify();
            }
        }));
        let owner=cx.weak_entity();
        let edit_workspace=crate::generated::workspace::workspace(owner.clone(),true,window,cx);
        let library_workspace=crate::generated::workspace::workspace(owner,false,window,cx);
        Self{state,search,header_search,sliders,seek,_subscriptions:subscriptions,edit_workspace,library_workspace,dragging_playhead:false,timeline_origin:0.,timeline_scale:1.,media_scroll:ScrollHandle::new(),preset_scroll:ScrollHandle::new()}
    }
    pub fn dispatch(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let clear=matches!(action,Action::ClearFilters|Action::ClearPresetFilter|Action::Category(_));
        let sync_controls=matches!(action,Action::ResetTransform|Action::ResetCrop);
        if matches!(action,Action::ResetWorkspace) {
            let editing=self.state.screen.has_timeline();
            let area=crate::generated::workspace::workspace(cx.weak_entity(),editing,window,cx);
            if editing {self.edit_workspace=area;}else{self.library_workspace=area;}
        }
        let reveal_browser=matches!(action,Action::Category(_)|Action::Folder(_));
        if clear || reveal_browser {
            self.media_scroll.set_offset(point(px(0.),px(0.)));
            self.preset_scroll.set_offset(point(px(0.),px(0.)));
        }
        self.state.apply(action);
        if reveal_browser {
            let workspace=if self.state.screen.has_timeline(){&self.edit_workspace}else{&self.library_workspace};
            workspace.area.update(cx,|area,cx|area.select_panel(workspace.browser,window,cx));
        }
        if clear {
            self.search.update(cx,|input,cx|input.set_value("",window,cx));
            self.header_search.update(cx,|input,cx|input.set_value("",window,cx));
        }
        if sync_controls {for (i,slider) in self.sliders.iter().enumerate(){slider.update(cx,|s,cx|s.set_value(self.state.controls[i],window,cx));}}
        self.seek.update(cx,|s,cx|s.set_value(self.state.position/self.state.seek_limit(),window,cx));
        cx.notify();
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
            {if let Some(notice)=self.state.notice.clone(){
                <div absolute bottom={px(34.)} right={px(20.)} flex items-center gap-3 p-3 bg={rgb(RAISED)} border-1 border-color={rgb(PURPLE)} rounded-lg shadow-lg>
                    <div text-size={px(12.)}>{notice}</div>
                    {tool("dismiss","Dismiss",Some(gpui_kit::assets::IconName::X),Action::Dismiss,false,cx)}
                </div>.into_any_element()
            } else {div().into_any_element()}}
        </div>
    }
}
