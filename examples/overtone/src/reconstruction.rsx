use gpui_kit::{prelude::*,*};
use gpui_kit::component::{Disableable,scroll::ScrollbarAxis};
use crate::{model::*,studio::{Studio,Action},primitives::*};
use std::{path::PathBuf,sync::Arc};
use rsx_overtone::waveform::{SourcePreview,SineDisplay};
#[derive(Clone,PartialEq)]
pub struct SourceKey {pub id:Id,pub path:PathBuf,pub hash:String,pub bytes:u64}
#[derive(Default)]
pub struct SourceState {pub key:Option<SourceKey>,pub loading:bool,pub error:Option<String>,pub preview:Option<Arc<SourcePreview>>}
pub struct PlotData {pub sines:SineDisplay,pub original:Option<Arc<[(f32,f32)]>>,pub hz:f64,pub time:f64,pub seconds:f64,pub source_from:f64}
fn wave_color(multiple:u8)->u32{[0x91b5d8,0xe1c18a,0xa8bda0,0xb8a6ce,0xcc9da4,0x89b9ba][(multiple as usize-1)%6]}
fn visible(bounds:Bounds<Pixels>,window:&Window)->bool {
    let mask=window.content_mask().bounds;
    bounds.right()>mask.left() && bounds.left()<mask.right() && bounds.bottom()>mask.top() && bounds.top()<mask.bottom()
}
fn line(values:&[f32],bounds:Bounds<Pixels>,color:Hsla,width:f32,window:&mut Window) {
    if values.len()<2 || !visible(bounds,window){return;}
    let mut path=PathBuilder::stroke(sp(width,window));
    for (i,&value) in values.iter().enumerate(){
        let p=point(bounds.left()+bounds.size.width*(i as f32/(values.len()-1) as f32),bounds.center().y-bounds.size.height*(value.clamp(-1.,1.)*0.42));
        if i==0 {path.move_to(p);}else{path.line_to(p);}
    }
    if let Ok(path)=path.build(){window.paint_path(path,color);}
}
fn grid(bounds:Bounds<Pixels>,dark:bool,window:&mut Window) {
    if !visible(bounds,window){return;}
    let mut path=PathBuilder::stroke(sp(0.6,window));
    for i in 0..=4 {let y=bounds.top()+bounds.size.height*i as f32/4.;path.move_to(point(bounds.left(),y));path.line_to(point(bounds.right(),y));}
    for i in 0..=8 {let x=bounds.left()+bounds.size.width*i as f32/8.;path.move_to(point(x,bounds.top()));path.line_to(point(x,bounds.bottom()));}
    if let Ok(path)=path.build(){window.paint_path(path,rgba((palette(dark).border<<8)|0x88));}
}
fn peaks(values:&[(f32,f32)],bounds:Bounds<Pixels>,window:&mut Window) {
    if values.is_empty() || !visible(bounds,window){return;}
    let mut path=PathBuilder::stroke(sp(0.8,window));let mut centers=Vec::with_capacity(values.len());
    for (i,&(lo,hi)) in values.iter().enumerate(){
        let x=bounds.left()+bounds.size.width*i as f32/(values.len()-1).max(1) as f32;
        if hi-lo>0.0001{path.move_to(point(x,bounds.center().y-bounds.size.height*(lo.clamp(-1.,1.)*0.42)));path.line_to(point(x,bounds.center().y-bounds.size.height*(hi.clamp(-1.,1.)*0.42)));}
        centers.push((lo+hi)/2.);
    }
    if let Ok(path)=path.build(){window.paint_path(path,rgb(0x91b5d8));}
    line(&centers,bounds,rgb(0x91b5d8).into(),1.1,window);
}
#[gpui]
pub fn view(studio:&Studio,cx:&mut Context<Studio>)->AnyElement {
    let p=palette(studio.project.dark);let dark=studio.project.dark;let sound=studio.reconstruction_sound();let instance=studio.reconstruction_instance;
    let Some(plot)=studio.reconstruction_plot.clone() else{return caption("Preparing waveform view…",dark);};
    let selected=if instance{studio.instance_harmonic}else{studio.selected_harmonic}.min(sound.harmonics.len()-1);
    let weak=sound.weak_harmonics(studio.project.reconstruction.weak_threshold);let noise=studio.project.reconstruction.remove_noise;
    let weak_count=weak.len();let energy=sound.harmonics.iter().map(|h|h.amplitude*h.amplitude).sum::<f32>().max(1e-12);
    let handle=studio.scroll("reconstruction-partials");let weak_owner=cx.entity().downgrade();
    <div id="reconstruction-editor" test-support flex flex-col min-w-0 gap={u(10.)}>
        <div flex items-center justify-between gap={u(8.)} flex-wrap>
            <div flex flex-col gap={u(3.)}><div text-size={u(15.)}>{sound.name.clone()}</div>{caption(if instance{"Editing this timeline instance"}else if studio.project.editing_sound.is_some(){"Editing library record · save to keep your changes"}else{"Editing draft sound"},dark)}</div>
            <div flex gap={u(5.)} flex-wrap>
                {button("reconstruction-expand","Full editor",Action::AutoHeight(Module::Reconstruction),studio.project.profile().panels.iter().any(|p|p.module==Module::Reconstruction && p.height.is_none()),studio,cx)}
                {button("reconstruction-draft","Draft / library",Action::ReconstructionTarget(false),!instance,studio,cx)}
                {button("reconstruction-instance","Timeline instance",Action::ReconstructionTarget(true),instance,studio,cx).disabled(studio.project.selected().is_none())}
                {button("reconstruction-preview",if studio.auditioning==Some(if instance{studio.project.selected_clip.unwrap_or(0)}else{0}){"Stop"}else{"Listen"},Action::PreviewReconstruction,false,studio,cx).disabled(studio.busy || studio.audio_starting || studio.recording_active() || !cfg!(feature="audio-output"))}
                {if !instance{button("reconstruction-save",if studio.project.editing_sound.is_some(){"Save changes"}else{"Save to library"},Action::SaveSound,true,studio,cx).into_any_element()}else{div().into_any_element()}}
            </div>
        </div>
        <div flex items-center gap={u(8.)} flex-wrap>
            {stepper("cleanup-threshold",format!("Weak ≤ {:.0}% of strongest",studio.project.reconstruction.weak_threshold*100.),Action::ReconstructionOption(0,-0.01),Action::ReconstructionOption(0,0.01),studio,cx)}
            {button("cleanup-noise",format!("{} Remove noise",if noise{"✓"}else{"○"}),Action::ReconstructionOption(1,0.),noise,studio,cx)}
            {button("clean-reconstruction",format!("Clean sound · {weak_count} weak"),Action::CleanReconstruction,true,studio,cx).disabled(weak_count==0 && !(noise && sound.noise.level>0.))}
            {caption(format!("Noise {:.0}% · Undo restores cleanup",sound.noise.level*100.),dark)}
        </div>
        <div flex items-center justify-between gap={u(8.)} flex-wrap>
            <div text-size={u(12.)}>Original recording</div>
            {stepper("wave-cycles",format!("{:.0} cycles · {:.2} ms",studio.project.reconstruction.cycles,plot.seconds*1000.),Action::ReconstructionOption(2,-1.),Action::ReconstructionOption(2,1.),studio,cx)}
            {stepper("wave-position",format!("At {:.3}s",plot.source_from+plot.time),Action::WaveformPosition(-1.),Action::WaveformPosition(1.),studio,cx)}
        </div>
        {if let Some(source)=&studio.waveform_source.preview {
            let source=source.clone();let overview=source.overview.clone();let selection=((plot.source_from+plot.time)/source.duration,plot.seconds/source.duration);let owner=weak_owner.clone();
            <div id="recorded-overview" test-support relative h={u(54.)} bg={rgb(p.soft)} cursor-pointer
                on-click={cx.listener(|this,_,window,cx|{if let Some(bounds)=this.waveform_bounds {let fraction=((window.mouse_position().x-bounds.left())/bounds.size.width) as f64;this.seek_reconstruction(fraction,cx);}})}>
                <canvas absolute size-full args={(move|bounds,_,cx|{let _=owner.update(cx,|s,_|s.waveform_bounds=Some(bounds));},move|bounds,_,window,_|{grid(bounds,dark,window);peaks(&overview,bounds,window);let left=bounds.left()+bounds.size.width*selection.0 as f32;let width=(bounds.size.width*selection.1 as f32).max(sp(2.,window));window.paint_quad(fill(Bounds{origin:point(left,bounds.top()),size:size(width,bounds.size.height)},rgba(0xe1c18a44)));})}/>
            </div>.into_any_element()
        }else{<div id="recorded-placeholder" test-support p={u(12.)} bg={rgb(p.soft)} text-size={u(12.)}>{if studio.waveform_source.loading{"Loading recorded waveform…".into()}else if let Some(error)=&studio.waveform_source.error{format!("Original waveform unavailable: {error}")}else{"Record or import audio and analyze it to compare the original waveform.".into()}}</div>.into_any_element()}}
        {caption(if let Some(source)=&studio.waveform_source.preview{format!("0s ── {:.2}s · click the recording to inspect a different moment",source.duration)}else{"The sine reconstruction remains editable without a source file.".into()},dark)}
        <div flex flex-col gap={u(5.)}>
            <div text-size={u(11.)} text-color={rgb(0x91b5d8)}>Recorded waveform · selected detail</div>
            <div id="recorded-detail" test-support relative h={u(92.)} bg={rgb(p.soft)}>
                {if let Some(original)=plot.original.clone(){<canvas absolute size-full args={(|_,_,_|(),move|bounds,_,window,_|{grid(bounds,dark,window);peaks(&original,bounds,window);})}/>.into_any_element()}else{caption("No original waveform to display",dark)}}
            </div>
            <div text-size={u(11.)} text-color={rgb(0x99c6aa)}>{format!("Reconstructed waveform · Σ {} sines · {:.1} Hz base",sound.harmonics.len(),plot.hz)}</div>
            {let plot=plot.clone();let colors=sound.harmonics.iter().map(|h|wave_color(h.multiple)).collect::<Vec<_>>();
            <div id="reconstructed-detail" test-support relative h={u(108.)} bg={rgb(p.soft)}>
                <canvas absolute size-full args={(|_,_,_|(),move|bounds,_,window,_|{grid(bounds,dark,window);for (i,values) in plot.sines.partials.iter().enumerate(){line(values,bounds,rgba((colors[i]<<8)|if i==selected{0xbb}else{0x40}).into(),if i==selected{1.2}else{0.65},window);}line(&plot.sines.sum,bounds,rgb(0x99c6aa).into(),1.8,window);})}/>
            </div>}
        </div>
        {caption("Harmonic-only view: sine mix with gain; noise, envelope and tone filtering are excluded. Listen to hear the complete sound.",dark)}
        <div flex items-center justify-between text-size={u(12.)}><div>Individual sine waves · select to tune</div>{caption("Dimmed = below cleanup threshold",dark)}</div>
        <div relative h={u(200.)} min-w-0>
            <div id="reconstruction-partials" test-support size-full overflow-y-scroll overflow-x-scroll track-scroll={&handle} pb={u(12.)}>
                <div flex flex-col gap={u(5.)} children={sound.harmonics.iter().enumerate().map(|(i,h)|{
                    let values=plot.sines.partials[i].clone();let color=wave_color(h.multiple);let is_weak=weak.contains(&h.multiple);let number=h.multiple;
                    <div id={format!("sine-row-{i}")} test-support flex items-center gap={u(8.)} min-w={u(480.)} px={u(6.)} py={u(4.)} bg={rgb(if i==selected{p.border}else{p.soft})} opacity={if is_weak{0.55}else{1.}}>
                        <div id={format!("sine-select-{i}")} test-support w={u(38.)} text-size={u(12.)} text-color={rgb(color)} cursor-pointer on-click={cx.listener(move|this,_,window,cx|{if instance{this.select_instance_harmonic(i,window,cx);}else{this.select_harmonic(i,window,cx);}})}>{format!("H{number}")}</div>
                        <div relative w={u(140.)} h={u(32.)} flex-shrink-0><canvas absolute size-full args={(|_,_,_|(),move|bounds,_,window,_|line(&values,bounds,rgb(color).into(),1.,window))}/></div>
                        <div w={u(76.)} text-size={u(10.)}>{format!("{:.0}% level\n{:.1}% energy",h.amplitude*100.,h.amplitude*h.amplitude/energy*100.)}</div>
                        <StudioSlider args={if instance{&studio.instance_sliders[i]}else{&studio.draft_sliders[i]}} horizontal flex-1/>
                        {button(format!("sine-remove-{i}"),"×",Action::RemoveHarmonic(instance,i),false,studio,cx).disabled(sound.harmonics.len()<=1)}
                    </div>
                })}/>
            </div>
            {scrollbar_layer("bar-reconstruction-partials".into(),handle,ScrollbarAxis::Both)}
        </div>
        <div flex gap={u(12.)}>
            {slider_row("Phase",32,format!("H{} · {:.0}°",sound.harmonics[selected].multiple,sound.harmonics[selected].phase),instance,studio)}
            {slider_row("Detune",33,format!("{:.0} cents",sound.harmonics[selected].detune),instance,studio)}
        </div>
        {caption(if instance{"Changes affect this timeline instance only. Save the project to retain them."}else{"Remove or lower unwanted sine waves, then save changes to the library record."},dark)}
    </div>.into_any_element()
}
