use gpui_kit::{prelude::*, *};
use gpui_kit::component::{Disableable as _, Sizable as _, Theme, ThemeMode, ActiveTheme as _, button::{Button, ButtonVariants as _}};
use crate::studio::{Studio, Action};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::scroll::{Scrollbar, ScrollbarAxis, ScrollbarMode};

/// Design dimensions at 100%; scoped rem sizes scale layout and typography together.
pub fn u(value:f32)->Rems{rems(value/16.)}
pub fn sp(value:f32,window:&Window)->Pixels{window.rem_size()*(value/16.)}
pub struct UiScale {scale:f32,child:AnyElement}
impl IntoElement for UiScale {type Element=Self;fn into_element(self)->Self{self}}
impl UiScale {pub fn new(scale:f32,child:AnyElement)->Self{Self{scale,child}}}
impl Element for UiScale {
    type RequestLayoutState=();type PrepaintState=();
    fn id(&self)->Option<ElementId>{None}
    fn source_location(&self)->Option<&'static std::panic::Location<'static>>{None}
    fn request_layout(&mut self,_:Option<&GlobalElementId>,_:Option<&InspectorElementId>,window:&mut Window,cx:&mut App)->(LayoutId,()){
        (window.with_rem_size(Some(px(16.*self.scale)),|window|self.child.request_layout(window,cx)),())
    }
    fn prepaint(&mut self,_:Option<&GlobalElementId>,_:Option<&InspectorElementId>,_:Bounds<Pixels>,_:&mut (),window:&mut Window,cx:&mut App){
        window.with_rem_size(Some(px(16.*self.scale)),|window|{self.child.prepaint(window,cx);});
    }
    fn paint(&mut self,_:Option<&GlobalElementId>,_:Option<&InspectorElementId>,_:Bounds<Pixels>,_:&mut (),_:&mut (),window:&mut Window,cx:&mut App){
        window.with_rem_size(Some(px(16.*self.scale)),|window|self.child.paint(window,cx));
    }
}
#[gpui]
pub fn scrollbar_layer(id:String,handle:ScrollHandle,axis:ScrollbarAxis)->AnyElement{
    let shadow_handle=handle.clone();
    <div absolute inset-0>
        <canvas absolute size-full args={(|_,_,_|(),move|bounds,_,window,_|paint_scroll_shadows(bounds,&shadow_handle,window))}/>
        <Scrollbar args={&handle} id={id} axis={axis} mode={ScrollbarMode::Always} viewport-from-layout />
    </div>.into_any_element()
}
/// Evaluated during paint, after layout has updated the scroll limits and offset.
pub fn scroll_edges(offset:Point<Pixels>,maximum:Point<Pixels>)->[bool;4]{
    let epsilon=px(0.5);
    [offset.x < -epsilon,offset.y < -epsilon,maximum.x+offset.x > epsilon,maximum.y+offset.y > epsilon]
}
fn paint_scroll_shadows(bounds:Bounds<Pixels>,handle:&ScrollHandle,window:&mut Window){
    let edges=scroll_edges(handle.offset(),handle.max_offset());let depth=sp(16.,window);let blur=sp(12.,window);let shift=sp(7.,window);
    for (edge,visible) in edges.into_iter().enumerate(){
        if !visible{continue;}
        let (clip,offset)=match edge{
            0=>(Bounds{origin:bounds.origin,size:size(depth.min(bounds.size.width),bounds.size.height)},point(shift,px(0.))),
            1=>(Bounds{origin:bounds.origin,size:size(bounds.size.width,depth.min(bounds.size.height))},point(px(0.),shift)),
            2=>(Bounds{origin:point(bounds.right()-depth.min(bounds.size.width),bounds.top()),size:size(depth.min(bounds.size.width),bounds.size.height)},point(-shift,px(0.))),
            _=>(Bounds{origin:point(bounds.left(),bounds.bottom()-depth.min(bounds.size.height)),size:size(bounds.size.width,depth.min(bounds.size.height))},point(px(0.),-shift)),
        };
        window.with_content_mask(Some(ContentMask{bounds:clip}),|window|window.paint_inset_shadows(bounds,Corners::all(px(0.)),&[BoxShadow{color:rgba(0x00000066).into(),offset,blur_radius:blur,spread_radius:px(0.),inset:true}]));
    }
}
#[derive(IntoElement)]
pub struct StudioSlider {state:Entity<gpui_kit::component::slider::SliderState>,axis:Axis,style:StyleRefinement}
impl StudioSlider {
    pub fn new(state:&Entity<gpui_kit::component::slider::SliderState>)->Self{Self{state:state.clone(),axis:Axis::Horizontal,style:StyleRefinement::default()}}
    pub fn horizontal(mut self)->Self{self.axis=Axis::Horizontal;self}
    pub fn vertical(mut self)->Self{self.axis=Axis::Vertical;self}
}
impl Styled for StudioSlider{fn style(&mut self)->&mut StyleRefinement{&mut self.style}}
impl RenderOnce for StudioSlider {
    #[gpui]
    fn render(self,_:&mut Window,cx:&mut App)->impl IntoElement{
        use gpui_kit::base::{Slider as BaseSlider,SliderTrack,SliderIndicator,SliderThumb};
        let axis=self.axis;let vertical=axis==Axis::Vertical;let percent=self.state.read(cx).percentage().end;let color=cx.theme().slider_bar;let thumb=cx.theme().slider_thumb;let border=cx.theme().border;
        <BaseSlider args={&self.state} axis={axis} flex flex-1 items-center justify-center when:args={(vertical,|d|d.h(u(120.)))} when:args={(!vertical,|d|d.w_full())} refine-style={&self.style}>
            <SliderTrack args={&self.state} axis={axis} flex items-center justify-center flex-shrink-0 when:args={(vertical,|d|d.w(u(24.)).h_full())} when:args={(!vertical,|d|d.h(u(24.)).w_full())}>
                <SliderIndicator args={&self.state} relative bg={color.opacity(0.25)} when:args={(vertical,|d|d.h_full().w(u(6.)))} when:args={(!vertical,|d|d.w_full().h(u(6.)))}>
                    <div absolute bg={color} when:args={(vertical,|d|d.w_full().bottom_0().top(relative(1.-percent)))} when:args={(!vertical,|d|d.h_full().left_0().right(relative(1.-percent)))}/>
                    <SliderThumb args={&self.state} axis={axis} absolute size={u(16.)} border={u(1.)} border-color={border} bg={thumb} when:args={(vertical,|d|d.bottom(relative(percent)).left(u(-5.)).mb(u(-8.)))} when:args={(!vertical,|d|d.left(relative(percent)).top(u(-5.)).ml(u(-8.)))}/>
                </SliderIndicator>
            </SliderTrack>
        </BaseSlider>
    }
}
use StudioSlider as Slider;

#[derive(Clone, Copy)]
pub struct Palette {pub bg:u32,pub panel:u32,pub soft:u32,pub border:u32,pub text:u32,pub muted:u32,pub strong:u32}
pub fn palette(dark:bool)->Palette{
    if dark{Palette{bg:0x191919,panel:0x222222,soft:0x2a2a2a,border:0x424242,text:0xeeeeee,muted:0xaaaaaa,strong:0xdddddd}}
    else{Palette{bg:0xfafafa,panel:0xffffff,soft:0xf1f1f1,border:0xd9d9d9,text:0x252525,muted:0x666666,strong:0x333333}}
}
pub fn apply_theme(dark:bool,window:&mut Window,cx:&mut App){
    let p=palette(dark);
    Theme::change(if dark{ThemeMode::Dark}else{ThemeMode::Light},Some(window),cx);
    Theme::update(cx,|theme|{
        theme.radius=px(0.);theme.radius_lg=px(0.);
        theme.colors.background=rgb(p.panel).into();theme.colors.foreground=rgb(p.text).into();
        theme.colors.border=rgb(p.border).into();theme.colors.input=rgb(p.border).into();
        theme.colors.muted=rgb(p.soft).into();theme.colors.muted_foreground=rgb(p.muted).into();
        theme.colors.primary=rgb(p.strong).into();theme.colors.primary_foreground=rgb(p.bg).into();
        theme.colors.ring=rgb(p.strong).into();theme.colors.slider_bar=rgb(p.strong).into();theme.colors.slider_thumb=rgb(p.panel).into();
    });
}
#[gpui]
pub fn button(id:impl Into<SharedString>,label:impl Into<SharedString>,action:Action,active:bool,studio:&Studio,cx:&mut Context<Studio>)->Button {
    let p=palette(studio.project.dark);let label=label.into();
    <Button args={id.into()} ghost small label={label.clone()} accessibility-label={label} rounded={px(0.)} h={u(28.)} text-size={u(11.)}
        bg={rgb(if active{p.strong}else{p.soft})} text-color={rgb(if active{p.bg}else{p.text})} border={u(1.)} border-color={rgb(p.border)}
        disabled={studio.busy} on-click={cx.listener(move|this,_,window,cx|this.dispatch(action.clone(),window,cx))} />
}
#[gpui]
pub fn caption(text:impl Into<SharedString>,dark:bool)->AnyElement{
    <div text-size={u(11.)} line-height={relative(1.3)} text-color={rgb(palette(dark).muted)}>{text.into()}</div>.into_any_element()
}
#[gpui]
pub fn slider_row(label:&'static str,index:usize,value:String,instance:bool,studio:&Studio)->AnyElement{
    let sliders=if instance{&studio.instance_sliders}else{&studio.draft_sliders};
    <div flex flex-col gap={u(4.)} min-w={u(90.)} flex-1>
        <div flex justify-between gap={u(4.)} text-size={u(11.)}>{label}{caption(value,studio.project.dark)}</div>
        <Slider args={&sliders[index]} horizontal />
    </div>.into_any_element()
}
#[gpui]
pub fn knob(label:&'static str,index:usize,value:f32,display:String,instance:bool,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let max=if [37,38,40].contains(&index){10000.}else{1.};let normalized=value/max;
    <div flex flex-col items-center gap={u(5.)} min-w={u(84.)} flex-1>
        {caption(label,studio.project.dark)}
        <div id={format!("knob-{instance}-{index}")} test-support size={u(52.)} rounded-full border={u(1.)} border-color={rgb(p.border)} bg={rgb(p.soft)} cursor-pointer
            on-mouse-down:args={(MouseButton::Left,cx.listener(move|this,event:&MouseDownEvent,_,cx|{this.knob_drag=Some((instance,index,event.position,value));cx.stop_propagation();}))}>
            <canvas args={(|_,_,_|(),move|bounds,_,window,_|{
                let center=bounds.center();let angle=(-225.+normalized*270.).to_radians();
                let mut needle=PathBuilder::stroke(sp(3.,window));
                needle.move_to(point(center.x+sp(angle.cos()*8.,window),center.y+sp(angle.sin()*8.,window)));
                needle.line_to(point(center.x+sp(angle.cos()*19.,window),center.y+sp(angle.sin()*19.,window)));
                if let Ok(path)=needle.build(){window.paint_path(path,rgb(p.strong));}
            })} size-full/>
        </div>
        <div text-size={u(11.)}>{display}</div>
    </div>.into_any_element()
}
#[gpui]
pub fn stepper(id:&str,label:String,minus:Action,plus:Action,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    <div flex items-center gap={u(5.)}>
        {button(format!("{id}-minus"),"−",minus,false,studio,cx)}
        <div min-w={u(44.)} text-size={u(11.)} text-center>{label}</div>
        {button(format!("{id}-plus"),"+",plus,false,studio,cx)}
    </div>.into_any_element()
}
