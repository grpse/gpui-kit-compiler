use gpui_kit::{prelude::*, *};
use gpui_base::Disableable as _;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::{button::{Button,ButtonVariants as _},input::Input,Sizable as _,menu::{ContextMenuExt as _,DropdownMenu as _}};
use crate::{editor::Editor,generated::primitives::*,state::{Action,ClipComponent}};
use rsx_video_editor::composition::{Category,Composition,Node,Operation};

#[gpui]
pub fn graph_node(editor:&Editor,node:&Node,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    let id=node.id;let category=node.operation.category();let color=category.color();let z=editor.graph_zoom;let selected=editor.composition.selected==Some(id);let owner=cx.entity();let label=Composition::source_label(node,&editor.state.assets);
    let frame=node.is_frame();let frame_size=editor.composition.frame_size(node);let reroute=node.operation.definition().is_some_and(|d|d.id=="NodeReroute");let inputs=node.operation.visible_inputs();let outputs=node.operation.visible_outputs();
    <div id={SharedString::from(format!("graph-node-{id}"))} absolute left={px(node.position[0]*z)} top={px(node.position[1]*z)} w={px(if frame{frame_size[0]}else{node.width()}*z)} h={px(if frame{frame_size[1]}else{node.height()}*z)} bg={if frame{rgba(0)}else{rgb(if node.muted{0x292b30}else{0x202630})}} rounded-md border-1 border-color={rgb(if selected{color}else{0x3a424e})} shadow-md occlude>
        <div id={SharedString::from(format!("graph-node-header-{id}"))} flex items-center justify-between h={px(if reroute{48.}else{32.}*z)} px-2 bg={rgb(0x303846)} rounded-t-md border-b-2 border-color={rgb(color)} cursor-grab role={Role::Button} aria-label={format!("Move {} node {id}",node.operation.label())}
            on-mouse-down:args={(MouseButton::Left,cx.listener(move |this,event:&MouseDownEvent,window,cx|{this.begin_node_drag(id,[event.position.x.into(),event.position.y.into()],window,cx);cx.stop_propagation();}))}
            context-menu={move |menu,window,cx|crate::interactions::node_menu(menu,owner.clone(),id,window,cx)}>
            <div text-size={px((12.*z).max(8.))} font-semibold truncate>{if matches!(node.operation,Operation::Source{..}){node.operation.signal().label().to_string()}else if frame{node.operation.parameters().iter().find(|(name,value)|name=="Label"&&!value.is_empty()).map(|(_,v)|v.as_str()).unwrap_or("Frame").to_string()}else{node.operation.label().to_string()}}</div>
            <div text-size={px(9.)} text-color={rgb(color)}>{if node.muted{"M".into()}else{format!("{:02}",id)}}</div>
        </div>
        {if !reroute&&!frame{<div absolute left={px(10.*z)} right={px(10.*z)} top={px(42.*z)} flex flex-col gap-1>
            <div text-size={px((10.*z).max(8.))} text-color={rgb(color)}>{category.label()}</div>
            <div text-size={px((10.*z).max(8.))} text-color={rgb(MUTED)} truncate>{if matches!(node.operation,Operation::Source{..}){label}else{node.operation.parameters().iter().find(|(name,value)|name=="Label"&&!value.is_empty()).map(|(_,v)|v.clone()).unwrap_or_else(||node.operation.summary())}}</div>
        </div>.into_any_element()}else{div().into_any_element()}}
        <div absolute inset-0 children={inputs.into_iter().map(|port|{
            let port_owner=cx.entity();let name=node.operation.input_label(port);let socket=rsx_video_editor::blender_catalog::socket_color(node.operation.socket_kind(port,false));let linked=node.inputs[port].is_some();
            <div id={SharedString::from(format!("graph-input-{id}-{port}"))} absolute left={px(-6.*z)} top={px((node.input_y(port)-9.)*z)} h={px(18.*z)} flex items-center gap-1 cursor-pointer role={Role::Button} aria-label={format!("Connect {name} input of node {id}")}
                on-click={cx.listener(move |this,_,_,cx|this.input_port(id,port,cx))}
                context-menu={move |menu,_,_|crate::interactions::graph_input_menu(menu,port_owner.clone(),id,port)}>
                <div size={px(10.*z)} rounded-full border-1 border-color={rgb(socket)} bg={rgb(if linked{socket}else{0x202630})} />
                <div text-size={px((10.*z).max(8.))} text-color={rgb(MUTED)}>{if reroute{"".into()}else{name}}</div>
            </div>
        })} />
        <div absolute inset-0 children={outputs.into_iter().map(|port|{
            let pending=editor.composition.pending==Some(id)&&editor.composition.pending_port==port;let socket=rsx_video_editor::blender_catalog::socket_color(node.operation.socket_kind(port,true));let name=node.operation.output_label(port);
            <div id={SharedString::from(format!("graph-output-{id}-{port}"))} absolute right={px(-6.*z)} top={px((node.output_y(port)-9.)*z)} h={px(18.*z)} flex items-center gap-1 cursor-pointer role={Role::Button} aria-label={format!("Connect {name} output of node {id}")}
                on-click={cx.listener(move |this,_,_,cx|this.output_port(id,port,cx))}>
                <div text-size={px((10.*z).max(8.))} text-color={rgb(if pending{TEXT}else{MUTED})}>{if reroute{"".into()}else{name}}</div>
                <div size={px(10.*z)} rounded-full bg={rgb(socket)} border={px(if pending{2.}else{1.})} border-color={rgb(if pending{TEXT}else{0x202630})} />
            </div>
        })} />
    </div>
}

#[gpui]
pub fn node_parameter(editor:&Editor,node:&Node,index:usize,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    let (label,value)=node.operation.parameters()[index].clone();let meta=node.operation.parameter_meta(index);let kind=meta.map(|p|p.kind.clone()).unwrap_or_else(||"FLOAT".into());let paint_kind=kind.clone();let current=editor.graph_inputs[index].read(cx).value().to_string();let owner=cx.entity();
    let fields=meta.map(|p|rsx_video_editor::blender_catalog::components(p,&value)).unwrap_or_default();
    let linked=meta.and_then(|p|p.key.strip_prefix("input:")).and_then(|s|s.parse::<usize>().ok()).is_some_and(|i|node.inputs.get(i).is_some_and(Option::is_some));
    let preview=editor.graph_values(cx).ok().and_then(|values|values.get(index).and_then(|text|serde_json::from_str::<serde_json::Value>(text).ok()));
    let key=meta.map(|p|p.key.clone()).unwrap_or_default();let paint_key=key.clone();let curves=preview.as_ref().and_then(|v|v.as_array()).map_or(0,Vec::len);
    <div flex flex-col gap-1>
        <div flex items-center justify-between text-size={px(10.)} text-color={rgb(MUTED)}><div>{label}</div><div>{if linked{"Linked"}else{""}}</div></div>
        {if kind=="JSON"||kind=="COLOR" {
            <div relative w-full h={px(if kind=="COLOR"{22.}else{76.})} bg={rgb(BG)} rounded-md>
                <canvas args={(|bounds,_,_|bounds,move |bounds,_,window,_|{
                    let Some(value)=&preview else{return;};
                    let color=|channels:&serde_json::Value|{let channel=|i|channels[i].as_f64().unwrap_or(0.).clamp(0.,1.)*255.;rgb(((channel(0)as u32)<<16)|((channel(1)as u32)<<8)|channel(2)as u32)};
                    if paint_kind=="COLOR" {window.paint_quad(fill(bounds,color(value)));}
                    else if paint_key=="color_ramp" {
                        let mut stops:Vec<_>=value.as_array().into_iter().flatten().collect();stops.sort_by(|a,b|a["position"].as_f64().unwrap_or(0.).total_cmp(&b["position"].as_f64().unwrap_or(0.)));
                        for x in 0..128 {let position=x as f64/127.;let left=stops.iter().rev().find(|s|s["position"].as_f64().unwrap_or(0.)<=position).copied().or_else(||stops.first().copied());let right=stops.iter().find(|s|s["position"].as_f64().unwrap_or(1.)>=position).copied().or_else(||stops.last().copied());if let (Some(left),Some(right))=(left,right){let a=left["position"].as_f64().unwrap_or(0.);let b=right["position"].as_f64().unwrap_or(1.);let t=if b>a{(position-a)/(b-a)}else{0.};let channels=serde_json::json!((0..4).map(|i|left["color"][i].as_f64().unwrap_or(0.)*(1.-t)+right["color"][i].as_f64().unwrap_or(0.)*t).collect::<Vec<_>>());window.paint_quad(fill(Bounds::new(point(bounds.origin.x+bounds.size.width*x as f32/128.,bounds.origin.y),size(bounds.size.width/128.+px(1.),bounds.size.height)),color(&channels)));}}
                    } else if paint_key=="curves" {
                        for line in 1..4 {window.paint_quad(fill(Bounds::new(point(bounds.origin.x,bounds.origin.y+bounds.size.height*line as f32/4.),size(bounds.size.width,px(1.))),rgb(BORDER)));window.paint_quad(fill(Bounds::new(point(bounds.origin.x+bounds.size.width*line as f32/4.,bounds.origin.y),size(px(1.),bounds.size.height)),rgb(BORDER)));}
                        for (i,curve) in value.as_array().into_iter().flatten().enumerate(){let mut points:Vec<_>=curve.as_array().into_iter().flatten().filter_map(|point|Some([point[0].as_f64()? as f32,point[1].as_f64()? as f32])).collect();points.sort_by(|a,b|a[0].total_cmp(&b[0]));let mut path=PathBuilder::stroke(px(2.));for (j,p) in points.iter().enumerate(){let point=point(bounds.origin.x+bounds.size.width*p[0].clamp(0.,1.),bounds.origin.y+bounds.size.height*(1.-p[1].clamp(0.,1.)));if j==0{path.move_to(point);}else{path.line_to(point);}}if let Ok(path)=path.build(){window.paint_path(path,rgb([0xd77979,0x79be95,0x7a9ee0,0xd8dce3][i%4]));}}
                    }
                })} absolute inset-0 />
            </div>.into_any_element()
        } else {div().into_any_element()}}
        {if kind=="ENUM" {
            let choices=meta.unwrap().options.clone();let label=choices.iter().find(|c|c.value==current).map(|c|c.label.clone()).unwrap_or(current.clone());
            <Button args={SharedString::from(format!("parameter-{index}"))} ghost small justify-start w-full label={format!("{label} ▾")} dropdown-menu={move |menu,_,_|crate::interactions::graph_enum_menu(menu,owner.clone(),index,choices.clone())} />.into_any_element()
        } else if kind=="BOOLEAN" {
            <Button args={SharedString::from(format!("parameter-{index}"))} ghost small justify-start label={if current=="true"{"☑ Enabled"}else{"☐ Disabled"}} on-click={cx.listener(move |this,_,window,cx|{let enabled=this.graph_inputs[index].read(cx).value()=="true";this.set_graph_parameter(index,(!enabled).to_string(),window,cx);})} />.into_any_element()
        } else if !fields.is_empty() {
            <div flex flex-col gap-1 children={fields.into_iter().enumerate().filter_map(|(i,(label,_))|editor.graph_parts.get(index).and_then(|parts|parts.get(i)).map(|input|<div flex items-center gap-2><div min-w={px(if kind=="VECTOR"||kind=="COLOR"{16.}else{115.})} text-size={px(10.)} text-color={rgb(MUTED)}>{label}</div><div flex-1 min-w-0><Input args={input} small /></div></div>))} />.into_any_element()
        } else { <Input args={&editor.graph_inputs[index]} small />.into_any_element()}}
        {if key=="color_ramp" {<div flex gap-2><Button args={SharedString::from(format!("ramp-add-{index}"))} ghost xsmall label="Add stop" on-click={cx.listener(move |this,_,window,cx|this.edit_graph_points(index,None,true,window,cx))} /><Button args={SharedString::from(format!("ramp-remove-{index}"))} ghost xsmall label="Remove last" on-click={cx.listener(move |this,_,window,cx|this.edit_graph_points(index,None,false,window,cx))} /></div>.into_any_element()}else if key=="curves" {<div flex flex-col gap-1 children={(0..curves).map(|curve|<div flex items-center justify-start gap-2><div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("Curve {}",curve+1)}</div><Button args={SharedString::from(format!("curve-add-{index}-{curve}"))} ghost xsmall label="+ Point" on-click={cx.listener(move |this,_,window,cx|this.edit_graph_points(index,Some(curve),true,window,cx))} /><Button args={SharedString::from(format!("curve-remove-{index}-{curve}"))} ghost xsmall label="− Point" on-click={cx.listener(move |this,_,window,cx|this.edit_graph_points(index,Some(curve),false,window,cx))} /></div>)} />.into_any_element()}else{div().into_any_element()}}
        {if kind=="FILE"{<div flex flex-col gap-1><Button args={SharedString::from(format!("browse-asset-{index}"))} ghost xsmall label="Browse…" on-click={cx.listener(move |this,_,window,cx|this.browse_graph_asset(index,window,cx))} /><div text-size={px(9.)} text-color={rgb(MUTED)}>Choose an image, movie, or Blender asset file</div></div>.into_any_element()}else{div().into_any_element()}}
    </div>
}

#[gpui]
pub fn compositing_view(editor:&Editor,cx:&mut Context<Editor>)->impl IntoElement + use<> {
    let owner=cx.weak_entity();let bounds_owner=cx.weak_entity();let menu_owner=cx.entity();let z=editor.graph_zoom;let search=editor.graph_search.read(cx).value().to_lowercase();
    let nodes=editor.composition.nodes.clone();let connections:Vec<_>=nodes.iter().flat_map(|target|target.inputs.iter().enumerate().filter_map(|(port,id)|id.and_then(|id|nodes.iter().find(|n|n.id==id)).map(|source|([source.position[0]+source.width(),source.position[1]+source.output_y(target.input_ports[port])],[target.position[0],target.position[1]+target.input_y(port)],rsx_video_editor::blender_catalog::socket_color(source.operation.socket_kind(target.input_ports[port],true)))))).collect();
    let frames:Vec<_>=nodes.iter().filter(|node|node.is_frame()).map(|node|(node.position,editor.composition.frame_size(node))).collect();
    let width=nodes.iter().map(|n|n.position[0]+if n.is_frame(){editor.composition.frame_size(n)[0]}else{n.width()}+80.).fold(600.,f32::max)*z;let height=nodes.iter().map(|n|n.position[1]+if n.is_frame(){editor.composition.frame_size(n)[1]}else{n.height()}+100.).fold(600.,f32::max)*z;
    let sources:Vec<_>=editor.state.assets.iter().enumerate().flat_map(|(asset,a)|a.prepared.iter().flat_map(move |m|m.videos.iter().map(move |v|(asset,ClipComponent::Video(v.index))).chain(m.audio.iter().flat_map(move |s|s.channels.iter().map(move |c|(asset,ClipComponent::AudioChannel{stream:s.index,channel:c.index})))))).collect();
    let selected=editor.composition.selected.and_then(|id|editor.composition.node(id));let fresh=editor.graph_render_revision==Some(editor.composition.revision);
    <div id="compositing-workspace" flex flex-col size-full min-h-0 bg={rgb(BG)} track-focus={&editor.graph_focus}
        capture-key-down={cx.listener(|this,event:&KeyDownEvent,window,cx|{if (event.keystroke.modifiers.platform||event.keystroke.modifiers.control)&&event.keystroke.key=="enter"{this.apply_node_settings(window,cx);cx.stop_propagation();}else if this.graph_focus.is_focused(window)&&(event.keystroke.modifiers.platform||event.keystroke.modifiers.control)&&event.keystroke.key=="z"{this.undo_graph(event.keystroke.modifiers.shift,window,cx);cx.stop_propagation();}else if event.keystroke.key=="escape"{this.cancel_graph_gesture(cx);cx.stop_propagation();}else if this.graph_focus.is_focused(window) && let Some(id)=this.composition.selected { match event.keystroke.key.as_str(){"delete"|"backspace"=>this.delete_node(id,cx),"m"=>this.mute_node(id,cx),"d" if event.keystroke.modifiers.shift=>this.duplicate_node(id,window,cx),_=>return};cx.stop_propagation();}})}>
        <div flex flex-wrap items-center flex-shrink-0 gap-3 px-4 py-3 border-b-1 border-color={rgb(BORDER)}>
            <div flex flex-col><div font-semibold text-size={px(15.)}>Compositing</div><div text-size={px(10.)} text-color={rgb(MUTED)}>Blender 4.5 LTS · image, color, masks and effects</div></div>
            <Button args={"graph-undo"} ghost small label="Undo" disabled={editor.graph_history.is_empty()} on-click={cx.listener(|this,_,window,cx|this.undo_graph(false,window,cx))} />
            <Button args={"graph-redo"} ghost small label="Redo" disabled={editor.graph_future.is_empty()} on-click={cx.listener(|this,_,window,cx|this.undo_graph(true,window,cx))} />
            <Button args={"graph-zoom-out"} ghost small label="−" on-click={cx.listener(|this,_,_,cx|this.zoom_graph(0.8,cx))} />
            <div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("{:.0}%",z*100.)}</div>
            <Button args={"graph-zoom-in"} ghost small label="+" on-click={cx.listener(|this,_,_,cx|this.zoom_graph(1.25,cx))} />
            <Button args={"graph-fit"} ghost small label="Fit" on-click={cx.listener(|this,_,_,cx|this.fit_graph(cx))} />
            <Button args={"graph-arrange"} ghost small label="Arrange" on-click={cx.listener(|this,_,_,cx|this.arrange_graph(cx))} />
            <Button args={"graph-reset"} ghost small label="Reset graph" disabled={editor.graph_rendering} on-click={cx.listener(|this,_,window,cx|this.reset_graph(window,cx))} />
            <Button args={"graph-render"} small label={if editor.graph_rendering{"Rendering…"}else{"Render preview"}} disabled={editor.graph_rendering} bg={rgb(PURPLE)} on-click={cx.listener(|this,_,window,cx|this.render_composition(window,cx))} />
        </div>
        <div flex flex-1 min-h-0>
            <div id="graph-palette" flex flex-col flex-shrink-0 w={px(230.)} min-h-0 overflow-y-scroll bg={rgb(PANEL)} border-r-1 border-color={rgb(BORDER)} p-3 gap-3>
                <div flex items-center justify-between><div font-semibold text-size={px(12.)}>Node library</div><div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("{} types",Operation::palette().len())}</div></div>
                <Input args={&editor.graph_search} small />
                <div flex flex-wrap gap-1 children={std::iter::once(None).chain(Category::ALL.into_iter().filter(|category|Operation::palette().iter().any(|op|op.category()==*category)).map(Some)).map(|category|<Button args={SharedString::from(format!("graph-category-{category:?}"))} ghost xsmall label={category.map(|c|c.label()).unwrap_or("All")} bg={rgb(if editor.graph_category==category{0x303846}else{PANEL})} on-click={cx.listener(move |this,_,_,cx|{this.graph_category=category;cx.notify();})} />)} />
                {tool("graph-import","Import media",None,Action::Import,false,cx)}
                <div text-size={px(10.)} text-color={rgb(Category::Input.color())} font-semibold>MEDIA SOURCES</div>
                <div flex flex-col gap-1 children={sources.into_iter().map(|(asset,component)|{
                    let operation=Operation::Source{asset,component};let label=Composition::source_label(&Node{id:0,operation:operation.clone(),position:[0.,0.],inputs:vec![],input_ports:vec![],muted:false,frame:None},&editor.state.assets);
                    <Button args={SharedString::from(format!("graph-source-{asset}-{component:?}"))} ghost small justify-start w-full label={label.clone()} tooltip={label} text-size={px(10.)} text-color={rgb(if matches!(component,ClipComponent::Video(_)){Category::Input.color()}else{GREEN})}
                        on-click={cx.listener(move |this,_,window,cx|this.add_node(operation.clone(),window,cx))} />
                })} />
                <div flex flex-col gap-3 children={Category::ALL.into_iter().filter(|category|editor.graph_category.is_none_or(|c|c==*category)).filter(|category|Operation::palette().iter().any(|op|op.category()==*category && (search.is_empty()||format!("{} {}",op.label(),op.summary()).to_lowercase().contains(&search)))).map(|category|{
                    <div flex flex-col gap-1 border-t-1 border-color={rgb(BORDER)} pt-3>
                        <div flex items-center gap-2 pb-1><div size={px(6.)} rounded-full bg={rgb(category.color())} /><div text-size={px(10.)} font-semibold text-color={rgb(category.color())}>{category.label().to_uppercase()}</div></div>
                        <div flex flex-col children={Operation::palette().into_iter().filter(|operation|operation.category()==category&&(search.is_empty()||format!("{} {}",operation.label(),operation.summary()).to_lowercase().contains(&search))).map(|operation|
                            <Button args={SharedString::from(format!("add-{}",operation.definition().map(|d|d.id.as_str()).unwrap_or(operation.label())))} ghost small justify-start w-full label={operation.label()} tooltip={operation.summary()} text-size={px(11.)} on-click={cx.listener(move |this,_,window,cx|this.add_node(operation.clone(),window,cx))} />
                        )} />
                    </div>
                })} />
            </div>
            <div relative flex flex-col flex-1 min-w-0 min-h-0>
                <canvas args={(move |bounds,_,cx|{let _=bounds_owner.update(cx,|this,_|this.graph_bounds=[bounds.size.width.into(),f32::from(bounds.size.height)-30.]);bounds},|_,_,_,_|{})} absolute inset-0 />
                <div flex-shrink-0 px-3 py-2 text-size={px(10.)} text-color={rgb(if editor.composition.pending.is_some(){PURPLE}else{MUTED})}>{if editor.composition.pending.is_some(){"Choose an input · Blender converts compatible socket types · Escape cancels"}else{"Drag node headers to move · Click a right socket, then a left socket · Right-click for actions · M mutes · Shift+D duplicates"}}</div>
                <div id="graph-scroll" track-scroll={&editor.graph_scroll} flex-1 min-h-0 min-w-0 overflow-x-scroll overflow-y-scroll>
                    <div id="graph-canvas" relative w={px(width)} h={px(height)} flex-shrink-0 bg={rgb(0x171c24)}>
                        <div id="graph-background" absolute inset-0 context-menu={move |menu,window,cx|crate::interactions::graph_menu(menu,menu_owner.clone(),window,cx)}>
                        <canvas args={(|bounds,_,_|bounds,move |bounds,_,window,_|{
                            for x in (0..width as usize).step_by(24){for y in (0..height as usize).step_by(24){window.paint_quad(fill(Bounds::new(point(bounds.origin.x+px(x as f32),bounds.origin.y+px(y as f32)),size(px(1.),px(1.))),rgb(0x2c3440)));}}
                            for (position,dimensions) in &frames {window.paint_quad(fill(Bounds::new(point(bounds.origin.x+px(position[0]*z),bounds.origin.y+px(position[1]*z)),size(px(dimensions[0]*z),px(dimensions[1]*z))),rgb(0x242a34)));}
                            for (from,to,color) in &connections {let from=point(bounds.origin.x+px(from[0]*z),bounds.origin.y+px(from[1]*z));let to=point(bounds.origin.x+px(to[0]*z),bounds.origin.y+px(to[1]*z));let mid=point((from.x+to.x)/2.,(from.y+to.y)/2.);let reach=((to.x-from.x).abs()/2.).max(px(35.));let mut path=PathBuilder::stroke(px(2.));path.move_to(from);path.curve_to(mid,point(from.x+reach,from.y));path.curve_to(to,point(to.x-reach,to.y));if let Ok(path)=path.build(){window.paint_path(path,rgb(*color));}}
                            let move_owner=owner.clone();window.on_mouse_event(move |event:&MouseMoveEvent,phase,_,cx|{if phase==DispatchPhase::Bubble&&event.pressed_button==Some(MouseButton::Left){let _=move_owner.update(cx,|this,cx|this.update_node_drag([event.position.x.into(),event.position.y.into()],cx));}});
                            let up_owner=owner.clone();window.on_mouse_event(move |event:&MouseUpEvent,phase,_,cx|{if phase==DispatchPhase::Bubble&&event.button==MouseButton::Left{let _=up_owner.update(cx,|this,cx|{if this.graph_drag.is_none(){return;}this.update_node_drag([event.position.x.into(),event.position.y.into()],cx);this.graph_drag=None;this.graph_drag_recorded=false;cx.notify();});}});
                        })} absolute left-0 top-0 size-full />
                        </div>
                        <div children={nodes.iter().filter(|node|node.is_frame()).chain(nodes.iter().filter(|node|!node.is_frame())).map(|node|graph_node(editor,node,cx))} />
                        {if nodes.is_empty(){<div absolute left={px(60.)} top={px(80.)} text-color={rgb(MUTED)}>Import media, then add sources from the left panel.</div>.into_any_element()}else{div().into_any_element()}}
                    </div>
                </div>
            </div>
            <div id="graph-settings" flex flex-col flex-shrink-0 w={px(290.)} min-h-0 bg={rgb(PANEL)} border-l-1 border-color={rgb(BORDER)} p-3 gap-3>
                <div id="graph-properties-scroll" flex flex-col flex-1 min-h-0 overflow-y-scroll gap-3>
                <div text-size={px(10.)} text-color={rgb(MUTED)}>INSPECTOR</div>
                <div text-size={px(14.)} font-semibold>{selected.map(|n|n.operation.label()).unwrap_or("Node settings")}</div>
                {if let Some(node)=selected{<div flex flex-col gap-3>
                    <div text-size={px(10.)} text-color={rgb(MUTED)}>{Composition::source_label(node,&editor.state.assets)}</div>
                    <div flex flex-col gap-2 children={node.operation.parameters().into_iter().enumerate().filter(|(i,_)|node.operation.parameter_meta(*i).and_then(|p|p.key.strip_prefix("input:")).and_then(|s|s.parse::<usize>().ok()).is_none_or(|port|node.operation.visible_inputs().contains(&port)&&node.inputs.get(port).is_none_or(Option::is_none))).map(|(i,_)|node_parameter(editor,node,i,cx))} />
                </div>.into_any_element()}else{<div text-size={px(10.)} text-color={rgb(MUTED)}>Select a node to edit its settings.</div>.into_any_element()}}
                <div border-t-1 border-color={rgb(BORDER)} pt-3 font-semibold text-size={px(12.)}>Preview range</div>
                <div text-size={px(10.)} text-color={rgb(MUTED)}>Source time (seconds)</div><Input args={&editor.graph_time} small />
                <div text-size={px(10.)} text-color={rgb(MUTED)}>Audio excerpt (0.1–10 seconds)</div><Input args={&editor.graph_duration} small />
                </div>
                {if let Some(node)=selected{<div flex flex-shrink-0 gap-2>
                    {if !node.operation.parameters().is_empty(){<Button args={"apply-node-settings"} small label="Apply settings" on-click={cx.listener(|this,_,window,cx|this.apply_node_settings(window,cx))} />.into_any_element()}else{div().into_any_element()}}
                    <Button args={"remove-selected-node"} ghost small label="Remove" on-click={cx.listener({let id=node.id;move |this,_,_,cx|this.delete_node(id,cx)})} />
                </div>.into_any_element()}else{div().into_any_element()}}
                <div id="graph-viewer" flex flex-col flex-shrink-0 gap-2>
                <div border-t-1 border-color={rgb(BORDER)} pt-3 font-semibold text-size={px(12.)}>Viewer</div>
                {if let Some(frame)=&editor.graph_frame{<img args={frame.clone()} w={px(264.)} h={px(144.)} object-fit={ObjectFit::Contain} rounded-md />.into_any_element()}else{<div flex items-center justify-center h={px(120.)} text-size={px(11.)} text-color={rgb(MUTED)} bg={rgb(BG)} rounded-md>Render preview to update the viewer</div>.into_any_element()}}
                {if let Some(result)=&editor.graph_result {<div flex flex-col gap-2>
                    <div text-size={px(10.)} text-color={rgb(if fresh{GREEN}else{PURPLE})}>{if fresh{"Up to date"}else{"Graph changed · render again"}}</div>
                    {if result.audio.is_some(){<div text-size={px(10.)} text-color={rgb(MUTED)}>{format!("Stereo · 48 kHz · {:.1}s · peak {:.3}",result.duration,result.peak)}</div>.into_any_element()}else{div().into_any_element()}}
                    {if result.audio.is_some(){<Button args={"listen-composite"} ghost small label={if editor.graph_listening{"Stop audio"}else{"Listen to audio"}} disabled={(!fresh&&!editor.graph_listening)||editor.graph_rendering} on-click={cx.listener(|this,_,window,cx|this.listen_composition(window,cx))} />.into_any_element()}else{div().into_any_element()}}
                    <Button args={"add-composite-result"} small label="Add result to project" disabled={!fresh||editor.graph_rendering||editor.importing} on-click={cx.listener(|this,_,window,cx|this.add_composition_result(window,cx))} />
                    <Button args={"reveal-composite-files"} ghost xsmall label="Show rendered files" on-click={cx.listener(|this,_,_,cx|{if let Some(path)=this.graph_result.as_ref().and_then(|r|r.image.as_ref().or(r.audio.as_ref()))&&let Some(directory)=path.parent(){cx.reveal_path(directory);}})} />
                    <div text-size={px(10.)} text-color={rgb(MUTED)}>Audio adds separate timeline channels. The rendered frame goes into Media.</div>
                </div>.into_any_element()}else{div().into_any_element()}}
                </div>
            </div>
        </div>
        <div flex items-center justify-between flex-shrink-0 h={px(28.)} px-3 bg={rgb(PANEL)} border-t-1 border-color={rgb(BORDER)} text-size={px(10.)} text-color={rgb(MUTED)}>
            <div>{format!("{} nodes · {} connections",editor.composition.nodes.len(),editor.composition.nodes.iter().map(|n|n.inputs.iter().flatten().count()).sum::<usize>())}</div>
            <div>Image · Value · Vector · Boolean · Audio</div>
            <div>{if editor.graph_rendering{"Rendering…"}else if fresh{"Preview up to date"}else{"Preview needs render"}}</div>
        </div>
    </div>
}
