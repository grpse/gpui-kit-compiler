use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{slider::Slider,switch::Switch,Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::Action};



#[gpui]
pub fn property_slider(editor:&Editor,label:&'static str,index:usize,unit:&str,_cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex items-center gap-3 h={px(33.)}>
        <div w={px(78.)} flex-shrink-0 text-size={px(12.)}>{label}</div>
        <div flex-1 min-w-0><Slider args={&editor.sliders[index]} /></div>
        <div flex items-center justify-center w={px(57.)} h={px(26.)} flex-shrink-0 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-md text-size={px(12.)}>{format!("{:.0}{unit}",editor.state.controls[index])}</div>
    </div>
}

#[gpui]
pub fn toggle_property(editor:&Editor,label:&'static str,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex items-center justify-between h={px(38.)} text-size={px(12.)}>
        <div>{label}</div>
        <Switch args={SharedString::from(format!("toggle-{label}"))} small checked={editor.state.toggles.contains(label)} color={rgb(PURPLE)} accessibility-label={label}
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Toggle(label),window,cx))} />
    </div>
}

#[gpui]
pub fn inspector_section(editor:&Editor,title:&'static str,reset:Option<Action>,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex items-center justify-between h={px(39.)}>
        {section_title(title,cx,editor.state.collapsed.contains(title))}
        {if let Some(action)=reset{tool(format!("reset-{title}"),"",Some(IconName::RotateCcw),action,false,cx).into_any_element()}else{div().into_any_element()}}
    </div>
}
