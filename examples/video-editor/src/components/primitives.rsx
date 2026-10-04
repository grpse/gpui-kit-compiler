// Shared visual language. Native RSX keeps builders, callbacks and geometry in markup.
use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable as _, button::{Button, ButtonVariants as _}};
use crate::{generated::ui::Editor, state::{Action, Asset, Kind}};

pub const BG: u32 = 0x0b1016;
pub const PANEL: u32 = 0x11171f;
pub const RAISED: u32 = 0x1b222c;
pub const BORDER: u32 = 0x28313c;
pub const TEXT: u32 = 0xe8eaf4;
pub const MUTED: u32 = 0x9ca6bb;
pub const PURPLE: u32 = 0x7c4dff;
pub const GREEN: u32 = 0x57d5a3;

#[gpui]
pub fn glyph(name: IconName, size: f32) -> impl IntoElement + use<> {
    <Icon args={name} size={px(size)} />
}

#[gpui]
pub fn tool(id: impl Into<SharedString>, label: impl Into<SharedString>, icon: Option<IconName>, action: Action, active: bool, cx: &mut Context<Editor>) -> Button {
    let id=id.into();
    let label=label.into();
    let description=if label.is_empty(){SharedString::from(id.replace('-'," "))}else{label.clone()};
    <Button args={id} ghost small label={label.clone()} tooltip={description.clone()} accessibility-label={description}
        when-some:args={(icon, |button, icon| button.icon(icon))}
        rounded={px(6.)} h={px(32.)} text-size={px(12.)}
        bg={rgb(if active {PURPLE} else {RAISED})}
        text-color={rgb(TEXT)} border-1 border-color={rgb(if active {PURPLE} else {BORDER})}
        on-click={cx.listener(move |this, _, window, cx| this.dispatch(action.clone(),window,cx))} />
}

/// Displays a source rectangle by clipping an unchanged reference image.
/// Replace this component with `img` of real thumbnails when connecting media services.
#[gpui]
pub fn photo(asset: &Asset, width: f32, height: f32) -> impl IntoElement + use<> {
    let [x,y,w,h]=asset.crop;
    let source="compositing-editing.png";
    let scale=(width/w).max(height/h);
    let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/mocks").join(source);
    <div relative w={px(width)} h={px(height)} flex-shrink-0 overflow-hidden bg={rgb(RAISED)}>
        <img args={path} absolute left={px(-x*scale-(w*scale-width)/2.)} top={px(-y*scale-(h*scale-height)/2.)}
            w={px(1536.*scale)} h={px(1024.*scale)} object-fit={ObjectFit::Fill} />
    </div>
}

#[gpui]
pub fn waveform(width: f32, height: f32, color: u32, seed: usize) -> impl IntoElement + use<> {
    let count=(width/3.).max(12.) as usize;
    <div relative flex items-center w={px(width)} h={px(height)} overflow-hidden children={ (0..count).map(|i| {
        let t=i as f32/count as f32;
        let noise=((i*37+seed*19)%53) as f32/53.;
        let envelope=(t*19.+seed as f32).sin().abs()*0.45+0.12;
        let amplitude=(envelope*(0.25+noise)*height).max(2.);
        <div flex-shrink-0 w={px(width/count as f32)} h={px(amplitude)} bg={rgb(color)} />
        }) }>
        <div absolute left-0 right-0 top={relative(0.5)} h={px(1.)} bg={rgb(color)} />
    </div>
}

#[gpui]
pub fn thumbnail(asset: &Asset, width: f32, height: f32) -> AnyElement {
    if asset.kind==Kind::Audio {
        let color=if asset.name=="voiceover.wav" {GREEN} else if asset.name=="music.wav" {PURPLE} else {MUTED};
        <div flex items-center w={px(width)} h={px(height)} bg={rgb(if color==GREEN {0x102b29} else {0x1b1830})}>{waveform(width,height*0.8,color,asset.name.len())}</div>.into_any_element()
    } else {photo(asset,width,height).into_any_element()}
}

#[gpui]
pub fn meter(progress: f32, color: u32, height: f32) -> impl IntoElement + use<> {
    <div relative w-full h={px(height)} bg={rgb(0x343c4a)} rounded-full overflow-hidden>
        <div absolute left-0 top-0 h-full w={relative(progress.clamp(0.,1.))} bg={rgb(color)} rounded-full />
    </div>
}

#[gpui]
pub fn section_title(title: &'static str, cx: &mut Context<Editor>, collapsed: bool) -> impl IntoElement + use<> {
    <div flex items-center gap-2 h={px(39.)}>
        {tool(format!("section-{title}"),title,Some(if collapsed {IconName::ChevronRight}else{IconName::ChevronDown}),Action::Section(title),false,cx)}
    </div>
}

#[gpui]
pub fn metadata(rows: Vec<(&'static str,String)>) -> impl IntoElement + use<> {
    <div flex flex-col gap={px(8.)} children={rows.into_iter().map(|(label,value)| <div flex gap-3 text-size={px(12.)}>
        <div w={px(90.)} flex-shrink-0 text-color={rgb(MUTED)}>{label}</div>
        <div flex-1 min-w-0>{value}</div>
        </div>)}>
    </div>
}
