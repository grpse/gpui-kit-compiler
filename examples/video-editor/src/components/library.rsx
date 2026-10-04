use gpui_kit::component::StyledExt as _;
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{button::{Button,ButtonVariants as _},input::Input,checkbox::Checkbox,Sizable as _};
use crate::{editor::Editor,generated::primitives::*,state::{Action,Screen,Kind}};
#[gpui]
pub fn media_card(editor:&Editor,id:usize,width:f32,compact:bool,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let asset=&editor.state.assets[id];
    let selected=editor.state.selected==id;
    let height=if compact {width*0.65}else{width*0.59};
    <div flex flex-col w={px(width)} gap={px(7.)} flex-shrink-0>
        <div id={("media-card",id)} role={Role::Button} aria-label={asset.name} relative w={px(width)} h={px(height)} rounded={px(7.)} overflow-hidden border={px(if selected {2.}else{1.})}
            border-color={rgb(if selected {PURPLE}else{BORDER})} cursor-pointer
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Select(id),window,cx))}>
            {thumbnail(asset,width,height)}
            {if !compact {<div id={("check-wrap",id)} absolute top={px(7.)} left={px(7.)} on-click={|_,_,cx|cx.stop_propagation()}>
                    <Checkbox args={SharedString::from(format!("select-{id}"))} checked={editor.state.checked.contains(&id)}
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
pub fn media_library(editor:&Editor,width:f32,_height:f32,compact:bool,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let padding=if compact {16.}else{22.};
    let available=(width-padding*2.).max(60.);
    let columns=((available+12.)/if compact {120.}else{176.}).floor().clamp(1.,4.) as usize;
    let gap=if compact {12.}else{16.};
    let card_width=(available-gap*(columns-1) as f32)/columns as f32;
    let mut assets=editor.state.visible_assets();
    if editor.state.screen==Screen::Overview && editor.state.category=="All" {assets.retain(|&id|editor.state.assets[id].kind==Kind::Video && id!=6);}
    if compact && editor.state.category=="All" {assets.retain(|&id|editor.state.assets[id].kind==Kind::Video);}
    if matches!(editor.state.category.as_str(),"Text"|"Effects"|"Transitions"|"Elements"|"Captions") {
        return crate::generated::presets::preset_browser(editor,width,cx).into_any_element();
    }
    let title=match editor.state.category.as_str(){"Audio"=>"Audio Library","Images"=>"Image Library",_=>"Media Library"};
    let subtitle=match editor.state.category.as_str(){"Audio"=>"Music, voiceovers, and sound effects.","Images"=>"Still images and artwork for your project.",_=>"Import, preprocess, and manage all your media assets."};
    <div id="media-panel-scroll" track-scroll={&editor.media_scroll} flex flex-col size-full min-h-0 overflow-y-scroll overflow-x-scroll bg={rgb(PANEL)} border-1 border-color={rgb(BORDER)} rounded={px(if compact{8.}else{0.})} p={px(padding)} gap={px(if compact {8.}else{12.})}>
        <div flex flex-col flex-shrink-0 gap={px(8.)}>
            <div flex flex-wrap items-center justify-between gap-2>
                <div text-size={px(if compact{19.}else{25.})} font-semibold>{title}</div>
                {if compact||editor.state.screen==Screen::Overview {
                    tool("import-library",if compact{"Import ⌄"}else{"Import Media"},Some(IconName::FileUp),Action::Import,true,cx).into_any_element()
                }else{
                    <div flex flex-wrap items-center gap-2>
                        {tool("grid-layout","",Some(IconName::LayoutGrid),Action::Layout(false),!editor.state.list,cx)}
                        {tool("list-layout","",Some(IconName::List),Action::Layout(true),editor.state.list,cx)}
                        {tool("sort-order",if editor.state.sort_ascending{"Name A–Z ⌄"}else{"Date Added ⌄"},None,Action::Sort,false,cx)}
                        {tool("filters","",Some(IconName::ListFilter),Action::FilterPanel,editor.state.filters_open,cx)}
                    </div>.into_any_element()
                }}
            </div>
            {if !compact&&editor.state.screen!=Screen::Overview {<div text-color={rgb(MUTED)} text-size={px(13.)}>{subtitle}</div>.into_any_element()}else{div().into_any_element()}}
            {if compact||editor.state.screen==Screen::Overview {
                <div flex flex-wrap items-center gap={px(5.)} border-b-1 border-color={rgb(BORDER)} children={["All","Videos","Audio","Images","Favorites"].into_iter().map(|category| <Button args={SharedString::from(format!("category-{category}"))} ghost small label={category}
                    px={px(if compact{6.}else{10.})} h={px(31.)} text-size={px(11.)} border-b={px(if editor.state.category==category {2.}else{0.})} border-color={rgb(PURPLE)}
                    on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Category(category.into()),window,cx))} />)}>
                    <div flex-1 />
                    {if !compact {<div flex gap-2>
                            {tool("overview-sort",if editor.state.sort_ascending{"Name A–Z ⌄"}else{"Date Added ⌄"},None,Action::Sort,false,cx)}
                            {tool("overview-grid","",Some(IconName::LayoutGrid),Action::Layout(false),!editor.state.list,cx)}
                            {tool("overview-list","",Some(IconName::List),Action::Layout(true),editor.state.list,cx)}
                        </div>.into_any_element()}else{div().into_any_element()}}
                    {tool("filter-panel","",Some(IconName::ListFilter),Action::FilterPanel,editor.state.filters_open,cx)}
                </div>.into_any_element()
            }else{div().into_any_element()}}
        </div>
        {if (!compact&&editor.state.screen!=Screen::Overview)||editor.state.filters_open {<div flex flex-wrap flex-shrink-0 items-center gap-2>
                <div w={px(if compact {available}else{(available*0.4).max(120.).min(available)})}><Input args={&editor.search} small prefix={glyph(IconName::Search,15.)} /></div>
                {tool("type-filter",["All Types ⌄","Videos ⌄","Audio ⌄","Images ⌄"][editor.state.type_filter],None,Action::TypeFilter,editor.state.type_filter!=0,cx)}
                {tool("resolution-filter",if editor.state.resolution_filter{"4K ⌄"}else{"All Resolutions ⌄"},None,Action::ResolutionFilter,editor.state.resolution_filter,cx)}
                {tool("duration-filter",if editor.state.duration_filter{"Under 20s ⌄"}else{"All Durations ⌄"},None,Action::DurationFilter,editor.state.duration_filter,cx)}
                {if !compact{tool("source-filter",if editor.state.source_filter{"MP4 ⌄"}else{"All Sources ⌄"},None,Action::SourceFilter,editor.state.source_filter,cx).into_any_element()}else{div().into_any_element()}}
                {tool("clear-filters","Clear",None,Action::ClearFilters,false,cx)}
            </div>.into_any_element()}else{div().into_any_element()}}
        <div flex flex-col flex-shrink-0 gap={px(20.)} children={if editor.state.screen==Screen::Overview {Some(crate::generated::processing::import_overview(editor,available,cx).into_any_element())}else{None}}>
            {if assets.is_empty(){<div flex flex-col items-center justify-center gap-3 h={px(170.)} text-color={rgb(MUTED)}>
                    {glyph(IconName::Search,28.)}<div>No matching media</div>{tool("reset-search","Clear filters",None,Action::ClearFilters,false,cx)}
                </div>.into_any_element()}else if editor.state.list && !compact {
                <div flex flex-col flex-shrink-0 gap-2 children={assets.iter().map(|&id| {
                    let asset=&editor.state.assets[id];
                    <div id={("asset-row",id)} role={Role::Button} aria-label={asset.name} flex items-center gap-4 p-2 bg={rgb(if editor.state.selected==id{0x26223f}else{RAISED})} rounded-md cursor-pointer on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Select(id),window,cx))}>
                    {thumbnail(asset,72.,43.)}<div flex-1>{asset.name}</div><div text-color={rgb(MUTED)}>{asset.duration_label()}</div><div w={px(75.)}>{format!("{} MB",asset.size)}</div>
                    </div>
                    })}></div>.into_any_element()
            }else{
                <div flex flex-col flex-shrink-0 gap={px(if compact{12.}else{16.})} children={assets.chunks(columns).map(|row| <div flex gap={px(gap)} children={row.iter().map(|&id|crate::generated::library::media_card(editor,id,card_width,compact,cx))}>
                    </div>)}></div>.into_any_element()
            }}
        </div>

    </div>.into_any_element()
}
