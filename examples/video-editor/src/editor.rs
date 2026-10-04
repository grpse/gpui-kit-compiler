use crate::state::{Action, EditorState, Screen};
use gpui_kit::component::{
    input::{InputEvent, InputState},
    slider::{SliderEvent, SliderState},
};
use gpui_kit::{prelude::*, *};

pub struct Editor {
    pub state: EditorState,
    pub search: Entity<InputState>,
    pub header_search: Entity<InputState>,
    pub sliders: Vec<Entity<SliderState>>,
    pub seek: Entity<SliderState>,
    pub _subscriptions: Vec<Subscription>,
    pub edit_workspace: crate::workspace::Workspace,
    pub library_workspace: crate::workspace::Workspace,
    pub dragging_playhead: bool,
    pub timeline_origin: f32,
    pub track_controls_width: f32,
    pub dragging_track_divider: bool,
    pub timeline_scroll: ScrollHandle,
    pub timeline_zoom: Entity<SliderState>,
    pub timeline_scale: f32,
    pub media_scroll: ScrollHandle,
    pub preset_scroll: ScrollHandle,
}
impl Editor {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = EditorState::default();
        if let Some(screen) = std::env::args().find_map(|arg| match arg.as_str() {
            "--library" => Some(Screen::Library),
            "--overview" => Some(Screen::Overview),
            "--tracking" => Some(Screen::Tracking),
            _ => None,
        }) {
            state.apply(Action::Screen(screen));
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search media…"));
        let header_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search media, folders, or tags…"));
        let mut subscriptions = vec![
            cx.subscribe_in(
                &search,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        this.state.apply(Action::Search(value.clone()));
                        this.header_search
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                        cx.notify();
                    }
                },
            ),
            cx.subscribe_in(
                &header_search,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        this.state.apply(Action::Search(value.clone()));
                        this.search
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                        cx.notify();
                    }
                },
            ),
        ];
        let sliders = state
            .controls
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let (min, max) = match i {
                    0 => (10., 200.),
                    1 => (-180., 180.),
                    _ => (0., 100.),
                };
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(min)
                        .max(max)
                        .step(1.)
                        .default_value(value)
                });
                subscriptions.push(cx.subscribe(
                    &slider,
                    move |this, _, event: &SliderEvent, cx| {
                        if let SliderEvent::Change(value) = event {
                            this.state.apply(Action::Control(i, value.end()));
                            cx.notify();
                        }
                    },
                ));
                slider
            })
            .collect();
        let seek = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(1.)
                .step(0.001)
                .default_value(state.position / state.seek_limit())
        });
        subscriptions.push(cx.subscribe_in(
            &seek,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.state
                        .apply(Action::Seek(value.end() * this.state.seek_limit()));
                    this.seek.update(cx, |slider, cx| {
                        slider.set_value(this.state.position / this.state.seek_limit(), window, cx)
                    });
                    cx.notify();
                }
            },
        ));
        let timeline_zoom = cx.new(|_| {
            SliderState::new()
                .min(0.25)
                .max(8.)
                .step(0.05)
                .default_value(state.zoom)
        });
        subscriptions.push(cx.subscribe_in(
            &timeline_zoom,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if let SliderEvent::Change(value) = event {
                    this.dispatch(Action::Zoom(value.end() - this.state.zoom), window, cx);
                }
            },
        ));
        let owner = cx.weak_entity();
        let edit_workspace = crate::workspace::workspace(owner.clone(), true, window, cx);
        let library_workspace = crate::workspace::workspace(owner, false, window, cx);
        Self {
            state,
            search,
            header_search,
            sliders,
            seek,
            _subscriptions: subscriptions,
            edit_workspace,
            library_workspace,
            dragging_playhead: false,
            timeline_origin: 0.,
            track_controls_width: 176.,
            dragging_track_divider: false,
            timeline_scroll: ScrollHandle::new(),
            timeline_zoom,
            timeline_scale: 1.,
            media_scroll: ScrollHandle::new(),
            preset_scroll: ScrollHandle::new(),
        }
    }
    pub fn dispatch(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let clear = matches!(
            action,
            Action::ClearFilters | Action::ClearPresetFilter | Action::Category(_)
        );
        let sync_controls = matches!(action, Action::ResetTransform | Action::ResetCrop);
        if matches!(action, Action::ResetWorkspace) {
            let editing = self.state.screen.has_timeline();
            let area = crate::workspace::workspace(cx.weak_entity(), editing, window, cx);
            if editing {
                self.edit_workspace = area;
            } else {
                self.library_workspace = area;
            }
        }
        let reveal_browser = matches!(action, Action::Category(_) | Action::Folder(_));
        if clear || reveal_browser {
            self.media_scroll.set_offset(point(px(0.), px(0.)));
            self.preset_scroll.set_offset(point(px(0.), px(0.)));
        }
        let zooming = matches!(action, Action::Zoom(_));
        let old_zoom = self.state.zoom;
        let offset = self.timeline_scroll.offset();
        let playhead_x = self.state.position * self.timeline_scale + f32::from(offset.x);
        self.state.apply(action);
        if zooming {
            let new_scale = self.timeline_scale * self.state.zoom / old_zoom;
            self.timeline_scroll.set_offset(point(
                px((playhead_x - self.state.position * new_scale).min(0.)),
                offset.y,
            ));
            self.timeline_zoom.update(cx, |slider, cx| {
                slider.set_value(self.state.zoom, window, cx)
            });
        }
        if reveal_browser {
            let workspace = if self.state.screen.has_timeline() {
                &self.edit_workspace
            } else {
                &self.library_workspace
            };
            workspace.area.update(cx, |area, cx| {
                area.select_panel(workspace.browser, window, cx)
            });
        }
        if clear {
            self.search
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.header_search
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        if sync_controls {
            for (i, slider) in self.sliders.iter().enumerate() {
                slider.update(cx, |s, cx| s.set_value(self.state.controls[i], window, cx));
            }
        }
        self.seek.update(cx, |s, cx| {
            s.set_value(self.state.position / self.state.seek_limit(), window, cx)
        });
        cx.notify();
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::generated::ui::editor_view(self, window, cx)
    }
}
