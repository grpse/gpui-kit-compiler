use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::ButtonVariants as _,Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen,Kind}};
#[gpui]
pub fn details_panel(editor:&Editor,width:f32,_height:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let asset=&editor.state.assets[editor.state.selected];
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
                <div flex items-center justify-between font-semibold><div>{asset.name}</div>{tool("favorite","",Some(IconName::Star),Action::Favorite,asset.favorite,cx)}</div>
                {if editor.state.detail_tab==0 {
                    <div flex flex-col gap-3>
                        <div p-3 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-lg>
                            {metadata(vec![("Type",if asset.kind==Kind::Audio{"Audio (WAV)".into()}else if asset.kind==Kind::Image{"Image (RGB)".into()}else{"Video (MP4)".into()}),("Resolution",if asset.kind==Kind::Audio{"—".into()}else{"3840 × 2160 (4K)".into()}),("Frame Rate",if asset.kind!=Kind::Video{"—".into()}else{"59.94 fps".into()}),("Duration",asset.duration_label()),("File Size",format!("{} MB",asset.size)),("Codec",if asset.kind==Kind::Audio{"PCM".into()}else if asset.kind==Kind::Image{"JPEG / PNG".into()}else{"H.264 / AAC".into()}),("Date Added",asset.added.into()),("Status","✓  Processed".into())])}
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
                    <div>File Location</div><div text-color={rgb(MUTED)} text-size={px(11.)}>{format!("/Projects/Travel/{}",asset.name)}</div>
                    {tool("show-in-finder","Show in Finder",Some(IconName::FolderOpen),Action::Mock("Show in Finder"),false,cx)}
                </div>
            </div>
        </div>

    </div>
}

#[gpui]
pub fn generated_assets(editor:&Editor,_width:f32,_cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let kind=editor.state.assets[editor.state.selected].kind;
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
    </div>
}
