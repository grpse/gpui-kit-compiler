use gpui_kit::{prelude::*, *};
use gpui_kit::component::{slider::Slider};
use crate::{editor::Editor,generated::primitives::*};



#[gpui]
pub fn property_slider(editor:&Editor,label:&'static str,index:usize,unit:&str,_cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex items-center gap-3 h={px(33.)}>
        <div w={px(78.)} flex-shrink-0 text-size={px(12.)}>{label}</div>
        <div flex-1 min-w-0><Slider args={&editor.sliders[index]} /></div>
        <div flex items-center justify-center w={px(57.)} h={px(26.)} flex-shrink-0 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-md text-size={px(12.)}>{format!("{:.0}{unit}",editor.state.controls[index])}</div>
    </div>
}
