use gpui_kit::{prelude::*, *};
use gpui_kit::component::input::Input;
use crate::primitives::StudioSlider as Slider;
use crate::{model::*, studio::{Studio,Action}, primitives::*};

#[gpui]
pub fn module_view(kind:Module,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    match kind {Module::Library=>library(studio,cx),Module::Source=>source(studio,cx),Module::Harmonics=>harmonics(studio,cx),Module::Controls=>controls(false,studio,cx),Module::Keyboard=>keyboard(studio,cx),Module::Timeline=>timeline(studio,cx),Module::Inspector=>inspector(studio,cx)}
}
#[gpui]
pub fn library(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let query=studio.query.to_lowercase();
    let sounds=studio.project.library.iter().filter(|s|s.name.to_lowercase().contains(&query)).collect::<Vec<_>>();
    <div flex flex-col gap={u(10.)}>
        <Input args={&studio.search} id="sound-search" />
        <div flex justify-between text-size={u(11.)}>{caption(format!("{} sounds",sounds.len()),studio.project.dark)}{button("library-new","New sound",Action::NewSound,false,studio,cx)}</div>
        {if sounds.is_empty(){<div p={u(14.)} bg={rgb(p.soft)} text-size={u(12.)}>{if query.is_empty(){"Save a sound from the builder to start your library."}else{"No sounds match your search."}}</div>.into_any_element()}else{<div flex flex-col gap={u(8.)} children={sounds.into_iter().map(|sound|{
            <div flex flex-col gap={u(7.)} border={u(1.)} border-color={rgb(p.border)} p={u(10.)}>
                <div text-size={u(13.)}>{sound.name.clone()}</div>
                {caption(format!("{} harmonics · {} noise {:.0}%",sound.harmonics.len(),sound.noise.color.label(),sound.noise.level*100.),studio.project.dark)}
                <div flex gap={u(6.)} flex-wrap>{button(format!("edit-{}",sound.id),"Edit copy",Action::LoadSound(sound.id),false,studio,cx)}{button(format!("place-{}",sound.id),"+ Timeline",Action::AddLibrary(sound.id),false,studio,cx)}</div>
            </div>
        })}></div>.into_any_element()}}
    </div>.into_any_element()
}
#[gpui]
pub fn source(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);
    <div flex flex-col gap={u(12.)}>
        <div text-size={u(13.)}>From voice to texture</div>
        {caption("Import a recording as a project reference. Decoding and harmonic matching will be added with the engine.",studio.project.dark)}
        <div flex gap={u(6.)} flex-wrap>{button("import-voice","Import voice",Action::ImportVoice,false,studio,cx)}{disabled("record-voice","Record",studio.project.dark)}</div>
        <div h={u(64.)} flex items-center justify-center bg={rgb(p.soft)} border={u(1.)} border-color={rgb(p.border)} text-size={u(11.)} text-color={rgb(p.muted)}>Reference audio · analysis pending</div>
        {if studio.project.sources.is_empty(){caption("No reference attached",studio.project.dark)}else{<div flex flex-col gap={u(8.)} children={studio.project.sources.iter().map(|s|{
            <div flex flex-col gap={u(4.)} p={u(8.)} border={u(1.)} border-color={rgb(p.border)}>
                <div text-size={u(12.)}>{s.name.clone()}</div>
                {caption(format!("{:.1} MB · {}",s.bytes as f64/1_000_000.,if s.path.is_file(){"stored reference"}else{"file missing"}),studio.project.dark)}
                {button(format!("attach-{}",s.id),if studio.project.draft.source==Some(s.id){"Attached to draft"}else{"Attach to draft"},Action::AttachVoice(s.id),studio.project.draft.source==Some(s.id),studio,cx)}
            </div>
        })}></div>.into_any_element()}}
        {caption("Harmonic matching limit: 32 partials",studio.project.dark)}
        {disabled("analyze-voice","Analyze recording",studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
pub fn harmonics(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let sound=&studio.project.draft;let start=studio.harmonic_bank*8;
    <div flex flex-col gap={u(12.)}>
        <div flex items-center gap={u(8.)} flex-wrap><div flex-1 min-w={u(140.)}><Input args={&studio.sound_name} id="draft-sound-name"/></div>{button("save-sound","Save to library",Action::SaveSound,true,studio,cx)}{button("add-draft","+ Timeline",Action::AddDraft,false,studio,cx)}</div>
        <div flex items-center justify-between gap={u(8.)} flex-wrap>
            {stepper("harmonic-count",format!("{} / 32 partials",sound.harmonics.len()),Action::HarmonicCount(-1),Action::HarmonicCount(1),studio,cx)}
            {stepper("harmonic-bank",format!("Bank {}",studio.harmonic_bank+1),Action::HarmonicBank(-1),Action::HarmonicBank(1),studio,cx)}
        </div>
        <div flex gap={u(6.)} h={u(128.)} children={sound.harmonics.iter().enumerate().skip(start).take(8).map(|(i,h)|{
            <div flex flex-col items-center flex-1 min-w={u(26.)} gap={u(5.)}>
                <div id={format!("partial-select-{i}")} test-support cursor-pointer text-size={u(11.)} px={u(5.)} bg={rgb(if studio.selected_harmonic==i{p.border}else{p.soft})} on-click={cx.listener(move|this,_,window,cx|this.select_harmonic(i,window,cx))}>{format!("H{}",i+1)}</div>
                <Slider args={&studio.draft_sliders[i]} vertical h={u(86.)}/>
                {caption(format!("{:.0}%",h.amplitude*100.),studio.project.dark)}
            </div>
        })}></div>
        <div flex gap={u(16.)}>{slider_row("Phase",32,format!("{:.0}°",sound.harmonics[studio.selected_harmonic.min(sound.harmonics.len()-1)].phase),false,studio)}{slider_row("Detune",33,format!("{:.0} cents",sound.harmonics[studio.selected_harmonic.min(sound.harmonics.len()-1)].detune),false,studio)}</div>
        {caption(format!("H{} selected · add up to 32 harmonics · controls store composition data",studio.selected_harmonic+1),studio.project.dark)}
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
        {caption("Each click stores a 1-beat MIDI note. Audio preview is unavailable until the engine is added.",studio.project.dark)}
    </div>.into_any_element()
}
#[derive(Clone)]
pub struct ClipDrag {pub id:Id,pub label:String}
impl Render for ClipDrag {
    #[gpui]
    fn render(&mut self,_:&mut Window,_:&mut Context<Self>)->impl IntoElement{<div p={u(12.)} bg={rgb(0x444444)} text-color={rgb(0xffffff)}>{self.label.clone()}</div>}
}
#[gpui]
pub fn timeline(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let span=studio.project.clips.iter().map(|c|c.start_seconds+studio.project.clip_seconds(c)).fold(studio.timeline_span,f64::max).max(studio.project.insert_seconds+4.);let weak=cx.entity().downgrade();
    <div flex flex-col gap={u(10.)}>
        <div flex items-center gap={u(10.)} flex-wrap>
            {disabled("play-timeline","Play",studio.project.dark)}{button("add-track","+ Track",Action::AddTrack,false,studio,cx)}
            {stepper("session-tempo",format!("{:.0} BPM",studio.project.session_bpm),Action::SessionTempo(-1.),Action::SessionTempo(1.),studio,cx)}
            {stepper("insertion",format!("Insert {:.1}s",studio.project.insert_seconds),Action::Insertion(-0.5),Action::Insertion(0.5),studio,cx)}
        </div>
        <div flex ml={u(190.)} justify-between text-size={u(10.)} text-color={rgb(p.muted)} children={(0..=4).map(|i|<div>{format!("{:.1}s",span*i as f64/4.)}</div>)}></div>
        <div flex flex-col gap={u(7.)} children={studio.project.tracks.iter().map(|track|{
            let id=track.id;let weak=weak.clone();let clips=studio.project.clips.iter().filter(|c|c.track==id).collect::<Vec<_>>();
            <div flex gap={u(10.)} min-h={u(88.)}>
                <div w={u(180.)} flex-shrink-0 flex flex-col gap={u(5.)}>
                    {button(format!("target-{id}"),track.name.clone(),Action::TargetTrack(id),studio.project.target_track==id,studio,cx)}
                    <div flex gap={u(4.)}>{button(format!("mute-{id}"),"M",Action::TrackMute(id),track.muted,studio,cx)}{button(format!("solo-{id}"),"S",Action::TrackSolo(id),track.solo,studio,cx)}{button(format!("link-{id}"),if track.follow_session{"Linked"}else{"Own BPM"},Action::TrackLink(id),track.follow_session,studio,cx)}</div>
                    {stepper(&format!("tempo-{id}"),format!("{:.0}",studio.project.tempo(id)),Action::Tempo(id,-1.),Action::Tempo(id,1.),studio,cx)}
                </div>
                <div id={format!("lane-{id}")} test-support relative flex-1 min-w={u(140.)} bg={rgb(p.soft)} border={u(1.)} border-color={rgb(p.border)}
                    on-drop={cx.listener(move|this,drag:&ClipDrag,window,cx|{
                        let bounds=this.track_bounds.get(&id).copied();
                        if let Some(bounds)=bounds {let fraction=((window.mouse_position().x-bounds.origin.x)/bounds.size.width).clamp(0.,1.) as f64;let start=(fraction*span*4.).round()/4.;
                            if let Some(c)=this.project.clips.iter_mut().find(|c|c.id==drag.id){c.track=id;c.start_seconds=start;this.project.selected_clip=Some(drag.id);this.sync_sliders(true,window,cx);this.changed(cx);}}
                    })}>
                    <canvas args={(move|bounds,_,cx|{let _=weak.update(cx,|this,_|{this.track_bounds.insert(id,bounds);});},|_,_,_,_|{})} absolute size-full />
                    <div absolute left={relative((studio.project.insert_seconds/span) as f32)} top-0 bottom-0 w={u(1.)} bg={rgb(p.strong)}/>
                    <div absolute size-full children={clips.into_iter().map(|clip|{
                        let selected=studio.project.selected_clip==Some(clip.id);let clip_id=clip.id;let duration=studio.project.clip_seconds(clip);
                        <div id={format!("clip-{}",clip.id)} test-support absolute left={relative((clip.start_seconds/span) as f32)} top={u(9.)} w={relative((duration/span) as f32)} min-w={u(38.)} h={u(58.)} px={u(7.)} py={u(6.)} overflow-hidden cursor-pointer border={u(1.)} border-color={rgb(if selected{p.strong}else{p.border})} bg={rgb(if selected{p.border}else{p.panel})} text-size={u(11.)}
                            on-click={cx.listener(move|this,_,window,cx|this.dispatch(Action::SelectClip(clip_id),window,cx))}
                            on-drag:args={(ClipDrag{id:clip.id,label:clip.name.clone()},|drag,_,_,cx|cx.new(|_|drag.clone()))}>
                            <div truncate>{clip.name.clone()}</div><div text-size={u(10.)} truncate text-color={rgb(p.muted)}>{format!("{}n · {:.1}b{}",clip.notes.len(),clip.duration_beats,if clip.muted{" · M"}else{""})}</div>
                        </div>
                    })}></div>
                </div>
            </div>
        })}></div>
        {caption("Drag instances between tracks and along the timeline. Starts use seconds; note lengths follow each track’s tempo.",studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
pub fn inspector(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let Some(clip)=studio.project.selected() else{return caption("Select a timeline instance to adjust its own sound, timing, and notes.",studio.project.dark);};
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
        <div flex flex-col gap={u(7.)} children={clip.sound.harmonics.iter().enumerate().map(|(i,h)|<div flex items-center gap={u(8.)}><div id={format!("instance-partial-{i}")} test-support w={u(24.)} cursor-pointer text-size={u(11.)} on-click={cx.listener(move|this,_,window,cx|this.select_instance_harmonic(i,window,cx))}>{format!("H{}",i+1)}</div><Slider args={&studio.instance_sliders[i]} horizontal flex-1/>{caption(format!("{:.0}%",h.amplitude*100.),studio.project.dark)}</div>)}></div>
        {caption(format!("H{} selected",studio.instance_harmonic.min(clip.sound.harmonics.len()-1)+1),studio.project.dark)}
        {slider_row("Phase",32,format!("{:.0}°",clip.sound.harmonics[studio.instance_harmonic.min(clip.sound.harmonics.len()-1)].phase),true,studio)}
        {slider_row("Detune",33,format!("{:.0} cents",clip.sound.harmonics[studio.instance_harmonic.min(clip.sound.harmonics.len()-1)].detune),true,studio)}
        <div text-size={u(12.)}>{format!("Notes · {}",clip.notes.len())}</div>
        <div flex flex-col gap={u(6.)} children={clip.notes.iter().enumerate().map(|(i,n)|<div flex items-center gap={u(4.)} flex-wrap>
            {caption(format!("MIDI {} · beat {:.1}",n.midi,n.start_beat),studio.project.dark)}{button(format!("note-earlier-{i}"),"←",Action::MoveNote(i,-0.25),false,studio,cx)}{button(format!("note-later-{i}"),"→",Action::MoveNote(i,0.25),false,studio,cx)}{button(format!("note-remove-{i}"),"×",Action::DeleteNote(i),false,studio,cx)}
        </div>)}></div>
    </div>.into_any_element()
}
