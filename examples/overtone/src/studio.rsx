use gpui_kit::{prelude::*, *};
use gpui_kit::component::{Theme, input::{InputEvent, InputState}, slider::{SliderEvent, SliderState}};
use crate::{model::*, persistence};
use std::{collections::HashMap, path::PathBuf};

actions!(overtone, [SaveProject, SaveProjectAs, OpenProject, NewProject, ZoomIn, ZoomOut, ResetZoom, OpenSettings, UndoProject, RedoProject, ChangeTheme, CloseSession, ShowAbout, RenameProject]);
#[derive(Clone, PartialEq, serde::Deserialize, gpui_kit::Action)]
#[action(namespace = overtone, no_json)]
pub struct SelectWorkspace {pub id:Id}
#[derive(Clone)]
pub enum Action {
    DismissDialog, Save(bool), Open, New, ConfirmDiscard, Cancel, SaveContinue, ToggleTheme,
    Profile(Id), NewProfile(bool), RenameProfile, PanelVisible(Module), PanelDock(Module), PanelHalf(Module), PanelOrder(Module,i32), Sidebar(bool,f32), LowerHeight(f32),
    NewSound, LoadSound(Id), SaveSound, AddDraft, AddLibrary(Id), HarmonicCount(i32), HarmonicBank(i32), NoiseColor(bool), ResetInstance,
    ImportVoice, AttachVoice(Id), AddTrack, TargetTrack(Id), SelectClip(Id), DuplicateClip, DeleteClip, MoveClipTrack(Id), InstanceHarmonicCount(i32),
    Tempo(Id,f64), TrackLink(Id), TrackMute(Id), TrackSolo(Id), Note(u8), ClearNotes, MoveNote(usize,f64), DeleteNote(usize),
    ClipTiming(bool,f64), ClipMute, ClipGain(f32), Insertion(f64), SessionTempo(f64),
    Spacing(u8,f32), AutoHeight(Module), DefaultGaps(Module), WorkspaceFile(bool), Zoom(Option<Module>,f32),
}
#[derive(Clone,Copy)]
pub enum ResizeTarget {Left,Right,Lower,Pair(Module,Module),Panel(Module),PanelGap(Module,bool),DockGap(bool)}
pub struct ResizeGesture {pub target:ResizeTarget,pub profile:Id,pub origin:Point<Pixels>,pub value:f32,pub extent:f32,pub scale:f32}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct DropPreview {pub module:Module,pub dock:Dock,pub before:Option<Module>}
#[derive(Clone,Copy,PartialEq)]
pub enum Pending { New, Open, Close }
pub struct Studio {
    pub project: Project, pub path: Option<PathBuf>, pub revision: u64, pub saved_revision: u64, pub generation: u64,
    pub renaming:bool,pub show_status:bool,pub notice: String, pub busy: bool, pub pending: Option<Pending>, pub continue_after_save: bool,
    pub focus: FocusHandle, pub search: Entity<InputState>, pub query: String,
    pub title: Entity<InputState>, pub sound_name: Entity<InputState>, pub profile_name: Entity<InputState>,
    pub harmonic_bank: usize, pub selected_harmonic: usize, pub instance_harmonic: usize,
    pub draft_sliders: Vec<Entity<SliderState>>, pub instance_sliders: Vec<Entity<SliderState>>,
    pub track_bounds: HashMap<Id,Bounds<Pixels>>, pub timeline_span: f64,
    pub knob_drag: Option<(bool,usize,Point<Pixels>,f32)>, pub allow_close: bool,
    pub resizing:Option<ResizeGesture>,pub panel_bounds:HashMap<Module,Bounds<Pixels>>,pub pair_bounds:HashMap<Module,Bounds<Pixels>>,
    pub dock_bounds:HashMap<Dock,Bounds<Pixels>>,pub drop_preview:Option<DropPreview>,pub drag_profile:Option<Profile>,pub drag_module:Option<Module>,
    pub drag_panels:HashMap<Module,Bounds<Pixels>>,pub drag_docks:HashMap<Dock,Bounds<Pixels>>,pub scrolls:HashMap<String,ScrollHandle>,
    pub settings_window:Option<AnyWindowHandle>,pub settings_opening:bool,pub undo_stack:Vec<Project>,pub redo_stack:Vec<Project>,pub checkpoint:Project,pub history_group:bool,
    pub _subscriptions: Vec<Subscription>,
}
impl Studio {
    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (project,path,notice) = if let Some(path)=path {
            match persistence::load(&path) {Ok(loaded)=>(loaded.project,Some(path),if loaded.missing_sources.is_empty(){"Project opened".into()}else{format!("Missing sources: {}",loaded.missing_sources.join(", "))}),Err(e)=>(Project::default(),None,format!("Could not open project: {e}"))}
        } else {(Project::default(),None,"UI workspace · audio engine not implemented".into())};
        crate::primitives::apply_theme(project.dark,window,cx);
        cx.bind_keys([KeyBinding::new("secondary-s",SaveProject,Some("Overtone")),KeyBinding::new("secondary-shift-s",SaveProjectAs,Some("Overtone")),KeyBinding::new("secondary-o",OpenProject,Some("Overtone")),KeyBinding::new("secondary-n",NewProject,Some("Overtone"))]);
        cx.bind_keys([KeyBinding::new("secondary-=",ZoomIn,Some("Overtone")),KeyBinding::new("secondary--",ZoomOut,Some("Overtone")),KeyBinding::new("secondary-0",ResetZoom,Some("Overtone"))]);
        cx.bind_keys([KeyBinding::new("ctrl-s",SaveProject,Some("Overtone")),KeyBinding::new("cmd-s",SaveProject,Some("Overtone")),KeyBinding::new("secondary-z",UndoProject,Some("Overtone")),KeyBinding::new("secondary-shift-z",RedoProject,Some("Overtone")),KeyBinding::new("ctrl-y",RedoProject,Some("Overtone")),KeyBinding::new("secondary-,",OpenSettings,Some("Overtone"))]);
        let search=cx.new(|cx|InputState::new(window,cx).placeholder("Search synthesized sounds"));
        let title=cx.new(|cx|InputState::new(window,cx).default_value(project.title.clone()).validate(|v,_|!v.trim().is_empty() && v.len()<=512));
        let sound_name=cx.new(|cx|InputState::new(window,cx).default_value(project.draft.name.clone()).placeholder("Sound name").validate(|v,_|!v.trim().is_empty() && v.len()<=512));
        let profile_name=cx.new(|cx|InputState::new(window,cx).placeholder("Profile name").validate(|v,_|v.len()<=512));
        let mut subscriptions=vec![
            cx.subscribe(&search,|this,input,event:&InputEvent,cx|{if matches!(event,InputEvent::Change){this.query=input.read(cx).value().to_string();cx.notify();}}),
            cx.subscribe(&title,|this,input,event:&InputEvent,cx|{if matches!(event,InputEvent::Change){let value=input.read(cx).value().to_string();if !value.trim().is_empty() && value!=this.project.title{this.project.title=value;this.changed(cx);}}}),
            cx.subscribe(&sound_name,|this,input,event:&InputEvent,cx|{if matches!(event,InputEvent::Change){let value=input.read(cx).value().to_string();if !value.trim().is_empty() && value!=this.project.draft.name{this.project.draft.name=value;this.changed(cx);}}}),
        ];
        let mut draft_sliders=vec![];let mut instance_sliders=vec![];
        for instance in [false,true] {
            for index in 0..41 {
                let (min,max,step) = match index {32=>(-180.,180.,1.),33=>(-100.,100.,1.),37|38|40=>(0.,10000.,10.),_=>(0.,1.,0.01)};
                let slider=cx.new(|_|SliderState::new().min(min).max(max).step(step).default_value(0.));
                subscriptions.push(cx.subscribe_in(&slider,window,move|this,_,event:&SliderEvent,_,cx|{
                    if let SliderEvent::Change(v)=event {this.set_param(instance,index,v.end(),cx);}
                }));
                if instance {instance_sliders.push(slider);}else{draft_sliders.push(slider);}
            }
        }
        cx.observe_global::<Theme>(|_,cx|cx.notify()).detach();
        let weak=cx.entity().downgrade();
        window.on_window_should_close(cx,move|window,cx|weak.update(cx,|this,cx|{
            if this.allow_close || !this.dirty() && !this.busy {true}else{this.pending=Some(Pending::Close);cx.notify();let _=window;false}
        }).unwrap_or(true));
        let checkpoint=project.clone();
        let mut this=Self{project,path,revision:0,saved_revision:0,generation:0,renaming:false,show_status:false,notice,busy:false,pending:None,continue_after_save:false,
            focus:cx.focus_handle(),search,query:String::new(),title,sound_name,profile_name,harmonic_bank:0,selected_harmonic:0,instance_harmonic:0,draft_sliders,instance_sliders,
            track_bounds:HashMap::new(),timeline_span:16.,knob_drag:None,allow_close:false,resizing:None,panel_bounds:HashMap::new(),pair_bounds:HashMap::new(),
            dock_bounds:HashMap::new(),drop_preview:None,drag_profile:None,drag_module:None,drag_panels:HashMap::new(),drag_docks:HashMap::new(),scrolls:HashMap::new(),settings_window:None,settings_opening:false,undo_stack:vec![],redo_stack:vec![],checkpoint,history_group:false,_subscriptions:subscriptions};
        this.sync_sliders(false,window,cx);this.sync_sliders(true,window,cx);this.focus.focus(window,cx);this
    }
    pub fn changed(&mut self,cx:&mut Context<Self>) {
        if self.project!=self.checkpoint{
            let gesture=self.resizing.is_some() || self.knob_drag.is_some();
            if !gesture || !self.history_group{self.undo_stack.push(self.checkpoint.clone());if self.undo_stack.len()>50{self.undo_stack.remove(0);}}
            self.history_group=gesture;self.redo_stack.clear();self.checkpoint=self.project.clone();self.revision+=1;
        }cx.notify();
    }
    pub fn history(&mut self,redo:bool,window:&mut Window,cx:&mut Context<Self>){
        if self.busy || self.pending.is_some(){return;}
        let state=if redo{self.redo_stack.pop()}else{self.undo_stack.pop()};let Some(state)=state else{return;};
        if redo{self.undo_stack.push(self.project.clone());}else{self.redo_stack.push(self.project.clone());}
        self.project=state;self.checkpoint=self.project.clone();self.revision+=1;self.history_group=false;self.clear_drag();self.resizing=None;self.knob_drag=None;
        self.harmonic_bank=self.harmonic_bank.min((self.project.draft.harmonics.len()-1)/8);self.selected_harmonic=self.selected_harmonic.min(self.project.draft.harmonics.len()-1);self.instance_harmonic=0;
        self.title.update(cx,|s,cx|s.set_value(self.project.title.clone(),window,cx));self.sound_name.update(cx,|s,cx|s.set_value(self.project.draft.name.clone(),window,cx));self.sync_sliders(false,window,cx);self.sync_sliders(true,window,cx);crate::primitives::apply_theme(self.project.dark,window,cx);cx.notify();
    }
    pub fn open_settings(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if self.settings_opening{return;}
        if let Some(handle)=self.settings_window{if handle.update(cx,|_,window,_|window.activate_window()).is_ok(){return;}}
        let owner=cx.entity().downgrade();let parent=window.window_handle();
        let options=WindowOptions{window_bounds:Some(WindowBounds::Windowed(Bounds::centered(None,size(px(1100.),px(700.)),cx))),window_min_size:Some(size(px(720.),px(480.))),titlebar:Some(TitlebarOptions{title:Some("Overtone · Settings".into()),..gpui_kit::component::TitleBar::title_bar_options()}),window_decorations:Some(WindowDecorations::Client),..gpui_kit::component::TitleBar::window_options()};
        self.settings_opening=true;cx.defer(move|cx|{
            let result_owner=owner.clone();
            let result=gpui_kit::open_window(options,cx,move|window,cx|cx.new(|cx|crate::settings::Settings::new(owner,parent,window,cx)));
            let _=result_owner.update(cx,|s,cx|{s.settings_opening=false;match result{Ok((handle,_))=>s.settings_window=Some(handle),Err(e)=>s.notice=format!("Could not open settings: {e}")};cx.notify();});
        });
    }
    pub fn dirty(&self)->bool {self.revision!=self.saved_revision}
    pub fn scroll(&self,id:&str)->ScrollHandle{self.scrolls.get(id).cloned().unwrap_or_default()}
    pub fn prepare_scrolls(&mut self){for id in ["left-scroll","right-scroll","center-scroll","lower-scroll","configuration-scroll"]{self.scrolls.entry(id.into()).or_default();}for module in Module::ALL{self.scrolls.entry(format!("body-{module:?}")).or_default();}}
    pub fn clear_drag(&mut self){self.drop_preview=None;self.drag_profile=None;self.drag_module=None;self.drag_panels.clear();self.drag_docks.clear();}
    pub fn begin_module_drag(&mut self,module:Module,cx:&mut Context<Self>){self.clear_drag();self.drag_module=Some(module);self.drag_profile=Some(self.project.profile().clone());self.drag_panels=self.panel_bounds.clone();self.drag_docks=self.dock_bounds.clone();cx.notify();}
    pub fn preview_drag(&mut self,module:Module,position:Point<Pixels>,cx:&mut Context<Self>){
        if self.drag_module!=Some(module){self.begin_module_drag(module,cx);}
        let Some(profile)=&self.drag_profile else{return;};if profile.id!=self.project.active_profile{self.clear_drag();cx.notify();return;}
        let dock=[Dock::Left,Dock::Main,Dock::Right,Dock::Lower].into_iter().find(|d|self.drag_docks.get(d).is_some_and(|b|b.contains(&position)));
        let preview=dock.map(|dock|{let before=profile.panels.iter().filter(|p|p.visible && p.dock==dock && p.module!=module).find(|p|self.drag_panels.get(&p.module).is_some_and(|b|b.contains(&position))).map(|p|p.module);DropPreview{module,dock,before}});
        if self.drop_preview!=preview{self.drop_preview=preview;cx.notify();}
    }
    pub fn displayed_profile(&self)->Profile{let mut p=self.project.profile().clone();if let Some(preview)=self.drop_preview{p.move_panel(preview.module,preview.dock,preview.before);}p}
    pub fn drop_module(&mut self,module:Module,dock:Dock,before:Option<Module>,cx:&mut Context<Self>){let target=self.drop_preview.filter(|p|p.module==module).unwrap_or(DropPreview{module,dock,before});self.project.profile_mut().move_panel(module,target.dock,target.before);self.clear_drag();self.changed(cx);}
    pub fn begin_resize(&mut self,target:ResizeTarget,origin:Point<Pixels>,window:&Window,cx:&mut Context<Self>){
        let p=self.project.profile();
        // A paired row owns its vertical gap through its first placement.
        let target=if let ResizeTarget::PanelGap(module,true)=target{
            let mut owner=module;
            if let Some(placement)=p.panels.iter().find(|p|p.module==module){
                let panels=p.panels.iter().filter(|p|p.visible && p.dock==placement.dock).collect::<Vec<_>>();let mut i=0;
                while i<panels.len(){let paired=panels[i].half && panels.get(i+1).is_some_and(|p|p.half);if paired && panels[i+1].module==module{owner=panels[i].module;break;}i+=if paired{2}else{1};}
            }
            ResizeTarget::PanelGap(owner,true)
        }else{target};
        let scale=p.zoom*if let ResizeTarget::Panel(module)=target{p.panels.iter().find(|p|p.module==module).map(|p|p.zoom).unwrap_or(1.)}else{1.};
        let lower_limit=((f32::from(window.bounds().size.height)-250.*p.zoom)/p.zoom).clamp(180.,700.);
        let value=match target {ResizeTarget::Left=>p.left_width,ResizeTarget::Right=>p.right_width,ResizeTarget::Lower=>p.lower_height.min(lower_limit),
            ResizeTarget::Pair(left,_)=>p.panels.iter().find(|p|p.module==left).map(|p|p.share).unwrap_or(0.5),
            ResizeTarget::Panel(module)=>self.panel_bounds.get(&module).map(|b|f32::from(b.size.height)/scale).unwrap_or(240.),
            ResizeTarget::PanelGap(module,vertical)=>p.panels.iter().find(|p|p.module==module).and_then(|p|if vertical{p.gap_after}else{p.pair_gap}).unwrap_or(p.panel_gap),ResizeTarget::DockGap(_)=>p.dock_gap};
        let extent=match target{ResizeTarget::Pair(left,_)=>{let gap=p.panels.iter().find(|p|p.module==left).and_then(|p|p.pair_gap).unwrap_or(p.panel_gap);self.pair_bounds.get(&left).map(|b|f32::from(b.size.width)-gap*p.zoom).unwrap_or(600.).max(1.)},ResizeTarget::Lower=>lower_limit,_=>1.};
        self.resizing=Some(ResizeGesture{target,profile:p.id,origin,value,extent,scale});self.knob_drag=None;cx.stop_propagation();
    }
    pub fn resize_motion(&mut self,event:&MouseMoveEvent,cx:&mut Context<Self>){
        let Some(gesture)=&self.resizing else{return;};
        if !event.dragging() || gesture.profile!=self.project.active_profile {self.resizing=None;return;}
        let raw_dx=f32::from(event.position.x-gesture.origin.x);let dx=raw_dx/gesture.scale;let dy=f32::from(event.position.y-gesture.origin.y)/gesture.scale;let value=gesture.value;let extent=gesture.extent;let target=gesture.target;
        let p=self.project.profile_mut();
        match target {
            ResizeTarget::Left=>p.left_width=(value+dx).clamp(200.,420.),ResizeTarget::Right=>p.right_width=(value-dx).clamp(200.,420.),
            ResizeTarget::Lower=>p.lower_height=(value-dy).clamp(180.,extent),
            ResizeTarget::Pair(left,right)=>{let share=(value+raw_dx/extent).clamp(0.15,0.85);for panel in &mut p.panels{if panel.module==left{panel.share=share;}if panel.module==right{panel.share=1.-share;}}},
            ResizeTarget::Panel(module)=>{if let Some(panel)=p.panels.iter_mut().find(|p|p.module==module){panel.height=Some((value+dy).clamp(100.,1800.));}},
            ResizeTarget::PanelGap(module,vertical)=>{if let Some(panel)=p.panels.iter_mut().find(|p|p.module==module){let gap=Some((value+if vertical{dy}else{dx}).clamp(4.,40.));if vertical{panel.gap_after=gap;}else{panel.pair_gap=gap;}}},ResizeTarget::DockGap(vertical)=>p.dock_gap=(value+if vertical{dy}else{dx}).clamp(6.,32.),
        }
        self.changed(cx);
    }
    pub fn edit_sound(&mut self,instance:bool)->Option<&mut Sound> {if instance {self.project.selected_mut().map(|c|&mut c.sound)}else{Some(&mut self.project.draft)}}
    pub fn set_param(&mut self,instance:bool,index:usize,value:f32,cx:&mut Context<Self>) {
        let selected=if instance{self.instance_harmonic}else{self.selected_harmonic};
        if let Some(sound)=self.edit_sound(instance) {
            let selected=selected.min(sound.harmonics.len()-1);
            match index {
                0..=31=>{if let Some(h)=sound.harmonics.get_mut(index){h.amplitude=value;}else{return;}},
                32=>{if let Some(h)=sound.harmonics.get_mut(selected){h.phase=value;}},
                33=>{if let Some(h)=sound.harmonics.get_mut(selected){h.detune=value;}},
                34=>sound.noise.level=value,35=>sound.brightness=value,36=>sound.gain=value,
                37=>sound.envelope.attack_ms=value,38=>sound.envelope.decay_ms=value,39=>sound.envelope.sustain=value,40=>sound.envelope.release_ms=value,_=>return,
            } self.changed(cx);
        }
    }
    pub fn sync_sliders(&mut self,instance:bool,window:&mut Window,cx:&mut Context<Self>) {
        let sound=if instance {self.project.selected().map(|c|c.sound.clone())}else{Some(self.project.draft.clone())};
        let Some(sound)=sound else{return;};
        let sliders=if instance{&self.instance_sliders}else{&self.draft_sliders};
        for (i,slider) in sliders.iter().enumerate().take(41) {
            let selected=if instance{self.instance_harmonic}else{self.selected_harmonic};
            let h=sound.harmonics.get(selected.min(sound.harmonics.len()-1)).unwrap();
            let value=match i{0..=31=>sound.harmonics.get(i).map(|h|h.amplitude).unwrap_or(0.),32=>h.phase,33=>h.detune,34=>sound.noise.level,35=>sound.brightness,36=>sound.gain,37=>sound.envelope.attack_ms,38=>sound.envelope.decay_ms,39=>sound.envelope.sustain,40=>sound.envelope.release_ms,_=>0.};
            slider.update(cx,|slider,cx|slider.set_value(value,window,cx));
        }
    }
    pub fn dispatch(&mut self,action:Action,window:&mut Window,cx:&mut Context<Self>) {
        if self.pending.is_some() && !matches!(&action,Action::Save(_)|Action::Cancel|Action::ConfirmDiscard|Action::SaveContinue){return;}
        let mut changed=true;let mut sync_draft=false;let mut sync_instance=false;
        match action {
            Action::Zoom(module,delta)=>{self.clear_drag();self.resizing=None;let p=self.project.profile_mut();let value=if let Some(module)=module{let Some(panel)=p.panels.iter_mut().find(|p|p.module==module)else{return;};&mut panel.zoom}else{&mut p.zoom};*value=if delta==0.{1.}else{((*value+delta)*100.).round().clamp(50.,200.)/100.};},
            Action::WorkspaceFile(export)=>{self.workspace_file(export,window,cx);return;},
            Action::Spacing(which,delta)=>{let p=self.project.profile_mut();match which{0=>p.panel_gap=(p.panel_gap+delta).clamp(4.,40.),1=>p.dock_gap=(p.dock_gap+delta).clamp(6.,32.),_=>p.panel_padding=(p.panel_padding+delta).clamp(4.,32.)}},
            Action::AutoHeight(module)=>{if let Some(panel)=self.project.profile_mut().panels.iter_mut().find(|p|p.module==module){panel.height=None;}},
            Action::DefaultGaps(module)=>{if let Some(panel)=self.project.profile_mut().panels.iter_mut().find(|p|p.module==module){panel.gap_after=None;panel.pair_gap=None;}},
            Action::Save(as_new)=>{self.save(as_new,window,cx);return;},
            Action::Open|Action::New=>{let pending=if matches!(action,Action::Open){Pending::Open}else{Pending::New};self.request_replace(pending,window,cx);return;},
            Action::DismissDialog=>{self.renaming=false;self.show_status=false;changed=false;},
            Action::Cancel=>{self.pending=None;self.continue_after_save=false;changed=false;},
            Action::ConfirmDiscard=>{if let Some(pending)=self.pending.take(){self.replace(pending,window,cx);}return;},
            Action::SaveContinue=>{self.continue_after_save=true;self.save(false,window,cx);return;},
            Action::ToggleTheme=>{self.project.dark=!self.project.dark;crate::primitives::apply_theme(self.project.dark,window,cx);},
            Action::Profile(id)=>{self.clear_drag();self.project.active_profile=id;},
            Action::NewProfile(empty)=>{let name=self.profile_name.read(cx).value().to_string();self.project.add_profile(if name.trim().is_empty(){format!("Workspace {}",self.project.profiles.len()+1)}else{name},empty);},
            Action::RenameProfile=>{let name=self.profile_name.read(cx).value().to_string();if !name.trim().is_empty(){self.project.profile_mut().name=name;}},
            Action::PanelVisible(module)=>{if let Some(p)=self.project.profile_mut().panels.iter_mut().find(|p|p.module==module){p.visible=!p.visible;}},
            Action::PanelDock(module)=>{let profile=self.project.profile_mut();if let Some(p)=profile.panels.iter_mut().find(|p|p.module==module){p.dock=p.dock.next();}},
            Action::PanelHalf(module)=>{if let Some(p)=self.project.profile_mut().panels.iter_mut().find(|p|p.module==module){p.half=!p.half;}},
            Action::PanelOrder(module,delta)=>{let profile=self.project.profile_mut();if let Some(i)=profile.panels.iter().position(|p|p.module==module){let j=(i as i32+delta).clamp(0,profile.panels.len() as i32-1) as usize;profile.panels.swap(i,j);}},
            Action::Sidebar(left,delta)=>{let p=self.project.profile_mut();let width=if left{&mut p.left_width}else{&mut p.right_width};*width=(*width+delta).clamp(200.,420.);},
            Action::LowerHeight(delta)=>{let p=self.project.profile_mut();p.lower_height=(p.lower_height+delta).clamp(180.,700.);},
            Action::NewSound=>{self.project.draft=Sound::new(0,"New sound");self.harmonic_bank=0;self.selected_harmonic=0;sync_draft=true;},
            Action::LoadSound(id)=>{if let Some(sound)=self.project.library.iter().find(|s|s.id==id){self.project.draft=sound.clone();self.harmonic_bank=0;self.selected_harmonic=0;sync_draft=true;}},
            Action::SaveSound=>{self.project.save_sound();self.notice="Saved a new library sound. Existing instances keep their own settings.".into();},
            Action::AddDraft=>{self.project.add_clip(self.project.draft.clone(),None);sync_instance=true;},
            Action::AddLibrary(id)=>{if let Some(sound)=self.project.library.iter().find(|s|s.id==id).cloned(){self.project.add_clip(sound,Some(id));sync_instance=true;}},
            Action::HarmonicCount(delta)=>{let count=(self.project.draft.harmonics.len() as i32+delta).clamp(1,32) as usize;self.project.draft.resize_harmonics(count);self.harmonic_bank=self.harmonic_bank.min((count-1)/8);self.selected_harmonic=self.selected_harmonic.min(count-1);sync_draft=true;},
            Action::InstanceHarmonicCount(delta)=>{if let Some(c)=self.project.selected_mut(){let count=(c.sound.harmonics.len() as i32+delta).clamp(1,32) as usize;c.sound.resize_harmonics(count);sync_instance=true;}},
            Action::HarmonicBank(delta)=>{self.harmonic_bank=(self.harmonic_bank as i32+delta).clamp(0,(self.project.draft.harmonics.len() as i32-1)/8) as usize;changed=false;},
            Action::NoiseColor(instance)=>{if let Some(s)=self.edit_sound(instance){s.noise.color=s.noise.color.next();}},
            Action::ResetInstance=>{if let Some(clip)=self.project.selected_mut(){clip.sound=clip.original_sound.clone();sync_instance=true;}},
            Action::ImportVoice=>{self.import_voice(window,cx);return;},
            Action::AttachVoice(id)=>{self.project.draft.source=Some(id);self.notice="Reference attached. Harmonics can be authored manually; analysis will come with the engine.".into();},
            Action::AddTrack=>self.project.add_track(),Action::TargetTrack(id)=>self.project.target_track=id,
            Action::SelectClip(id)=>{self.project.selected_clip=Some(id);sync_instance=true;},
            Action::DuplicateClip=>{if let Some(mut clip)=self.project.selected().cloned(){clip.start_seconds+=self.project.clip_seconds(&clip);clip.id=self.project.allocate();self.project.selected_clip=Some(clip.id);self.project.clips.push(clip);sync_instance=true;}},
            Action::DeleteClip=>{let id=self.project.selected_clip.take();self.project.clips.retain(|c|Some(c.id)!=id);},
            Action::MoveClipTrack(id)=>{if let Some(clip)=self.project.selected_mut(){clip.track=id;}},
            Action::Tempo(id,delta)=>{let current=self.project.tempo(id);if let Some(t)=self.project.tracks.iter_mut().find(|t|t.id==id){t.bpm=(current+delta).clamp(20.,300.);t.follow_session=false;}},
            Action::TrackLink(id)=>{let bpm=self.project.session_bpm;if let Some(t)=self.project.tracks.iter_mut().find(|t|t.id==id){if t.follow_session{t.bpm=bpm;}t.follow_session=!t.follow_session;}},
            Action::TrackMute(id)=>{if let Some(t)=self.project.tracks.iter_mut().find(|t|t.id==id){t.muted=!t.muted;}},
            Action::TrackSolo(id)=>{if let Some(t)=self.project.tracks.iter_mut().find(|t|t.id==id){t.solo=!t.solo;}},
            Action::Note(midi)=>{self.project.add_note(midi);sync_instance=true;},
            Action::ClearNotes=>{if let Some(c)=self.project.selected_mut(){c.notes.clear();}},
            Action::MoveNote(i,delta)=>{if let Some(c)=self.project.selected_mut(){if let Some(n)=c.notes.get_mut(i){n.start_beat=(n.start_beat+delta).max(0.);c.duration_beats=c.duration_beats.max(n.start_beat+n.duration_beats);}}},
            Action::DeleteNote(i)=>{if let Some(c)=self.project.selected_mut(){if i<c.notes.len(){c.notes.remove(i);}}},
            Action::ClipTiming(start,delta)=>{if let Some(c)=self.project.selected_mut(){if start{c.start_seconds=(c.start_seconds+delta).clamp(0.,86400.);}else{let note_end=c.notes.iter().map(|n|n.start_beat+n.duration_beats).fold(0.25,f64::max);c.duration_beats=(c.duration_beats+delta).clamp(note_end,100_000.);}}},
            Action::ClipMute=>{if let Some(c)=self.project.selected_mut(){c.muted=!c.muted;}},
            Action::ClipGain(delta)=>{if let Some(c)=self.project.selected_mut(){c.gain=(c.gain+delta).clamp(0.,1.);}},
            Action::Insertion(delta)=>self.project.insert_seconds=(self.project.insert_seconds+delta).clamp(0.,86400.),
            Action::SessionTempo(delta)=>self.project.session_bpm=(self.project.session_bpm+delta).clamp(20.,300.),
        }
        if sync_draft {let name=self.project.draft.name.clone();self.sound_name.update(cx,|s,cx|s.set_value(name,window,cx));self.sync_sliders(false,window,cx);}
        if sync_instance {self.sync_sliders(true,window,cx);}
        if changed {self.changed(cx);}else{cx.notify();}
    }
    pub fn select_harmonic(&mut self,index:usize,window:&mut Window,cx:&mut Context<Self>){self.selected_harmonic=index;self.sync_sliders(false,window,cx);self.sync_sliders(true,window,cx);cx.notify();}
    pub fn select_instance_harmonic(&mut self,index:usize,window:&mut Window,cx:&mut Context<Self>){self.instance_harmonic=index;self.sync_sliders(true,window,cx);cx.notify();}
    pub fn request_replace(&mut self,pending:Pending,window:&mut Window,cx:&mut Context<Self>){
        if self.busy{return;}
        if self.dirty(){self.renaming=false;self.show_status=false;self.pending=Some(pending);cx.notify();}else{self.replace(pending,window,cx);}
    }
    fn replace(&mut self,pending:Pending,window:&mut Window,cx:&mut Context<Self>) {
        if pending==Pending::Close {if let Some(handle)=self.settings_window.take(){let _=handle.update(cx,|_,window,_|window.remove_window());}self.allow_close=true;window.remove_window();return;}
        if pending==Pending::New {self.install(Project::default(),None,window,cx);self.revision=1;return;}
        let choice=cx.prompt_for_paths(PathPromptOptions{files:true,directories:false,multiple:false,prompt:Some("Open Overtone project".into())});
        self.busy=true;cx.notify();
        cx.spawn_in(window,async move|this,cx|{
            let path=match choice.await{Ok(Ok(Some(paths)))=>paths.into_iter().next(),_=>None};
            if let Some(path)=path {
                let load_path=path.clone();let result=cx.background_spawn(async move{persistence::load(&load_path)}).await;
                let _=this.update_in(cx,|this,window,cx|{this.busy=false;match result{Ok(loaded)=>{this.install(loaded.project,Some(path),window,cx);this.notice=if loaded.missing_sources.is_empty(){"Project opened".into()}else{format!("Missing sources: {}",loaded.missing_sources.join(", "))};},Err(e)=>{this.notice=e;cx.notify();}}});
            }else{let _=this.update(cx,|this,cx|{this.busy=false;cx.notify();});}
        }).detach();
    }
    fn install(&mut self,project:Project,path:Option<PathBuf>,window:&mut Window,cx:&mut Context<Self>){
        self.clear_drag();self.scrolls.clear();self.dock_bounds.clear();self.project=project;self.checkpoint=self.project.clone();self.undo_stack.clear();self.redo_stack.clear();self.history_group=false;self.path=path;self.revision=0;self.saved_revision=0;self.generation+=1;self.renaming=false;self.show_status=false;self.pending=None;self.continue_after_save=false;self.harmonic_bank=0;self.selected_harmonic=0;self.instance_harmonic=0;self.track_bounds.clear();self.query.clear();self.resizing=None;self.panel_bounds.clear();self.pair_bounds.clear();
        self.search.update(cx,|s,cx|s.set_value("",window,cx));
        let title=self.project.title.clone();self.title.update(cx,|s,cx|s.set_value(title,window,cx));
        let name=self.project.draft.name.clone();self.sound_name.update(cx,|s,cx|s.set_value(name,window,cx));
        self.sync_sliders(false,window,cx);self.sync_sliders(true,window,cx);
        crate::primitives::apply_theme(self.project.dark,window,cx);cx.notify();
    }
    fn save(&mut self,as_new:bool,window:&mut Window,cx:&mut Context<Self>){
        if self.busy{return;}self.busy=true;cx.notify();
        let existing=if as_new{None}else{self.path.clone()};
        let directory=self.path.as_ref().and_then(|p|p.parent()).map(|p|p.to_path_buf()).unwrap_or_else(||std::env::current_dir().unwrap_or_else(|_|".".into()));
        let choice=if existing.is_none(){Some(cx.prompt_for_new_path(&directory,Some("session.overtone")))}else{None};
        cx.spawn_in(window,async move|this,cx|{
            let path=if let Some(choice)=choice{match choice.await{Ok(Ok(path))=>path,_=>None}}else{existing};
            let Some(mut path)=path else{let _=this.update(cx,|this,cx|{this.busy=false;this.continue_after_save=false;cx.notify();});return;};
            if path.extension().is_none(){path.set_extension("overtone");}
            let snapshot=this.update(cx,|this,_|(this.project.clone(),this.revision,this.generation)).ok();
            let Some((project,revision,generation))=snapshot else{return;};let save_path=path.clone();
            let result=cx.background_spawn(async move{persistence::save(&project,&save_path)}).await;
            let _=this.update_in(cx,|this,window,cx|{
                this.busy=false;
                if this.generation!=generation{return;}
                match result {
                    Ok(saved)=>{for (id,path) in saved.sources{if let Some(s)=this.project.sources.iter_mut().find(|s|s.id==id){s.path=path;}}
                        this.checkpoint=this.project.clone();this.path=Some(path);this.saved_revision=revision;this.notice="Project saved with voice references".into();
                        if this.continue_after_save && !this.dirty(){if let Some(pending)=this.pending.take(){this.continue_after_save=false;this.replace(pending,window,cx);}}
                    },Err(e)=>{this.notice=format!("Save failed: {e}");this.continue_after_save=false;}
                }cx.notify();
            });
        }).detach();
    }
    fn import_voice(&mut self,window:&mut Window,cx:&mut Context<Self>){
        if self.busy{return;}self.busy=true;
        let choice=cx.prompt_for_paths(PathPromptOptions{files:true,directories:false,multiple:false,prompt:Some("Import voice reference (stored, not decoded)".into())});cx.notify();
        cx.spawn_in(window,async move|this,cx|{
            let path=match choice.await{Ok(Ok(Some(paths)))=>paths.into_iter().next(),_=>None};
            if let Some(path)=path {
                let result=cx.background_spawn(async move{persistence::import_source(&path)}).await;
                let _=this.update(cx,|this,cx|{this.busy=false;match result{Ok(mut source)=>{source.id=this.project.allocate();this.project.draft.source=Some(source.id);this.project.sources.push(source);this.notice="Voice reference attached. Analysis is pending the audio engine.".into();this.changed(cx);},Err(e)=>{this.notice=e;cx.notify();}}});
            }else{let _=this.update(cx,|this,cx|{this.busy=false;cx.notify();});}
        }).detach();
    }
    fn workspace_file(&mut self,export:bool,window:&mut Window,cx:&mut Context<Self>){
        if self.busy{return;}self.busy=true;cx.notify();
        if export{
            let directory=self.path.as_ref().and_then(|p|p.parent()).map(|p|p.to_path_buf()).unwrap_or_else(||std::env::current_dir().unwrap_or_else(|_|".".into()));
            let choice=cx.prompt_for_new_path(&directory,Some("workspace.settings.json"));
            cx.spawn_in(window,async move|this,cx|{
                let path=match choice.await{Ok(Ok(path))=>path,_=>None};
                let Some(path)=path else{let _=this.update(cx,|s,cx|{s.busy=false;cx.notify();});return;};
                let settings=this.update(cx,|s,_|crate::storage::WorkspaceSettings::from_project(&s.project)).ok();let Some(settings)=settings else{return;};
                let result=cx.background_spawn(async move{persistence::save_settings(&settings,&path)}).await;
                let _=this.update(cx,|s,cx|{s.busy=false;s.notice=match result{Ok(())=>"Workspace profiles and settings exported".into(),Err(e)=>format!("Settings export failed: {e}")};cx.notify();});
            }).detach();
        }else{
            let choice=cx.prompt_for_paths(PathPromptOptions{files:true,directories:false,multiple:false,prompt:Some("Load workspace settings JSON".into())});
            cx.spawn_in(window,async move|this,cx|{
                let path=match choice.await{Ok(Ok(Some(paths)))=>paths.into_iter().next(),_=>None};
                let Some(path)=path else{let _=this.update(cx,|s,cx|{s.busy=false;cx.notify();});return;};
                let result=cx.background_spawn(async move{persistence::load_settings(&path)}).await;
                let _=this.update_in(cx,|s,window,cx|{s.busy=false;match result{
                    Ok(settings)=>match settings.apply(&mut s.project){Ok(())=>{s.clear_drag();s.resizing=None;s.panel_bounds.clear();s.pair_bounds.clear();crate::primitives::apply_theme(s.project.dark,window,cx);s.notice="Workspace settings loaded; save the project to keep them".into();s.changed(cx);},Err(e)=>{s.notice=e;cx.notify();}},
                    Err(e)=>{s.notice=format!("Settings load failed: {e}");cx.notify();}
                }});
            }).detach();
        }
    }
}
impl Focusable for Studio {fn focus_handle(&self,_:&App)->FocusHandle{self.focus.clone()}}
impl Render for Studio {fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{crate::workspace::studio_view(self,window,cx)}}
