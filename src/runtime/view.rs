use std::collections::HashMap;
#[cfg(feature = "debug-fps")]
use std::time::Duration;
#[cfg(feature = "debug-fps")]
use std::time::Instant;

use crate::runtime::binding::{self, Control, Element, InlineStyle, Node, Page};
use crate::runtime::{Definition, Engine, OutputFormatter, Snapshot, Value};
use gpui_kit::component::{
    IndexPath, TitleBar,
    button::{Button, ButtonVariants},
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub struct HtmlView {
    title: String,
    page: Page,
    engine: Engine,
    snapshot: Snapshot,
    sliders: HashMap<String, Entity<SliderState>>,
    selects: HashMap<String, Entity<SelectState<Vec<String>>>>,
    subscriptions: Vec<Subscription>,
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
            subscriptions: Vec::new(),
            #[cfg(feature = "debug-fps")]
            fps_overlay,
        };
        view.build_controls(window, cx);
        let initial = view.snapshot.clone();
        view.sync_controls(&initial, true, window, cx);
        #[cfg(feature = "debug-fps")]
        sample_fps(window, view.fps_overlay.clone());
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
        view
    }

    fn apply_snapshot(&mut self, next: Snapshot, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_controls(&next, false, window, cx);
        self.snapshot = next;
        cx.notify();
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
                        .position(|option| option.value == default)
                        .unwrap_or(0);
                    let labels = options
                        .iter()
                        .map(|option| option.label.clone())
                        .collect();
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
            if !initial && control.binding().get(&self.snapshot).as_ref() == Some(&value) {
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
                state.update(cx, |select, cx| select.set_selected_value(&label, window, cx));
            }
        }
    }

    pub fn render_node(
        &self,
        node: &Node,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            Node::Text(value) => div().child(value.clone()).into_any_element(),
            Node::Element(element) => self.render_element(element, viewport_width, cx),
        }
    }

    pub fn child_element<'a>(&self, parent: &'a Element, index: usize) -> &'a Element {
        match &parent.children[index] {
            Node::Element(element) => element,
            Node::Text(_) => unreachable!("compiled child must be an element"),
        }
    }

    fn render_element(
        &self,
        element: &Element,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        element
            .render
            .expect("compiled element needs a GPUI renderer")(
            self,
            element,
            viewport_width,
            cx,
        )
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
        &self,
        element: &Element,
        mut container: Div,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected_branch = match element
            .binding
            .as_ref()
            .and_then(|binding| binding.get(&self.snapshot))
        {
            Some(Value::Text(value))
                if element.attr("data-rsc-equals") == Some(value.as_str()) =>
            {
                "rsc-then"
            }
            _ => "rsc-else",
        };
        for child in &element.children {
            if let Node::Element(branch) = child {
                if branch.tag == selected_branch {
                    container = container.child(self.render_node(child, viewport_width, cx));
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
        let root = self.render_element(&self.page.root, viewport_width, cx);
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
