use gpui_kit::{prelude::*, *};
use gpui_kit::component::{TitleBar, WindowBorder};
use crate::{studio::{Studio,Action,SaveProject,SaveProjectAs,NewProject,OpenProject,UndoProject,RedoProject},primitives::*};

pub struct Settings {owner:WeakEntity<Studio>,parent:AnyWindowHandle,focus:FocusHandle,_subscription:Option<Subscription>}
impl Settings {
    pub fn new(owner:WeakEntity<Studio>,parent:AnyWindowHandle,window:&mut Window,cx:&mut Context<Self>)->Self{
        let subscription=owner.upgrade().map(|entity|cx.observe(&entity,|_,_,cx|cx.notify()));
        let focus=cx.focus_handle();focus.focus(window,cx);Self{owner,parent,focus,_subscription:subscription}
    }
    fn forward(&mut self,action:Action,cx:&mut Context<Self>){let owner=self.owner.clone();let _=self.parent.update(cx,|_,window,cx|owner.update(cx,|s,cx|s.dispatch(action,window,cx)));}
    fn edit_history(&mut self,redo:bool,cx:&mut Context<Self>){let owner=self.owner.clone();let _=self.parent.update(cx,|_,window,cx|owner.update(cx,|s,cx|s.history(redo,window,cx)));}
}
impl Render for Settings {
    #[gpui]
    fn render(&mut self,window:&mut Window,cx:&mut Context<Self>)->impl IntoElement{
        window.set_rem_size(px(16.));
        let content=self.owner.update(cx,|studio,cx|{studio.prepare_scrolls();crate::workspace::configuration_view(studio,cx)}).unwrap_or_else(|_|<div p={u(20.)}>This session has closed.</div>.into_any_element());
        <WindowBorder><div size-full flex flex-col key-context="Overtone" track-focus={&self.focus}
            on-action={cx.listener(|this,_:&SaveProject,_,cx|this.forward(Action::Save(false),cx))}
            on-action={cx.listener(|this,_:&SaveProjectAs,_,cx|this.forward(Action::Save(true),cx))}
            on-action={cx.listener(|this,_:&OpenProject,_,cx|this.forward(Action::Open,cx))}
            on-action={cx.listener(|this,_:&NewProject,_,cx|this.forward(Action::New,cx))}
            on-action={cx.listener(|this,_:&UndoProject,_,cx|this.edit_history(false,cx))}
            on-action={cx.listener(|this,_:&RedoProject,_,cx|this.edit_history(true,cx))}>
            {if cfg!(target_os="linux") && !matches!(window.window_decorations(),Decorations::Client{..}){<div h={u(32.)} px={u(14.)}>Overtone · Settings</div>.into_any_element()}else{<TitleBar><div>Overtone · Settings</div></TitleBar>.into_any_element()}}
            {content}
        </div></WindowBorder>
    }
}
