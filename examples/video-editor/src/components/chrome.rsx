use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},input::Input,Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen}};
#[gpui]
pub fn topbar(editor:&Editor,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex items-center h={px(58.)} px={px(20.)} gap={px(18.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)}>
        <div flex items-center gap-2 text-color={rgb(PURPLE)}>
            <div flex items-center justify-center size={px(29.)} border-2 border-color={rgb(PURPLE)} rounded={px(5.)}>{glyph(IconName::Play,20.)}</div>
            <div text-size={px(23.)} font-semibold text-color={rgb(TEXT)}>FlowCut</div>
        </div>
        <div flex items-center gap-1 ml={px(15.)} children={Screen::ALL.into_iter().map(|screen| <Button args={SharedString::from(format!("screen-{screen:?}"))} ghost label={screen.label()} h={px(38.)} px={px(17.)} text-size={px(13.)}
            bg={rgb(if editor.state.screen==screen {0x211b34}else{BG})} text-color={rgb(if editor.state.screen==screen {0xb394ff}else{MUTED})}
            border-b={px(if editor.state.screen==screen {2.}else{0.})} border-color={rgb(PURPLE)}
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Screen(screen),window,cx))} />)}>
        </div>
        <div flex-1 />
        {if !editor.state.screen.has_timeline(){
            <div w={px(260.)}>{<Input args={&editor.header_search} small prefix={glyph(IconName::Search,15.)} />}</div>.into_any_element()
        }else{div().into_any_element()}}
        {tool("reset-workspace","Reset layout",Some(IconName::PanelsTopLeft),Action::ResetWorkspace,false,cx)}
        {tool("project-menu","My Project ⌄",None,Action::Project,false,cx)}
        {if editor.state.screen.has_timeline(){tool("share","Share",Some(IconName::Share2),Action::Share,false,cx).into_any_element()}else{tool("notifications","",Some(IconName::Bell),Action::Mock("Notifications"),false,cx).into_any_element()}}
        {tool("export-top","Export",Some(IconName::Upload),Action::Export,true,cx)}
        <Button args={"profile"} ghost label="A" size={px(34.)} rounded-full bg={rgb(PURPLE)} text-color={rgb(TEXT)} on-click={cx.listener(|this,_,window,cx|this.dispatch(Action::Mock("Account menu"),window,cx))} />
    </div>
}
