use std::collections::{HashMap, HashSet};
#[cfg(feature = "debug-fps")]
use std::time::Duration;
#[cfg(feature = "debug-fps")]
use std::time::Instant;

use crate::runtime::binding::{self, Control, Element, InlineStyle, Node, Page};
use crate::runtime::{ComponentProps, Definition, Engine, OutputFormatter, Snapshot, Value};
use gpui_kit::component::{
    IndexPath, TitleBar,
    button::{Button, ButtonVariants},
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone, Copy, Debug, Default)]
pub enum StartupWindowState {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum StartupDecorations {
    Server,
    #[default]
    Client,
}

#[derive(Clone, Copy, Debug)]
pub struct StartupConfig {
    pub width: f32,
    pub height: f32,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub state: StartupWindowState,
    pub decorations: StartupDecorations,
    pub resizable: bool,
    pub minimizable: bool,
    pub movable: bool,
    pub focus: bool,
    pub show: bool,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            width: 1240.0,
            height: 870.0,
            min_width: None,
            min_height: None,
            state: StartupWindowState::Windowed,
            decorations: StartupDecorations::Client,
            resizable: true,
            minimizable: true,
            movable: true,
            focus: true,
            show: true,
        }
    }
}

pub struct HtmlView {
    title: String,
    page: Page,
    engine: Engine,
    snapshot: Snapshot,
    props: ComponentProps,
    embedded: bool,
    sliders: HashMap<String, Entity<SliderState>>,
    selects: HashMap<String, Entity<SelectState<Vec<String>>>>,
    subscriptions: Vec<Subscription>,
    component_views: HashMap<String, Entity<HtmlView>>,
    rendered_component_ids: HashSet<String>,
    #[cfg(feature = "debug-fps")]
    fps_overlay: Entity<FpsOverlay>,
}

#[cfg(feature = "debug-fps")]
struct FpsOverlay {
    since: Instant,
    frames: u32,
    fps: f32,
}

#[cfg(feature = "debug-fps")]
impl FpsOverlay {
    fn record(&mut self, cx: &mut Context<Self>) {
        self.frames += 1;
        let elapsed = self.since.elapsed();
        if elapsed >= Duration::from_millis(250) {
            self.fps = self.frames as f32 / elapsed.as_secs_f32();
            self.frames = 0;
            self.since = Instant::now();
            cx.notify();
        }
    }
}

#[cfg(feature = "debug-fps")]
impl Render for FpsOverlay {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(52.0))
            .right(px(8.0))
            .rounded(px(6.0))
            .bg(rgb(0x20242a))
            .text_color(rgb(0xffffff))
            .px(px(8.0))
            .py(px(4.0))
            .text_size(px(11.0))
            .child(format!("{:.0} FPS", self.fps))
    }
}

#[cfg(feature = "debug-fps")]
fn sample_fps(window: &Window, overlay: Entity<FpsOverlay>) {
    window.on_next_frame(move |window, cx| {
        overlay.update(cx, |view, cx| view.record(cx));
        sample_fps(window, overlay);
    });
}

impl HtmlView {
    fn new(
        page: Page,
        title: String,
        calculate: fn(&HashMap<String, Value>, u64) -> Snapshot,
        on_change: Option<fn(&mut HashMap<String, Value>, &str, &Value)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let engine = Engine::start(page.defaults.clone(), calculate, on_change);
        Self::new_with_engine(page, title, engine, false, window, cx)
    }

    fn new_component(
        page: Page,
        engine: Engine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_engine(page, String::new(), engine, true, window, cx)
    }

    fn new_with_engine(
        page: Page,
        title: String,
        engine: Engine,
        embedded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        page.bind_signals(&engine);
        let updates = (!page.inputs.is_empty()).then(|| engine.subscribe());
        let signal_updates = page
            .inputs
            .iter()
            .filter_map(|binding| binding.subscribe_signal())
            .collect::<Vec<_>>();
        let snapshot = engine.snapshot();
        let props = page.props(&snapshot);
        #[cfg(feature = "debug-fps")]
        let fps_overlay = cx.new(|_| FpsOverlay {
            since: Instant::now(),
            frames: 0,
            fps: 0.0,
        });
        let mut view = Self {
            title,
            page,
            engine,
            snapshot,
            props,
            embedded,
            sliders: HashMap::new(),
            selects: HashMap::new(),
            subscriptions: Vec::new(),
            component_views: HashMap::new(),
            rendered_component_ids: HashSet::new(),
            #[cfg(feature = "debug-fps")]
            fps_overlay,
        };
        view.build_controls(window, cx);
        let initial = view.snapshot.clone();
        view.sync_controls(&initial, true, window, cx);
        #[cfg(feature = "debug-fps")]
        if !view.embedded {
            sample_fps(window, view.fps_overlay.clone());
        }
        if let Some(updates) = updates {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    let receiver = updates.clone();
                    let next = cx
                        .background_spawn(async move {
                            receiver
                                .lock()
                                .expect("update receiver lock poisoned")
                                .recv()
                        })
                        .await;
                    let Ok(mut next) = next else {
                        break;
                    };
                    while let Ok(latest) = updates
                        .lock()
                        .expect("update receiver lock poisoned")
                        .try_recv()
                    {
                        next = latest;
                    }
                    if this
                        .update_in(cx, |view, window, cx| view.apply_snapshot(next, window, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        for updates in signal_updates {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    let receiver = updates.clone();
                    let changed = cx
                        .background_spawn(async move {
                            receiver
                                .lock()
                                .expect("signal update receiver lock poisoned")
                                .recv()
                        })
                        .await;
                    if changed.is_err() {
                        break;
                    }
                    if this
                        .update_in(cx, |view, window, cx| {
                            let next = view.engine.snapshot();
                            view.apply_snapshot(next, window, cx);
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        view
    }

    fn apply_snapshot(&mut self, next: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        let next_props = self.page.props(&next);
        let props_changed = next_props != self.props;
        if props_changed {
            self.sync_controls(&next, false, window, cx);
            self.props = next_props;
        }
        self.snapshot = next;
        if props_changed {
            cx.notify();
        }
    }

    fn build_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.clear();
        self.sliders.clear();
        self.selects.clear();
        for control in self.page.controls.clone() {
            match control {
                Control::Range {
                    id,
                    binding,
                    min,
                    max,
                    step,
                    default,
                } => {
                    let state = cx.new(|_| {
                        SliderState::new()
                            .min(min)
                            .max(max)
                            .step(step)
                            .default_value(default.unwrap_or(min))
                    });
                    self.subscriptions.push(cx.subscribe(
                        &state,
                        move |view, _, event: &SliderEvent, _| {
                            if let SliderEvent::Change(value) = event {
                                binding.set(&view.engine, Value::Number(value.start()));
                            }
                        },
                    ));
                    self.sliders.insert(id, state);
                }
                Control::Select {
                    id,
                    binding,
                    options,
                    default,
                } => {
                    let selected = options
                        .iter()
                        .position(|option| option.value == default)
                        .unwrap_or(0);
                    let labels = options.iter().map(|option| option.label.clone()).collect();
                    let state = cx.new(|cx| {
                        SelectState::new(
                            labels,
                            Some(IndexPath::default().row(selected)),
                            window,
                            cx,
                        )
                    });
                    self.subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |view, _, event: &SelectEvent<Vec<String>>, _, _| {
                            let SelectEvent::Confirm(value) = event;
                            if let Some(value) = value {
                                if let Some(option) =
                                    options.iter().find(|option| option.label == *value)
                                {
                                    binding.set(&view.engine, Value::Text(option.value.clone()));
                                }
                            }
                        },
                    ));
                    self.selects.insert(id, state);
                }
            }
        }
    }

    fn sync_controls(
        &mut self,
        snapshot: &Snapshot,
        initial: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for control in &self.page.controls {
            let Some(value) = control.binding().get(snapshot) else {
                continue;
            };
            if !initial && self.props.get(control.binding().name) == Some(&value) {
                continue;
            }
            let id = control.id();
            if let (Some(state), Some(value)) = (self.sliders.get(id), value.number()) {
                state.update(cx, |slider, cx| slider.set_value(value, window, cx));
            }
            if let (Some(state), Value::Text(value)) = (self.selects.get(id), value) {
                let label = match control {
                    Control::Select { options, .. } => options
                        .iter()
                        .find(|option| option.value == value)
                        .map(|option| option.label.clone())
                        .unwrap_or(value),
                    Control::Range { .. } => value,
                };
                state.update(cx, |select, cx| {
                    select.set_selected_value(&label, window, cx)
                });
            }
        }
    }

    pub fn render_node(
        &mut self,
        node: &Node,
        props: &ComponentProps,
        viewport_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            Node::Text(value) => div().child(value.clone()).into_any_element(),
            Node::Element(element) => {
                self.render_element(element, props, viewport_width, window, cx)
            }
        }
    }

    pub fn child_element<'a>(&self, parent: &'a Element, index: usize) -> &'a Element {
        match &parent.children[index] {
            Node::Element(element) => element,
            Node::Text(_) => unreachable!("compiled child must be an element"),
        }
    }

    fn render_element(
        &mut self,
        element: &Element,
        props: &ComponentProps,
        viewport_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let render = element
            .render
            .expect("compiled element needs a GPUI renderer");
        render(self, element, props, viewport_width, window, cx)
    }

    pub fn render_root(
        &mut self,
        props: &ComponentProps,
        viewport_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let root = self.page.root.clone();
        div()
            .w_full()
            .child(self.render_node(&Node::Element(root), props, viewport_width, window, cx))
            .into_any_element()
    }

    pub fn render_component(
        &mut self,
        element: &Element,
        viewport_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let instance = element
            .component
            .as_ref()
            .expect("compiled component element needs a component instance");
        let id = instance.id.clone();
        self.rendered_component_ids.insert(id.clone());
        let page = instance.page.clone();
        let child = if let Some(child) = self.component_views.get(&id) {
            child.clone()
        } else {
            let engine = self.engine.clone();
            let child = cx.new(|cx| Self::new_component(*page, engine, window, cx));
            self.component_views.insert(id, child.clone());
            child
        };
        let _ = viewport_width;
        child.into_any_element()
    }

    pub fn is_mobile(&self, viewport_width: f32) -> bool {
        self.page
            .mobile_breakpoint
            .is_some_and(|breakpoint| viewport_width <= breakpoint)
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn render_output(&self, element: &Element, container: Div) -> AnyElement {
        let value = element
            .binding
            .as_ref()
            .and_then(|binding| binding.get(&self.snapshot));
        if element.attr("data-render") == Some("bar") {
            let percentage = value
                .as_ref()
                .and_then(Value::number)
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);
            container.w(relative(percentage / 100.0)).into_any_element()
        } else {
            container
                .child(display_value(
                    value.as_ref(),
                    element,
                    self.page.output_formatter,
                ))
                .into_any_element()
        }
    }

    pub fn render_slider(
        &self,
        element: &Element,
        container: Div,
        style: InlineStyle,
    ) -> AnyElement {
        let key = element
            .control_id
            .as_deref()
            .expect("range input needs an id");
        let color = style.text.color;
        container
            .w_full()
            .when_some(self.sliders.get(key), |container, state| {
                let slider = Slider::new(state);
                let slider = if let Some(color) = color {
                    slider.bg(color).text_color(color)
                } else {
                    slider
                };
                container.child(slider)
            })
            .into_any_element()
    }

    pub fn render_select(&self, element: &Element, container: Div) -> AnyElement {
        let key = element.control_id.as_deref().expect("select needs an id");
        container
            .w_full()
            .when_some(self.selects.get(key), |container, state| {
                container.child(Select::new(state).w_full())
            })
            .into_any_element()
    }

    pub fn render_if(
        &mut self,
        element: &Element,
        props: &ComponentProps,
        mut container: Div,
        viewport_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected_branch = match element
            .binding
            .as_ref()
            .and_then(|binding| binding.get(&self.snapshot))
        {
            Some(Value::Text(value)) if element.attr("data-rsc-equals") == Some(value.as_str()) => {
                "rsc-then"
            }
            _ => "rsc-else",
        };
        for child in &element.children {
            if let Node::Element(branch) = child {
                if branch.tag == selected_branch {
                    container =
                        container.child(self.render_node(child, props, viewport_width, window, cx));
                }
            }
        }
        container.into_any_element()
    }

    pub fn render_button(
        &self,
        element: &Element,
        container: Div,
        style: InlineStyle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let binding = element
            .binding
            .as_ref()
            .expect("button needs a binding")
            .clone();
        let args = element.args.clone();
        let label = element.text_content();
        let mut button = Button::new(element.attr("id").unwrap_or(binding.name).to_owned())
            .label(label)
            .secondary()
            .on_click(cx.listener(move |view, _, _, _| {
                let values = args
                    .iter()
                    .filter_map(|arg| arg.get(&view.snapshot))
                    .collect();
                binding.set(&view.engine, Value::Arguments(values));
            }));
        button.style().refine(&style);
        container.child(button).into_any_element()
    }
}

impl Render for HtmlView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport_width = f32::from(window.viewport_size().width);
        let props = self.props.clone();
        let render_component = self.page.renderer;
        self.rendered_component_ids.clear();
        let root = render_component(self, &props, viewport_width, window, cx);
        self.component_views
            .retain(|id, _| self.rendered_component_ids.contains(id));
        if self.embedded {
            return div().w_full().child(root);
        }
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .when(
                matches!(window.window_decorations(), Decorations::Client { .. }),
                |view| view.child(TitleBar::new().child(self.title.clone())),
            )
            .child(
                div()
                    .id("html-document-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(root),
            )
            .when(cfg!(feature = "debug-fps"), |view| {
                #[cfg(feature = "debug-fps")]
                {
                    view.child(self.fps_overlay.clone())
                }
                #[cfg(not(feature = "debug-fps"))]
                {
                    view
                }
            })
    }
}

fn display_value(
    value: Option<&Value>,
    element: &Element,
    formatter: Option<OutputFormatter>,
) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let rendered = match (element.attr("data-format"), formatter) {
        (Some(name), Some(format)) => format(name, value),
        _ => value.text(),
    };
    format!("{}{}", rendered, element.attr("data-suffix").unwrap_or(""))
}

pub fn run(definition: Definition) {
    run_with_config(definition, StartupConfig::default());
}

pub fn run_with_config(definition: Definition, startup: StartupConfig) {
    let calculate = definition.calculate.unwrap_or(identity_snapshot);
    let on_change = definition.on_change;
    let title = definition.title.to_owned();
    let page = binding::compile(&definition).unwrap_or_else(|error| {
        eprintln!("component binding error: {error}");
        std::process::exit(1);
    });
    if std::env::args().nth(1).as_deref() == Some("--validate") {
        return;
    }
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        let restore_bounds =
            Bounds::centered(None, size(px(startup.width), px(startup.height)), cx);
        let window_bounds = match startup.state {
            StartupWindowState::Windowed => WindowBounds::Windowed(restore_bounds),
            StartupWindowState::Maximized => WindowBounds::Maximized(restore_bounds),
            StartupWindowState::Fullscreen => WindowBounds::Fullscreen(restore_bounds),
        };
        let decorations = match startup.decorations {
            StartupDecorations::Server => WindowDecorations::Server,
            StartupDecorations::Client => WindowDecorations::Client,
        };
        open_window(
            WindowOptions {
                window_bounds: Some(window_bounds),
                window_min_size: startup
                    .min_width
                    .zip(startup.min_height)
                    .map(|(width, height)| size(px(width), px(height))),
                window_decorations: Some(decorations),
                is_resizable: startup.resizable,
                is_minimizable: startup.minimizable,
                is_movable: startup.movable,
                focus: startup.focus,
                show: startup.show,
                titlebar: Some(TitlebarOptions {
                    title: Some(title.clone().into()),
                    ..TitleBar::title_bar_options()
                }),
                ..TitleBar::window_options()
            },
            cx,
            move |window, cx| {
                cx.new(|cx| HtmlView::new(page, title, calculate, on_change, window, cx))
            },
        )
        .expect("failed to open GPUI window");
    });
}

fn identity_snapshot(values: &HashMap<String, Value>, reset_epoch: u64) -> Snapshot {
    Snapshot {
        values: values.clone(),
        reset_epoch,
    }
}
