use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::slider::Slider;
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen,Kind,ClipComponent}};
#[gpui]
pub fn tracking_overlay(editor:&Editor,width:f32,height:f32) -> impl IntoElement + use<> {
    let color=GREEN;
    let box_mode=editor.state.tracking_mode==1;
    <div absolute left={px(width*0.35)} top={px(height*0.075)} w={px(width*0.29)} h={px(height*0.72)}
        border={px(if box_mode {1.5}else{0.})} border-color={rgb(color)} children={if box_mode {(0..4).map(|corner|<div absolute left={px(if corner%2==0{-4.}else{width*0.29-4.})} top={px(if corner<2{-4.}else{height*0.72-4.})} size={px(8.)} bg={rgb(color)} border-1 border-color={rgb(0x244c3e)} />).collect::<Vec<_>>()}else{vec![]}}>

        <div absolute left={relative(0.5)} top={relative(0.37)} text-color={rgb(GREEN)}>{glyph(if editor.state.tracking_mode==2{IconName::ScanFace}else{IconName::Crosshair},24.)}</div>
        {if editor.state.toggles.contains("Show Track Path") {<div absolute bottom={px(12.)} left={px(5.)} flex items-center gap={px(6.)} children={(0..9).map(|i|<div size={px(3.)} rounded-full bg={rgb(color)} mt={px(((i as f32)*0.8).sin()*12.)} />)}>
            </div>.into_any_element()}else{div().into_any_element()}}
    </div>
}
#[gpui]
pub fn preview_image(editor:&Editor,width:f32,height:f32,tracking:bool) -> impl IntoElement + use<> {
    let Some(asset)=editor.state.assets.get(editor.state.selected) else {return <div flex items-center justify-center size-full text-color={rgb(MUTED)}>Import files or a folder to start editing</div>.into_any_element();};
    let channel=if editor.state.screen.has_timeline(){editor.state.clips.get(editor.state.selected_clip).filter(|clip|clip.asset==editor.state.selected).and_then(|clip|match clip.component {
        Some(ClipComponent::AudioChannel{stream,channel})=>asset.prepared.as_ref().and_then(|media|media.audio.iter().find(|audio|audio.index==stream)).map(|audio|(audio,channel,clip.source_start as f64,(clip.source_start+clip.length) as f64)),
        _=>None,
    })}else{None};
    let label=channel.map(|(audio,index,_,_)|format!("{} · S{} {}",asset.name,audio.index,audio.channels[index].name)).unwrap_or_else(||asset.name.clone());
    <div relative w={px(width)} h={px(height)} overflow-hidden rounded={px(5.)} bg={rgb(0x070b10)}>
        {if editor.state.playing&&let Some(frame)=&editor.preview_frame {<img args={frame.clone()} w={px(width)} h={px(height)} object-fit={ObjectFit::Contain} />.into_any_element()}else if editor.playback_gap {div().into_any_element()}else if let Some((audio,channel,start,end))=channel {<div flex items-center size-full>{channel_waveform(audio,channel,start,end,width,height*0.6,GREEN)}</div>.into_any_element()}else if let Some(frame)=&editor.preview_frame {<img args={frame.clone()} w={px(width)} h={px(height)} object-fit={ObjectFit::Contain} />.into_any_element()}else{thumbnail(asset,width,height)}}
        {if let Some(preset)=editor.state.selected_preset {
            match editor.state.category.as_str() {
                "Text"|"Captions"=><div absolute left-2 right-2 bottom={px(20.)} text-center font-semibold text-size={px((width/18.).clamp(14.,28.))} bg={rgba(0x00000080)} p-2 rounded-md>{preset}</div>.into_any_element(),
                "Elements"=><div absolute right={px(20.)} top={px(20.)} text-color={rgb(GREEN)}>{glyph(IconName::Star,48.)}</div>.into_any_element(),
                "Effects"=><div absolute left-0 top-0 size-full bg={rgba(0x8d53df40)} />.into_any_element(),
                "Transitions"=><div absolute left-0 top-0 w={relative(0.5)} h-full bg={rgba(0x00000099)} />.into_any_element(),
                _=>div().into_any_element(),
            }
        }else{div().into_any_element()}}
        {if tracking&&asset.kind==Kind::Video {crate::generated::player::tracking_overlay(editor,width,height).into_any_element()}else{div().into_any_element()}}
        {if channel.is_some()||asset.kind==Kind::Audio {<div absolute bottom={px(16.)} left={px(16.)} text-size={px(18.)} font-semibold>{label.clone()}</div>.into_any_element()}else{div().into_any_element()}}
    </div>.into_any_element()
}
#[gpui]
pub fn player_controls(editor:&Editor,small:bool,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let asset=editor.state.assets.get(editor.state.selected);
    let time=editor.state.position;
    let fps=asset.and_then(|asset|asset.frame_rate.as_deref()).and_then(|rate|rate.split_whitespace().next()).and_then(|rate|rate.parse::<f32>().ok()).filter(|rate|rate.is_finite()&&*rate>0.).unwrap_or(30.);
    let step=if asset.is_some_and(|asset|asset.path.is_some()){1./fps}else{1.};
    <div flex flex-wrap flex-shrink-0 items-center gap={px(if small{5.}else{10.})} justify-between min-h={px(52.)}>
        <div flex items-center gap-1 text-size={px(if small{11.}else{13.})}>
            <div text-color={rgb(PURPLE)} font-semibold>{if small{format!("{:02}:{:02}",time as u32/60,time as u32%60)}else{format!("{:02}:{:02}:{:02}:{:02}",time as u32/3600,time as u32/60%60,time as u32%60,((time.fract()*fps).round() as u32).min(fps.ceil() as u32-1))}}</div>
            <div text-color={rgb(MUTED)}>{format!("/ {}",if editor.state.screen.has_timeline(){format!("{:02}:{:02}",editor.state.project_duration() as u32/60,editor.state.project_duration() as u32%60)}else{asset.map_or_else(||"00:00".into(),|asset|asset.duration_label())})}</div>
        </div>
        <div flex gap-1>
            {if !small {tool("previous-frame","",Some(IconName::SkipBack),Action::Step(-step),false,cx).into_any_element()}else{div().into_any_element()}}
            {tool(if small{"play-small"}else{"play"},"",Some(if editor.state.playing{IconName::Pause}else{IconName::Play}),Action::Play,false,cx)}
            {tool("loop-preview","Loop",None,Action::Loop,editor.state.looping,cx)}
            {if !small {tool("next-frame","",Some(IconName::SkipForward),Action::Step(step),false,cx).into_any_element()}else{div().into_any_element()}}
        </div>
        <div flex gap-1>
            {tool("mute-preview","",Some(if editor.state.muted{IconName::VolumeX}else{IconName::Volume2}),Action::Mute,editor.state.muted,cx)}
            {tool("preview-quality",if asset.is_some_and(|asset|asset.path.is_some()){"Preview"}else{["4K","1/2","Fit"][editor.state.quality]},None,Action::Quality,false,cx)}
            {tool("fullscreen-preview","",Some(if editor.state.fullscreen{IconName::Minimize}else{IconName::Maximize}),Action::Fullscreen,false,cx)}
        </div>
    </div>
}
#[gpui]
pub fn player(editor:&Editor,width:f32,height:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let tracking=editor.state.screen==Screen::Tracking||editor.state.inspector==2;
    let picture_height=((width-12.).max(40.)*0.614).min((height-135.).max(60.));
    let picture_width=(picture_height/0.614).min((width-12.).max(40.));
    <div id="player-scroll" flex flex-col items-center size-full min-w-0 min-h-0 overflow-y-scroll p={px(6.)} gap-1>
        <div flex-shrink-0>{crate::generated::player::preview_image(editor,picture_width,picture_height,tracking)}</div>
        <div w-full flex-shrink-0>{crate::generated::player::player_controls(editor,width<420.,cx)}</div>
        {if editor.output_worker.clock.rate.load(std::sync::atomic::Ordering::Relaxed)>0 {
            <div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("Audio output · {} Hz",editor.output_worker.clock.rate.load(std::sync::atomic::Ordering::Relaxed))}</div>.into_any_element()
        }else{div().into_any_element()}}
        {if tracking&&!editor.state.assets.is_empty() {
            <div relative w-full flex-shrink-0 h={px(37.)} overflow-hidden rounded-sm>
                <div flex children={(0..10).map(|_|thumbnail(&editor.state.assets[editor.state.selected],width/10.,37.))}></div>
                <div absolute top-0 left={relative((editor.state.position/editor.state.seek_limit()).clamp(0.,1.))} h-full w={px(2.)} bg={rgb(TEXT)} />
            </div>.into_any_element()
        }else{div().into_any_element()}}
        <div w-full flex-shrink-0 px-1 py-2><Slider args={&editor.seek} /></div>
    </div>
}
