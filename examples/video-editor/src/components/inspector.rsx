use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},slider::Slider,switch::Switch,Sizable as _};
use crate::{generated::{ui::Editor,primitives::*},state::Action};
impl Editor {
    #[gpui]
    pub fn property_slider(&self,label:&'static str,index:usize,unit:&str,_cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex items-center gap-3 h={px(33.)}>
            <div w={px(78.)} flex-shrink-0 text-size={px(12.)}>{label}</div>
            <div flex-1 min-w-0><Slider args={&self.sliders[index]} /></div>
            <div flex items-center justify-center w={px(57.)} h={px(26.)} flex-shrink-0 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-md text-size={px(12.)}>{format!("{:.0}{unit}",self.state.controls[index])}</div>
        </div>
    }
    #[gpui]
    pub fn toggle_property(&self,label:&'static str,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex items-center justify-between h={px(38.)} text-size={px(12.)}>
            <div>{label}</div>
            <Switch args={SharedString::from(format!("toggle-{label}"))} small checked={self.state.toggles.contains(label)} color={rgb(PURPLE)} accessibility-label={label}
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Toggle(label),window,cx))} />
        </div>
    }
    #[gpui]
    pub fn inspector_section(&self,title:&'static str,reset:Option<Action>,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex items-center justify-between h={px(39.)}>
            {section_title(title,cx,self.state.collapsed.contains(title))}
            {if let Some(action)=reset{tool(format!("reset-{title}"),"",Some(IconName::RotateCcw),action,false,cx).into_any_element()}else{div().into_any_element()}}
        </div>
    }
    #[gpui]
    pub fn video_inspector(&self,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex flex-col>
            <div pb-3 border-b-1 border-color={rgb(BORDER)}>
                {self.inspector_section("Transform",Some(Action::ResetTransform),cx)}
                {if !self.state.collapsed.contains("Transform") {<div flex flex-col gap-1>
                        <div flex items-center gap-3 h={px(34.)}><div w={px(78.)}>Position</div>
                            {tool("position-x","X  960",None,Action::Mock("Set horizontal position"),false,cx)}
                            {tool("position-y","Y  540",None,Action::Mock("Set vertical position"),false,cx)}
                        </div>
                        {self.property_slider("Scale",0,"%",cx)}{self.property_slider("Rotation",1,"°",cx)}{self.property_slider("Opacity",2,"%",cx)}
                    </div>.into_any_element()}else{div().into_any_element()}}
            </div>
            <div py-2 border-b-1 border-color={rgb(BORDER)}>
                {self.inspector_section("Cropping",Some(Action::ResetCrop),cx)}
                {if !self.state.collapsed.contains("Cropping"){<div children={["Left","Right","Top","Bottom"].into_iter().enumerate().map(|(i,label)|self.property_slider(label,i+3,"%",cx))}></div>.into_any_element()}else{div().into_any_element()}}
            </div>
            <div border-b-1 border-color={rgb(BORDER)}>{self.toggle_property("Stabilization",cx)}</div>
            <div border-b-1 border-color={rgb(BORDER)}>{self.toggle_property("AI Tracking",cx)}</div>
            <div>{self.inspector_section("Compositing",None,cx)}
                {if !self.state.collapsed.contains("Compositing"){<div flex flex-col gap-2>
                        <div flex items-center justify-between text-size={px(12.)}><div>Blend Mode</div>{tool("blend-mode","Normal ⌄",None,Action::Mock("Blend mode menu"),false,cx)}</div>
                        {self.toggle_property("Enable Mask",cx)}
                        {tool("add-mask","Add Mask",Some(IconName::SquareDashed),Action::Mock("Add mask"),false,cx)}
                    </div>.into_any_element()}else{div().into_any_element()}}
            </div>
        </div>
    }
    #[gpui]
    pub fn tracking_inspector(&self,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex flex-col gap={px(12.)}>
            <div font-semibold mt-2>Object Tracking</div>
            <div flex gap-2 children={[(IconName::Crosshair,"Point"),(IconName::Scan,"Box"),(IconName::ScanFace,"Object (AI)")].into_iter().enumerate().map(|(i,(icon,label))|
                <Button args={SharedString::from(format!("tracking-mode-{i}"))} ghost flex-col gap-2 flex-1 h={px(67.)} label={label} icon={icon}
                text-size={px(11.)} border-1 rounded-md border-color={rgb(if self.state.tracking_mode==i{PURPLE}else{BORDER})}
                bg={rgb(if self.state.tracking_mode==i{0x29223e}else{RAISED})} text-color={rgb(if self.state.tracking_mode==i{0xb298ff}else{TEXT})}
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::TrackingMode(i),window,cx))} />)}>
            </div>
            <div flex gap-2>{tool("track-forward","Track Forward",None,Action::Track(true),true,cx)}{tool("track-backward","Track Backward",None,Action::Track(false),false,cx)}</div>
            <div flex items-center justify-between mt-2><div text-size={px(12.)}>Tracking Target</div>{tool("tracking-target",["Person ⌄","Board ⌄","Custom ⌄"][self.state.tracking_target],None,Action::TrackTarget,false,cx)}</div>
            {self.property_slider("Smoothing",7,"",cx)}
            {self.toggle_property("Show Track Path",cx)}
            {self.toggle_property("Stabilize",cx)}
            <div pt-3 border-t-1 border-color={rgb(BORDER)} flex items-center justify-between><div>Attach To</div>{tool("attach-tracking",if self.state.attach{"Mask 1 ⌄"}else{"None ⌄"},None,Action::Attach,false,cx)}</div>
            <div text-size={px(11.)} text-color={rgb(MUTED)}>Choose a point, box, or object to follow. The preview shows a mock target and path.</div>
        </div>
    }
    #[gpui]
    pub fn inspector_panel(&self,width:f32,_height:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex flex-col w={px(width)} h-full flex-shrink-0 bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded-lg overflow-hidden>
            <div flex flex-wrap flex-shrink-0 border-b-1 border-color={rgb(BORDER)} children={["Video","Audio","Tracking","Effects","Color"].into_iter().enumerate().map(|(i,label)|
                <Button args={SharedString::from(format!("inspector-{i}"))} ghost small flex-1 label={label} px={px(4.)} h={px(44.)} text-size={px(11.)}
                bg={rgb(if self.state.inspector==i{0x211b34}else{PANEL})} text-color={rgb(if self.state.inspector==i{0xb298ff}else{TEXT})}
                border-b={px(if self.state.inspector==i{2.}else{0.})} border-color={rgb(PURPLE)}
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Inspector(i),window,cx))} />)}>
            </div>
            <div id="inspector-scroll" flex-1 min-w-0 min-h-0 overflow-y-scroll overflow-x-scroll px={px(15.)} py={px(8.)}>
                <div min-w={px(265.)}>
                {match self.state.inspector {
                        0=>self.video_inspector(cx).into_any_element(),
                        1=><div flex flex-col gap-3><div font-semibold>Audio</div>{self.property_slider("Volume",8,"%",cx)}{self.toggle_property("Normalize",cx)}{self.toggle_property("Noise Reduction",cx)}
                            {tool("audio-mute","Mute Clip",Some(IconName::VolumeX),Action::Mute,self.state.muted,cx)}
                            {section_title("Fade",cx,false)}{tool("audio-fade","Add fade in / out",None,Action::Mock("Audio fade"),false,cx)}
                        </div>.into_any_element(),
                        2=>self.tracking_inspector(cx).into_any_element(),
                        3=><div flex flex-col gap-3><div font-semibold>Effects</div><div text-color={rgb(MUTED)} text-size={px(12.)}>Choose a look for your clip</div>
                            <div flex flex-col gap-2 children={["Cinematic","Soft Glow","Film Grain","Vignette"].into_iter().map(|label|<div flex items-center justify-between p-3 rounded-md bg={rgb(RAISED)}><div>{label}</div>{tool(format!("effect-{label}"),"Apply",Some(IconName::Plus),Action::Mock(label),false,cx)}</div>)} />
                        </div>.into_any_element(),
                        _=><div flex flex-col gap-3><div font-semibold>Color Correction</div>{self.property_slider("Exposure",9,"",cx)}{self.property_slider("Intensity",8,"%",cx)}{self.toggle_property("Auto Color",cx)}
                            {tool("color-lut","Choose LUT ⌄",None,Action::Mock("Color LUT menu"),false,cx)}
                        </div>.into_any_element(),
                    }}
                </div>
            </div>
        </div>
    }
}
