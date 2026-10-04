use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},input::Input,Sizable as _};
use crate::{generated::{ui::Editor,primitives::*},state::{Action,Screen}};
impl Editor {
    #[gpui]
    pub fn topbar(&self,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        <div flex items-center h={px(58.)} px={px(20.)} gap={px(18.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)}>
            <div flex items-center gap-2 text-color={rgb(PURPLE)}>
                <div flex items-center justify-center size={px(29.)} border-2 border-color={rgb(PURPLE)} rounded={px(5.)}>{glyph(IconName::Play,20.)}</div>
                <div text-size={px(23.)} font-semibold text-color={rgb(TEXT)}>FlowCut</div>
            </div>
            <div flex items-center gap-1 ml={px(15.)} children={Screen::ALL.into_iter().map(|screen| <Button args={SharedString::from(format!("screen-{screen:?}"))} ghost label={screen.label()} h={px(38.)} px={px(17.)} text-size={px(13.)}
                bg={rgb(if self.state.screen==screen {0x211b34}else{BG})} text-color={rgb(if self.state.screen==screen {0xb394ff}else{MUTED})}
                border-b={px(if self.state.screen==screen {2.}else{0.})} border-color={rgb(PURPLE)}
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Screen(screen),window,cx))} />)}>
            </div>
            <div flex-1 />
            {if !self.state.screen.has_timeline(){
                <div w={px(260.)}>{<Input args={&self.header_search} small prefix={glyph(IconName::Search,15.)} />}</div>.into_any_element()
            }else{div().into_any_element()}}
            {tool("reset-workspace","Reset layout",Some(IconName::PanelsTopLeft),Action::ResetWorkspace,false,cx)}
            {tool("project-menu","My Project ⌄",None,Action::Project,false,cx)}
            {if self.state.screen.has_timeline(){tool("share","Share",Some(IconName::Share2),Action::Share,false,cx).into_any_element()}else{tool("notifications","",Some(IconName::Bell),Action::Mock("Notifications"),false,cx).into_any_element()}}
            {tool("export-top","Export",Some(IconName::Upload),Action::Export,true,cx)}
            <Button args={"profile"} ghost label="A" size={px(34.)} rounded-full bg={rgb(PURPLE)} text-color={rgb(TEXT)} on-click={cx.listener(|this,_,window,cx|this.dispatch(Action::Mock("Account menu"),window,cx))} />
        </div>
    }
    #[gpui]
    pub fn sidebar(&self,width:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let library=self.state.screen==Screen::Library;
        let nav:Vec<(&str,IconName)>=if library {vec![("All",IconName::Images),("Videos",IconName::Film),("Audio",IconName::Music),("Images",IconName::Image),("Favorites",IconName::Star),("Recent",IconName::Clock)]}
            else {vec![("All",IconName::Folder),("Audio",IconName::Music),("Text",IconName::Type),("Effects",IconName::Sparkles),("Transitions",IconName::PanelsTopLeft),("Elements",IconName::Layers),("Captions",IconName::Captions)]};
        <div id="navigation" flex flex-col w={px(width)} h-full flex-shrink-0 bg={rgb(PANEL)} border-r-1 border-color={rgb(BORDER)} px={px(8.)} py={px(12.)} gap={px(8.)} overflow-y-scroll overflow-x-scroll>
            {if library {<div mb-3>{tool("import-side","Import Media",Some(IconName::FileUp),Action::Import,true,cx)}</div>.into_any_element()}else{div().into_any_element()}}
            <div flex flex-col flex-shrink-0 gap-2 children={nav.into_iter().map(|(category,icon)| {
                let selected=self.state.category==category&&self.state.folder.is_empty();
                let action=Action::Category(category.into());
                let label=if category=="All" {if library {"All Media"}else{"Media"}}else{category};
                <Button args={SharedString::from(format!("nav-{category}"))} ghost small justify-start w-full h={px(if library {40.}else{48.})} px={px(8.)} icon={icon} label={label}
                text-size={px(if width<140. {10.}else{12.})} text-color={rgb(if selected {0xb298ff}else{TEXT})}
                bg={rgb(if selected {0x26223f}else{PANEL})} border-l={px(if selected{2.}else{0.})} border-color={rgb(PURPLE)}
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(action.clone(),window,cx))} />
                })} />
            {if library {
                <div flex flex-col flex-shrink-0 gap-1 mt-3 border-t-1 border-color={rgb(BORDER)} pt-3>
                    <div flex items-center justify-between px-2><div text-color={rgb(MUTED)}>Folders</div>{tool("add-folder","",Some(IconName::Plus),Action::Mock("Create folder"),false,cx)}</div>
                    <div flex flex-col gap-2 children={[("Travel",0),("Projects",0),("House",1),("Documentary",1),("Ads",1),("SFX",0),("Music",0),("Stock",0),("Trash",0)].into_iter().map(|(folder,indent)| {
                        let selected=self.state.folder==folder;
                        <div pl={px(indent as f32*18.)}>
                        <Button args={SharedString::from(format!("folder-{folder}"))} ghost justify-start w-full h={px(39.)} icon={if folder=="Trash"{IconName::Trash}else{IconName::Folder}} label={folder}
                        bg={rgb(if selected{0x26223f}else{PANEL})} text-color={rgb(if selected {0xb298ff}else{TEXT})} text-size={px(12.)}
                        on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Folder(folder.into()),window,cx))} />
                        </div>
                        })} />
                </div>.into_any_element()
            } else {<div flex flex-col flex-shrink-0 mt={px(55.)} pt-3 border-t-1 border-color={rgb(BORDER)} gap-2>
                    {tool("projects","Projects",Some(IconName::Folder),Action::Project,false,cx)}
                    {tool("settings","Settings",Some(IconName::Settings),Action::Mock("Settings"),false,cx)}
                </div>.into_any_element()}}
        </div>
    }
}
