use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use gpui_kit::component::{menu::ContextMenuExt as _,input::Input,button::{Button,ButtonVariants as _},Sizable as _};
use crate::{editor::{Editor,InlineEdit},generated::primitives::*,state::{Action,Kind,ClipComponent,ClipDragKind}};
#[gpui]
pub fn timeline_clip(editor:&Editor,index:usize,pixels_per_second:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let owner=cx.entity();
    let clip=&editor.state.clips[index];
    let asset=&editor.state.assets[clip.asset];
    let width=clip.length*pixels_per_second;
    let channel=match clip.component {
        Some(ClipComponent::AudioChannel{stream,channel})=>asset.prepared.as_ref().and_then(|media|media.audio.iter().find(|audio|audio.index==stream)).filter(|audio|channel<audio.channels.len()).map(|audio|(audio,channel)),
        _=>None,
    };
    let audio=channel.is_some()||asset.kind==Kind::Audio;
    let label=editor.state.clip_label(index);
    let editing=editor.inline_edit==Some(InlineEdit::Clip(index));
    let selected=editor.state.selected_clip==index;
    let unlocked=!editor.state.linked_locked(index);
    let color=if asset.name=="voiceover.wav"{GREEN}else if clip.asset==1{0x66b7e6}else{PURPLE};
    <div id={("timeline-clip",index)} role={Role::Button} aria-label={format!("{} on {}",label,editor.state.tracks[clip.track].name)} absolute left={px(clip.start*pixels_per_second)} top={px(5.)} w={px(width)} h={px(52.)}
        rounded={px(5.)} overflow-hidden border={px(if selected{2.}else{1.})} border-color={rgb(if selected{0xc1a4ff}else{color})}
        bg={rgb(if color==GREEN{0x123d35}else{0x302357})} cursor-grab block-mouse-except-scroll
        on-mouse-down:args={(MouseButton::Left,cx.listener(move |this,event:&MouseDownEvent,window,cx|this.begin_clip_gesture(index,ClipDragKind::Move,event.position.x.into(),window,cx)))}
        on-click={cx.listener(move |this,event:&ClickEvent,window,cx|{
            this.update_clip_gesture(event.position().x.into(),cx);
            this.finish_clip_gesture(window,cx);
            if this.suppress_clip_click {this.suppress_clip_click=false;return;}
            if event.click_count()>=2 {this.begin_inline(InlineEdit::Clip(index),window,cx);}else{this.dispatch(Action::SelectClip(index),window,cx);}
        })}
        context-menu={move |menu,window,cx|crate::interactions::clip_menu(menu,owner.clone(),index,window,cx)}>
        {if let Some((audio,channel))=channel {<div absolute left-0 bottom-0 opacity={0.8}>{channel_waveform(audio,channel,clip.source_start as f64,(clip.source_start+clip.length) as f64,width,35.,color)}</div>.into_any_element()}else if audio {<div absolute left-0 bottom-0 opacity={0.8}>{waveform(width,35.,color,clip.asset)}</div>.into_any_element()}else {
            let frames=((width/75.).ceil() as usize).min(128);
            <div flex opacity={if editor.state.tracks[clip.track].visible{1.}else{0.35}} children={(0..frames).map(|_|photo(asset,75.,52.))}></div>.into_any_element()
        }}
        <div absolute left-0 right-0 top-0 flex items-center gap-2 px={px(7.)} h={px(if editing{30.}else{21.})} bg={rgba(if color==GREEN{0x146149bd}else if clip.asset==1{0x287bb2bd}else{0x6742bdbd})} text-size={px(11.)}>
            {glyph(if audio{IconName::Music}else{IconName::Film},12.)}
            {if editing {<div flex-1 min-w-0>{crate::generated::tracks::name_field(editor,cx)}</div>.into_any_element()}else{<div truncate>{label.clone()}</div>.into_any_element()}}
        </div>
        {if asset.path.is_none()&&asset.name=="voiceover.wav"{automation_curve(width,50.).into_any_element()}else{div().into_any_element()}}
        {if unlocked&&!editing {<div absolute inset-0 children={[(ClipDragKind::Start,"start"),(ClipDragKind::End,"end")].into_iter().map(|(kind,edge)| {
            <div id={(if kind==ClipDragKind::Start{"clip-start-handle"}else{"clip-end-handle"},index)} role={Role::Slider} aria-label={format!("Trim {edge} of {label}")}
                absolute top-0 bottom-0 left={if kind==ClipDragKind::Start{px(0.)}else{px((width-7.).max(0.))}} w={px(width.clamp(1.,7.))}
                cursor-col-resize block-mouse-except-scroll bg={rgba(if selected{0xc1a4ff88}else{0xffffff33})} hover:args={|this|this.bg(rgb(0xc1a4ff))}
                on-mouse-down:args={(MouseButton::Left,cx.listener(move |this,event:&MouseDownEvent,window,cx|{this.begin_clip_gesture(index,kind,event.position.x.into(),window,cx);cx.stop_propagation();}))}
                on-click={|_,_,cx|cx.stop_propagation()} />
        })} />.into_any_element()}else{div().into_any_element()}}
    </div>
}
#[gpui]
pub fn timeline(editor:&Editor,width:f32,cx:&mut Context<Editor>) -> impl IntoElement + use<> {
    let toolbar_menu_owner=cx.entity();
    let ruler_menu_owner=cx.entity();
    let space_menu_owner=cx.entity();
    let labels=editor.track_controls_width.clamp(100.,(width-160.).clamp(100.,420.));
    let resize_owner=cx.weak_entity();
    let owner=cx.weak_entity();
    let measure_owner=owner.clone();
    let drag_owner=owner.clone();
    let content_width=(width-labels-9.).max(1.)*editor.state.zoom;
    let duration=editor.clip_gesture.as_ref().map_or_else(||editor.state.project_duration(),|gesture|gesture.duration);
    let scale=content_width/duration;
    let label_step=(if scale<25. {4}else if scale<60. {2}else{1}).max((duration/80.).ceil() as u32);
    let tick_count=(duration*5./label_step as f32).ceil() as u32;
    <div id="timeline-editor" track-focus={&editor.timeline_focus} relative flex flex-col size-full min-h-0 rounded-xl overflow-hidden border-1 border-color={rgb(BORDER)} bg={rgb(PANEL)}
        on-mouse-down:args={(MouseButton::Left,cx.listener(|this,_,window,cx|{if this.inline_edit.is_none(){this.timeline_focus.focus(window,cx);}}))}
        capture-key-down={cx.listener(|this,event:&KeyDownEvent,window,cx|this.timeline_key_down(event,window,cx))}>
        <canvas args={(|bounds,window,_|(bounds,window.insert_hitbox(bounds,HitboxBehavior::Normal)),move |_,(bounds,hitbox),window,_| {
            let owner=resize_owner.clone();
            window.on_mouse_event(move |event:&ScrollWheelEvent,phase,window,cx| {
                if phase==DispatchPhase::Capture&&hitbox.should_handle_scroll(window) {
                    let _=owner.update(cx,|this,cx| {
                        if this.timeline_vertical_scroll.bounds().contains(&event.position) {
                            this.scroll_timeline(event,window,cx);
                        }
                    });
                }
            });
            let owner=resize_owner.clone();
            window.on_mouse_event(move |event:&MouseMoveEvent,phase,_,cx| {
                if phase==DispatchPhase::Bubble && event.pressed_button==Some(MouseButton::Left) {
                    let _=owner.update(cx,|this,cx| {
                        this.update_clip_gesture(event.position.x.into(),cx);
                        if this.dragging_track_divider {
                            this.track_controls_width=(f32::from(event.position.x)-f32::from(bounds.origin.x)).clamp(100.,(width-160.).clamp(100.,420.));
                            cx.notify();
                        }
                    });
                }
            });
            let owner=resize_owner.clone();
            window.on_mouse_event(move |event:&MouseUpEvent,phase,window,cx| {
                if phase==DispatchPhase::Bubble&&event.button==MouseButton::Left {let _=owner.update(cx,|this,cx|{this.dragging_track_divider=false;this.update_clip_gesture(event.position.x.into(),cx);this.finish_clip_gesture(window,cx);});}
            });
        })} absolute left-0 top-0 size-full />
        <div id="timeline-zoom-toolbar" flex items-center overflow-x-scroll px={px(16.)} h={px(38.)} flex-shrink-0 gap={px(7.)} border-b-1 border-color={rgb(BORDER)}
            context-menu={move |menu,window,cx|crate::interactions::timeline_menu(menu,toolbar_menu_owner.clone(),window,cx)}>
            {if editor.clip_gesture.is_some(){editor.state.clips.get(editor.state.selected_clip).map(|clip|<div text-size={px(11.)} text-color={rgb(TEXT)}>{format!("{:.3}s – {:.3}s",clip.start,clip.start+clip.length)}</div>.into_any_element()).unwrap_or_else(||div().into_any_element())}else{div().into_any_element()}}
            <div flex-1 />{tool("zoom-out","",Some(IconName::Minus),Action::Zoom(-0.2),false,cx)}
            <div id="timeline-zoom-control" w={px(100.)} flex-shrink-0 role={Role::Slider} aria-label="Timeline zoom"><gpui_kit::component::slider::Slider args={&editor.timeline_zoom} /></div>
            <div w={px(42.)} text-size={px(11.)}>{format!("{:.0}%",editor.state.zoom*100.)}</div>
            {tool("zoom-in","",Some(IconName::Plus),Action::Zoom(0.2),false,cx)}
            {tool("timeline-fit","Fit",None,Action::Zoom(1.-editor.state.zoom),false,cx)}
        </div>
        {crate::generated::timeline::timing_editor(editor,cx)}
        <div id="timeline-vertical" track-scroll={&editor.timeline_vertical_scroll} flex flex-col flex-1 min-h-0 overflow-y-scroll>
            <div flex flex-shrink-0 min-h-full h={px(editor.state.tracks.len() as f32*62.+79.)}>
                {crate::generated::tracks::track_controls(editor,labels,cx)}
                {crate::generated::tracks::divider(cx)}
                <div id="timeline-scroll" track-scroll={&editor.timeline_scroll} flex-1 min-w-0 h-full overflow-x-scroll>
                    <div relative flex flex-col w={px(content_width)} min-h-full flex-shrink-0>
                        <canvas args={(move |bounds,_,cx| {
                            let _=measure_owner.update(cx,|this,_|{this.timeline_origin=f32::from(bounds.origin.x);this.timeline_scale=scale;});
                            },move |bounds,_,window,_| {
                            let owner=drag_owner.clone();
                            window.on_mouse_event(move |event:&MouseMoveEvent,phase,window,cx| {
                            if phase==DispatchPhase::Bubble {
                            let _=owner.update(cx,|this,cx| {
                            if this.dragging_playhead && event.pressed_button==Some(MouseButton::Left) {
                            this.dispatch(Action::Seek((f32::from(event.position.x)-f32::from(bounds.origin.x))/scale),window,cx);
                            }
                            });
                            }
                            });
                            let owner=drag_owner.clone();
                            window.on_mouse_event(move |event:&MouseUpEvent,phase,_,cx| {
                            if phase==DispatchPhase::Bubble && event.button==MouseButton::Left {
                            let _=owner.update(cx,|this,_|this.dragging_playhead=false);
                            }
                            });
                            })} absolute left-0 top-0 size-full />
                        <div id="timeline-ruler" role={Role::Slider} aria-label="Timeline scrubber" relative h={px(39.)} cursor-pointer
                            on-mouse-down:args={(MouseButton::Left,cx.listener(|this,event:&MouseDownEvent,window,cx| {
                            this.timeline_focus.focus(window,cx);
                            this.dragging_playhead=true;
                            this.dispatch(Action::Seek((f32::from(event.position.x)-this.timeline_origin)/this.timeline_scale),window,cx);
                            cx.stop_propagation();
                            }))} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} children={(0..=tick_count).map(|tick| <div absolute left={px(tick as f32*scale*label_step as f32/5.)} top-0 w={px(1.)} h={px(if tick%5==0{15.}else{8.})} bg={rgb(BORDER)} />)}
                            context-menu={move |menu,window,cx|crate::interactions::timeline_menu(menu,ruler_menu_owner.clone(),window,cx)}>
                            <div absolute left-0 top-0 size-full children={(0..=duration as u32/label_step).map(|i| <div absolute left={px((i*label_step) as f32*scale+3.)} top={px(16.)} text-size={px(10.)} text-color={rgb(MUTED)}>{format!("{:02}:{:02}",i*label_step/60,i*label_step%60)}</div>)} />
                        </div>
                        <div flex flex-col children={editor.state.tracks.iter().enumerate().map(|(track_index,_)| {
                            let menu_owner=cx.entity();
                            <div relative h={px(62.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} children={(0..=duration as u32/label_step).map(|i|<div absolute top-0 bottom-0 left={px(i as f32*scale*label_step as f32)} w={px(1.)} bg={rgb(0x1b232d)} />)}>
                            <div id={("timeline-track-space",track_index)} absolute left-0 top-0 size-full
                                context-menu={move |menu,window,cx|crate::interactions::timeline_menu(menu,menu_owner.clone(),window,cx)} />
                            <div absolute left-0 top-0 size-full children={editor.state.clips.iter().enumerate().filter(|(_,clip)|clip.track==track_index).map(|(i,_)|crate::generated::timeline::timeline_clip(editor,i,scale,cx))} />
                            </div>
                        })} />
                        <div id="timeline-empty-space" h={px(40.)}
                            context-menu={move |menu,window,cx|crate::interactions::timeline_menu(menu,space_menu_owner.clone(),window,cx)} />
                        <div absolute top={px(29.)} bottom-0 left={px(editor.state.position*scale)} w={px(2.)} bg={rgb(PURPLE)}>
                            <div id="playhead-handle" role={Role::Slider} aria-label="Drag timeline playhead" absolute top={px(-12.)} left={px(-9.)} w={px(20.)} h={px(24.)} cursor-col-resize rounded-b-md bg={rgb(PURPLE)}
                                on-mouse-down:args={(MouseButton::Left,cx.listener(|this,_,window,cx|{this.timeline_focus.focus(window,cx);this.dragging_playhead=true;cx.stop_propagation();}))} />
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
}

/// Native painting is embedded directly in RSX for a connected automation path.
#[gpui]
fn automation_curve(width:f32,height:f32) -> impl IntoElement + use<> {
<canvas args={(|_,_,_|(), move |bounds,_,window,_| {
    let points=[(0.,0.88),(0.12,0.5),(0.23,0.79),(0.47,0.52),(0.61,0.85),(0.83,0.47),(1.,0.88)];
    let mut path=PathBuilder::stroke(px(1.5));
    for (i,(x,y)) in points.iter().enumerate(){let p=point(bounds.origin.x+px(x*width),bounds.origin.y+px(y*height));if i==0{path.move_to(p);}else{path.line_to(p);}}
    if let Ok(path)=path.build(){window.paint_path(path,rgb(GREEN));}
    for (x,y) in points {window.paint_quad(fill(Bounds::new(point(bounds.origin.x+px(x*width-2.),bounds.origin.y+px(y*height-2.)),size(px(4.),px(4.))),rgb(GREEN)));}
    })} absolute left-0 top-0 w={px(width)} h={px(height)} />

}

#[gpui]
pub fn timing_editor(editor:&Editor,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    let Some(InlineEdit::Timing(index))=editor.inline_edit else {return div().into_any_element();};
    let label=editor.state.clip_label(index);
    <div id="inline-timing-editor" flex flex-wrap items-center gap-3 px-3 py-2 flex-shrink-0 bg={rgb(RAISED)} border-b-1 border-color={rgb(PURPLE)}
        capture-key-down={cx.listener(|this,event:&KeyDownEvent,_,cx|{if event.keystroke.key=="escape" {this.cancel_inline(cx);cx.stop_propagation();}})}>
        <div w={px(150.)} truncate text-size={px(11.)}>{label}</div>
        <div flex items-center gap-2 children={editor.timing_inputs.iter().enumerate().map(|(i,input)|<div flex flex-col gap-1>
            <div text-size={px(10.)} text-color={rgb(MUTED)}>{["Start (s)","Source-in (s)","Duration (s)"][i]}</div>
            <div w={px(112.)}><Input args={input} small /></div>
        </div>)} />
        <Button args={"apply-clip-timing"} primary small label="Apply" on-click={cx.listener(|this,_,window,cx|this.commit_inline(window,cx))} />
        <div id="cancel-clip-timing" role={Role::Button} aria-label="Cancel timing edit" cursor-pointer text-size={px(12.)} px-2 py-1 on-click={cx.listener(|this,_,_,cx|this.cancel_inline(cx))}>Cancel</div>
    </div>.into_any_element()
}
