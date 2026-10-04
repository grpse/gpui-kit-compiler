use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use crate::{editor::Editor,generated::primitives::*,state::Action};

#[gpui]
pub fn track_controls(editor:&Editor,labels:f32,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    <div flex flex-col w={px(labels)} flex-shrink-0 border-r-1 border-color={rgb(BORDER)}>
        <div h={px(39.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} />
        <div flex flex-col children={editor.state.tracks.iter().enumerate().map(|(index,track)| <div flex items-center gap-1 h={px(62.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} px={px(7.)} bg={rgb(if index==editor.state.clips[editor.state.selected_clip].track{0x1b222c}else{PANEL})}>
            {tool(format!("visible-track-{index}"),"",Some(if track.visible{IconName::Eye}else{IconName::EyeOff}),Action::TrackVisible(index),false,cx)}
            <div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("{}{}",if track.audio{"A"}else{"V"},if track.audio{index.saturating_sub(1)}else{index+1})}</div>
            <div flex-1 truncate text-size={px(11.)}>{track.name.clone()}</div>
            {tool(format!("lock-track-{index}"),"",Some(if track.locked{IconName::Lock}else{IconName::LockOpen}),Action::TrackLocked(index),track.locked,cx)}
            </div>)}>
            <div p-2>{tool("add-track","Add Track",Some(IconName::CirclePlus),Action::AddTrack,false,cx)}</div>
        </div>
    </div>
}

#[gpui]
pub fn divider(cx:&mut Context<Editor>)->impl IntoElement + use<> {
    <div id="track-controls-divider" role={Role::Slider} aria-label="Resize track controls" relative w={px(9.)} h-full flex-shrink-0 cursor-col-resize bg={rgb(BG)} hover:args={|this|this.bg(rgb(0x29223d))}
        on-mouse-down:args={(MouseButton::Left,cx.listener(|this,_,_,cx|{this.dragging_track_divider=true;cx.stop_propagation();}))}>
        <div absolute top={relative(0.5)} left={px(3.)} w={px(3.)} h={px(28.)} rounded-full bg={rgb(BORDER)} />
    </div>
}
