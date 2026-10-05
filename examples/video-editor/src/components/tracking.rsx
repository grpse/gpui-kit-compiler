use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button,ButtonVariants as _};
use crate::{editor::Editor,generated::primitives::*,state::Action};



#[gpui]
#[allow(dead_code)] // Retained reference prototype; Inspector currently offers Audio and Video.
pub fn tracking_inspector(editor:&Editor,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex flex-col gap={px(12.)}>
        <div font-semibold mt-2>Object Tracking</div>
        <div flex gap-2 children={[(IconName::Crosshair,"Point"),(IconName::Scan,"Box"),(IconName::ScanFace,"Object (AI)")].into_iter().enumerate().map(|(i,(icon,label))|
            <Button args={SharedString::from(format!("tracking-mode-{i}"))} ghost flex-col gap-2 flex-1 h={px(67.)} label={label} icon={icon}
            text-size={px(11.)} border-1 rounded-md border-color={rgb(if editor.state.tracking_mode==i{PURPLE}else{BORDER})}
            bg={rgb(if editor.state.tracking_mode==i{0x29223e}else{RAISED})} text-color={rgb(if editor.state.tracking_mode==i{0xb298ff}else{TEXT})}
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::TrackingMode(i),window,cx))} />)}>
        </div>
        <div flex gap-2>{tool("track-forward","Track Forward",None,Action::Track(true),true,cx)}{tool("track-backward","Track Backward",None,Action::Track(false),false,cx)}</div>
        <div flex items-center justify-between mt-2><div text-size={px(12.)}>Tracking Target</div>{tool("tracking-target",["Person ⌄","Board ⌄","Custom ⌄"][editor.state.tracking_target],None,Action::TrackTarget,false,cx)}</div>
        {crate::generated::properties::property_slider(editor,"Smoothing",7,"",cx)}
        {crate::generated::properties::toggle_property(editor,"Show Track Path",cx)}
        {crate::generated::properties::toggle_property(editor,"Stabilize",cx)}
        <div pt-3 border-t-1 border-color={rgb(BORDER)} flex items-center justify-between><div>Attach To</div>{tool("attach-tracking",if editor.state.attach{"Mask 1 ⌄"}else{"None ⌄"},None,Action::Attach,false,cx)}</div>
        <div text-size={px(11.)} text-color={rgb(MUTED)}>Choose a point, box, or object to follow. The preview shows a mock target and path.</div>
    </div>
}
