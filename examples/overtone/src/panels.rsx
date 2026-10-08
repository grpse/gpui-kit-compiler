use gpui_kit::{prelude::*, *};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable,button::{Button,ButtonVariants as _},menu::DropdownMenu,Sizable as _,scroll::ScrollbarAxis};
use crate::studio::LibraryMenu;
use crate::primitives::StudioSlider as Slider;
use crate::{model::*, studio::{Studio,Action}, primitives::*};

#[gpui]
pub fn module_view(kind:Module,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    match kind {Module::Library=>library(studio,cx),Module::Source=>source(studio,cx),Module::Harmonics=>harmonics(studio,cx),Module::Controls=>controls(false,studio,cx),Module::Keyboard=>keyboard(studio,cx),Module::Timeline=>timeline(studio,cx),Module::Inspector=>inspector(studio,cx),Module::Reconstruction=>crate::reconstruction::view(studio,cx)}
}
#[gpui]
pub fn library(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let query=studio.query.to_lowercase();
    let sounds=studio.project.library.iter().filter(|s|s.name.to_lowercase().contains(&query)).collect::<Vec<_>>();
    <div flex flex-col gap={u(10.)}>
        <Input args={&studio.search} id="sound-search" />
        <div flex justify-between text-size={u(11.)}>{caption(format!("{} sounds",sounds.len()),studio.project.dark)}</div>
        {if sounds.is_empty(){<div p={u(14.)} bg={rgb(p.soft)} text-size={u(12.)}>{if query.is_empty(){"Record a take to create your first sounds."}else{"No sounds match your search."}}</div>.into_any_element()}else{<div flex flex-col gap={u(8.)} children={sounds.into_iter().map(|sound|{
            <div id={format!("library-sound-{}",sound.id)} test-support cursor-grab flex flex-col gap={u(7.)} border={u(1.)} border-color={rgb(p.border)} p={u(10.)}
                on-drag:args={(ClipDrag{id:sound.id,label:sound.name.clone(),library:true},|drag,_,_,cx|cx.new(|_|drag.clone()))}>
                <div flex items-center justify-between gap={u(6.)}><div flex-1 min-w-0 truncate text-size={u(13.)}>{sound.name.clone()}</div>
                {let id=sound.id;let editing=studio.project.editing_sound==Some(id);let focus=studio.focus.clone();let recorded=sound.recorded_sample;Button::new(format!("sound-menu-{id}")).ghost().small().label("⋯").dropdown_menu(move|menu,_,_|{
                    let menu=menu.action_context(focus.clone());
                    if recorded{menu.menu("Remove from sounds",Box::new(LibraryMenu{id,operation:3}))}
                    else{menu.menu("Edit sound",Box::new(LibraryMenu{id,operation:0})).menu_with_disabled("Save changes",Box::new(LibraryMenu{id,operation:1}),!editing).menu_with_disabled("Save as new sound",Box::new(LibraryMenu{id,operation:2}),!editing).separator().menu("Remove from library",Box::new(LibraryMenu{id,operation:3}))}
                })}</div>
                {if sound.recorded_sample{sound_waveform(sound,studio,30.)}else{caption(format!("{} harmonics",sound.harmonics.len()),studio.project.dark)}}
                {if let Some(c)=&sound.capture{caption(format!("{:.2}s · {}",c.duration_seconds,if sound.recorded_sample{"Recorded sound"}else{"Synthesized sound"}),studio.project.dark)}else{div().into_any_element()}}
                <div flex gap={u(6.)} flex-wrap>{button(format!("play-sound-{}",sound.id),if studio.auditioning==Some(sound.id){"Stop"}else{"Listen"},Action::PlaySound(sound.id),studio.auditioning==Some(sound.id),studio,cx).disabled(studio.busy || studio.recording_active() || !cfg!(feature="audio-output"))}{button(format!("place-{}",sound.id),"+ Timeline",Action::AddLibrary(sound.id),false,studio,cx)}</div>
            </div>
        })}></div>.into_any_element()}}
    </div>.into_any_element()
}
#[gpui]
pub fn source(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let active=studio.recording_active();let settings=&studio.project.recording;
    <div flex flex-col gap={u(12.)}>
        <div text-size={u(13.)}>Record → split → compose</div>
        {caption("Record your voice, taps, or instruments. Pause between sounds; each sound becomes its own clip on the timeline.",studio.project.dark)}
        <div flex gap={u(6.)} flex-wrap>
            {button("import-voice","Import audio",Action::ImportVoice,false,studio,cx).disabled(studio.busy || active)}
            {if active{<div flex flex-wrap gap={u(6.)}>{button("finish-recording","Stop & split",Action::FinishRecord(false),true,studio,cx).disabled(studio.capture_starting)}{button("discard-recording","Discard take",Action::FinishRecord(true),false,studio,cx).disabled(studio.capture_starting)}</div>.into_any_element()}else{button("record-voice","Record microphone",Action::Record,true,studio,cx).disabled(studio.busy || !cfg!(feature="audio-output")).into_any_element()}}
        </div>
        <div flex flex-col gap={u(7.)} p={u(10.)} bg={rgb(p.soft)} border={u(1.)} border-color={rgb(p.border)} text-size={u(11.)}>
            <div>{if active{format!("● Recording {:.1}s / 120s",studio.capture_seconds)}else if studio.busy{"Processing…".into()}else{"Microphone ready · maximum take 120s".into()}}</div>
            {if active{<div flex flex-col gap={u(5.)}><div>{studio.capture_device.clone()}</div><div h={u(6.)} w={u(160.)} bg={rgb(p.border)}><div h-full w={u(160.*studio.capture_peak)} bg={rgb(if studio.capture_peak>0.95{0xe26767}else{0x7ba78c})}/></div></div>.into_any_element()}else{caption(studio.notice.clone(),studio.project.dark)}}
        </div>
        <div flex flex-wrap gap={u(8.)}>
        {stepper("analysis-gap",format!("Split after {} ms silence",settings.split_gap_ms),Action::AnalysisOption(2,-50),Action::AnalysisOption(2,50),studio,cx)}
        {stepper("analysis-minimum",format!("Minimum sound {} ms",settings.min_sound_ms),Action::AnalysisOption(3,-50),Action::AnalysisOption(3,50),studio,cx)}
        {stepper("analysis-silence",format!("Silence {:.0} dB",settings.silence_db),Action::AnalysisOption(1,-5),Action::AnalysisOption(1,5),studio,cx)}
        </div>
        {if studio.project.sources.is_empty(){caption("No recording attached",studio.project.dark)}else{<div flex flex-col gap={u(8.)} children={studio.project.sources.iter().map(|s|{
            <div flex flex-col gap={u(5.)} p={u(8.)} border={u(1.)} border-color={rgb(p.border)}>
                <div text-size={u(12.)}>{s.name.clone()}</div>
                {caption(format!("{:.1} MB · {}",s.bytes as f64/1_000_000.,if s.path.is_file(){"stored recording"}else{"file missing"}),studio.project.dark)}
                <div flex flex-wrap gap={u(5.)}>{button(format!("attach-{}",s.id),if studio.project.draft.source==Some(s.id){"Attached"}else{"Attach"},Action::AttachVoice(s.id),studio.project.draft.source==Some(s.id),studio,cx)}{button(format!("analyze-{}",s.id),"Split → timeline",Action::AnalyzeVoice(s.id),false,studio,cx).disabled(studio.busy || active || !s.path.is_file())}</div>
            </div>
        })}></div>.into_any_element()}}
        {caption("Clips keep the original recorded audio. Adjust the silence settings, or select a clip and split it at Insert.",studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
pub fn harmonics(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let sound=&studio.project.draft;let start=studio.harmonic_bank*8;
    <div flex flex-col gap={u(12.)}>
        <div flex items-center gap={u(8.)} flex-wrap><div flex-1 min-w={u(140.)}><Input args={&studio.sound_name} id="draft-sound-name"/></div>{button("new-draft","New sound",Action::NewSound,false,studio,cx)}{button("save-sound",if studio.project.editing_sound.is_some(){"Save changes"}else{"Save to library"},Action::SaveSound,true,studio,cx)}{button("add-draft","+ Timeline",Action::AddDraft,false,studio,cx)}</div>
        {caption(if studio.project.editing_sound.is_some(){"Editing library record · Save changes updates this sound"}else{"New sound · Save to library creates a record"},studio.project.dark)}
        <div flex items-center justify-between gap={u(8.)} flex-wrap>
            {stepper("harmonic-count",format!("{} / 32 partials",sound.harmonics.len()),Action::HarmonicCount(-1),Action::HarmonicCount(1),studio,cx)}
            {stepper("harmonic-bank",format!("Bank {}",studio.harmonic_bank+1),Action::HarmonicBank(-1),Action::HarmonicBank(1),studio,cx)}
        </div>
        <div flex gap={u(6.)} h={u(162.)} children={sound.harmonics.iter().enumerate().skip(start).take(8).map(|(i,h)|{
            <div flex flex-col items-center flex-1 min-w={u(26.)} gap={u(5.)}>
                <div id={format!("partial-select-{i}")} test-support cursor-pointer text-size={u(11.)} px={u(5.)} bg={rgb(if studio.selected_harmonic==i{p.border}else{p.soft})} on-click={cx.listener(move|this,_,window,cx|this.select_harmonic(i,window,cx))}>{format!("H{}",h.multiple)}</div>
                <Slider args={&studio.draft_sliders[i]} vertical h={u(86.)}/>
                {caption(format!("{:.0}%",h.amplitude*100.),studio.project.dark)}
                {button(format!("remove-harmonic-{i}"),"×",Action::RemoveHarmonic(false,i),false,studio,cx).disabled(sound.harmonics.len()<=1)}
            </div>
        })}></div>
        <div flex gap={u(16.)}>{slider_row("Phase",32,format!("{:.0}°",sound.harmonics[studio.selected_harmonic.min(sound.harmonics.len()-1)].phase),false,studio)}{slider_row("Detune",33,format!("{:.0} cents",sound.harmonics[studio.selected_harmonic.min(sound.harmonics.len()-1)].detune),false,studio)}</div>
        {caption(format!("H{} selected · add up to 32 harmonics · × removes a harmonic; zero its level to mute it",sound.harmonics[studio.selected_harmonic.min(sound.harmonics.len()-1)].multiple),studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
pub fn controls(instance:bool,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let sound=if instance{studio.project.selected().map(|c|&c.sound).unwrap_or(&studio.project.draft)}else{&studio.project.draft};
    <div flex flex-col gap={u(14.)}>
        <div flex gap={u(8.)} flex-wrap>{knob("Brightness",35,sound.brightness,format!("{:.0}%",sound.brightness*100.),instance,studio,cx)}{knob("Sound gain",36,sound.gain,format!("{:.0}%",sound.gain*100.),instance,studio,cx)}{knob("Noise level",34,sound.noise.level,format!("{:.0}%",sound.noise.level*100.),instance,studio,cx)}</div>
        <div flex items-center gap={u(12.)}>{caption("Noise color",studio.project.dark)}{button(if instance{"instance-noise-color"}else{"noise-color"},sound.noise.color.label(),Action::NoiseColor(instance),false,studio,cx)}</div>
        <div flex gap={u(12.)} flex-wrap>
            {slider_row("Attack",37,format!("{:.0} ms",sound.envelope.attack_ms),instance,studio)}{slider_row("Decay",38,format!("{:.0} ms",sound.envelope.decay_ms),instance,studio)}
            {slider_row("Sustain",39,format!("{:.0}%",sound.envelope.sustain*100.),instance,studio)}{slider_row("Release",40,format!("{:.0} ms",sound.envelope.release_ms),instance,studio)}
        </div>
    </div>.into_any_element()
}
#[gpui]
pub fn keyboard(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let names=["C","C♯","D","D♯","E","F","F♯","G","G♯","A","A♯","B","C"];
    <div flex flex-col gap={u(10.)}>
        <div flex justify-between gap={u(10.)} flex-wrap>{caption(if let Some(c)=studio.project.selected(){format!("Write notes into {} · {} notes",c.name,c.notes.len())}else{"Press a key to create a draft instance on the target track".into()},studio.project.dark)}{button("clear-notes","Clear notes",Action::ClearNotes,false,studio,cx)}</div>
        <div flex gap={u(3.)} children={names.into_iter().enumerate().map(|(i,name)|{
            let black=name.contains('♯');
            <div id={format!("key-{i}")} test-support flex-1 min-w={u(20.)} h={u(if black{60.}else{84.})} flex items-end justify-center pb={u(8.)} cursor-pointer bg={rgb(if black{0x333333}else{0xf8f8f8})} text-color={rgb(if black{0xffffff}else{0x333333})} border={u(1.)} border-color={rgb(p.border)} text-size={u(11.)} on-click={cx.listener(move|this,_,window,cx|this.dispatch(Action::Note(60+i as u8),window,cx))}>{format!("{name}{}",if i==12{5}else{4})}</div>
        })}></div>
        {caption("Each click stores a 1-beat MIDI note. Start Audio → Preview draft to enable keyboard audition.",studio.project.dark)}
    </div>.into_any_element()
}
#[derive(Clone)]
pub struct ClipDrag {pub id:Id,pub label:String,pub library:bool}
impl Render for ClipDrag {
    #[gpui]
    fn render(&mut self,_:&mut Window,_:&mut Context<Self>)->impl IntoElement{<div p={u(12.)} bg={rgb(0x444444)} text-color={rgb(0xffffff)}>{self.label.clone()}</div>}
}
#[gpui]
pub fn timeline(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);
    let span=studio.project.clips.iter().map(|c|c.start_seconds+studio.project.clip_seconds(c)).fold(studio.timeline_span,f64::max).max(studio.project.insert_seconds+4.).max(studio.playhead_seconds+2.);
    let pps=48.*studio.project.timeline.zoom as f64;
    let weak=cx.entity().downgrade();let handle=studio.scroll("timeline-scroll");
    let snap=studio.project.timeline.snap_beats;
    let tick=(64./pps).max(1.).log2().ceil().exp2();
    <div flex flex-col min-w-0 gap={u(10.)}>
        <div flex items-center gap={u(6.)} flex-wrap>
            {button("export-timeline","Export WAV",Action::ExportTimeline,false,studio,cx).disabled(studio.busy || studio.recording_active())}
            {button("timeline-play",if studio.timeline_paused{"Resume"}else{"Play"},Action::TimelinePlay,studio.timeline_playing && !studio.timeline_paused,studio,cx).disabled(studio.busy || studio.audio_starting || studio.recording_active() || !cfg!(feature="audio-output"))}
            {button("timeline-pause","Pause",Action::TimelinePause,studio.timeline_paused,studio,cx).disabled(!studio.timeline_playing || studio.timeline_paused)}
            {button("timeline-stop","Stop",Action::TimelineStop,false,studio,cx)}
            {caption(format!("{:.2}s",studio.playhead_seconds),studio.project.dark)}
            {stepper("timeline-zoom",format!("Time {:.0}%",studio.project.timeline.zoom*100.),Action::TimelineZoom(0.5),Action::TimelineZoom(2.),studio,cx)}
            {button("timeline-grid",if snap==0.{"Snap off".into()}else{format!("Snap {snap} beat{}",if snap==1.{""}else{"s"})},Action::TimelineGrid,false,studio,cx)}
            {button("add-track","+ Track",Action::AddTrack,false,studio,cx)}
            {stepper("session-tempo",format!("{:.0} BPM",studio.project.session_bpm),Action::SessionTempo(-1.),Action::SessionTempo(1.),studio,cx)}
            {stepper("insertion",format!("Insert {:.2}s",studio.project.insert_seconds),Action::Insertion(-if snap>0.{snap*60./studio.project.tempo(studio.project.target_track)}else{0.25}),Action::Insertion(if snap>0.{snap*60./studio.project.tempo(studio.project.target_track)}else{0.25}),studio,cx)}
        </div>
        <div relative min-w-0>
        <div id="timeline-scroll" test-support min-w-0 overflow-x-scroll track-scroll={&handle} pb={u(14.)}>
        <div w={u((190.+span*pps) as f32)} flex flex-col gap={u(7.)}>
        <div relative ml={u(190.)} h={u(18.)} text-size={u(10.)} text-color={rgb(p.muted)} children={(0..=((span/tick).ceil() as usize).min(2048)).map(|i|<div absolute left={u((i as f64*tick*pps) as f32)}>{format!("{:.0}s",i as f64*tick)}</div>)}></div>
        <div flex flex-col gap={u(7.)} children={studio.project.tracks.iter().map(|track|{
            let id=track.id;let weak=weak.clone();let clips=studio.project.clips.iter().filter(|c|c.track==id).collect::<Vec<_>>();
            let beat=60./studio.project.tempo(id);let subdivision=if snap>0.{snap}else{1.};
            let stride=((4./(beat*subdivision*pps)).ceil().max(1.)).max((span/(beat*subdivision)/2048.).ceil()).log2().ceil().exp2();
            let grid=beat*subdivision*stride;
            <div flex gap={u(10.)} h={u(88.)}>
                <div w={u(180.)} flex-shrink-0 flex flex-col gap={u(5.)}>
                    {button(format!("target-{id}"),track.name.clone(),Action::TargetTrack(id),studio.project.target_track==id,studio,cx)}
                    <div flex gap={u(4.)}>{button(format!("mute-{id}"),"M",Action::TrackMute(id),track.muted,studio,cx)}{button(format!("solo-{id}"),"S",Action::TrackSolo(id),track.solo,studio,cx)}{button(format!("link-{id}"),if track.follow_session{"Linked"}else{"Own BPM"},Action::TrackLink(id),track.follow_session,studio,cx)}</div>
                    {stepper(&format!("tempo-{id}"),format!("{:.0} BPM",studio.project.tempo(id)),Action::Tempo(id,-1.),Action::Tempo(id,1.),studio,cx)}
                </div>
                <div id={format!("lane-{id}")} test-support relative flex-shrink-0 w={u((span*pps) as f32)} bg={rgb(p.soft)} border={u(1.)} border-color={rgb(p.border)}
                    on-click={cx.listener(move|this,_,window,cx|{
                        if this.busy || this.recording_active(){return;}
                        if let Some(bounds)=this.track_bounds.get(&id){
                            let fraction=((window.mouse_position().x-bounds.left())/bounds.size.width).clamp(0.,1.) as f64;
                            this.project.target_track=id;this.project.insert_seconds=this.project.snap_seconds(fraction*span,id);this.changed(cx);
                        }
                    })}
                    on-drop={cx.listener(move|this,drag:&ClipDrag,window,cx|{
                        if this.busy || this.pending.is_some() || this.recording_active(){return;}
                        if let Some(bounds)=this.track_bounds.get(&id).copied() {
                            let fraction=((window.mouse_position().x-bounds.origin.x)/bounds.size.width).clamp(0.,1.) as f64;
                            if drag.library {this.project.place_library_sound(drag.id,id,fraction*span);}else{this.project.place_clip(drag.id,id,fraction*span);}
                            this.sync_sliders(true,window,cx);this.changed(cx);
                        }
                    })}>
                    <canvas args={(move|bounds,_,cx|{let _=weak.update(cx,|this,_|{this.track_bounds.insert(id,bounds);});},|_,_,_,_|{})} absolute size-full />
                    <div absolute size-full children={(0..=((span/grid).ceil() as usize).min(2048)).map(|i|{
                        let bar=((i as f64*subdivision*stride/4.).fract()).abs()<0.001;
                        <div absolute left={u((i as f64*grid*pps) as f32)} top-0 bottom-0 w={u(if bar{2.}else{1.})} bg={rgba(if bar{(p.muted<<8)|0x80}else{(p.border<<8)|0x88})}/>
                    })}/>
                    <div absolute size-full children={clips.into_iter().map(|clip|{
                        let selected=studio.project.selected_clip==Some(clip.id);let clip_id=clip.id;let duration=studio.project.clip_seconds(clip);
                        <div id={format!("clip-{}",clip.id)} test-support absolute left={u((clip.start_seconds*pps) as f32)} top={u(9.)} w={u((duration*pps) as f32)} h={u(58.)} px={u(7.)} py={u(6.)} overflow-hidden cursor-pointer border={u(1.)} border-color={rgb(if selected{p.strong}else{p.border})} bg={rgb(if selected{p.border}else{p.panel})} text-size={u(11.)}
                            on-click={cx.listener(move|this,_,window,cx|this.dispatch(Action::SelectClip(clip_id),window,cx))}
                            on-drag:args={(ClipDrag{id:clip.id,label:clip.name.clone(),library:false},|drag,_,_,cx|cx.new(|_|drag.clone()))}>
                            <div truncate>{clip.name.clone()}</div>
                            {if clip.sound.recorded_sample{sound_waveform(&clip.sound,studio,19.)}else{caption(format!("{} notes",clip.notes.len()),studio.project.dark)}}
                            <div text-size={u(10.)} truncate text-color={rgb(p.muted)}>{format!("{:.2}s{}",duration,if clip.muted{" · M"}else{""})}</div>
                        </div>
                    })}></div>
                    <div absolute left={u((studio.project.insert_seconds*pps) as f32)} top-0 bottom-0 w={u(1.)} bg={rgb(p.strong)}/>
                    {if studio.timeline_playing || studio.playhead_seconds>0.{<div absolute left={u((studio.playhead_seconds*pps) as f32)} top-0 bottom-0 w={u(2.)} bg={rgb(0xe8b665)}/>.into_any_element()}else{div().into_any_element()}}
                </div>
            </div>
        })}></div>
        </div></div>
        {scrollbar_layer("bar-timeline-scroll".into(),handle,ScrollbarAxis::Horizontal)}
        </div>
        {caption("Drag sounds onto tracks, move clips, or duplicate them to build a rhythm. Click a lane to set Insert; recorded clips keep their original length.",studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
pub fn inspector(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let Some(clip)=studio.project.selected() else{return caption("Select a clip to move, split, duplicate, or adjust its level.",studio.project.dark);};
    if clip.sound.recorded_sample{return recorded_inspector(clip,studio,cx);}
    <div flex flex-col gap={u(12.)}>
        <div text-size={u(14.)}>{clip.name.clone()}</div>{caption("Edits affect this instance only",studio.project.dark)}
        <div flex gap={u(5.)} flex-wrap>{button("duplicate-clip","Duplicate",Action::DuplicateClip,false,studio,cx)}{button("delete-clip","Delete",Action::DeleteClip,false,studio,cx)}{button("reset-instance","Reset sound",Action::ResetInstance,false,studio,cx)}</div>
        {stepper("clip-start",format!("Start {:.2}s",clip.start_seconds),Action::ClipTiming(true,-0.25),Action::ClipTiming(true,0.25),studio,cx)}
        {stepper("clip-duration",format!("{:.2} beats",clip.duration_beats),Action::ClipTiming(false,-0.25),Action::ClipTiming(false,0.25),studio,cx)}
        {stepper("clip-gain",format!("Level {:.0}%",clip.gain*100.),Action::ClipGain(-0.05),Action::ClipGain(0.05),studio,cx)}
        <div flex flex-wrap gap={u(4.)}>{button("mute-clip",if clip.muted{"Unmute instance"}else{"Mute instance"},Action::ClipMute,clip.muted,studio,cx)}<div flex flex-wrap gap={u(4.)} children={studio.project.tracks.iter().map(|t|button(format!("move-to-{}",t.id),t.name.clone(),Action::MoveClipTrack(t.id),clip.track==t.id,studio,cx))}></div></div>
        {controls(true,studio,cx)}
        <div text-size={u(12.)}>Instance harmonics</div>
        {stepper("instance-partials",format!("{} / 32",clip.sound.harmonics.len()),Action::InstanceHarmonicCount(-1),Action::InstanceHarmonicCount(1),studio,cx)}
        <div flex flex-col gap={u(7.)} children={clip.sound.harmonics.iter().enumerate().map(|(i,h)|<div flex items-center gap={u(8.)}><div id={format!("instance-partial-{i}")} test-support w={u(24.)} cursor-pointer text-size={u(11.)} on-click={cx.listener(move|this,_,window,cx|this.select_instance_harmonic(i,window,cx))}>{format!("H{}",h.multiple)}</div><Slider args={&studio.instance_sliders[i]} horizontal flex-1/>{caption(format!("{:.0}%",h.amplitude*100.),studio.project.dark)}{button(format!("instance-remove-harmonic-{i}"),"×",Action::RemoveHarmonic(true,i),false,studio,cx).disabled(clip.sound.harmonics.len()<=1)}</div>)}></div>
        {caption(format!("H{} selected",clip.sound.harmonics[studio.instance_harmonic.min(clip.sound.harmonics.len()-1)].multiple),studio.project.dark)}
        {slider_row("Phase",32,format!("{:.0}°",clip.sound.harmonics[studio.instance_harmonic.min(clip.sound.harmonics.len()-1)].phase),true,studio)}
        {slider_row("Detune",33,format!("{:.0} cents",clip.sound.harmonics[studio.instance_harmonic.min(clip.sound.harmonics.len()-1)].detune),true,studio)}
        <div text-size={u(12.)}>{format!("Notes · {}",clip.notes.len())}</div>
        <div flex flex-col gap={u(6.)} children={clip.notes.iter().enumerate().map(|(i,n)|<div flex items-center gap={u(4.)} flex-wrap>
            {caption(format!("MIDI {} · beat {:.1}",n.midi,n.start_beat),studio.project.dark)}{button(format!("note-earlier-{i}"),"←",Action::MoveNote(i,-0.25),false,studio,cx)}{button(format!("note-later-{i}"),"→",Action::MoveNote(i,0.25),false,studio,cx)}{button(format!("note-remove-{i}"),"×",Action::DeleteNote(i),false,studio,cx)}
        </div>)}></div>
    </div>.into_any_element()
}

#[gpui]
fn recorded_inspector(clip:&Clip,studio:&Studio,cx:&mut Context<Studio>)->AnyElement {
    <div flex flex-col gap={u(12.)}>
        <div text-size={u(14.)}>{clip.name.clone()}</div>
        {sound_waveform(&clip.sound,studio,72.)}
        {caption(format!("Recorded sound · {:.2}s",studio.project.clip_seconds(clip)),studio.project.dark)}
        <div flex gap={u(5.)} flex-wrap>{button("listen-clip",if studio.auditioning==Some(clip.id){"Stop"}else{"Listen"},Action::PreviewClip,false,studio,cx).disabled(studio.busy || studio.recording_active() || !cfg!(feature="audio-output"))}{button("duplicate-clip","Duplicate",Action::DuplicateClip,false,studio,cx)}{button("delete-clip","Delete",Action::DeleteClip,false,studio,cx)}</div>
        {stepper("clip-start",format!("Start {:.2}s",clip.start_seconds),Action::ClipTiming(true,-0.25),Action::ClipTiming(true,0.25),studio,cx)}
        {stepper("clip-gain",format!("Level {:.0}%",clip.gain*100.),Action::ClipGain(-0.05),Action::ClipGain(0.05),studio,cx)}
        {button("mute-clip",if clip.muted{"Unmute"}else{"Mute"},Action::ClipMute,clip.muted,studio,cx)}
        {caption("Move to track",studio.project.dark)}
        <div flex flex-wrap gap={u(4.)} children={studio.project.tracks.iter().map(|t|button(format!("move-to-{}",t.id),t.name.clone(),Action::MoveClipTrack(t.id),clip.track==t.id,studio,cx))}></div>
        {button("split-clip",format!("Split at Insert · {:.2}s",studio.project.insert_seconds),Action::SplitClip,false,studio,cx)}
        {caption("Click inside this clip on the timeline to set Insert, then split. Undo restores the cut.",studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
fn sound_waveform(sound:&Sound,studio:&Studio,height:f32)->AnyElement {
    let source=sound.source.and_then(|id|studio.project.sources.iter().find(|s|s.id==id));
    let preview=source.and_then(|s|studio.sound_waveforms.get(&s.sha256)).and_then(|p|p.as_ref());
    let Some(preview)=preview else{return caption(if source.is_some_and(|s|!s.path.is_file()){"Recording missing"}else{"Loading waveform…"},studio.project.dark);};
    let Some(capture)=&sound.capture else{return div().into_any_element();};
    let points=preview.excerpt(capture.start_seconds,capture.duration_seconds);
    <canvas h={u(height)} w-full args={(|_,_,_|{},move|bounds,_,window,_|{
        let mut path=PathBuilder::stroke(px(1.));
        let count=points.len().div_ceil(16);
        for (i,chunk) in points.chunks(16).enumerate() {
            let (lo,hi)=chunk.iter().fold((1_f32,-1_f32),|(lo,hi),&(a,b)|(lo.min(a),hi.max(b)));
            let x=bounds.left()+bounds.size.width*i as f32/(count-1).max(1) as f32;
            path.move_to(point(x,bounds.center().y-bounds.size.height*lo*0.45));
            path.line_to(point(x,bounds.center().y-bounds.size.height*hi*0.45));
        }
        if let Ok(path)=path.build(){window.paint_path(path,rgb(0x91b5d8));}
    })}/>.into_any_element()
}
