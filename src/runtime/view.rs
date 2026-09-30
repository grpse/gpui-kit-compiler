use std::collections::HashMap;
#[cfg(feature = "debug-fps")]
use std::time::{Duration, Instant};

use crate::runtime::binding::{self, Control, Element, InlineStyle, Length, Node, Page};
use crate::runtime::{Definition, Engine, Snapshot, Value};
use gpui_kit::component::{
    IndexPath, TitleBar,
    button::{Button, ButtonVariants},
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

struct HtmlView {
    title: String,
    page: Page,
    engine: Engine,
    snapshot: Snapshot,
    sliders: HashMap<String, Entity<SliderState>>,
    selects: HashMap<String, Entity<SelectState<Vec<String>>>>,
    outputs: HashMap<String, Entity<OutputView>>,
    subscriptions: Vec<Subscription>,
    #[cfg(feature = "debug-fps")]
    fps_overlay: Entity<FpsOverlay>,
}

struct OutputView {
    element: Element,
    value: Option<Value>,
}

impl Render for OutputView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        if self.element.attr("data-render") == Some("bar") {
            let score = self
                .value
                .as_ref()
                .and_then(Value::number)
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);
            styled_div(&self.element.style)
                .w(relative(score / 100.0))
                .into_any_element()
        } else {
            styled_div(&self.element.style)
                .child(display_value(self.value.as_ref(), &self.element))
                .into_any_element()
        }
    }
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
            .bg(rgb(0x201a16))
            .text_color(rgb(0xf0b46b))
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
        let snapshot = engine.snapshot();
        let updates = engine.subscribe();
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
            sliders: HashMap::new(),
            selects: HashMap::new(),
            outputs: HashMap::new(),
            subscriptions: Vec::new(),
            #[cfg(feature = "debug-fps")]
            fps_overlay,
        };
        view.build_controls(window, cx);
        view.build_outputs(cx);
        let initial = view.snapshot.clone();
        view.sync_controls(&initial, true, window, cx);
        #[cfg(feature = "debug-fps")]
        sample_fps(window, view.fps_overlay.clone());
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(mut next) = updates.recv().await {
                while let Ok(latest) = updates.try_recv() {
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
        view
    }

    fn apply_snapshot(&mut self, next: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_controls(&next, false, window, cx);
        for output in self.outputs.values() {
            let state = output.read(cx);
            let value = state
                .element
                .binding
                .as_ref()
                .and_then(|binding| binding.get(&next));
            if value != state.value {
                output.update(cx, |state, cx| {
                    state.value = value;
                    cx.notify();
                });
            }
        }
        self.snapshot = next;
    }

    fn build_outputs(&mut self, cx: &mut Context<Self>) {
        fn collect(element: &Element, outputs: &mut Vec<Element>) {
            if element.output_id.is_some() {
                outputs.push(element.clone());
            }
            for child in &element.children {
                if let Node::Element(child) = child {
                    collect(child, outputs);
                }
            }
        }
        let mut elements = Vec::new();
        collect(&self.page.root, &mut elements);
        for element in elements {
            let value = element
                .binding
                .as_ref()
                .and_then(|binding| binding.get(&self.snapshot));
            let id = element.output_id.clone().unwrap();
            self.outputs
                .insert(id, cx.new(|_| OutputView { element, value }));
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
                            .default_value(default)
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
                        .position(|value| value == &default)
                        .unwrap_or(0);
                    let state = cx.new(|cx| {
                        SelectState::new(
                            options,
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
                                binding.set(&view.engine, Value::Text(value.clone()));
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
            if !initial && control.binding().get(&self.snapshot).as_ref() == Some(&value) {
                continue;
            }
            let id = control.id();
            if let (Some(state), Some(value)) = (self.sliders.get(id), value.number()) {
                state.update(cx, |slider, cx| slider.set_value(value, window, cx));
            }
            if let (Some(state), Value::Text(value)) = (self.selects.get(id), value) {
                state.update(cx, |select, cx| {
                    select.set_selected_value(&value, window, cx)
                });
            }
        }
    }

    fn render_node(&self, node: &Node, cx: &mut Context<Self>) -> AnyElement {
        match node {
            Node::Text(value) => div().child(value.clone()).into_any_element(),
            Node::Element(element) => self.render_element(element, cx),
        }
    }

    fn render_element(&self, element: &Element, cx: &mut Context<Self>) -> AnyElement {
        if let Some(id) = &element.output_id {
            return self.outputs.get(id).unwrap().clone().into_any_element();
        }
        if element.control_id.is_some() {
            let key = element.control_id.as_deref().unwrap();
            if element.tag == "input" {
                return styled_div(&element.style)
                    .w_full()
                    .when_some(self.sliders.get(key), |container, state| {
                        container.child(Slider::new(state))
                    })
                    .into_any_element();
            }
            if element.tag == "select" {
                return styled_div(&element.style)
                    .w_full()
                    .when_some(self.selects.get(key), |container, state| {
                        container.child(Select::new(state).w_full())
                    })
                    .into_any_element();
            }
        }
        if element.attr("data-out").is_some() && element.tag == "button" {
            let binding = element.binding.as_ref().unwrap().clone();
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
            if let Some(color) = element.style.background {
                button = button.bg(rgb(color));
            }
            if let Some(color) = element.style.color {
                button = button.text_color(rgb(color));
            }
            if let Some(radius) = element.style.border_radius {
                button = button.rounded(px(radius));
            }
            return styled_div(&element.style).child(button).into_any_element();
        }
        if element.tag == "option" {
            return div().into_any_element();
        }
        let mut container = styled_div(&element.style);
        for child in &element.children {
            container = container.child(self.render_node(child, cx));
        }
        container.into_any_element()
    }
}

impl Render for HtmlView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = self.render_element(&self.page.root, cx);
        let view = div()
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
            });
        view
    }
}

fn display_value(value: Option<&Value>, element: &Element) -> String {
    let Some(value) = value else {
        return "—".into();
    };
    let rendered = match element.attr("data-format") {
        Some("score") => format!("{:.0}", value.number().unwrap_or(0.0)),
        Some("ratio") => format!("1 : {:.1}", value.number().unwrap_or(0.0)),
        Some("duration") => {
            let seconds = value.number().unwrap_or(0.0).round() as u32;
            format!("{}:{:02} min", seconds / 60, seconds % 60)
        }
        _ => value.text(),
    };
    format!("{}{}", rendered, element.attr("data-suffix").unwrap_or(""))
}

fn styled_div(style: &InlineStyle) -> Div {
    let mut element = div();
    if style.display_flex {
        element = element.flex();
    }
    if style.column {
        element = element.flex_col();
    }
    if style.flex_wrap {
        element = element.flex_wrap();
    }
    if style.flex_grow {
        element = element.flex_1();
    }
    if let Some(value) = style.gap {
        element = element.gap(px(value));
    }
    if let Some(padding) = style.padding {
        element = element
            .pt(px(padding.top))
            .pr(px(padding.right))
            .pb(px(padding.bottom))
            .pl(px(padding.left));
    }
    if let Some(value) = style.background {
        element = element.bg(rgb(value));
    }
    if let Some(value) = style.color {
        element = element.text_color(rgb(value));
    }
    if let Some(value) = style.font_size {
        element = element.text_size(px(value));
    }
    if let Some(value) = style.font_weight {
        element = element.font_weight(if value >= 700 {
            FontWeight::BOLD
        } else {
            FontWeight::SEMIBOLD
        });
    }
    if style.border_width.unwrap_or(0.0) > 0.0 {
        element = element.border_1();
    }
    if let Some(value) = style.border_color {
        element = element.border_color(rgb(value));
    }
    if let Some(value) = style.border_radius {
        element = element.rounded(px(value));
    }
    if let Some(value) = style.width {
        element = match value {
            Length::Px(v) => element.w(px(v)),
            Length::Percent(v) => element.w(relative(v)),
        };
    }
    if let Some(value) = style.height {
        element = match value {
            Length::Px(v) => element.h(px(v)),
            Length::Percent(v) => element.h(relative(v)),
        };
    }
    if let Some(value) = style.min_width {
        element = element.min_w(px(value));
    }
    if let Some(value) = style.max_width {
        element = element.max_w(px(value));
    }
    if style.margin_auto {
        element = element.mx_auto();
    }
    element = match style.justify.as_deref() {
        Some("space-between") => element.justify_between(),
        Some("center") => element.justify_center(),
        Some("flex-end") => element.justify_end(),
        _ => element,
    };
    match style.align.as_deref() {
        Some("center") => element.items_center(),
        Some("flex-start") => element.items_start(),
        Some("flex-end") => element.items_end(),
        _ => element,
    }
}

pub fn run(definition: Definition) {
    let calculate = definition
        .calculate
        .expect("root component needs calculate callback");
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
        let bounds = Bounds::centered(None, size(px(1240.0), px(870.0)), cx);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_decorations: Some(WindowDecorations::Client),
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
