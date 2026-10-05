use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::{Action,Kind}};
#[gpui]
pub fn details_panel(editor:&Editor,width:f32,_height:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let Some(asset)=editor.state.assets.get(editor.state.selected) else {return <div p-4 text-color={rgb(MUTED)}>Import media to see its details.</div>.into_any_element();};
    <div id="details-scroll" flex flex-col size-full min-h-0 p-3 gap-3 bg={rgb(PANEL)} overflow-y-scroll>
        <div flex flex-wrap justify-start gap-1 children={["Details","Preview","Audio","Metadata"].into_iter().enumerate().map(|(i,label)|
            <Button args={SharedString::from(format!("details-tab-{i}"))} ghost small label={label} text-color={rgb(if editor.state.detail_tab==i{PURPLE}else{TEXT})}
                on-click={cx.listener(move|this,_,window,cx|this.dispatch(Action::DetailTab(i),window,cx))} />)} />
        <div font-semibold>{asset.name.clone()}</div>
        {if let Some(error)=&asset.metadata_error{<div text-color={rgb(0xff8a80)} text-size={px(12.)}>{error.clone()}</div>.into_any_element()}else{div().into_any_element()}}
        {match editor.state.detail_tab {
            1=>thumbnail(asset,(width-24.).max(40.),150.),
            2=>if let Some(media)=&asset.prepared{native_audio_details(media,(width-24.).max(40.)).into_any_element()}else{<div text-color={rgb(MUTED)}>No audio streams.</div>.into_any_element()},
            3=>if let Some(media)=&asset.prepared{
                <div flex flex-col gap-3 children={media.videos.iter().map(|video|metadata(vec![("Stream",format!("Video {} · {}",video.index,video.codec)),("Pixel format",video.pixel_format.clone()),("Color space",video.color_space.clone()),("Color range",video.color_range.clone()),("Primaries",video.color_primaries.clone()),("Transfer",video.color_transfer.clone()),("Time base",format!("{}/{}",video.timing.time_base.numerator,video.timing.time_base.denominator)),("Keyframes",video.keyframes.entries.to_string())]))}>
                    {native_audio_details(media,(width-24.).max(40.))}
                </div>.into_any_element()
            }else{<div text-color={rgb(MUTED)}>No decoded stream metadata.</div>.into_any_element()},
            _=>metadata(vec![("Type",format!("{:?}",asset.kind)),("Resolution",asset.resolution_label()),("Frame rate",asset.frame_rate.clone().unwrap_or_else(||"—".into())),("Duration",asset.duration_label()),("File size",asset.size_label()),("Codec",asset.codec.clone().unwrap_or_else(||"Unknown".into())),("Status",if asset.metadata_error.is_some(){"Failed".into()}else if asset.kind==Kind::Image||asset.prepared.is_some(){"Ready".into()}else{"Not prepared".into()})]).into_any_element(),
        }}
        <div text-size={px(11.)} text-color={rgb(MUTED)}>{asset.location()}</div>
        <div flex flex-wrap justify-start gap-2>{tool("show-in-finder","Show in Finder",Some(IconName::FolderOpen),Action::ShowInFinder,false,cx)}{tool("favorite","Favorite",Some(IconName::Star),Action::Favorite,asset.favorite,cx)}</div>
    </div>.into_any_element()
}
#[gpui]
pub fn generated_assets(editor:&Editor,_width:f32,_cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let Some(asset)=editor.state.assets.get(editor.state.selected) else {return <div p-3 text-color={rgb(MUTED)}>Import media to prepare it for editing.</div>.into_any_element();};
    <div flex flex-col gap-3 p-3>
        <div font-semibold>Prepared media</div>
        {if let Some(media)=&asset.prepared {
            metadata(vec![("Video index",format!("{} keyframes",media.videos.iter().map(|video|video.keyframes.entries).sum::<u64>())),("Audio",format!("{} channel caches",media.audio.iter().map(|audio|audio.channels.len()).sum::<usize>())),("Waveforms","Measured from decoded audio".into())]).into_any_element()
        }else{<div text-color={rgb(MUTED)}>{if asset.kind==Kind::Image{"Still image ready"}else{"No prepared media available"}}</div>.into_any_element()}}
        {thumbnail(asset,160.,90.)}
    </div>.into_any_element()
}
#[gpui]
fn native_audio_details(media:&crate::preprocess::PreparedMedia,width:f32) -> impl IntoElement + use<> {
    <div flex flex-col gap-3 children={media.audio.iter().map(|audio|<div flex flex-col gap-2>
        {metadata(vec![("Stream",format!("Audio {} · {}",audio.index,audio.codec)),("Sample Rate",format!("{} Hz",audio.sample_rate)),("Precision",audio.encoding.label().into()),("Layout",audio.layout.clone()),("Samples",audio.samples.to_string())])}
        <div flex flex-col gap-2 children={audio.channels.iter().enumerate().map(|(index,channel)|<div flex flex-col gap-1>
            <div text-size={px(11.)} text-color={rgb(GREEN)}>{channel.name.clone()}</div>
            {channel_waveform(audio,index,0.,audio.duration(),width,48.,GREEN)}
        </div>)} />
    </div>)}>
        {if media.audio.is_empty(){<div text-color={rgb(MUTED)}>This source has no audio streams.</div>.into_any_element()}else{div().into_any_element()}}
    </div>
}
