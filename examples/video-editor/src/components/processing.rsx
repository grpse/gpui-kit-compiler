use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;

use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen}};
#[gpui]
pub fn processing_queue(editor:&Editor,compact:bool,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    if editor.importing {
        return <div flex flex-col gap-3 p-4>
            <div font-semibold>Importing media…</div>
            <div text-color={rgb(MUTED)} text-size={px(12.)}>Indexing video and separating lossless audio channels and waveforms.</div>
            {tool("cancel-file-import","Cancel import",None,Action::CancelAll,false,cx)}
        </div>.into_any_element();
    }
    let jobs:Vec<_>=editor.state.jobs.iter().enumerate().filter(|(_,job)|!job.cancelled).collect();
    if jobs.is_empty() {
        return <div flex flex-col gap-3 p-4>
            <div font-semibold>No pending imports</div>
            <div text-color={rgb(MUTED)} text-size={px(12.)}>Imported files are ready in the library and timeline, with separate audio channel tracks.</div>
            {tool("queue-import-folder","Import folder",Some(IconName::FolderOpen),Action::ImportFolder,false,cx)}
        </div>.into_any_element();
    }
    let overall=if jobs.is_empty(){0.}else{jobs.iter().map(|(_,j)|j.progress as f32).sum::<f32>()/jobs.len() as f32};
    <div flex flex-col flex-shrink-0 gap={px(10.)} p={px(if compact{12.}else{16.})} rounded-lg border-1 border-color={rgb(BORDER)} bg={rgb(PANEL)}>
        <div flex flex-wrap items-center gap-3>
            <div text-color={rgb(PURPLE)}>{glyph(IconName::Settings,28.)}</div>
            <div flex-1 min-w-0><div font-semibold>{if editor.state.screen==Screen::Overview {"Preprocessing Media"}else{"Media Processing"}}</div><div whitespace-normal text-size={px(11.)} text-color={rgb(MUTED)}>Generating previews, thumbnails, and audio waveforms…</div></div>
            <div text-size={px(11.)}>{format!("{overall:.0}%")}</div>
            {tool("cancel-processing","Cancel",None,Action::CancelAll,false,cx)}
        </div>
        {meter(overall/100.,PURPLE,8.)}
        {if jobs.is_empty(){<div text-color={rgb(MUTED)} text-size={px(12.)}>No files in the processing queue. Use Import files or Import folder to add media.</div>.into_any_element()}else{div().into_any_element()}}
        <div flex flex-col gap-2 children={jobs.into_iter().map(|(index,job)| {
            let asset=&editor.state.assets[job.asset];
            <div flex flex-wrap items-center gap-2 text-size={px(11.)} flex-shrink-0 py-1>
            <div text-color={rgb(if job.progress==100 {GREEN}else{PURPLE})}>{glyph(if job.progress==100{IconName::CircleCheck}else{IconName::Clock},14.)}</div>
            {thumbnail(asset,40.,24.)}
            <div w={px(112.)} truncate>{asset.name.clone()}</div>
            <div w-full min-w-0 text-color={rgb(MUTED)} truncate>{if job.progress==100 {"Processed"}else if job.progress==0 {"Queued…"}else if job.asset==1 {"Analyzing audio…"}else {"Generating preview (480p)…"}}</div>
            <div flex-1 min-w-0>{meter(job.progress as f32/100.,PURPLE,5.)}</div>
            <div w={px(32.)} text-color={rgb(if job.progress==100{GREEN}else{MUTED})}>{if job.progress==100 {"Done".into()}else{format!("{}%",job.progress)}}</div>
            {tool(format!("cancel-job-{index}"),"",Some(IconName::X),Action::CancelJob(index),false,cx)}
            </div>
            })} />
    </div>.into_any_element()
}

#[gpui]
pub fn import_overview(editor:&Editor,width:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let jobs:Vec<_>=editor.state.jobs.iter().filter(|j|!j.cancelled).take(3).collect();
    <div flex flex-col flex-shrink-0 gap={px(16.)} mb={px(20.)}>
        <div id="import-drop-zone" role={Role::Button} aria-label="Import files" flex flex-col items-center justify-center gap-3 w-full h={px(150.)} bg={rgb(PANEL)} border-1 border-color={rgb(0x4f5666)} rounded-lg cursor-pointer
            on-click={cx.listener(|this,_,window,cx|this.dispatch(Action::Import,window,cx))}>
            <div text-color={rgb(0xb3abda)}>{glyph(IconName::CloudUpload,36.)}</div>
            <div font-semibold text-color={rgb(0xa086ff)}>Import Files</div>
            <div text-size={px(12.)} text-color={rgb(MUTED)}>Videos, audio or images</div>
            <div text-size={px(12.)} text-color={rgb(MUTED)}>or click to browse</div>
        </div>
        <div flex flex-wrap gap-2>
            {tool("overview-import-folder","Import folder",Some(IconName::FolderOpen),Action::ImportFolder,false,cx)}
            {if editor.importing{tool("overview-cancel-import","Cancel import",None,Action::CancelAll,false,cx).into_any_element()}else{div().into_any_element()}}
        </div>
        {if !jobs.is_empty(){<div min-w-0 flex items-center gap-4 p-4 bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded-lg>
            {photo(&editor.state.assets[0],(width*0.2).min(175.),145.)}
            <div flex flex-col gap-4 flex-1>
                <div flex items-center justify-between><div font-semibold>{format!("Processing {} files…",jobs.len())}</div>{tool("overview-cancel","Cancel",None,Action::CancelAll,false,cx)}</div>
                <div flex flex-col gap-2 children={jobs.into_iter().map(|job| <div flex items-center gap-3 text-size={px(11.)}>
                    <div w={px(100.)} truncate>{editor.state.assets[job.asset].name.clone()}</div>
                    <div flex-1>{meter(job.progress as f32/100.,PURPLE,6.)}</div>
                    <div w={px(30.)}>{format!("{}%",job.progress)}</div>
                    </div>)} />
            </div>
        </div>.into_any_element()}else{div().into_any_element()}}
    </div>
}
