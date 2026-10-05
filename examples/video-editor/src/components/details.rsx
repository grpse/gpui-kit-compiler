use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::ButtonVariants as _,Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen,Kind}};
#[gpui]
pub fn details_panel(editor:&Editor,width:f32,_height:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let Some(asset)=editor.state.assets.get(editor.state.selected) else {return <div flex items-center justify-center size-full text-color={rgb(MUTED)}>Import media to see its details</div>.into_any_element();};
    let overview=editor.state.screen==Screen::Overview;
    let tabs=if overview{["Info","Audio","Analysis","Markers"]}else if asset.kind==Kind::Audio{["Details","Waveform","Audio","Metadata"]}else{["Details","Thumbnails","Audio","Metadata"]};
    <div id="details-scroll" flex flex-col size-full min-w-0 min-h-0 p={px(12.)} gap={px(12.)} bg={rgb(PANEL)} border-l-1 border-color={rgb(BORDER)} overflow-y-scroll overflow-x-scroll>
        <div flex-shrink-0 border-1 border-color={rgb(BORDER)} rounded-lg overflow-hidden>
            <div flex border-b-1 border-color={rgb(BORDER)} children={tabs.into_iter().enumerate().map(|(i,label)|
                <gpui_kit::component::button::Button args={SharedString::from(format!("details-tab-{i}"))} ghost small label={label} flex-1 px-1 h={px(36.)} text-size={px(11.)}
                bg={rgb(if editor.state.detail_tab==i{0x25203c}else{PANEL})} text-color={rgb(if editor.state.detail_tab==i{0xb298ff}else{MUTED})}
                border-b={px(if editor.state.detail_tab==i{2.}else{0.})} border-color={rgb(PURPLE)}
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::DetailTab(i),window,cx))} />)}>
            </div>
            <div flex flex-col gap-3 p={px(12.)}>
                <div flex items-center justify-between font-semibold><div>{asset.name.clone()}</div>{tool("favorite","",Some(IconName::Star),Action::Favorite,asset.favorite,cx)}</div>
                {if asset.path.is_some() {
                    <div flex flex-col gap-3>
                        {if let Some(media)=&asset.prepared {
                            if editor.state.detail_tab==2||(asset.kind==Kind::Audio&&editor.state.detail_tab==1) {
                                native_audio_details(media,(width-52.).max(40.)).into_any_element()
                            }else if editor.state.detail_tab==1 {
                                <div flex flex-col gap-2>{thumbnail(asset,(width-52.).max(40.),110.)}<div text-size={px(11.)} text-color={rgb(MUTED)}>Source poster · encoded video preserved</div></div>.into_any_element()
                            }else if editor.state.detail_tab==3 {
                                <div flex flex-col gap-3 children={media.videos.iter().map(|video|metadata(vec![("Stream",format!("Video {} · {}",video.index,video.codec)),("Pixel Format",video.pixel_format.clone()),("Color Space",video.color_space.clone()),("Color Range",video.color_range.clone()),("Primaries",video.color_primaries.clone()),("Transfer",video.color_transfer.clone()),("Time Base",format!("{}/{}",video.timing.time_base.numerator,video.timing.time_base.denominator)),("Keyframes",video.keyframes.entries.to_string())]))}>
                                    <div text-size={px(11.)} text-color={rgb(MUTED)}>Audio cache uses native-endian samples. Original media remains the source for export.</div>
                                </div>.into_any_element()
                            }else {
                                <div flex flex-col gap-3>{metadata(vec![("Type",format!("{:?}",asset.kind)),("Resolution",asset.resolution_label()),("Frame Rate",asset.frame_rate.clone().unwrap_or_else(||"—".into())),("Duration",asset.duration_label()),("File Size",asset.size_label()),("Codec",asset.codec.clone().unwrap_or_else(||"Unknown".into())),("Streams",format!("{} video · {} audio",media.videos.len(),media.audio.len())),("Channels",format!("{} separate tracks",media.audio.iter().map(|audio|audio.channels.len()).sum::<usize>())),("PCM / Index",format!("{:.1} MiB cache",media.cache_bytes as f64/1048576.)),("Status","Preprocessed".into())])}<div text-size={px(11.)} text-color={rgb(MUTED)}>Native sample rate and precision retained. Playback mixes unmuted channels for stereo monitoring. Project export is planned.</div></div>.into_any_element()
                            }
                        }else{metadata(vec![("Type",format!("{:?}",asset.kind)),("Resolution",asset.resolution_label()),("Duration",asset.duration_label()),("File Size",asset.size_label())]).into_any_element()}}
                        {if let Some(error)=&asset.metadata_error {<div text-size={px(11.)} text-color={rgb(MUTED)}>{error.clone()}</div>.into_any_element()}else{div().into_any_element()}}
                    </div>.into_any_element()
                }else if editor.state.detail_tab==0 {
                    <div flex flex-col gap-3>
                        <div p-3 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-lg>
                            {metadata(vec![("Type",if asset.kind==Kind::Audio{"Audio (WAV)".into()}else if asset.kind==Kind::Image{"Image (RGB)".into()}else{"Video (MP4)".into()}),("Resolution",if asset.kind==Kind::Audio{"—".into()}else{"3840 × 2160 (4K)".into()}),("Frame Rate",if asset.kind!=Kind::Video{"—".into()}else{"59.94 fps".into()}),("Duration",asset.duration_label()),("File Size",asset.size_label()),("Codec",match asset.kind{Kind::Audio=>"PCM".into(),Kind::Image=>"JPEG / PNG".into(),Kind::Video=>"H.264 / AAC".into()}),("Date Added",asset.added.clone()),("Status","✓  Processed".into())])}
                        </div>
                        {if overview{<div p-3 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-lg>{metadata(vec![("Audio","48 kHz".into()),("Channels","Stereo".into()),("Codec","AAC".into())])}</div>.into_any_element()}else{div().into_any_element()}}
                    </div>.into_any_element()
                }else if editor.state.detail_tab==1&&!overview{
                    <div flex flex-wrap gap-2 children={(0..6).map(|_|thumbnail(asset,(width-58.)/2.,70.))}><div text-color={rgb(MUTED)} text-size={px(11.)}>{match asset.kind{Kind::Video=>"Cached thumbnails · 1 frame / sec",Kind::Image=>"Cached still image",Kind::Audio=>"Cached waveform previews"}}</div></div>.into_any_element()
                }else if (editor.state.detail_tab==1||editor.state.detail_tab==2&&!overview)&&asset.kind==Kind::Image{
                    <div text-color={rgb(MUTED)}>This still image has no audio track.</div>.into_any_element()
                }else if editor.state.detail_tab==1||editor.state.detail_tab==2&&!overview{
                    <div flex flex-col gap-3>{waveform(width-52.,90.,GREEN,9)}{metadata(vec![("Sample Rate","48 kHz".into()),("Channels","Stereo".into()),("Codec","AAC".into())])}{tool("audio-detail-mute","Mute preview",Some(IconName::VolumeX),Action::Mute,editor.state.muted,cx)}</div>.into_any_element()
                }else if editor.state.detail_tab==2{
                    <div flex flex-col gap-3><div text-color={rgb(GREEN)}>✓ Scene and audio analysis ready</div>{metadata(vec![("Scenes","3 detected".into()),("Loudness","−14 LUFS".into()),("Motion","High".into())])}</div>.into_any_element()
                }else if overview{
                    <div flex flex-col gap-3><div text-color={rgb(MUTED)}>No markers yet</div>{tool("add-detail-marker","Add Marker",Some(IconName::Flag),Action::Tool("Add marker"),false,cx)}</div>.into_any_element()
                }else{
                    metadata(vec![("Container",match asset.kind{Kind::Video=>"QuickTime / MP4",Kind::Audio=>"WAV",Kind::Image=>"JPEG / PNG"}.into()),("Color Space",if asset.kind==Kind::Audio{"—"}else{"sRGB / Rec. 709"}.into()),("Bit Rate",if asset.kind==Kind::Audio{"1.5 Mbps"}else if asset.kind==Kind::Image{"—"}else{"142.7 Mbps"}.into()),("Source","Local import (fixture)".into())]).into_any_element()
                }}
                <div p-3 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-lg flex flex-col gap-2>
                    <div>File Location</div><div text-color={rgb(MUTED)} text-size={px(11.)}>{asset.location()}</div>
                    {tool("show-in-finder","Show in Finder",Some(IconName::FolderOpen),Action::ShowInFinder,false,cx)}
                </div>
            </div>
        </div>

    </div>.into_any_element()
}

#[gpui]
pub fn generated_assets(editor:&Editor,_width:f32,_cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let Some(asset)=editor.state.assets.get(editor.state.selected) else {return <div flex items-center justify-center size-full text-color={rgb(MUTED)}>Import media to see its details</div>.into_any_element();};
    if asset.path.is_some() {
        return <div flex flex-col gap-3 p-3>
            <div font-semibold>Preprocessed media</div>
            {if let Some(media)=&asset.prepared {
                <div flex flex-col gap-2>
                    {metadata(vec![("Video Index",format!("{} keyframes",media.videos.iter().map(|video|video.keyframes.entries).sum::<u64>())),("Channels",format!("{} lossless PCM caches",media.audio.iter().map(|audio|audio.channels.len()).sum::<usize>())),("Waveforms","Measured min/max pyramids".into()),("Status","Ready for editing".into())])}
                </div>.into_any_element()
            }else{div().into_any_element()}}
            {thumbnail(asset,160.,90.)}
            <div text-size={px(11.)} text-color={rgb(MUTED)}>{if asset.poster.is_some(){"Source thumbnail available"}else{"No thumbnail generated"}}</div>
        </div>.into_any_element();
    }
    let kind=asset.kind;
    let items=match kind {
        Kind::Video=>vec![(IconName::Image,"Low-resolution preview","480p (cached)"),(IconName::LayoutGrid,"Video thumbnails","1 frame / sec"),(IconName::AudioLines,"Audio waveform","Waveform data"),(IconName::FileVideoCamera,"Proxy file","1080p (H.264)")],
        Kind::Audio=>vec![(IconName::AudioLines,"Audio waveform","Waveform data"),(IconName::Music,"Audio preview","48 kHz stereo")],
        Kind::Image=>vec![(IconName::Image,"Image thumbnail","RGB preview"),(IconName::Image,"Optimized image","Cached artwork")],
    };
    <div id="generated-assets-scroll" size-full min-h-0 overflow-y-scroll overflow-x-scroll p-3>
        <div flex flex-col gap-3 children={items.into_iter().map(|(icon,title,subtitle)| <div flex flex-shrink-0 items-center gap-3 p-2 bg={rgb(RAISED)} rounded-md>
            <div text-color={rgb(PURPLE)}>{glyph(icon,20.)}</div>
            <div flex-1 min-w-0><div text-size={px(11.)}>{title}</div><div text-size={px(10.)} text-color={rgb(MUTED)}>{subtitle}</div></div>
            <div text-size={px(11.)} text-color={rgb(GREEN)}>● Ready</div>
            </div>)} />
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
