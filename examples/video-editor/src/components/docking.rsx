use gpui_kit::{prelude::*, *};
use gpui_base::{ResizeHandleContext,ResizeHandleState};
use crate::generated::primitives::*;

/// Native resize handles own their nine-pixel grab band. These four-pixel insets
/// put the visible gaps exactly under that band, including after re-docking.
#[gpui]
pub fn island_frame(frame:Stateful<Div>)->Stateful<Div> {
    <div ctor={frame} p={px(4.)} bg={rgb(BG)} />
}
#[gpui]
pub fn split_frame(frame:Stateful<Div>)->Stateful<Div> {
    <div ctor={frame} bg={rgb(BG)} />
}
#[gpui]
pub fn island_header(content:AnyElement)->impl IntoElement {
    <div flex-shrink-0 rounded-t-lg overflow-hidden border-1 border-color={rgb(BORDER)} bg={rgb(PANEL)}>
        {content}
    </div>
}
#[gpui]
pub fn island_body(frame:Stateful<Div>)->Stateful<Div> {
    <div ctor={frame} rounded-b-lg border-l-1 border-r-1 border-b-1 border-color={rgb(BORDER)} bg={rgb(PANEL)} />
}
#[gpui]
pub fn divider(handle:&ResizeHandleContext)->impl IntoElement + use<> {
    let engaged=handle.state()!=ResizeHandleState::Idle;
    let color=if engaged{PURPLE}else{BORDER};
    if handle.axis()==Axis::Horizontal {
        <div relative flex-shrink-0 w={px(1.)} h-full>
            <div absolute top-0 bottom-0 left={px(-4.)} w={px(9.)} bg={rgb(BG)} />
            <div absolute left={px(-1.)} top={relative(0.5)} w={px(3.)} h={px(if engaged{36.}else{20.})} mt={px(if engaged{-18.}else{-10.})} rounded-full bg={rgb(color)} />
        </div>.into_any_element()
    }else {
        <div relative flex-shrink-0 h={px(1.)} w-full>
            <div absolute left-0 right-0 top={px(-4.)} h={px(9.)} bg={rgb(BG)} />
            <div absolute top={px(-1.)} left={relative(0.5)} h={px(3.)} w={px(if engaged{36.}else{20.})} ml={px(if engaged{-18.}else{-10.})} rounded-full bg={rgb(color)} />
        </div>.into_any_element()
}
}
