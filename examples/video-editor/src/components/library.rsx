use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},input::Input,checkbox::Checkbox,Sizable as _};
use crate::{generated::{ui::Editor,primitives::*},state::{Action,Screen,Kind}};
impl Editor {
    #[gpui]
    pub fn media_card(&self,id:usize,width:f32,compact:bool,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let asset=&self.state.assets[id];
        let selected=self.state.selected==id;
        let height=if compact {width*0.65}else{width*0.59};
        <div flex flex-col w={px(width)} gap={px(7.)} flex-shrink-0>
            <div id={("media-card",id)} role={Role::Button} aria-label={asset.name} relative w={px(width)} h={px(height)} rounded={px(7.)} overflow-hidden border={px(if selected {2.}else{1.})}
                border-color={rgb(if selected {PURPLE}else{BORDER})} cursor-pointer
                on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Select(id),window,cx))}>
                {thumbnail(asset,width,height)}
                {if !compact {<div id={("check-wrap",id)} absolute top={px(7.)} left={px(7.)} on-click={|_,_,cx|cx.stop_propagation()}>
                        <Checkbox args={SharedString::from(format!("select-{id}"))} checked={self.state.checked.contains(&id)}
                            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::MultiSelect(id),window,cx))} />
                    </div>.into_any_element()}else{div().into_any_element()}}
                <div absolute bottom={px(4.)} left={px(4.)} flex items-center gap-1 px={px(5.)} py={px(2.)} bg={rgba(0x080c12de)} rounded={px(4.)} text-size={px(10.)}>
                    {glyph(if asset.kind==Kind::Audio{IconName::Music}else if asset.kind==Kind::Image{IconName::Image}else{IconName::Film},12.)}<div>{asset.duration_label()}</div>
                </div>
                {if asset.kind!=Kind::Audio {<div absolute right={px(4.)} bottom={px(4.)} px={px(4.)} py={px(2.)} bg={rgba(0x080c12de)} rounded={px(3.)} text-size={px(10.)}>{if asset.kind==Kind::Image{"IMG"}else{"4K"}}</div>.into_any_element()}else{div().into_any_element()}}
            </div>
            <div flex items-center justify-between text-size={px(if compact{11.}else{13.})}>
                <div truncate>{asset.name}</div>
                {if !compact {tool(format!("asset-menu-{id}"),"",Some(IconName::EllipsisVertical),Action::Mock("Media context menu"),false,cx).h(px(20.)).w(px(20.)).p(px(0.)).into_any_element()}else{div().into_any_element()}}
            </div>
            {if !compact {<div text-size={px(11.)} text-color={rgb(MUTED)}>{format!("{}  ·  {} MB",asset.added,asset.size)}</div>.into_any_element()}else{div().into_any_element()}}
        </div>
    }
    #[gpui]
    pub fn processing_queue(&self,compact:bool,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let jobs:Vec<_>=self.state.jobs.iter().enumerate().filter(|(_,job)|!job.cancelled).collect();
        let overall=if jobs.is_empty(){0.}else{jobs.iter().map(|(_,j)|j.progress as f32).sum::<f32>()/jobs.len() as f32};
        <div flex flex-col flex-shrink-0 gap={px(10.)} p={px(if compact{12.}else{16.})} rounded-lg border-1 border-color={rgb(BORDER)} bg={rgb(PANEL)}>
            <div flex flex-wrap items-center gap-3>
                <div text-color={rgb(PURPLE)}>{glyph(IconName::Settings,28.)}</div>
                <div flex-1 min-w-0><div font-semibold>{if self.state.screen==Screen::Overview {"Preprocessing Media"}else{"Media Processing"}}</div><div whitespace-normal text-size={px(11.)} text-color={rgb(MUTED)}>Generating previews, thumbnails, and audio waveforms…</div></div>
                <div text-size={px(11.)}>{format!("{overall:.0}%")}</div>
                {tool("cancel-processing","Cancel",None,Action::CancelAll,false,cx)}
            </div>
            {meter(overall/100.,PURPLE,8.)}
            {if jobs.is_empty(){<div text-color={rgb(MUTED)} text-size={px(12.)}>No files in the processing queue. Import Media to try the mock flow.</div>.into_any_element()}else{div().into_any_element()}}
            <div flex flex-col gap-2 children={jobs.into_iter().map(|(index,job)| {
                let asset=&self.state.assets[job.asset];
                <div flex flex-wrap items-center gap-2 text-size={px(11.)} flex-shrink-0 py-1>
                <div text-color={rgb(if job.progress==100 {GREEN}else{PURPLE})}>{glyph(if job.progress==100{IconName::CircleCheck}else{IconName::Clock},14.)}</div>
                {thumbnail(asset,40.,24.)}
                <div w={px(112.)} truncate>{asset.name}</div>
                <div w-full min-w-0 text-color={rgb(MUTED)} truncate>{if job.progress==100 {"Processed"}else if job.progress==0 {"Queued…"}else if job.asset==1 {"Analyzing audio…"}else {"Generating preview (480p)…"}}</div>
                <div flex-1 min-w-0>{meter(job.progress as f32/100.,PURPLE,5.)}</div>
                <div w={px(32.)} text-color={rgb(if job.progress==100{GREEN}else{MUTED})}>{if job.progress==100 {"Done".into()}else{format!("{}%",job.progress)}}</div>
                {tool(format!("cancel-job-{index}"),"",Some(IconName::X),Action::CancelJob(index),false,cx)}
                </div>
                })} />
        </div>
    }
    #[gpui]
    pub fn import_overview(&self,width:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let jobs:Vec<_>=self.state.jobs.iter().filter(|j|!j.cancelled).take(3).collect();
        <div flex flex-col flex-shrink-0 gap={px(16.)} mb={px(20.)}>
            <div id="import-drop-zone" role={Role::Button} aria-label="Import files" flex flex-col items-center justify-center gap-3 w-full h={px(150.)} bg={rgb(PANEL)} border-1 border-color={rgb(0x4f5666)} rounded-lg cursor-pointer
                on-click={cx.listener(|this,_,window,cx|this.dispatch(Action::Import,window,cx))}>
                <div text-color={rgb(0xb3abda)}>{glyph(IconName::CloudUpload,36.)}</div>
                <div font-semibold text-color={rgb(0xa086ff)}>Import Files</div>
                <div text-size={px(12.)} text-color={rgb(MUTED)}>Videos, audio or images</div>
                <div text-size={px(12.)} text-color={rgb(MUTED)}>or click to browse</div>
            </div>
            <div min-w-0 flex items-center gap-4 p-4 bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded-lg>
                {photo(&self.state.assets[0],(width*0.2).min(175.),145.)}
                <div flex flex-col gap-4 flex-1>
                    <div flex items-center justify-between><div font-semibold>{format!("Processing {} files…",jobs.len())}</div>{tool("overview-cancel","Cancel",None,Action::CancelAll,false,cx)}</div>
                    <div flex flex-col gap-2 children={jobs.into_iter().map(|job| <div flex items-center gap-3 text-size={px(11.)}>
                        <div w={px(100.)} truncate>{self.state.assets[job.asset].name}</div>
                        <div flex-1>{meter(job.progress as f32/100.,PURPLE,6.)}</div>
                        <div w={px(30.)}>{format!("{}%",job.progress)}</div>
                        </div>)} />
                </div>
            </div>
        </div>
    }
    #[gpui]
    pub fn media_library(&self,width:f32,_height:f32,compact:bool,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let padding=if compact {16.}else{22.};
        let available=(width-padding*2.).max(60.);
        let columns=((available+12.)/if compact {120.}else{176.}).floor().clamp(1.,4.) as usize;
        let gap=if compact {12.}else{16.};
        let card_width=(available-gap*(columns-1) as f32)/columns as f32;
        let mut assets=self.state.visible_assets();
        if self.state.screen==Screen::Overview && self.state.category=="All" {assets.retain(|&id|self.state.assets[id].kind==Kind::Video && id!=6);}
        if compact && self.state.category=="All" {assets.retain(|&id|self.state.assets[id].kind==Kind::Video);}
        if matches!(self.state.category.as_str(),"Text"|"Effects"|"Transitions"|"Elements"|"Captions") {
            return self.preset_browser(width,cx).into_any_element();
        }
        let title=match self.state.category.as_str(){"Audio"=>"Audio Library","Images"=>"Image Library",_=>"Media Library"};
        let subtitle=match self.state.category.as_str(){"Audio"=>"Music, voiceovers, and sound effects.","Images"=>"Still images and artwork for your project.",_=>"Import, preprocess, and manage all your media assets."};
        <div id="media-panel-scroll" track-scroll={&self.media_scroll} flex flex-col size-full min-h-0 overflow-y-scroll overflow-x-scroll bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded={px(if compact{8.}else{0.})} p={px(padding)} gap={px(if compact {8.}else{12.})}>
            <div flex flex-col flex-shrink-0 gap={px(8.)}>
                <div flex flex-wrap items-center justify-between gap-2>
                    <div text-size={px(if compact{19.}else{25.})} font-semibold>{title}</div>
                    {if compact||self.state.screen==Screen::Overview {
                        tool("import-library",if compact{"Import ⌄"}else{"Import Media"},Some(IconName::FileUp),Action::Import,true,cx).into_any_element()
                    }else{
                        <div flex flex-wrap items-center gap-2>
                            {tool("grid-layout","",Some(IconName::LayoutGrid),Action::Layout(false),!self.state.list,cx)}
                            {tool("list-layout","",Some(IconName::List),Action::Layout(true),self.state.list,cx)}
                            {tool("sort-order",if self.state.sort_ascending{"Name A–Z ⌄"}else{"Date Added ⌄"},None,Action::Sort,false,cx)}
                            {tool("filters","",Some(IconName::ListFilter),Action::FilterPanel,self.state.filters_open,cx)}
                        </div>.into_any_element()
                    }}
                </div>
                {if !compact&&self.state.screen!=Screen::Overview {<div text-color={rgb(MUTED)} text-size={px(13.)}>{subtitle}</div>.into_any_element()}else{div().into_any_element()}}
                {if compact||self.state.screen==Screen::Overview {
                    <div flex flex-wrap items-center gap={px(5.)} border-b-1 border-color={rgb(BORDER)} children={["All","Videos","Audio","Images","Favorites"].into_iter().map(|category| <Button args={SharedString::from(format!("category-{category}"))} ghost small label={category}
                        px={px(if compact{6.}else{10.})} h={px(31.)} text-size={px(11.)} border-b={px(if self.state.category==category {2.}else{0.})} border-color={rgb(PURPLE)}
                        on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Category(category.into()),window,cx))} />)}>
                        <div flex-1 />
                        {if !compact {<div flex gap-2>
                                {tool("overview-sort",if self.state.sort_ascending{"Name A–Z ⌄"}else{"Date Added ⌄"},None,Action::Sort,false,cx)}
                                {tool("overview-grid","",Some(IconName::LayoutGrid),Action::Layout(false),!self.state.list,cx)}
                                {tool("overview-list","",Some(IconName::List),Action::Layout(true),self.state.list,cx)}
                            </div>.into_any_element()}else{div().into_any_element()}}
                        {tool("filter-panel","",Some(IconName::ListFilter),Action::FilterPanel,self.state.filters_open,cx)}
                    </div>.into_any_element()
                }else{div().into_any_element()}}
            </div>
            {if (!compact&&self.state.screen!=Screen::Overview)||self.state.filters_open {<div flex flex-wrap flex-shrink-0 items-center gap-2>
                    <div w={px(if compact {available}else{(available*0.4).max(120.).min(available)})}><Input args={&self.search} small prefix={glyph(IconName::Search,15.)} /></div>
                    {tool("type-filter",["All Types ⌄","Videos ⌄","Audio ⌄","Images ⌄"][self.state.type_filter],None,Action::TypeFilter,self.state.type_filter!=0,cx)}
                    {tool("resolution-filter",if self.state.resolution_filter{"4K ⌄"}else{"All Resolutions ⌄"},None,Action::ResolutionFilter,self.state.resolution_filter,cx)}
                    {tool("duration-filter",if self.state.duration_filter{"Under 20s ⌄"}else{"All Durations ⌄"},None,Action::DurationFilter,self.state.duration_filter,cx)}
                    {if !compact{tool("source-filter",if self.state.source_filter{"MP4 ⌄"}else{"All Sources ⌄"},None,Action::SourceFilter,self.state.source_filter,cx).into_any_element()}else{div().into_any_element()}}
                    {tool("clear-filters","Clear",None,Action::ClearFilters,false,cx)}
                </div>.into_any_element()}else{div().into_any_element()}}
            <div flex flex-col flex-shrink-0 gap={px(20.)} children={if self.state.screen==Screen::Overview {Some(self.import_overview(available,cx).into_any_element())}else{None}}>
                {if assets.is_empty(){<div flex flex-col items-center justify-center gap-3 h={px(170.)} text-color={rgb(MUTED)}>
                        {glyph(IconName::Search,28.)}<div>No matching media</div>{tool("reset-search","Clear filters",None,Action::ClearFilters,false,cx)}
                    </div>.into_any_element()}else if self.state.list && !compact {
                    <div flex flex-col flex-shrink-0 gap-2 children={assets.iter().map(|&id| {
                        let asset=&self.state.assets[id];
                        <div id={("asset-row",id)} role={Role::Button} aria-label={asset.name} flex items-center gap-4 p-2 bg={rgb(if self.state.selected==id{0x26223f}else{RAISED})} rounded-md cursor-pointer on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Select(id),window,cx))}>
                        {thumbnail(asset,72.,43.)}<div flex-1>{asset.name}</div><div text-color={rgb(MUTED)}>{asset.duration_label()}</div><div w={px(75.)}>{format!("{} MB",asset.size)}</div>
                        </div>
                        })}></div>.into_any_element()
                }else{
                    <div flex flex-col flex-shrink-0 gap={px(if compact{12.}else{16.})} children={assets.chunks(columns).map(|row| <div flex gap={px(gap)} children={row.iter().map(|&id|self.media_card(id,card_width,compact,cx))}>
                        </div>)}></div>.into_any_element()
                }}
            </div>

        </div>.into_any_element()
    }
}
