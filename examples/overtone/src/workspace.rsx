use gpui_kit::{prelude::*, *};
use gpui_kit::component::input::Input;
use gpui_kit::component::{TitleBar, WindowBorder};
use gpui_kit::component::scroll::ScrollbarAxis;
use gpui_kit::component::{button::{Button,ButtonVariants as _},menu::DropdownMenu,Sizable as _};
use crate::{model::*,studio::{Studio,Action,ResizeTarget,SaveProject,SaveProjectAs,OpenProject,NewProject,ZoomIn,ZoomOut,ResetZoom,OpenSettings,UndoProject,RedoProject,ChangeTheme,CloseSession,ShowAbout,RenameProject,SelectWorkspace},primitives::*,panels};

pub fn application_menus()->Vec<Menu>{menus_for_project(&Project::default())}
pub fn menus_for_project(project:&Project)->Vec<Menu>{let mut menus=vec![
    Menu::new("File").items([MenuItem::action("New session",NewProject),MenuItem::action("Open project…",OpenProject),MenuItem::separator(),MenuItem::action("Save project",SaveProject),MenuItem::action("Save project as…",SaveProjectAs),MenuItem::separator(),MenuItem::action("Close session",CloseSession),MenuItem::action("Rename project…",RenameProject)]),
    Menu::new("Edit").items([MenuItem::action("Undo",UndoProject),MenuItem::action("Redo",RedoProject)]),
    Menu::new("View").items([MenuItem::action("Workspace settings…",OpenSettings),MenuItem::separator(),MenuItem::action("Zoom in",ZoomIn),MenuItem::action("Zoom out",ZoomOut),MenuItem::action("Reset zoom",ResetZoom),MenuItem::separator(),MenuItem::action("Switch light/dark theme",ChangeTheme)]),
    Menu::new("Help").items([MenuItem::action("About Overtone",ShowAbout)])
];menus[2].items.push(MenuItem::separator());for profile in &project.profiles{menus[2].items.push(MenuItem::action(profile.name.clone(),SelectWorkspace{id:profile.id}));}menus}
#[gpui]
fn toolbar(studio:&Studio)->AnyElement{
    let profiles=studio.project.profiles.iter().map(|p|(p.id,p.name.clone())).collect::<Vec<_>>();let active=studio.project.active_profile;
    let focus=studio.focus.clone();let file_focus=focus.clone();let edit_focus=focus.clone();let view_focus=focus.clone();let busy=studio.busy;let undo=studio.undo_stack.is_empty() || busy;let redo=studio.redo_stack.is_empty() || busy;
    <div flex items-center h-full gap={u(2.)} w-full>
        {Button::new("file-menu").ghost().small().label("File").dropdown_menu(move|menu,_,_|menu.action_context(file_focus.clone()).menu_with_disabled("New session",Box::new(NewProject),busy).menu_with_disabled("Open project…",Box::new(OpenProject),busy).separator().menu_with_disabled("Save project",Box::new(SaveProject),busy).menu_with_disabled("Save project as…",Box::new(SaveProjectAs),busy).separator().menu("Close session",Box::new(CloseSession)).menu_with_disabled("Rename project…",Box::new(RenameProject),busy))}
        {Button::new("edit-menu").ghost().small().label("Edit").dropdown_menu(move|menu,_,_|menu.action_context(edit_focus.clone()).menu_with_disabled("Undo",Box::new(UndoProject),undo).menu_with_disabled("Redo",Box::new(RedoProject),redo))}
        {Button::new("view-menu").ghost().small().label("View").dropdown_menu(move|menu,_,_|{let menu=menu.action_context(view_focus.clone()).menu("Workspace settings…",Box::new(OpenSettings)).separator().menu("Zoom in",Box::new(ZoomIn)).menu("Zoom out",Box::new(ZoomOut)).menu("Reset zoom",Box::new(ResetZoom)).separator().menu("Switch light/dark theme",Box::new(ChangeTheme));let mut menu=menu.separator();for (id,name) in &profiles{menu=menu.menu_with_check(name.clone(),*id==active,Box::new(SelectWorkspace{id:*id}));}menu})}
        {Button::new("help-menu").ghost().small().label("Help").dropdown_menu(move|menu,_,_|menu.action_context(focus.clone()).menu("About Overtone",Box::new(ShowAbout)))}
        <div flex-1/><div text-size={u(11.)} pr={u(10.)}>{format!("{}{} · {}",studio.project.title,if studio.dirty(){" *"}else{""},studio.project.profile().name)}</div>
    </div>.into_any_element()
}

#[derive(Clone)]
pub struct ModuleDrag {pub module:Module}
impl Render for ModuleDrag {
    #[gpui]
    fn render(&mut self,_:&mut Window,_:&mut Context<Self>)->impl IntoElement{<div p={u(12.)} bg={rgb(0x444444)} text-color={rgb(0xffffff)}>{self.module.label()}</div>}
}
#[gpui]
fn divider(id:String,target:ResizeTarget,vertical:bool,thickness:f32,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);
    <div id={id} test-support flex flex-shrink-0 items-center justify-center cursor={if vertical{CursorStyle::ResizeUpDown}else{CursorStyle::ResizeLeftRight}}
        when:args={(vertical,|d|d.w_full().h(u(thickness)))} when:args={(!vertical,|d|d.self_stretch().w(u(thickness)))}
        bg={rgb(p.bg)} hover={move|d|d.bg(rgb(p.soft))}
        on-mouse-down:args={(MouseButton::Left,cx.listener(move|this,event:&MouseDownEvent,window,cx|{
            let target=if event.modifiers.alt{match target{ResizeTarget::Panel(module)|ResizeTarget::Pair(module,_)=>ResizeTarget::PanelGap(module,vertical),ResizeTarget::PanelGap(_,_)=>target,_=>ResizeTarget::DockGap(vertical)}}else{target};
            this.begin_resize(target,event.position,window,cx);
        }))}>
        <div when:args={(vertical,|d|d.w(u(28.)).h(u(2.)))} when:args={(!vertical,|d|d.w(u(2.)).h(u(28.)))} bg={rgb(p.border)}/>
    </div>.into_any_element()
}
fn minimum(module:Module)->f32{match module{Module::Harmonics=>300.,Module::Controls=>260.,Module::Timeline=>360.,Module::Keyboard=>300.,_=>180.}}
#[gpui]
fn panel(placement:&Placement,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let module=placement.module;let dock=placement.dock;
    let weak=cx.entity().downgrade();let drag_owner=weak.clone();let handle=studio.scroll(&format!("body-{module:?}"));
    let ghost=studio.drop_preview.is_some_and(|p|p.module==module);
    let element=<div id={format!("module-{module:?}")} test-support relative flex flex-col w-full min-w={u(minimum(module))} min-h={u(100.)} when-some:args={(placement.height,|d,h|d.h(u(h)))} flex-shrink-0 border={u(1.)} border-color={rgb(p.border)} bg={rgb(p.panel)} opacity={if ghost{0.28}else{1.}}
        on-drop={cx.listener(move|this,drag:&ModuleDrag,_,cx|this.drop_module(drag.module,dock,Some(module),cx))}>
        <canvas args={(move|bounds,_,cx|{let _=weak.update(cx,|s,_|{s.panel_bounds.insert(module,bounds);if s.drag_module.is_some() && s.drop_preview.is_none(){s.drag_panels.insert(module,bounds);}});},|_,_,_,_|{})} absolute size-full/>
        <div id={format!("handle-{module:?}")} test-support flex flex-wrap gap={u(4.)} flex-shrink-0 items-center justify-between px={u(8.)} py={u(7.)} bg={rgb(p.soft)} text-size={u(12.)} cursor-grab
            on-drag:args={(ModuleDrag{module},move|drag,_,_,cx|{let _=drag_owner.update(cx,|s,cx|s.begin_module_drag(module,cx));cx.new(|_|drag.clone())})}>
            <div flex-1 min-w={u(70.)}>{format!("⠿  {}",module.label())}</div>
            <div flex gap={u(2.)}>
                {button(format!("zoom-{module:?}-minus"),"−",Action::Zoom(Some(module),-0.1),false,studio,cx).h(u(22.)).px(u(3.)).text_size(u(9.))}
                {button(format!("zoom-{module:?}-reset"),format!("{:.0}%",placement.zoom*100.),Action::Zoom(Some(module),0.),false,studio,cx).h(u(22.)).px(u(3.)).text_size(u(9.))}
                {button(format!("zoom-{module:?}-plus"),"+",Action::Zoom(Some(module),0.1),false,studio,cx).h(u(22.)).px(u(3.)).text_size(u(9.))}
            </div>
        </div>
        <div relative min-w-0 when:args={(placement.height.is_some(),|d|d.flex_1().min_h_0())}>
            <div id={format!("body-{module:?}")} test-support p={u(studio.project.profile().panel_padding)} pb={u(studio.project.profile().panel_padding+10.)} min-w-0 overflow-x-scroll track-scroll={&handle} when:args={(placement.height.is_some(),|d|d.h_full().min_h_0().overflow_y_scroll())}>{panels::module_view(module,studio,cx)}</div>
            {scrollbar_layer(format!("bar-body-{module:?}"),handle,ScrollbarAxis::Both)}
        </div>
        {divider(format!("resize-panel-{module:?}"),ResizeTarget::Panel(module),true,6.,studio,cx)}
    </div>.into_any_element();
    UiScale::new(studio.project.profile().zoom*placement.zoom,element).into_any_element()
}
#[gpui]
fn pair(left:&Placement,right:&Placement,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let l=left.module;let r=right.module;let weak=cx.entity().downgrade();let gap=left.pair_gap.unwrap_or(studio.project.profile().panel_gap);
    <div id={format!("pair-{l:?}-{r:?}")} test-support relative flex items-start w-full min-w={u(minimum(l)*left.zoom+minimum(r)*right.zoom+gap)}>
        <canvas args={(move|bounds,_,cx|{let _=weak.update(cx,|s,_|{s.pair_bounds.insert(l,bounds);});},|_,_,_,_|{})} absolute size-full/>
        <div flex-basis={u(0.)} flex-grow={left.share} min-w={u(minimum(l)*left.zoom)}>{panel(left,studio,cx)}</div>
        {divider(format!("resize-pair-{l:?}-{r:?}"),ResizeTarget::Pair(l,r),false,gap,studio,cx)}
        <div flex-basis={u(0.)} flex-grow={1.-left.share} min-w={u(minimum(r)*right.zoom)}>{panel(right,studio,cx)}</div>
    </div>.into_any_element()
}
#[gpui]
fn dock(dock:Dock,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);let profile=studio.displayed_profile();let placements=profile.panels.iter().filter(|p|p.visible && p.dock==dock).collect::<Vec<_>>();
    let mut required_width:f32=0.;let mut index=0;
    while index<placements.len(){let left=placements[index];let mut width=minimum(left.module)*left.zoom;if left.half && placements.get(index+1).is_some_and(|r|r.half){let right=placements[index+1];width+=minimum(right.module)*right.zoom+left.pair_gap.unwrap_or(profile.panel_gap);index+=2;}else{index+=1;}required_width=required_width.max(width);}
    let mut rows=vec![];let mut i=0;
    while i<placements.len(){let left=placements[i];if left.half && placements.get(i+1).is_some_and(|r|r.half){rows.push(pair(left,placements[i+1],studio,cx));i+=2;}else{rows.push(panel(left,studio,cx));i+=1;}if i<placements.len(){rows.push(divider(format!("resize-gap-{:?}",left.module),ResizeTarget::PanelGap(left.module,true),true,left.gap_after.unwrap_or(studio.project.profile().panel_gap),studio,cx));}}
    <div id={format!("dock-{dock:?}")} test-support flex flex-col w-full min-w={u(required_width)} min-h={u(36.)}
        on-drop={cx.listener(move|this,drag:&ModuleDrag,_,cx|this.drop_module(drag.module,dock,None,cx))}>
        {if rows.is_empty(){<div flex-1 border={u(1.)} border-color={rgb(p.border)} p={u(12.)} text-size={u(11.)} text-color={rgb(p.muted)}>{format!("{} dock · drop a panel here",dock.label())}</div>.into_any_element()}else{<div flex flex-col w-full children={rows}></div>.into_any_element()}}
    </div>.into_any_element()
}
#[gpui]
fn viewport(id:&str,dock:Option<Dock>,content:AnyElement,studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let handle=studio.scroll(id);let weak=cx.entity().downgrade();
    <div relative size-full min-w-0 min-h-0>
        <canvas args={(move|bounds,_,cx|{if let Some(dock)=dock{let _=weak.update(cx,|s,_|{s.dock_bounds.insert(dock,bounds);if s.drag_module.is_some() && s.drop_preview.is_none(){s.drag_docks.insert(dock,bounds);}});}},|_,_,_,_|{})} absolute size-full/>
        <div id={id.to_string()} test-support size-full min-w-0 min-h-0 overflow-x-scroll overflow-y-scroll track-scroll={&handle} pb={u(12.)} pr={u(12.)}>{content}</div>
        {scrollbar_layer(format!("bar-{id}"),handle,ScrollbarAxis::Both)}
    </div>.into_any_element()
}
#[gpui]
fn configuration(studio:&Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);
    <div flex flex-col gap={u(10.)} p={u(14.)} border={u(1.)} border-color={rgb(p.border)} bg={rgb(p.panel)}>
        <div flex gap={u(8.)} items-center flex-wrap><div text-size={u(13.)}>{format!("Configure {}",studio.project.profile().name)}</div><div w={u(200.)}><Input args={&studio.profile_name}/></div>
            {button("rename-profile","Rename",Action::RenameProfile,false,studio,cx)}{button("copy-profile","+ Copy profile",Action::NewProfile(false),false,studio,cx)}{button("empty-profile","+ Empty profile",Action::NewProfile(true),false,studio,cx)}
            {button("export-settings","Save settings",Action::WorkspaceFile(true),false,studio,cx)}{button("import-settings","Load settings",Action::WorkspaceFile(false),false,studio,cx)}
        </div>
        <div flex flex-wrap gap={u(8.)} children={studio.project.profile().panels.iter().map(|placement|{
            let m=placement.module;
            <div flex flex-col gap={u(5.)} p={u(8.)} border={u(1.)} border-color={rgb(p.border)}>
                {button(format!("show-{m:?}"),format!("{} {}",if placement.visible{"✓"}else{"+"},m.label()),Action::PanelVisible(m),placement.visible,studio,cx)}
                <div flex gap={u(4.)}>{button(format!("dock-config-{m:?}"),placement.dock.label(),Action::PanelDock(m),false,studio,cx)}{button(format!("width-{m:?}"),if placement.half{"Half"}else{"Full"},Action::PanelHalf(m),placement.half,studio,cx)}{button(format!("up-{m:?}"),"↑",Action::PanelOrder(m,-1),false,studio,cx)}{button(format!("down-{m:?}"),"↓",Action::PanelOrder(m,1),false,studio,cx)}</div>
                {button(format!("auto-height-{m:?}"),if let Some(height)=placement.height{format!("{height:.0}px · Reset height")}else{"Auto height".into()},Action::AutoHeight(m),false,studio,cx)}
                {button(format!("default-gaps-{m:?}"),if placement.gap_after.is_some() || placement.pair_gap.is_some(){"Custom gaps · Reset"}else{"Default gaps"},Action::DefaultGaps(m),false,studio,cx)}
                {stepper(&format!("settings-zoom-{m:?}"),format!("{:.0}%",placement.zoom*100.),Action::Zoom(Some(m),-0.1),Action::Zoom(Some(m),0.1),studio,cx)}
            </div>
        })}></div>
        <div flex gap={u(16.)} flex-wrap>{stepper("left-width",format!("Left {:.0}px",studio.project.profile().left_width),Action::Sidebar(true,-20.),Action::Sidebar(true,20.),studio,cx)}{stepper("right-width",format!("Right {:.0}px",studio.project.profile().right_width),Action::Sidebar(false,-20.),Action::Sidebar(false,20.),studio,cx)}{stepper("lower-height",format!("Lower {:.0}px",studio.project.profile().lower_height),Action::LowerHeight(-20.),Action::LowerHeight(20.),studio,cx)}</div>
        <div flex gap={u(16.)} flex-wrap>{stepper("panel-spacing",format!("Panels {:.0}px",studio.project.profile().panel_gap),Action::Spacing(0,-2.),Action::Spacing(0,2.),studio,cx)}{stepper("dock-spacing",format!("Docks {:.0}px",studio.project.profile().dock_gap),Action::Spacing(1,-2.),Action::Spacing(1,2.),studio,cx)}{stepper("panel-padding",format!("Padding {:.0}px",studio.project.profile().panel_padding),Action::Spacing(2,-2.),Action::Spacing(2,2.),studio,cx)}</div>
        {caption("Drag dock borders, paired dividers, and panel bottom grips to resize. Drag gaps between rows, or Alt-drag a divider, to set individual spacing. Save settings exports profiles and theme.",studio.project.dark)}
    </div>.into_any_element()
}
#[gpui]
pub fn configuration_view(studio:&mut Studio,cx:&mut Context<Studio>)->AnyElement{
    let p=palette(studio.project.dark);
    <div flex-1 min-h-0 size-full flex flex-col bg={rgb(p.bg)} text-color={rgb(p.text)} text-size={u(13.)} p={u(14.)} gap={u(12.)}>
        <div flex flex-wrap gap={u(8.)} children={studio.project.profiles.iter().map(|profile|button(format!("profile-{}",profile.id),profile.name.clone(),Action::Profile(profile.id),studio.project.active_profile==profile.id,studio,cx))}></div>
        <div flex gap={u(12.)} flex-wrap>{stepper("workspace-zoom",format!("Workspace {:.0}%",studio.project.profile().zoom*100.),Action::Zoom(None,-0.1),Action::Zoom(None,0.1),studio,cx)}{button("workspace-zoom-reset","Reset zoom",Action::Zoom(None,0.),false,studio,cx)}{button("toggle-theme",if studio.project.dark{"Light theme"}else{"Dark theme"},Action::ToggleTheme,false,studio,cx)}</div>
        <div flex-1 min-h-0>{viewport("configuration-scroll",None,configuration(studio,cx),studio,cx)}</div>
    </div>.into_any_element()
}
#[gpui]
pub fn studio_view(studio:&mut Studio,window:&mut Window,cx:&mut Context<Studio>)->AnyElement{
    studio.prepare_scrolls();let p=palette(studio.project.dark);let profile=studio.project.profile();window.set_rem_size(px(16.*profile.zoom));window.set_window_title(&format!("{}{} · Overtone",studio.project.title,if studio.dirty(){" *"}else{""}));cx.set_menus(menus_for_project(&studio.project));
    let moving=studio.drag_module.is_some();let layout=studio.displayed_profile();let has_left=moving || layout.panels.iter().any(|p|p.visible && p.dock==Dock::Left);let has_right=moving || layout.panels.iter().any(|p|p.visible && p.dock==Dock::Right);
    let has_lower=moving || layout.panels.iter().any(|p|p.visible && p.dock==Dock::Lower);
    let lower_height=(profile.lower_height*profile.zoom).min((f32::from(window.bounds().size.height)-250.*profile.zoom).max(100.))/profile.zoom;
    <WindowBorder><div id="overtone-root" test-support key-context="Overtone" track-focus={&studio.focus} relative size-full flex flex-col bg={rgb(p.bg)} text-color={rgb(p.text)} text-size={u(13.)} line-height={relative(1.3)}
        on-mouse-move={cx.listener(|this,event:&MouseMoveEvent,window,cx|{
            if this.resizing.is_some(){this.resize_motion(event,cx);return;}
            if let Some((instance,index,origin,value))=this.knob_drag {
                if !event.dragging(){this.knob_drag=None;return;}
                let max=if [37,38,40].contains(&index){10000.}else{1.};
                let next=(value+f32::from(origin.y-event.position.y)*max/140.).clamp(0.,max);
                this.set_param(instance,index,next,cx);let sliders=if instance{&this.instance_sliders}else{&this.draft_sliders};
                sliders[index].update(cx,|s,cx|s.set_value(next,window,cx));
            }
        })}
        on-mouse-up:args={(MouseButton::Left,cx.listener(|this,_,_,cx|{this.knob_drag=None;this.resizing=None;this.history_group=false;let weak=cx.entity().downgrade();cx.defer(move|cx|{let _=weak.update(cx,|this,cx|{if this.drag_module.is_some(){this.clear_drag();cx.notify();}});});}))}
        on-drag-move={cx.listener(|this,event:&DragMoveEvent<ModuleDrag>,_,cx|{if let Some(drag)=event.dragged_item().downcast_ref::<ModuleDrag>(){this.preview_drag(drag.module,event.event.position,cx);}})}
        on-action={cx.listener(|this,_:&SaveProject,window,cx|this.dispatch(Action::Save(false),window,cx))}
        on-action={cx.listener(|this,_:&SaveProjectAs,window,cx|this.dispatch(Action::Save(true),window,cx))}
        on-action={cx.listener(|this,_:&OpenProject,window,cx|this.dispatch(Action::Open,window,cx))}
        on-action={cx.listener(|this,_:&NewProject,window,cx|this.dispatch(Action::New,window,cx))}
        on-action={cx.listener(|this,_:&ZoomIn,window,cx|this.dispatch(Action::Zoom(None,0.1),window,cx))}
        on-action={cx.listener(|this,_:&ZoomOut,window,cx|this.dispatch(Action::Zoom(None,-0.1),window,cx))}
        on-action={cx.listener(|this,_:&ResetZoom,window,cx|this.dispatch(Action::Zoom(None,0.),window,cx))}
        on-action={cx.listener(|this,_:&OpenSettings,window,cx|this.open_settings(window,cx))}
        on-action={cx.listener(|this,_:&UndoProject,window,cx|this.history(false,window,cx))}
        on-action={cx.listener(|this,_:&RedoProject,window,cx|this.history(true,window,cx))}
        on-action={cx.listener(|this,_:&ChangeTheme,window,cx|this.dispatch(Action::ToggleTheme,window,cx))}
        on-action={cx.listener(|this,_:&CloseSession,window,cx|this.request_replace(crate::studio::Pending::Close,window,cx))}
        on-action={cx.listener(|this,action:&SelectWorkspace,window,cx|this.dispatch(Action::Profile(action.id),window,cx))}
        on-action={cx.listener(|this,_:&RenameProject,_,cx|{this.renaming=true;cx.notify();})}
        on-action={cx.listener(|this,_:&ShowAbout,_,cx|{this.show_status=true;cx.notify();})}>
        {UiScale::new(1.,if cfg!(target_os="linux") && !matches!(window.window_decorations(),Decorations::Client{..}){<div h={u(32.)} flex-shrink-0 bg={rgb(p.soft)}>{toolbar(studio)}</div>.into_any_element()}else{
            <TitleBar bg={rgb(p.soft)} border-color={rgb(p.border)} text-size={u(12.)}
                on-close-window={cx.listener(|this,_,window,cx|this.request_replace(crate::studio::Pending::Close,window,cx))}>{toolbar(studio)}</TitleBar>.into_any_element()
        }).into_any_element()}
        <div id="workspace-scroll" test-support flex-1 min-h-0 flex flex-col px={u(18.)} pb={u(18.)} gap={u(12.)} pt={u(12.)}>
            {
                <div flex flex-col flex-1 min-h-0>
                    <div flex flex-1 min-h-0 items-start>
                        {if has_left{<div h-full min-h-0 w={u(studio.project.profile().left_width)} flex-shrink-0>{viewport("left-scroll",Some(Dock::Left),dock(Dock::Left,studio,cx),studio,cx)}</div>.into_any_element()}else{<div/>.into_any_element()}}
                        {if has_left{divider("resize-left-dock".into(),ResizeTarget::Left,false,studio.project.profile().dock_gap,studio,cx)}else{<div/>.into_any_element()}}
                        <div h-full flex-1 min-w-0 min-h-0>{viewport("center-scroll",Some(Dock::Main),dock(Dock::Main,studio,cx),studio,cx)}</div>
                        {if has_right{divider("resize-right-dock".into(),ResizeTarget::Right,false,studio.project.profile().dock_gap,studio,cx)}else{<div/>.into_any_element()}}
                        {if has_right{<div h-full min-h-0 w={u(studio.project.profile().right_width)} flex-shrink-0>{viewport("right-scroll",Some(Dock::Right),dock(Dock::Right,studio,cx),studio,cx)}</div>.into_any_element()}else{<div/>.into_any_element()}}
                    </div>
                    {if has_lower{divider("resize-lower-dock".into(),ResizeTarget::Lower,true,studio.project.profile().dock_gap,studio,cx)}else{<div/>.into_any_element()}}
                    {if has_lower{<div h={u(lower_height)} flex-shrink-0>{viewport("lower-scroll",Some(Dock::Lower),dock(Dock::Lower,studio,cx),studio,cx)}</div>.into_any_element()}else{<div/>.into_any_element()}}
                </div>.into_any_element()
            }
        </div>
        {if studio.pending.is_some() || studio.renaming || studio.show_status{<div id="project-dialog" test-support absolute inset-0 flex items-center justify-center bg={rgba(0x00000088)} occlude>
            <div w={u(460.)} max-w-full p={u(24.)} flex flex-col gap={u(16.)} bg={rgb(p.panel)} border={u(1.)} border-color={rgb(p.border)} occlude>
                {if studio.pending.is_some(){<div flex flex-col gap={u(16.)}>
                    <div text-size={u(18.)}>Save changes?</div><div>{format!("Save changes to “{}” before {}?",studio.project.title,if studio.pending==Some(crate::studio::Pending::Close){"exiting"}else{"continuing"})}</div>
                    {caption(studio.notice.clone(),studio.project.dark)}
                    <div flex gap={u(8.)} flex-wrap>{button("confirm-save","Save",Action::SaveContinue,true,studio,cx)}{button("confirm-discard","Don't save",Action::ConfirmDiscard,false,studio,cx)}{button("confirm-cancel","Cancel",Action::Cancel,false,studio,cx)}</div>
                </div>.into_any_element()}else if studio.renaming{<div flex flex-col gap={u(16.)}><div text-size={u(18.)}>Rename project</div><Input args={&studio.title} id="project-title"/>{button("rename-done","Done",Action::DismissDialog,true,studio,cx)}</div>.into_any_element()}else{<div flex flex-col gap={u(16.)}><div text-size={u(18.)}>Overtone</div><div>{studio.notice.clone()}</div>{button("status-done","Close",Action::DismissDialog,true,studio,cx)}</div>.into_any_element()}}
            </div>
        </div>.into_any_element()}else{<div/>.into_any_element()}}
    </div></WindowBorder>.into_any_element()
}
