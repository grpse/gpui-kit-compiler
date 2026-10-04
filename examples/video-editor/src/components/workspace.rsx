use gpui_kit::{prelude::*, *};
use crate::{workspace::{WorkspacePanel,WorkspaceCard},generated::primitives::*};

#[gpui]
pub fn workspace_panel(panel_view:&WorkspacePanel,_window:&mut Window,cx:&mut Context<WorkspacePanel>)->impl IntoElement {
    let panel=cx.weak_entity();
    let width=panel_view.width.max(1.);
    let height=panel_view.height.max(1.);
    let card=panel_view.card;
    let content=panel_view.owner.update(cx,|editor,cx|match card {
        WorkspaceCard::Navigation=>crate::generated::navigation::sidebar(editor,width,cx).into_any_element(),
        WorkspaceCard::Library=>crate::generated::library::media_library(editor,width,height,editor.state.screen.has_timeline(),cx).into_any_element(),
        WorkspaceCard::Preview=>crate::generated::player::player(editor,width,height,cx).into_any_element(),
        WorkspaceCard::Inspector=>crate::generated::inspector::inspector_panel(editor,width,height,cx).into_any_element(),
        WorkspaceCard::Timeline=>crate::generated::timeline::timeline(editor,width,cx).into_any_element(),
        WorkspaceCard::Processing=>div().id("processing-scroll").size_full().min_h_0().overflow_y_scroll().overflow_x_scroll().child(crate::generated::processing::processing_queue(editor,width<500.,cx)).into_any_element(),
        WorkspaceCard::Details=>crate::generated::details::details_panel(editor,width,height,cx).into_any_element(),
        WorkspaceCard::Generated=>crate::generated::details::generated_assets(editor,width,cx).into_any_element(),
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
