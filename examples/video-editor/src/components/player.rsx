use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::slider::Slider;
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen,Kind}};
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
    let asset=&editor.state.assets[editor.state.selected];
    <div relative w={px(width)} h={px(height)} overflow-hidden rounded={px(5.)} bg={rgb(0x070b10)}>
        {thumbnail(asset,width,height)}
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
        {if asset.kind==Kind::Audio {<div absolute bottom={px(16.)} left={px(16.)} text-size={px(18.)} font-semibold>{asset.name}</div>.into_any_element()}else{div().into_any_element()}}
    </div>
}
#[gpui]
pub fn player_controls(editor:&Editor,small:bool,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let asset=&editor.state.assets[editor.state.selected];
    let time=editor.state.position;
    <div flex flex-wrap flex-shrink-0 items-center gap={px(if small{5.}else{10.})} justify-between min-h={px(52.)}>
        <div flex items-center gap-1 text-size={px(if small{11.}else{13.})}>
            <div text-color={rgb(PURPLE)} font-semibold>{if small{format!("{:02}:{:02}",time as u32/60,time as u32%60)}else{format!("00:00:{:02}:{:02}",time as u32,((time.fract()*30.).round() as u32).min(29))}}</div>
            <div text-color={rgb(MUTED)}>{format!("/ {}",if editor.state.screen.has_timeline(){"00:18".into()}else{asset.duration_label()})}</div>
        </div>
        <div flex gap-1>
            {if !small {tool("previous-frame","",Some(IconName::SkipBack),Action::Step(-1.),false,cx).into_any_element()}else{div().into_any_element()}}
            {tool(if small{"play-small"}else{"play"},"",Some(if editor.state.playing{IconName::Pause}else{IconName::Play}),Action::Play,false,cx)}
            {if !small {tool("next-frame","",Some(IconName::SkipForward),Action::Step(1.),false,cx).into_any_element()}else{div().into_any_element()}}
        </div>
        <div flex gap-1>
            {if small{tool("mute-preview","",Some(if editor.state.muted{IconName::VolumeX}else{IconName::Volume2}),Action::Mute,editor.state.muted,cx).into_any_element()}else{div().into_any_element()}}
            {tool("preview-quality",["4K","1/2","Fit"][editor.state.quality],None,Action::Quality,false,cx)}
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
        {if tracking {
            <div relative w-full flex-shrink-0 h={px(37.)} overflow-hidden rounded-sm>
                <div flex children={(0..10).map(|_|thumbnail(&editor.state.assets[editor.state.selected],width/10.,37.))}></div>
                <div absolute top-0 left={relative((editor.state.position/editor.state.seek_limit()).clamp(0.,1.))} h-full w={px(2.)} bg={rgb(TEXT)} />
            </div>.into_any_element()
        }else{div().into_any_element()}}
        <div w-full flex-shrink-0 px-1 py-2><Slider args={&editor.seek} /></div>
    </div>
}
