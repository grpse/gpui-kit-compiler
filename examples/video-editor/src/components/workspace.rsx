use std::sync::Arc;
use gpui_kit::{prelude::*, *};
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, PanelHandle, PanelControl, DockArea, DockLayout, DockSkin, PanelStyle, PanelId};
use crate::generated::{ui::Editor, primitives::*};

#[derive(Clone, Copy)]
pub enum WorkspaceCard { Navigation, Library, Preview, Inspector, Timeline, Processing, Details, Generated }
impl WorkspaceCard {
    fn title(self) -> &'static str {
        match self {
            Self::Navigation=>"Tools & folders", Self::Library=>"Media & presets", Self::Preview=>"Preview player",
            Self::Inspector=>"Inspector", Self::Timeline=>"Timeline", Self::Processing=>"Media processing",
            Self::Details=>"Media details", Self::Generated=>"Generated assets",
        }
    }
}

/// One reusable dock host. Content observes the shared editor, while the dock owns its layout.
pub struct WorkspacePanel {
    owner: WeakEntity<Editor>,
    card: WorkspaceCard,
    focus: FocusHandle,
    width: f32,
    height: f32,
    _subscription: Subscription,
}
impl WorkspacePanel {
    fn new(owner: WeakEntity<Editor>, card: WorkspaceCard, cx: &mut Context<Self>) -> Self {
        let subscription=cx.observe(&owner.upgrade().expect("editor is alive"),|_,_,cx|cx.notify());
        Self {owner,card,focus:cx.focus_handle(),width:360.,height:400.,_subscription:subscription}
    }
}
impl Focusable for WorkspacePanel {fn focus_handle(&self,_cx:&App)->FocusHandle{self.focus.clone()}}
impl EventEmitter<PanelEvent> for WorkspacePanel {}
impl BasePanel for WorkspacePanel {
    fn panel_name(&self)->&'static str{self.card.title()}
    fn closable(&self,_cx:&App)->bool{false}
}
impl Panel for WorkspacePanel {
    fn title(&mut self,_window:&mut Window,_cx:&mut Context<Self>)->impl IntoElement{self.card.title()}
    fn inner_padding(&self,_cx:&App)->bool{false}
    fn zoom_control(&self,_cx:&App)->Option<PanelControl>{Some(PanelControl::Both)}
}
impl Render for WorkspacePanel {
    #[gpui]
    fn render(&mut self,_window:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let panel=cx.weak_entity();
        let width=self.width.max(1.);
        let height=self.height.max(1.);
        let card=self.card;
        let content=self.owner.update(cx,|editor,cx|match card {
            WorkspaceCard::Navigation=>editor.sidebar(width,cx).into_any_element(),
            WorkspaceCard::Library=>editor.media_library(width,height,editor.state.screen.has_timeline(),cx).into_any_element(),
            WorkspaceCard::Preview=>editor.player(width,height,cx).into_any_element(),
            WorkspaceCard::Inspector=>editor.inspector_panel(width,height,cx).into_any_element(),
            WorkspaceCard::Timeline=>editor.timeline(width,cx).into_any_element(),
            WorkspaceCard::Processing=>div().id("processing-scroll").size_full().min_h_0().overflow_y_scroll().overflow_x_scroll().child(editor.processing_queue(width<500.,cx)).into_any_element(),
            WorkspaceCard::Details=>editor.details_panel(width,height,cx).into_any_element(),
            WorkspaceCard::Generated=>editor.generated_assets(width,cx).into_any_element(),
        }).unwrap_or_else(|_|div().into_any_element());
        <div relative flex flex-col size-full min-w-0 min-h-0 overflow-hidden bg={rgb(PANEL)}>
            <canvas args={(move |bounds,_,cx| {
                let width=f32::from(bounds.size.width);
                let height=f32::from(bounds.size.height);
                let _=panel.update(cx,|this,cx| {
                    if (this.width-width).abs()>0.5 || (this.height-height).abs()>0.5 {
                        this.width=width;this.height=height;cx.notify();
                    }
                });
            },|_,_,_,_|{})} absolute left-0 top-0 size-full />
            {content}
        </div>
    }
}

pub struct Workspace {pub area:Entity<DockArea>,pub browser:PanelId}

pub fn workspace(owner:WeakEntity<Editor>,editing:bool,window:&mut Window,cx:&mut App)->Workspace {
    let (area,skin)=DockSkin::dock_area(if editing{"edit-workspace"}else{"library-workspace"},Some(1),window,cx);
    skin.set_panel_style(PanelStyle::TabBar,cx);
    let pane=|card, cx:&mut App| {
        let panel=cx.new(|cx|WorkspacePanel::new(owner.clone(),card,cx));
        DockLayout::tabs().panel_view(Arc::new(PanelHandle::new(panel)),cx)
    };
    let navigation=pane(WorkspaceCard::Navigation,cx);
    let browser=cx.new(|cx|WorkspacePanel::new(owner.clone(),WorkspaceCard::Library,cx));
    let browser_id=PanelId::from(browser.entity_id());
    let library=DockLayout::tabs().panel_view(Arc::new(PanelHandle::new(browser)),cx);
    let preview=pane(WorkspaceCard::Preview,cx);
    let processing=pane(WorkspaceCard::Processing,cx);
    let layout=if editing {
        let inspector=pane(WorkspaceCard::Inspector,cx);
        let timeline=pane(WorkspaceCard::Timeline,cx);
        DockLayout::v_split()
            .child(DockLayout::h_split()
                .child(navigation,Some(px(140.)))
                .child(library,Some(px(360.)))
                .child(preview,None)
                .child(DockLayout::v_split().child(inspector,None).child(processing,Some(px(160.))),Some(px(330.))),Some(px(520.)))
            .child(timeline,None)
    }else {
        let details=pane(WorkspaceCard::Details,cx);
        let generated=pane(WorkspaceCard::Generated,cx);
        DockLayout::h_split()
            .child(navigation,Some(px(210.)))
            .child(DockLayout::v_split().child(library,None).child(processing,Some(px(215.))),None)
            .child(DockLayout::v_split().child(preview,Some(px(260.))).child(details,None).child(generated,Some(px(230.))),Some(px(370.)))
    };
    area.update(cx,|area,cx|area.set_center(layout,window,cx));
    Workspace{area,browser:browser_id}
}
