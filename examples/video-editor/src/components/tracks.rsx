use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{menu::ContextMenuExt as _,input::Input,Sizable as _};
use crate::{editor::{Editor,InlineEdit},generated::primitives::*,state::Action,interactions::{TrackDrag,TrackDragPreview}};

#[gpui]
pub fn track_controls(editor:&Editor,labels:f32,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    let owner=cx.entity();
    <div flex flex-col w={px(labels)} flex-shrink-0 border-r-1 border-color={rgb(BORDER)}>
        <div h={px(39.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} />
        <div flex flex-col children={editor.state.tracks.iter().enumerate().map(|(index,track)| {
            let menu_owner=owner.clone();
            let drag=TrackDrag{index,revision:editor.state.track_revision,owner:cx.entity_id(),name:track.name.clone()};
            let editor_id=cx.entity_id();
            let editing=editor.inline_edit==Some(InlineEdit::Track(index));
            let ordinal=editor.state.tracks[..=index].iter().filter(|candidate|candidate.audio==track.audio).count();
            <div id={("track-row",index)} role={Role::Group} aria-label={format!("Track {}",track.name)} flex items-center gap-1 h={px(62.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} px={px(5.)} bg={rgb(if editor.state.clips.get(editor.state.selected_clip).is_some_and(|clip|index==clip.track){RAISED}else{PANEL})}
                drag-over:args={move |style,drag:&TrackDrag,_,_|if drag.owner==editor_id{style.border_2().border_color(rgb(PURPLE))}else{style}}
                on-drop={cx.listener(move |this,drag:&TrackDrag,window,cx| {
                    if drag.owner==cx.entity_id()&&drag.revision==this.state.track_revision {this.dispatch(Action::MoveTrack{from:drag.index,to:index},window,cx);}
                })}
                context-menu={move |menu,window,cx|crate::interactions::track_menu(menu,menu_owner.clone(),index,window,cx)}>
                <div id={("track-drag",index)} role={Role::Button} aria-label={format!("Reorder track {}",track.name)} w={px(13.)} flex-shrink-0 cursor-grab text-color={rgb(MUTED)}
                    on-drag:args={(drag,move |drag:&TrackDrag,_,_,cx| {cx.stop_propagation();let name=drag.name.clone();cx.new(|_|TrackDragPreview{name})})}>
                    <div text-size={px(17.)}>⠿</div>
                </div>
                {tool(format!("{}-track-{index}",if track.audio{"mute"}else{"visible"}),"",Some(if track.audio{if track.muted{IconName::VolumeX}else{IconName::Volume2}}else if track.visible{IconName::Eye}else{IconName::EyeOff}),if track.audio{Action::TrackMuted(index)}else{Action::TrackVisible(index)},track.audio&&track.muted,cx)}
                <div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("{}{ordinal}",if track.audio{"A"}else{"V"})}</div>
                {if editing {<div flex-1 min-w-0>{crate::generated::tracks::name_field(editor,cx)}</div>.into_any_element()}else{
                    <div id={("track-name",index)} role={Role::Button} aria-label={format!("Rename track {}",track.name)} flex-1 truncate text-size={px(11.)} cursor-text
                        on-click={cx.listener(move |this,event:&ClickEvent,window,cx| {if event.click_count()>=2 {this.begin_inline(InlineEdit::Track(index),window,cx);}cx.stop_propagation();})}>{track.name.clone()}</div>.into_any_element()
                }}
                {tool(format!("lock-track-{index}"),"",Some(if track.locked{IconName::Lock}else{IconName::LockOpen}),Action::TrackLocked(index),track.locked,cx)}
            </div>
        })}>
            <div p-2>{tool("add-track","Add Track",Some(IconName::CirclePlus),Action::AddTrack,false,cx)}</div>
        </div>
    </div>
}

#[gpui]
pub fn name_field(editor:&Editor,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    <div id="inline-name-editor" w-full min-w-0 on-click={|_,_,cx|cx.stop_propagation()}
        capture-key-down={cx.listener(|this,event:&KeyDownEvent,_,cx|{if event.keystroke.key=="escape" {this.cancel_inline(cx);cx.stop_propagation();}})}>
        <Input args={&editor.name_input} small h={px(24.)} text-size={px(11.)} />
    </div>
}

#[gpui]
pub fn drag_preview(name:String)->impl IntoElement + use<> {
    <div w={px(200.)} h={px(40.)} flex items-center px-3 bg={rgb(RAISED)} border-2 border-color={rgb(PURPLE)} rounded-md text-color={rgb(TEXT)}><div truncate>{name}</div></div>
}

#[gpui]
pub fn divider(cx:&mut Context<Editor>)->impl IntoElement + use<> {
    <div id="track-controls-divider" role={Role::Slider} aria-label="Resize track controls" relative w={px(9.)} h-full flex-shrink-0 cursor-col-resize bg={rgb(BG)} hover:args={|this|this.bg(rgb(0x29223d))}
        on-mouse-down:args={(MouseButton::Left,cx.listener(|this,_,_,cx|{this.dragging_track_divider=true;cx.stop_propagation();}))}>
        <div absolute top={relative(0.5)} left={px(3.)} w={px(3.)} h={px(28.)} rounded-full bg={rgb(BORDER)} />
    </div>
}
