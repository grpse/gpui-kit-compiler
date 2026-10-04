use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::slider::Slider;
use crate::{generated::{ui::Editor,primitives::*},state::{Action,Screen,Kind}};
impl Editor {
    #[gpui]
    pub fn tracking_overlay(&self,width:f32,height:f32) -> impl IntoElement + use<> {
        let color=GREEN;
        let box_mode=self.state.tracking_mode==1;
        <div absolute left={px(width*0.35)} top={px(height*0.075)} w={px(width*0.29)} h={px(height*0.72)}
            border={px(if box_mode {1.5}else{0.})} border-color={rgb(color)} children={if box_mode {(0..4).map(|corner|<div absolute left={px(if corner%2==0{-4.}else{width*0.29-4.})} top={px(if corner<2{-4.}else{height*0.72-4.})} size={px(8.)} bg={rgb(color)} border-1 border-color={rgb(0x244c3e)} />).collect::<Vec<_>>()}else{vec![]}}>

            <div absolute left={relative(0.5)} top={relative(0.37)} text-color={rgb(GREEN)}>{glyph(if self.state.tracking_mode==2{IconName::ScanFace}else{IconName::Crosshair},24.)}</div>
            {if self.state.toggles.contains("Show Track Path") {<div absolute bottom={px(12.)} left={px(5.)} flex items-center gap={px(6.)} children={(0..9).map(|i|<div size={px(3.)} rounded-full bg={rgb(color)} mt={px(((i as f32)*0.8).sin()*12.)} />)}>
                </div>.into_any_element()}else{div().into_any_element()}}
        </div>
    }
    #[gpui]
    pub fn preview_image(&self,width:f32,height:f32,tracking:bool) -> impl IntoElement + use<> {
        let asset=&self.state.assets[self.state.selected];
        <div relative w={px(width)} h={px(height)} overflow-hidden rounded={px(5.)} bg={rgb(0x070b10)}>
            {thumbnail(asset,width,height)}
            {if let Some(preset)=self.state.selected_preset {
                match self.state.category.as_str() {
                    "Text"|"Captions"=><div absolute left-2 right-2 bottom={px(20.)} text-center font-semibold text-size={px((width/18.).clamp(14.,28.))} bg={rgba(0x00000080)} p-2 rounded-md>{preset}</div>.into_any_element(),
                    "Elements"=><div absolute right={px(20.)} top={px(20.)} text-color={rgb(GREEN)}>{glyph(IconName::Star,48.)}</div>.into_any_element(),
                    "Effects"=><div absolute left-0 top-0 size-full bg={rgba(0x8d53df40)} />.into_any_element(),
                    "Transitions"=><div absolute left-0 top-0 w={relative(0.5)} h-full bg={rgba(0x00000099)} />.into_any_element(),
                    _=>div().into_any_element(),
                }
            }else{div().into_any_element()}}
            {if tracking&&asset.kind==Kind::Video {self.tracking_overlay(width,height).into_any_element()}else{div().into_any_element()}}
            {if asset.kind==Kind::Audio {<div absolute bottom={px(16.)} left={px(16.)} text-size={px(18.)} font-semibold>{asset.name}</div>.into_any_element()}else{div().into_any_element()}}
        </div>
    }
    #[gpui]
    pub fn player_controls(&self,small:bool,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let asset=&self.state.assets[self.state.selected];
        let time=self.state.position;
        <div flex flex-wrap flex-shrink-0 items-center gap={px(if small{5.}else{10.})} justify-between min-h={px(52.)}>
            <div flex items-center gap-1 text-size={px(if small{11.}else{13.})}>
                <div text-color={rgb(PURPLE)} font-semibold>{if small{format!("{:02}:{:02}",time as u32/60,time as u32%60)}else{format!("00:00:{:02}:{:02}",time as u32,((time.fract()*30.).round() as u32).min(29))}}</div>
                <div text-color={rgb(MUTED)}>{format!("/ {}",if self.state.screen.has_timeline(){"00:18".into()}else{asset.duration_label()})}</div>
            </div>
            <div flex gap-1>
                {if !small {tool("previous-frame","",Some(IconName::SkipBack),Action::Step(-1.),false,cx).into_any_element()}else{div().into_any_element()}}
                {tool(if small{"play-small"}else{"play"},"",Some(if self.state.playing{IconName::Pause}else{IconName::Play}),Action::Play,false,cx)}
                {if !small {tool("next-frame","",Some(IconName::SkipForward),Action::Step(1.),false,cx).into_any_element()}else{div().into_any_element()}}
            </div>
            <div flex gap-1>
                {if small{tool("mute-preview","",Some(if self.state.muted{IconName::VolumeX}else{IconName::Volume2}),Action::Mute,self.state.muted,cx).into_any_element()}else{div().into_any_element()}}
                {tool("preview-quality",["4K","1/2","Fit"][self.state.quality],None,Action::Quality,false,cx)}
                {tool("fullscreen-preview","",Some(if self.state.fullscreen{IconName::Minimize}else{IconName::Maximize}),Action::Fullscreen,false,cx)}
            </div>
        </div>
    }
    #[gpui]
    pub fn player(&self,width:f32,height:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let tracking=self.state.screen==Screen::Tracking||self.state.inspector==2;
        let picture_height=((width-12.).max(40.)*0.614).min((height-135.).max(60.));
        let picture_width=(picture_height/0.614).min((width-12.).max(40.));
        <div id="player-scroll" flex flex-col items-center size-full min-w-0 min-h-0 overflow-y-scroll p={px(6.)} gap-1>
            <div flex-shrink-0>{self.preview_image(picture_width,picture_height,tracking)}</div>
            <div w-full flex-shrink-0>{self.player_controls(width<420.,cx)}</div>
            {if tracking {
                <div relative w-full flex-shrink-0 h={px(37.)} overflow-hidden rounded-sm>
                    <div flex children={(0..10).map(|_|thumbnail(&self.state.assets[self.state.selected],width/10.,37.))}></div>
                    <div absolute top-0 left={relative((self.state.position/self.state.seek_limit()).clamp(0.,1.))} h-full w={px(2.)} bg={rgb(TEXT)} />
                </div>.into_any_element()
            }else{div().into_any_element()}}
            <div w-full flex-shrink-0 px-1 py-2><Slider args={&self.seek} /></div>
        </div>
    }
    #[gpui]
    pub fn details_panel(&self,width:f32,_height:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let asset=&self.state.assets[self.state.selected];
        let overview=self.state.screen==Screen::Overview;
        let tabs=if overview{["Info","Audio","Analysis","Markers"]}else if asset.kind==Kind::Audio{["Details","Waveform","Audio","Metadata"]}else{["Details","Thumbnails","Audio","Metadata"]};
        <div id="details-scroll" flex flex-col size-full min-w-0 min-h-0 p={px(12.)} gap={px(12.)} bg={rgb(PANEL)} border-l-1 border-color={rgb(BORDER)} overflow-y-scroll overflow-x-scroll>
            <div flex-shrink-0 border-1 border-color={rgb(BORDER)} rounded-lg overflow-hidden>
                <div flex border-b-1 border-color={rgb(BORDER)} children={tabs.into_iter().enumerate().map(|(i,label)|
                    <gpui_kit::component::button::Button args={SharedString::from(format!("details-tab-{i}"))} ghost small label={label} flex-1 px-1 h={px(36.)} text-size={px(11.)}
                    bg={rgb(if self.state.detail_tab==i{0x25203c}else{PANEL})} text-color={rgb(if self.state.detail_tab==i{0xb298ff}else{MUTED})}
                    border-b={px(if self.state.detail_tab==i{2.}else{0.})} border-color={rgb(PURPLE)}
                    on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::DetailTab(i),window,cx))} />)}>
                </div>
                <div flex flex-col gap-3 p={px(12.)}>
                    <div flex items-center justify-between font-semibold><div>{asset.name}</div>{tool("favorite","",Some(IconName::Star),Action::Favorite,asset.favorite,cx)}</div>
                    {if self.state.detail_tab==0 {
                        <div flex flex-col gap-3>
                            <div p-3 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-lg>
                                {metadata(vec![("Type",if asset.kind==Kind::Audio{"Audio (WAV)".into()}else if asset.kind==Kind::Image{"Image (RGB)".into()}else{"Video (MP4)".into()}),("Resolution",if asset.kind==Kind::Audio{"—".into()}else{"3840 × 2160 (4K)".into()}),("Frame Rate",if asset.kind!=Kind::Video{"—".into()}else{"59.94 fps".into()}),("Duration",asset.duration_label()),("File Size",format!("{} MB",asset.size)),("Codec",if asset.kind==Kind::Audio{"PCM".into()}else if asset.kind==Kind::Image{"JPEG / PNG".into()}else{"H.264 / AAC".into()}),("Date Added",asset.added.into()),("Status","✓  Processed".into())])}
                            </div>
                            {if overview{<div p-3 bg={rgb(RAISED)} border-1 border-color={rgb(BORDER)} rounded-lg>{metadata(vec![("Audio","48 kHz".into()),("Channels","Stereo".into()),("Codec","AAC".into())])}</div>.into_any_element()}else{div().into_any_element()}}
                        </div>.into_any_element()
                    }else if self.state.detail_tab==1&&!overview{
                        <div flex flex-wrap gap-2 children={(0..6).map(|_|thumbnail(asset,(width-58.)/2.,70.))}><div text-color={rgb(MUTED)} text-size={px(11.)}>{match asset.kind{Kind::Video=>"Cached thumbnails · 1 frame / sec",Kind::Image=>"Cached still image",Kind::Audio=>"Cached waveform previews"}}</div></div>.into_any_element()
                    }else if (self.state.detail_tab==1||self.state.detail_tab==2&&!overview)&&asset.kind==Kind::Image{
                        <div text-color={rgb(MUTED)}>This still image has no audio track.</div>.into_any_element()
                    }else if self.state.detail_tab==1||self.state.detail_tab==2&&!overview{
                        <div flex flex-col gap-3>{waveform(width-52.,90.,GREEN,9)}{metadata(vec![("Sample Rate","48 kHz".into()),("Channels","Stereo".into()),("Codec","AAC".into())])}{tool("audio-detail-mute","Mute preview",Some(IconName::VolumeX),Action::Mute,self.state.muted,cx)}</div>.into_any_element()
                    }else if self.state.detail_tab==2{
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
    pub fn generated_assets(&self,_width:f32,_cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let kind=self.state.assets[self.state.selected].kind;
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
}
use gpui_kit::component::{Sizable as _,button::ButtonVariants as _};
