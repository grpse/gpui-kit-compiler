//! Native docking contracts delegate all presentation to small RSX functions.
use gpui_base::ResizeHandleContext;
use gpui_kit::component::dock::{
    DockAreaRenderer, DockContext, DockSkin, DropIndicator, NodeId, TabGroupContext,
    TabGroupRenderer,
};
use gpui_kit::{prelude::*, *};
use std::rc::Rc;

pub struct IslandSkin {
    inner: Rc<DockSkin>,
}
impl IslandSkin {
    pub fn new(inner: Rc<DockSkin>) -> Rc<Self> {
        Rc::new(Self { inner })
    }
}
impl DockAreaRenderer for IslandSkin {
    fn frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.frame(window, cx)
    }
    fn center_frame(&self, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        self.inner.center_frame(window, cx)
    }
    fn split_frame(
        &self,
        node: NodeId,
        axis: Axis,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        crate::generated::docking::split_frame(self.inner.split_frame(node, axis, window, cx))
    }
    fn render_split_handle(
        &self,
        handle: &ResizeHandleContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<AnyElement> {
        Some(crate::generated::docking::divider(handle).into_any_element())
    }
    fn render_dock(
        &self,
        dock: &DockContext,
        content: AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.inner.render_dock(dock, content, window, cx)
    }
    fn tab_group_renderer(&self) -> Rc<dyn TabGroupRenderer> {
        Rc::new(IslandTabs {
            inner: self.inner.tab_group_renderer(),
        })
    }
}
struct IslandTabs {
    inner: Rc<dyn TabGroupRenderer>,
}
impl TabGroupRenderer for IslandTabs {
    fn frame(&self, group: &TabGroupContext, window: &mut Window, cx: &mut App) -> Stateful<Div> {
        crate::generated::docking::island_frame(self.inner.frame(group, window, cx))
    }
    fn content_frame(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Stateful<Div> {
        crate::generated::docking::island_body(self.inner.content_frame(group, window, cx))
    }
    fn render_tab_bar(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let content = if group
            .active_panel()
            .is_some_and(|panel| panel.panel_name(cx) == "Tools & folders")
        {
            crate::generated::docking::tools_tabs(group, cx).into_any_element()
        } else {
            self.inner.render_tab_bar(group, window, cx)
        };
        crate::generated::docking::island_header(content).into_any_element()
    }
    fn render_active_panel(
        &self,
        panel: AnyView,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.inner.render_active_panel(panel, group, window, cx)
    }
    fn render_drop_indicator(
        &self,
        indicator: DropIndicator,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.inner.render_drop_indicator(indicator, window, cx)
    }
    fn render_empty(
        &self,
        group: &TabGroupContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        self.inner.render_empty(group, window, cx)
    }
}

pub struct ToolsTabPreview {
    pub title: &'static str,
}
impl Render for ToolsTabPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        crate::generated::docking::tools_tab_preview(self.title)
    }
}
