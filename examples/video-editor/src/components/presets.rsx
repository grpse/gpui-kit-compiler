use gpui_kit::{prelude::*, *};
use gpui_kit::component::StyledExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{input::Input,Sizable as _};
use crate::{generated::{ui::Editor,primitives::*},state::Action};

/// Fixture presets provide an interface for each tool without invoking a media pipeline.
fn presets(category:&str)->Vec<(&'static str,IconName,&'static str)> {
    match category {
        "Text"=>vec![("Simple title",IconName::Type,"TITLE"),("Lower third",IconName::Type,"Your name"),("Subtitle",IconName::Type,"A story worth telling"),("Chapter title",IconName::Type,"CHAPTER 01"),("Quote",IconName::Type,"“Make it happen”"),("End credits",IconName::Type,"DIRECTED BY"),("Bold headline",IconName::Type,"FLOW"),("Minimal label",IconName::Type,"THE MOMENT"),("Location",IconName::MapPin,"Los Angeles")],
        "Effects"=>vec![("Soft glow",IconName::Sparkles,"Glow"),("Film grain",IconName::Film,"Grain"),("Gaussian blur",IconName::Focus,"Blur"),("Vignette",IconName::Circle,"Vignette"),("Chromatic",IconName::Palette,"RGB"),("Vintage film",IconName::Film,"Vintage"),("Light leak",IconName::Sun,"Light"),("Duotone",IconName::Palette,"Duotone"),("Sharpen",IconName::Focus,"Detail")],
        "Transitions"=>vec![("Cross dissolve",IconName::Layers,"A → B"),("Fade to black",IconName::Circle,"Fade"),("Wipe left",IconName::ArrowLeft,"←"),("Wipe right",IconName::ArrowRight,"→"),("Slide up",IconName::ArrowUp,"↑"),("Zoom in",IconName::ZoomIn,"Zoom"),("Push",IconName::MoveHorizontal,"Push"),("Iris",IconName::Circle,"Iris"),("Flash",IconName::Sun,"Flash")],
        "Elements"=>vec![("Circle",IconName::Circle,"●"),("Rectangle",IconName::Square,"■"),("Arrow",IconName::ArrowRight,"→"),("Star",IconName::Star,"★"),("Heart",IconName::Heart,"♥"),("Frame",IconName::Scan,"Frame"),("Callout",IconName::MessageCircle,"Hello!"),("Location pin",IconName::MapPin,"Pin"),("Badge",IconName::CircleCheck,"✓")],
        _=>vec![("Classic captions",IconName::Captions,"Your caption here"),("Highlight word",IconName::Captions,"Make every word count"),("Bold captions",IconName::Type,"TELL YOUR STORY"),("Minimal captions",IconName::Captions,"A new perspective"),("Boxed captions",IconName::Square,"Keep moving"),("Karaoke",IconName::Music,"Sing along")],
    }
}
impl Editor {
    #[gpui]
    pub fn preset_browser(&self,width:f32,cx:&mut Context<Self>)->impl IntoElement + use<> {
        let category=self.state.category.clone();
        let items=presets(&category);
        let available=(width-32.).max(70.);
        let columns=((available+12.)/145.).floor().clamp(1.,3.) as usize;
        let card_width=(available-12.*(columns-1) as f32)/columns as f32;
        let filtered:Vec<_>=items.into_iter().filter(|(name,_,_)|name.to_lowercase().contains(&self.state.query.to_lowercase())).collect();
        <div id="preset-browser-scroll" track-scroll={&self.preset_scroll} flex flex-col size-full min-h-0 overflow-y-scroll overflow-x-scroll p-4 gap-4 bg={rgb(PANEL)}>
            <div flex flex-col flex-shrink-0 gap-2>
                <div font-semibold text-size={px(22.)}>{category.clone()}</div>
                <div text-size={px(12.)} text-color={rgb(MUTED)}>{match category.as_str(){"Text"=>"Titles, lower thirds, and typography.","Effects"=>"Browse visual effects for your clips.","Transitions"=>"Choose how two clips flow together.","Elements"=>"Shapes, stickers, and graphic overlays.",_=>"Caption styles and a sample transcript."}}</div>
                <Input args={&self.search} small prefix={glyph(IconName::Search,15.)} />
                <div flex flex-wrap gap-1>
                    {tool("preset-all","All presets",None,Action::ClearPresetFilter,self.state.query.is_empty(),cx)}
                    {tool("preset-favorite","Saved",Some(IconName::Star),Action::Mock("Saved presets"),false,cx)}
                </div>
            </div>
            {if category=="Captions" {
                <div flex flex-col flex-shrink-0 gap-3 p-3 border-1 border-color={rgb(BORDER)} rounded-lg bg={rgb(RAISED)}>
                    <div font-semibold>Auto captions</div>
                    <div text-color={rgb(MUTED)} text-size={px(11.)}>Language: English · sample transcript</div>
                    {tool("generate-captions","Generate captions",Some(IconName::Captions),Action::Mock("Generate captions from audio"),true,cx)}
                    <div flex flex-col gap-2 children={[("00:00 – 00:03","Find your own rhythm."),("00:03 – 00:06","Keep moving forward."),("00:06 – 00:10","Make the moment yours.")].into_iter().enumerate().map(|(i,(time,text))|
                        <div id={("caption-line",i)} flex flex-col gap-1 p-2 bg={rgb(PANEL)} rounded-md cursor-pointer on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Seek(i as f32*3.),window,cx))}>
                            <div text-color={rgb(PURPLE)} text-size={px(10.)}>{time}</div><div text-size={px(12.)}>{text}</div>
                        </div>)} />
                </div>.into_any_element()
            }else{div().into_any_element()}}
            <div flex flex-col flex-shrink-0 gap-3 children={filtered.chunks(columns).map(|row|<div flex gap-3 children={row.iter().enumerate().map(|(i,&(name,icon,sample))| {
                let selected=self.state.selected_preset==Some(name);
                <div id={SharedString::from(format!("preset-{name}"))} role={Role::Button} aria-label={name} flex flex-col flex-shrink-0 w={px(card_width)} gap-2 cursor-pointer on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::Preset(name),window,cx))}>
                    <div relative flex flex-col items-center justify-center gap-3 h={px(100.)} rounded-lg border={px(if selected{2.}else{1.})} border-color={rgb(if selected{PURPLE}else{BORDER})} bg={rgb([0x26203c,0x183a3c,0x302939][i%3])} overflow-hidden>
                        <div text-color={rgb(if category=="Elements"{GREEN}else{0xb9a0ff})}>{glyph(icon,22.)}</div>
                        <div text-center font-semibold text-size={px(if category=="Text"{14.}else{12.})}>{sample}</div>
                        {if selected{<div absolute right-1 top-1 text-color={rgb(PURPLE)}>{glyph(IconName::CircleCheck,16.)}</div>.into_any_element()}else{div().into_any_element()}}
                    </div>
                    <div text-size={px(12.)}>{name}</div>
                    <div text-size={px(10.)} text-color={rgb(MUTED)}>{if category=="Transitions"{"1.0 sec · click to preview"}else{"Click to preview"}}</div>
                </div>
            })} />)} />
            {if filtered.is_empty(){<div text-color={rgb(MUTED)}>No matching presets.</div>.into_any_element()}else{div().into_any_element()}}
        </div>
    }
}
