use gpui_base::Disableable as _;
use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},Sizable as _};
use crate::{editor::{Editor,InlineEdit},generated::primitives::*,state::Action};
#[gpui]
pub fn video_inspector(editor:&Editor,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let index=editor.state.selected_clip;
    let Some(clip)=editor.state.clips.get(index) else {return <div text-color={rgb(MUTED)}>Select a timeline clip to edit it.</div>.into_any_element();};
    let locked=editor.state.linked_locked(index);
    <div flex flex-col gap-3>
        <div font-semibold>{editor.state.clip_label(index)}</div>
        {metadata(vec![("Start",format!("{:.3} s",clip.start)),("Source in",format!("{:.3} s",clip.source_start)),("Duration",format!("{:.3} s",clip.length)),("Track",editor.state.tracks[clip.track].name.clone()),("Source",editor.state.assets[clip.asset].name.clone())])}
        <div flex flex-wrap justify-start gap-2>
            <Button args={"inspector-edit-timing"} ghost small label="Edit timing" disabled={locked} on-click={cx.listener(move|this,_,window,cx|this.begin_inline(InlineEdit::Timing(index),window,cx))} />
            <Button args={"inspector-rename-clip"} ghost small label="Rename" disabled={locked} on-click={cx.listener(move|this,_,window,cx|this.begin_inline(InlineEdit::Clip(index),window,cx))} />
            {tool("inspector-split","Split at playhead",None,Action::Tool("Split"),false,cx).disabled(locked||editor.state.position<=clip.start||editor.state.position>=clip.start+clip.length)}
        </div>
        <div text-size={px(11.)} text-color={rgb(MUTED)}>Drag clip edges to trim. Drag the clip to move it. Linked audio follows video edits.</div>
    </div>.into_any_element()
}
#[gpui]
pub fn inspector_panel(editor:&Editor,width:f32,_height:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div id="inspector-scroll" flex flex-col w={px(width)} h-full min-h-0 flex-shrink-0 bg={rgb(PANEL)} p-4 gap-4 overflow-y-scroll>
        {video_inspector(editor,cx)}
        {if let Some(clip)=editor.state.clips.get(editor.state.selected_clip) {
            let track=clip.track;
            if editor.state.tracks[track].audio {
                <div flex flex-col gap-3 border-t-1 border-color={rgb(BORDER)} pt-3>
                    <div font-semibold>Track audio</div>
                    {crate::generated::properties::property_slider(editor,"Volume",8,"%",cx)}
                    <div flex justify-start>{tool("audio-mute","Mute track",Some(IconName::VolumeX),Action::TrackMuted(track),editor.state.tracks[track].muted,cx)}</div>
                </div>.into_any_element()
            }else{
                <div flex justify-start>{tool("video-visible","Hide track",Some(IconName::Eye),Action::TrackVisible(track),!editor.state.tracks[track].visible,cx)}</div>.into_any_element()
            }
        }else{div().into_any_element()}}
    </div>
}
