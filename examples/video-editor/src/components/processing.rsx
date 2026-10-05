use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button,ButtonVariants as _};
use gpui_kit::component::Sizable as _;
use crate::{editor::Editor,generated::primitives::*,state::Action};
use rsx_video_editor::processing::Stage;
#[gpui]
pub fn processing_queue(editor:&Editor,_compact:bool,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let ready=editor.processing.iter().filter(|job|job.stage==Stage::Ready).count();
    <div id="processing-progress" flex flex-col size-full min-h-0 overflow-y-scroll gap-2 p-3>
        {if let Some(progress)=&editor.export_progress {
            <div flex flex-col flex-shrink-0 gap-2 border-t-1 border-color={rgb(BORDER)} pt-3>
                <div font-semibold>MP4 export</div>
                <div text-size={px(11.)} text-color={rgb(MUTED)}>{progress.step}</div>
                {meter(progress.percent as f32/100.,PURPLE,6.)}
                <div text-size={px(11.)}>{format!("{}%",progress.percent)}</div>
                <div flex justify-start gap-2>
                    {if editor.exporting {tool("cancel-export","Cancel export",Some(IconName::X),Action::CancelExport,false,cx).into_any_element()}else if let Some(path)=editor.export_path.clone(){
                        <Button args={"show-export"} ghost small label="Show exported video" on-click={cx.listener(move|_,_,_,cx|cx.reveal_path(&path))} />.into_any_element()
                    }else{div().into_any_element()}}
                </div>
            </div>.into_any_element()
        }else{div().into_any_element()}}
        <div font-semibold>{if editor.importing{"Processing media"}else if editor.processing.is_empty(){"Ready to import"}else{"Media processing"}}</div>
        {if !editor.processing.is_empty(){<div text-size={px(11.)} text-color={rgb(MUTED)}>{format!("{ready} of {} files ready",editor.processing.len())}</div>.into_any_element()}else{div().into_any_element()}}
        <div flex flex-col flex-shrink-0 gap-3 children={editor.processing.iter().map(|job| {
            let color=if job.stage==Stage::Ready{GREEN}else if job.stage==Stage::Failed{0xff8a80}else{PURPLE};
            <div flex flex-col gap-1>
                <div text-size={px(12.)} truncate>{job.path.file_name().unwrap_or_default().to_string_lossy().into_owned()}</div>
                <div text-size={px(10.)} text-color={rgb(color)}>{job.stage.label()}</div>
                {if let Some(percent)=job.progress.filter(|_|job.stage==Stage::Decoding){<div flex items-center gap-2><div flex-1>{meter(percent as f32/100.,color,5.)}</div><div text-size={px(10.)}>{format!("{percent}%")}</div></div>.into_any_element()}else{div().into_any_element()}}
                {if let Some(error)=&job.error{<div text-size={px(10.)} text-color={rgb(MUTED)}>{error.clone()}</div>.into_any_element()}else{div().into_any_element()}}
            </div>
        })} />
        <div flex flex-wrap flex-shrink-0 justify-start gap-2>
            {if editor.importing {tool("cancel-file-import","Cancel processing",Some(IconName::X),Action::CancelAll,false,cx).into_any_element()}else{
                <div flex flex-wrap gap-2>{tool("queue-import-files","Import files",Some(IconName::FileUp),Action::Import,false,cx)}{tool("queue-import-folder","Import folder",Some(IconName::FolderOpen),Action::ImportFolder,false,cx)}</div>.into_any_element()
            }}
        </div>

    </div>.into_any_element()
}
#[gpui]
pub fn import_overview(editor:&Editor,_width:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    <div flex flex-col gap-3 mb-4>
        <div id="import-drop-zone" role={Role::Button} aria-label="Import files" flex items-center gap-3 p-4 bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded-lg cursor-pointer
            on-click={cx.listener(|this,_,window,cx|this.dispatch(Action::Import,window,cx))}>
            <div text-color={rgb(PURPLE)}>{glyph(IconName::FileUp,28.)}</div>
            <div flex flex-col gap-1><div font-semibold>Import files</div><div text-size={px(12.)} text-color={rgb(MUTED)}>Choose videos, audio or images</div></div>
        </div>
        {processing_queue(editor,false,cx)}
    </div>
}
