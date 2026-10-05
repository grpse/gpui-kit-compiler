use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen}};
#[gpui]
pub fn sidebar(editor:&Editor,width:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let library=editor.state.screen==Screen::Library;
    let nav:Vec<(&str,IconName)>=if library {vec![("All",IconName::Images),("Videos",IconName::Film),("Audio",IconName::Music),("Images",IconName::Image),("Favorites",IconName::Star),("Recent",IconName::Clock)]}
        else {vec![("All",IconName::Folder),("Audio",IconName::Music),("Text",IconName::Type),("Effects",IconName::Sparkles),("Transitions",IconName::PanelsTopLeft),("Elements",IconName::Layers),("Captions",IconName::Captions)]};
    <div id="navigation" flex flex-col w={px(width)} h-full flex-shrink-0 bg={rgb(PANEL)} border-r-1 border-color={rgb(BORDER)} px={px(8.)} py={px(12.)} gap={px(8.)} overflow-y-scroll overflow-x-scroll>
        {if library {<div mb-3>{tool("import-side","Import Media",Some(IconName::FileUp),Action::Import,true,cx)}</div>.into_any_element()}else{div().into_any_element()}}
        <div flex flex-col flex-shrink-0 gap-2 children={nav.into_iter().map(|(category,icon)| {
            let selected=editor.state.category==category&&editor.state.folder.is_empty();
            let action=Action::Category(category.into());
            let label=if category=="All" {if library {"All Media"}else{"Media"}}else{category};
            <Button args={SharedString::from(format!("nav-{category}"))} ghost small justify-start w-full h={px(if library {40.}else{48.})} px={px(8.)} icon={icon} label={label}
            text-size={px(if width<140. {10.}else{12.})} text-color={rgb(if selected {0xb298ff}else{TEXT})}
            bg={rgb(if selected {0x26223f}else{PANEL})} border-l={px(if selected{2.}else{0.})} border-color={rgb(PURPLE)}
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(action.clone(),window,cx))} />
            })} />
        {if library {
            <div flex flex-col flex-shrink-0 gap-1 mt-3 border-t-1 border-color={rgb(BORDER)} pt-3>
                <div flex items-center justify-between px-2><div text-color={rgb(MUTED)}>Folders</div>{tool("add-folder","",Some(IconName::Plus),Action::ImportFolder,false,cx)}</div>
                <div flex flex-col gap-2 children={editor.state.mock_folders().into_iter().map(|(folder,indent)| {
                    let selected=editor.state.folder==folder;
                    <div pl={px(indent as f32*18.)}>
                    <Button args={SharedString::from(format!("folder-{folder}"))} ghost justify-start w-full h={px(39.)} icon={if folder=="Trash"{IconName::Trash}else{IconName::Folder}} label={folder.clone()}
                    bg={rgb(if selected{0x26223f}else{PANEL})} text-color={rgb(if selected {0xb298ff}else{TEXT})} text-size={px(12.)}
                    on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Folder(folder.clone()),window,cx))} />
                    </div>
                    })} />
                <div flex flex-col gap-2 children={editor.state.imported_folders().into_iter().map(|path| {
                    let folder=path.display().to_string();
                    let label=path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let selected=editor.state.folder==folder;
                    <Button args={SharedString::from(format!("import-folder-{folder}"))} ghost justify-start w-full h={px(39.)} icon={IconName::FolderOpen} label={label} tooltip={folder.clone()}
                        bg={rgb(if selected{0x26223f}else{PANEL})} text-size={px(12.)}
                        on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Folder(folder.clone()),window,cx))} />
                })} />
            </div>.into_any_element()
        } else {<div flex flex-col flex-shrink-0 mt={px(55.)} pt-3 border-t-1 border-color={rgb(BORDER)} gap-2>
                {tool("projects","Projects",Some(IconName::Folder),Action::Project,false,cx)}
                {tool("settings","Settings",Some(IconName::Settings),Action::Mock("Settings"),false,cx)}
            </div>.into_any_element()}}
    </div>
}
