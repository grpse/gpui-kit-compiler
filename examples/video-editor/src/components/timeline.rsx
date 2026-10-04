use gpui_kit::{prelude::*, *};
use gpui_kit::assets::IconName;
use crate::{generated::{ui::Editor,primitives::*},state::{Action,Kind}};
impl Editor {
    #[gpui]
    pub fn timeline_clip(&self,index:usize,pixels_per_second:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let clip=&self.state.clips[index];
        let asset=&self.state.assets[clip.asset];
        let width=clip.length*pixels_per_second;
        let audio=asset.kind==Kind::Audio;
        let selected=self.state.selected_clip==index;
        let color=if asset.name=="voiceover.wav"{GREEN}else if clip.asset==1{0x66b7e6}else{PURPLE};
        <div id={("timeline-clip",index)} role={Role::Button} aria-label={format!("{} on {}",asset.name,self.state.tracks[clip.track].name)} absolute left={px(clip.start*pixels_per_second)} top={px(5.)} w={px(width)} h={px(52.)}
            rounded={px(5.)} overflow-hidden border={px(if selected{2.}else{1.})} border-color={rgb(if selected{0xc1a4ff}else{color})}
            bg={rgb(if color==GREEN{0x123d35}else{0x302357})} cursor-pointer
            on-click={cx.listener(move |this,_,window,cx|this.dispatch(Action::SelectClip(index),window,cx))}>
            {if audio {<div absolute left-0 bottom-0 opacity={0.8}>{waveform(width,35.,color,clip.asset)}</div>.into_any_element()}else {
                let frames=(width/75.).ceil() as usize;
                <div flex opacity={if self.state.tracks[clip.track].visible{1.}else{0.35}} children={(0..frames).map(|_|photo(asset,75.,52.))}></div>.into_any_element()
            }}
            <div absolute left-0 right-0 top-0 flex items-center gap-2 px={px(7.)} h={px(21.)} bg={rgba(if color==GREEN{0x146149bd}else if clip.asset==1{0x287bb2bd}else{0x6742bdbd})} text-size={px(11.)}>
                {glyph(if audio{IconName::Music}else{IconName::Film},12.)}<div>{asset.name}</div>
            </div>
            {if asset.name=="voiceover.wav"{automation_curve(width,50.).into_any_element()}else{div().into_any_element()}}
        </div>
    }
    #[gpui]
    pub fn timeline(&self,width:f32,cx:&mut Context<Self>) -> impl IntoElement + use<> {
        let labels=if width<650. {142.}else{176.};
        let owner=cx.weak_entity();
        let measure_owner=owner.clone();
        let drag_owner=owner.clone();
        let content_width=(width-labels-45.).max(800.)*self.state.zoom;
        let scale=content_width/18.;
        <div flex flex-col size-full min-h-0 rounded-xl overflow-hidden border-1 border-color={rgb(BORDER)} bg={rgb(PANEL)}>
            <div id="timeline-toolbar-scroll" flex items-center overflow-x-scroll px={px(16.)} h={px(50.)} flex-shrink-0 gap={px(7.)} border-b-1 border-color={rgb(BORDER)}>
                {tool("undo","",Some(IconName::Undo2),Action::Tool("Undo"),false,cx)}{tool("redo","",Some(IconName::Redo2),Action::Tool("Redo"),false,cx)}
                <div w={px(1.)} h={px(23.)} bg={rgb(BORDER)} mx-2 />
                <div flex gap-1 children={[("Split",IconName::Scissors),("Delete",IconName::Trash),("Ripple",IconName::MoveHorizontal),("Speed",IconName::Gauge),("Crop",IconName::Crop),("Audio",IconName::AudioLines),("Fade",IconName::AudioLines),("Marker",IconName::Flag)].into_iter().map(|(label,icon)|
                    tool(format!("timeline-tool-{label}"),if width<1300.{""}else{label},Some(icon),Action::Tool(label),false,cx))} />
                <div flex-1 />{tool("zoom-out","",Some(IconName::Minus),Action::Zoom(-0.2),false,cx)}
                <div w={px(80.)}>{meter((self.state.zoom-0.6)/1.4,PURPLE,4.)}</div>
                {tool("zoom-in","",Some(IconName::Plus),Action::Zoom(0.2),false,cx)}
                {tool("timeline-fit","Fit",None,Action::Zoom(1.-self.state.zoom),false,cx)}
            </div>
            <div id="timeline-vertical" flex flex-col flex-1 min-h-0 overflow-y-scroll>
                <div flex flex-shrink-0 min-h-full h={px(self.state.tracks.len() as f32*62.+79.)}>
                <div flex flex-col w={px(labels)} flex-shrink-0 border-r-1 border-color={rgb(BORDER)}>
                    <div h={px(39.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} />
                    <div flex flex-col children={self.state.tracks.iter().enumerate().map(|(index,track)| <div flex items-center gap-1 h={px(62.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} px={px(7.)} bg={rgb(if index==self.state.clips[self.state.selected_clip].track{0x1b222c}else{PANEL})}>
                        {tool(format!("visible-track-{index}"),"",Some(if track.visible{IconName::Eye}else{IconName::EyeOff}),Action::TrackVisible(index),false,cx)}
                        <div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("{}{}",if track.audio{"A"}else{"V"},if track.audio{index.saturating_sub(1)}else{index+1})}</div>
                        <div flex-1 truncate text-size={px(11.)}>{track.name.clone()}</div>
                        {tool(format!("lock-track-{index}"),"",Some(if track.locked{IconName::Lock}else{IconName::LockOpen}),Action::TrackLocked(index),track.locked,cx)}
                        </div>)}>
                        <div p-2>{tool("add-track","Add Track",Some(IconName::CirclePlus),Action::AddTrack,false,cx)}</div>
                    </div>
                </div>
                <div id="timeline-scroll" flex-1 min-w-0 h-full overflow-x-scroll>
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
                                this.dragging_playhead=true;
                                this.dispatch(Action::Seek((f32::from(event.position.x)-this.timeline_origin)/this.timeline_scale),window,cx);
                                cx.stop_propagation();
                            }))} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} children={(0..=90).map(|tick| <div absolute left={px(tick as f32*scale/5.)} top-0 w={px(1.)} h={px(if tick%10==0{15.}else{8.})} bg={rgb(BORDER)} />)}>
                            <div absolute left-0 top-0 size-full children={(0..=8).map(|i| <div absolute left={px(i as f32*scale*2.+3.)} top={px(16.)} text-size={px(10.)} text-color={rgb(MUTED)}>{format!("00:00:{:02}",i*2)}</div>)} />
                        </div>
                        <div flex flex-col children={self.state.tracks.iter().enumerate().map(|(track_index,_)| <div relative h={px(62.)} flex-shrink-0 border-b-1 border-color={rgb(BORDER)} children={(0..9).map(|i|<div absolute top-0 bottom-0 left={px(i as f32*scale*2.)} w={px(1.)} bg={rgb(0x1b232d)} />)}>
                            <div absolute left-0 top-0 size-full children={self.state.clips.iter().enumerate().filter(|(_,clip)|clip.track==track_index).map(|(i,_)|self.timeline_clip(i,scale,cx))} />
                            </div>)} />
                        <div h={px(40.)} />
                        <div absolute top={px(29.)} bottom-0 left={px(self.state.position*scale)} w={px(2.)} bg={rgb(PURPLE)}>
                            <div id="playhead-handle" role={Role::Slider} aria-label="Drag timeline playhead" absolute top={px(-12.)} left={px(-9.)} w={px(20.)} h={px(24.)} cursor-col-resize rounded-b-md bg={rgb(PURPLE)}
                                on-mouse-down:args={(MouseButton::Left,cx.listener(|this,_,_,cx|{this.dragging_playhead=true;cx.stop_propagation();}))} />
                        </div>
                    </div>
                </div>
                </div>
            </div>
        </div>
    }
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
