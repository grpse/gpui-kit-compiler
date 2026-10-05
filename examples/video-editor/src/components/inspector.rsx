use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::Action};



#[gpui]
pub fn video_inspector(editor:&Editor,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex flex-col>
        <div pb-3 border-b-1 border-color={rgb(BORDER)}>
            {crate::generated::properties::inspector_section(editor,"Transform",Some(Action::ResetTransform),cx)}
            {if !editor.state.collapsed.contains("Transform") {<div flex flex-col gap-1>
                    <div flex items-center gap-3 h={px(34.)}><div w={px(78.)}>Position</div>
                        {tool("position-x","X  960",None,Action::Mock("Set horizontal position"),false,cx)}
                        {tool("position-y","Y  540",None,Action::Mock("Set vertical position"),false,cx)}
                    </div>
                    {crate::generated::properties::property_slider(editor,"Scale",0,"%",cx)}{crate::generated::properties::property_slider(editor,"Rotation",1,"°",cx)}{crate::generated::properties::property_slider(editor,"Opacity",2,"%",cx)}
                </div>.into_any_element()}else{div().into_any_element()}}
        </div>
        <div py-2 border-b-1 border-color={rgb(BORDER)}>
            {crate::generated::properties::inspector_section(editor,"Cropping",Some(Action::ResetCrop),cx)}
            {if !editor.state.collapsed.contains("Cropping"){<div children={["Left","Right","Top","Bottom"].into_iter().enumerate().map(|(i,label)|crate::generated::properties::property_slider(editor,label,i+3,"%",cx))}></div>.into_any_element()}else{div().into_any_element()}}
        </div>
        <div border-b-1 border-color={rgb(BORDER)}>{crate::generated::properties::toggle_property(editor,"Stabilization",cx)}</div>
        <div border-b-1 border-color={rgb(BORDER)}>{crate::generated::properties::toggle_property(editor,"AI Tracking",cx)}</div>
        <div>{crate::generated::properties::inspector_section(editor,"Compositing",None,cx)}
            {if !editor.state.collapsed.contains("Compositing"){<div flex flex-col gap-2>
                    <div flex items-center justify-between text-size={px(12.)}><div>Blend Mode</div>{tool("blend-mode","Normal ⌄",None,Action::Mock("Blend mode menu"),false,cx)}</div>
                    {crate::generated::properties::toggle_property(editor,"Enable Mask",cx)}
                    {tool("add-mask","Add Mask",Some(IconName::SquareDashed),Action::Mock("Add mask"),false,cx)}
                </div>.into_any_element()}else{div().into_any_element()}}
        </div>
    </div>
}

#[gpui]
pub fn inspector_panel(editor:&Editor,width:f32,_height:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex flex-col w={px(width)} h-full flex-shrink-0 bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded-lg overflow-hidden>
        <div flex flex-wrap flex-shrink-0 border-b-1 border-color={rgb(BORDER)} children={["Video","Audio"].into_iter().enumerate().map(|(i,label)|
            <Button args={SharedString::from(format!("inspector-{i}"))} ghost small flex-1 label={label} px={px(4.)} h={px(44.)} text-size={px(11.)}
            bg={rgb(if editor.state.inspector==i{0x211b34}else{PANEL})} text-color={rgb(if editor.state.inspector==i{0xb298ff}else{TEXT})}
            border-b={px(if editor.state.inspector==i{2.}else{0.})} border-color={rgb(PURPLE)}
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Inspector(i),window,cx))} />)}>
        </div>
        <div id="inspector-scroll" flex-1 min-w-0 min-h-0 overflow-y-scroll overflow-x-scroll px={px(15.)} py={px(8.)}>
            <div min-w={px(265.)}>
                {match editor.state.inspector {
                        0=>crate::generated::inspector::video_inspector(editor,cx).into_any_element(),
                        1=><div flex flex-col gap-3><div font-semibold>Audio</div>{crate::generated::properties::property_slider(editor,"Volume",8,"%",cx)}{crate::generated::properties::toggle_property(editor,"Normalize",cx)}{crate::generated::properties::toggle_property(editor,"Noise Reduction",cx)}
                            {if let Some(track)=editor.state.clips.get(editor.state.selected_clip).map(|clip|clip.track).filter(|&index|editor.state.screen.has_timeline()&&editor.state.tracks[index].audio) {tool("audio-mute","Mute track",Some(IconName::VolumeX),Action::TrackMuted(track),editor.state.tracks[track].muted,cx)}else{tool("audio-mute","Mute preview",Some(IconName::VolumeX),Action::Mute,editor.state.muted,cx)}}
                            {section_title("Fade",cx,false)}{tool("audio-fade","Add fade in / out",None,Action::Mock("Audio fade"),false,cx)}
                        </div>.into_any_element(),
                        _=>crate::generated::inspector::video_inspector(editor,cx).into_any_element(),
                    }}
            </div>
        </div>
    </div>
}
