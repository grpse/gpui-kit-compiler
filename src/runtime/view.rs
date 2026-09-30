use std::collections::HashMap;
use std::time::Duration;
#[cfg(feature = "debug-fps")]
use std::time::Instant;

use crate::runtime::binding::{self, Control, Element, InlineStyle, Length, Node, Page};
use crate::runtime::{Definition, Engine, Snapshot, StyleContext, StyleRule, StyleSheet, Value};
use gpui_kit::AnimationExt as _;
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
    output_colors: HashMap<String, u32>,
    brew_visualization: Option<Entity<BrewVisualization>>,
    selects: HashMap<String, Entity<SelectState<Vec<String>>>>,
    outputs: HashMap<String, Entity<OutputView>>,
    subscriptions: Vec<Subscription>,
    #[cfg(feature = "debug-fps")]
    fps_overlay: Entity<FpsOverlay>,
}

struct OutputView {
    element: Element,
    value: Option<Value>,
    mobile_breakpoint: Option<f32>,
    style_sheets: Vec<StyleSheet>,
    snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq)]
struct BrewVisualization {
    method: String,
    filter: String,
    water: f32,
    pours: f32,
    acidity: f32,
    bitterness: f32,
    body: f32,
    clarity: f32,
    intensity: f32,
    extraction_signal: String,
}

impl BrewVisualization {
    fn from_snapshot(snapshot: &Snapshot) -> Self {
        fn number(snapshot: &Snapshot, key: &str, fallback: f32) -> f32 {
            snapshot
                .get(key)
                .and_then(Value::number)
                .unwrap_or(fallback)
        }

        fn text(snapshot: &Snapshot, key: &str, fallback: &str) -> String {
            snapshot
                .get(key)
                .map(Value::text)
                .unwrap_or_else(|| fallback.to_owned())
        }

        Self {
            method: text(snapshot, "recipe.method", "V60"),
            filter: text(snapshot, "recipe.filter", "Paper"),
            water: number(snapshot, "recipe.water", 300.0),
            pours: number(snapshot, "recipe.pours", 3.0),
            acidity: number(snapshot, "profile.acidity", 50.0),
            bitterness: number(snapshot, "profile.bitterness", 35.0),
            body: number(snapshot, "profile.body", 50.0),
            clarity: number(snapshot, "profile.clarity", 60.0),
            intensity: number(snapshot, "profile.intensity", 55.0),
            extraction_signal: text(snapshot, "derived.extraction_signal", "balanced"),
        }
    }

    fn coffee_color(&self) -> u32 {
        let strength = ((self.intensity * 0.5 + self.bitterness * 0.3 + self.body * 0.2) / 100.0)
            .clamp(0.0, 1.0);
        blend_color(0xd8904e, 0x482315, strength)
    }

    fn cup_level(&self) -> f32 {
        (0.28 + ((self.water - 100.0) / 500.0).clamp(0.0, 1.0) * 0.48).clamp(0.22, 0.78)
    }

    fn character(&self) -> &'static str {
        if self.intensity > 69.0 || self.bitterness > 62.0 {
            "Bold & roasty"
        } else if self.clarity > 70.0 && self.acidity > 58.0 {
            "Bright & tea-like"
        } else if self.body > 66.0 {
            "Silky & full"
        } else {
            "Soft & balanced"
        }
    }
}

impl Render for BrewVisualization {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let coffee = self.coffee_color();
        let cup_level = self.cup_level();
        let filter_color = if self.filter == "Metal" {
            0xaeb5b2
        } else if self.filter == "Cloth" {
            0x8b6a4d
        } else {
            0xd8bd97
        };
        let kettle_color = if self.method == "Espresso" {
            0x6d7f75
        } else {
            0xc9824c
        };
        let duration =
            Duration::from_millis((2100.0 - (self.pours.clamp(1.0, 8.0) - 1.0) * 150.0) as u64);
        let extraction_label = self.extraction_signal.clone();
        let character = self.character();
        let intensity_label = format!("{:.0}%", self.intensity.clamp(0.0, 100.0));

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(14.0))
            .rounded(px(18.0))
            .border_1()
            .border_color(rgb(0x3b3128))
            .bg(rgb(0x211c17))
            .p(px(18.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .bg(rgb(0xe7a45e))
                                    .with_animation(
                                        "brew-live-pulse",
                                        Animation::new(Duration::from_millis(1200)).repeat_synced(),
                                        |this, delta| {
                                            this.opacity(
                                                0.55 + (delta * std::f32::consts::PI).sin().abs()
                                                    * 0.45,
                                            )
                                        },
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xe7a45e))
                                    .child("BREW PREVIEW"),
                            ),
                    )
                    .child(
                        div()
                            .rounded(px(99.0))
                            .bg(rgb(0x35281e))
                            .px(px(8.0))
                            .py(px(4.0))
                            .text_size(px(10.0))
                            .text_color(rgb(0xe8c095))
                            .child(extraction_label),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(250.0))
                    .overflow_hidden()
                    .rounded(px(16.0))
                    .bg(rgb(0xf4e6d3))
                    .child(
                        div()
                            .absolute()
                            .top(px(14.0))
                            .right(px(14.0))
                            .size(px(56.0))
                            .rounded_full()
                            .bg(rgb(0xf8eddf)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(26.0))
                            .left(px(56.0))
                            .w(px(32.0))
                            .h(px(28.0))
                            .rounded_full()
                            .border_2()
                            .border_color(rgb(kettle_color)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(30.0))
                            .left(px(17.0))
                            .w(px(66.0))
                            .h(px(43.0))
                            .rounded(px(16.0))
                            .border_2()
                            .border_color(rgb(0xa9663a))
                            .bg(rgb(kettle_color)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(39.0))
                            .left(px(23.0))
                            .w(px(26.0))
                            .h(px(7.0))
                            .rounded_full()
                            .bg(rgb(0xe7b17b)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(59.0))
                            .left(px(69.0))
                            .w(px(46.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgb(kettle_color)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(80.0))
                            .left(px(98.0))
                            .w(px(106.0))
                            .h(px(102.0))
                            .child(
                                canvas(
                                    |_, _, _| {},
                                    move |bounds, _, window, _| {
                                        let top_left = bounds.origin + point(px(4.0), px(6.0));
                                        let top_right =
                                            bounds.top_right() + point(px(-4.0), px(6.0));
                                        let bottom = point(
                                            bounds.origin.x + bounds.size.width.half(),
                                            bounds.origin.y + bounds.size.height - px(3.0),
                                        );
                                        let mut builder = PathBuilder::fill();
                                        builder.move_to(top_left);
                                        builder.line_to(top_right);
                                        builder.line_to(bottom);
                                        builder.close();
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, rgb(filter_color));
                                        }
                                    },
                                )
                                .size_full(),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(87.0))
                            .left(px(100.0))
                            .w(px(102.0))
                            .h(px(12.0))
                            .rounded_full()
                            .bg(rgb(0xefd9b9)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(97.0))
                            .left(px(116.0))
                            .w(px(70.0))
                            .h(px(15.0))
                            .rounded_full()
                            .bg(rgb(coffee)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(108.0))
                            .left(px(139.0))
                            .w(px(6.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgb(coffee))
                            .with_animation(
                                "brew-first-pour-drop",
                                Animation::new(duration).repeat_synced(),
                                |this, delta| {
                                    this.top(px(109.0 + delta * 43.0))
                                        .opacity((1.0 - delta * 0.8).clamp(0.0, 1.0))
                                },
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(108.0))
                            .left(px(153.0))
                            .w(px(5.0))
                            .h(px(7.0))
                            .rounded_full()
                            .bg(rgb(coffee))
                            .with_animation(
                                "brew-second-pour-drop",
                                Animation::new(duration + Duration::from_millis(280))
                                    .repeat_synced(),
                                |this, delta| {
                                    this.top(px(112.0 + delta * 39.0))
                                        .opacity((1.0 - delta * 0.8).clamp(0.0, 1.0))
                                },
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(169.0))
                            .left(px(150.0))
                            .w(px(5.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgb(coffee))
                            .with_animation(
                                "brew-drip-into-cup",
                                Animation::new(duration + Duration::from_millis(530))
                                    .repeat_synced(),
                                |this, delta| {
                                    this.top(px(170.0 + delta * 22.0))
                                        .opacity((1.0 - delta).clamp(0.0, 1.0))
                                },
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(13.0))
                            .left(px(83.0))
                            .w(px(112.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(rgb(0xd8c5ae)),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(19.0))
                            .left(px(98.0))
                            .w(px(88.0))
                            .h(px(54.0))
                            .rounded(px(16.0))
                            .border_2()
                            .border_color(rgb(0xcbb69e))
                            .bg(rgb(0xfffaf2))
                            .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left(px(2.0))
                                    .right(px(2.0))
                                    .h(relative(cup_level))
                                    .rounded(px(12.0))
                                    .bg(rgb(coffee)),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(31.0))
                            .left(px(178.0))
                            .w(px(22.0))
                            .h(px(25.0))
                            .rounded_full()
                            .border_2()
                            .border_color(rgb(0xcbb69e)),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(77.0))
                            .left(px(125.0))
                            .w(px(7.0))
                            .h(px(18.0))
                            .rounded_full()
                            .bg(rgb(0xb6a28b))
                            .with_animation(
                                "brew-steam-left",
                                Animation::new(Duration::from_millis(1800)).repeat_synced(),
                                |this, delta| {
                                    this.bottom(px(77.0 + delta * 21.0))
                                        .opacity((0.45 * (1.0 - delta)).clamp(0.0, 0.45))
                                },
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(79.0))
                            .left(px(145.0))
                            .w(px(6.0))
                            .h(px(16.0))
                            .rounded_full()
                            .bg(rgb(0xb6a28b))
                            .with_animation(
                                "brew-steam-right",
                                Animation::new(Duration::from_millis(2100)).repeat_synced(),
                                |this, delta| {
                                    this.bottom(px(79.0 + delta * 18.0))
                                        .opacity((0.42 * (1.0 - delta)).clamp(0.0, 0.42))
                                },
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0xa99b8d))
                                    .child("CUP CHARACTER"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xf4ece1))
                                    .child(character),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0xa99b8d))
                                    .child("INTENSITY"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xe7a45e))
                                    .child(intensity_label),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .rounded(px(10.0))
                    .bg(rgb(0x2a231d))
                    .px(px(10.0))
                    .py(px(8.0))
                    .text_size(px(10.0))
                    .text_color(rgb(0xc9b9a6))
                    .child(format!(
                        "{} · {} filter",
                        self.method,
                        self.filter.to_lowercase()
                    ))
                    .child(format!("{:.0} g water", self.water)),
            )
    }
}

fn blend_color(from: u32, to: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let from = ((from >> shift) & 0xff) as f32;
        let to = ((to >> shift) & 0xff) as f32;
        (from + (to - from) * amount).round() as u32
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

impl Render for OutputView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let viewport_width = f32::from(window.viewport_size().width);
        let dynamic_styles =
            resolve_dynamic_styles(&self.style_sheets, viewport_width, &self.snapshot);
        let style = style_for_element(
            &self.element,
            viewport_width,
            self.mobile_breakpoint,
            &dynamic_styles,
        );
        if self.element.attr("data-render") == Some("bar") {
            let score = self
                .value
                .as_ref()
                .and_then(Value::number)
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);
            styled_div(&style)
                .w(relative(score / 100.0))
                .into_any_element()
        } else {
            styled_div(&style)
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
            output_colors: HashMap::new(),
            brew_visualization: None,
            selects: HashMap::new(),
            outputs: HashMap::new(),
            subscriptions: Vec::new(),
            #[cfg(feature = "debug-fps")]
            fps_overlay,
        };
        view.build_controls(window, cx);
        view.build_output_colors();
        view.build_brew_visualization(cx);
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
        if let Some(visualization) = &self.brew_visualization {
            let next_visualization = BrewVisualization::from_snapshot(&next);
            visualization.update(cx, |view, cx| {
                if *view != next_visualization {
                    *view = next_visualization;
                    cx.notify();
                }
            });
        }
        for output in self.outputs.values() {
            output.update(cx, |state, cx| {
                let value = state
                    .element
                    .binding
                    .as_ref()
                    .and_then(|binding| binding.get(&next));
                if value != state.value {
                    state.value = value;
                }
                state.snapshot = next.clone();
                cx.notify();
            });
        }
        self.snapshot = next;
        cx.notify();
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
        let mobile_breakpoint = self.page.mobile_breakpoint;
        let style_sheets = self.page.style_sheets.clone();
        for element in elements {
            let value = element
                .binding
                .as_ref()
                .and_then(|binding| binding.get(&self.snapshot));
            let id = element.output_id.clone().unwrap();
            self.outputs.insert(
                id,
                cx.new(|_| OutputView {
                    element,
                    value,
                    mobile_breakpoint,
                    style_sheets: style_sheets.clone(),
                    snapshot: self.snapshot.clone(),
                }),
            );
        }
    }

    fn build_output_colors(&mut self) {
        fn collect(element: &Element, inherited: Option<u32>, colors: &mut HashMap<String, u32>) {
            let color = element.style.color.or(inherited);
            if element.output_id.is_some() {
                if let (Some(binding), Some(color)) = (&element.binding, color) {
                    colors.insert(binding.name.to_owned(), color);
                }
            }
            for child in &element.children {
                if let Node::Element(child) = child {
                    collect(child, color, colors);
                }
            }
        }

        collect(&self.page.root, None, &mut self.output_colors);
    }

    fn build_brew_visualization(&mut self, cx: &mut Context<Self>) {
        fn contains_visualization(element: &Element) -> bool {
            element.tag == "brew-visualization"
                || element.children.iter().any(|child| match child {
                    Node::Element(child) => contains_visualization(child),
                    Node::Text(_) => false,
                })
        }

        if contains_visualization(&self.page.root) {
            let state = BrewVisualization::from_snapshot(&self.snapshot);
            self.brew_visualization = Some(cx.new(|_| state));
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

    fn render_node(
        &self,
        node: &Node,
        viewport_width: f32,
        dynamic_styles: &[StyleRule],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node {
            Node::Text(value) => div().child(value.clone()).into_any_element(),
            Node::Element(element) => {
                self.render_element(element, viewport_width, dynamic_styles, cx)
            }
        }
    }

    fn render_element(
        &self,
        element: &Element,
        viewport_width: f32,
        dynamic_styles: &[StyleRule],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let style = style_for_element(
            element,
            viewport_width,
            self.page.mobile_breakpoint,
            dynamic_styles,
        );
        if element.tag == "brew-visualization" {
            let visualization = self
                .brew_visualization
                .as_ref()
                .expect("brew-visualization state was not initialized")
                .clone();
            return styled_div(&style).child(visualization).into_any_element();
        }
        if let Some(id) = &element.output_id {
            return self.outputs.get(id).unwrap().clone().into_any_element();
        }
        if element.control_id.is_some() {
            let key = element.control_id.as_deref().unwrap();
            if element.tag == "input" {
                let color = style.color.or_else(|| {
                    element
                        .binding
                        .as_ref()
                        .and_then(|binding| self.output_colors.get(binding.name).copied())
                });
                return styled_div(&style)
                    .w_full()
                    .when_some(self.sliders.get(key), |container, state| {
                        let slider = Slider::new(state);
                        let slider = if let Some(color) = color {
                            slider.bg(rgb(color)).text_color(rgb(color))
                        } else {
                            slider
                        };
                        container.child(slider)
                    })
                    .into_any_element();
            }
            if element.tag == "select" {
                return styled_div(&style)
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
            if let Some(color) = style.background {
                button = button.bg(rgb(color));
            }
            if let Some(color) = style.color {
                button = button.text_color(rgb(color));
            }
            if let Some(radius) = style.border_radius {
                button = button.rounded(px(radius));
            }
            return styled_div(&style).child(button).into_any_element();
        }
        if element.tag == "option" {
            return div().into_any_element();
        }
        let mut container = styled_div(&style);
        for child in &element.children {
            container =
                container.child(self.render_node(child, viewport_width, dynamic_styles, cx));
        }
        container.into_any_element()
    }
}

impl Render for HtmlView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport_width = f32::from(window.viewport_size().width);
        let dynamic_styles =
            resolve_dynamic_styles(&self.page.style_sheets, viewport_width, &self.snapshot);
        let root = self.render_element(&self.page.root, viewport_width, &dynamic_styles, cx);
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

fn resolve_dynamic_styles(
    style_sheets: &[StyleSheet],
    viewport_width: f32,
    snapshot: &Snapshot,
) -> Vec<StyleRule> {
    let context = StyleContext {
        viewport_width,
        snapshot,
    };
    style_sheets
        .iter()
        .flat_map(|style_sheet| style_sheet(&context))
        .collect()
}

fn style_for_element(
    element: &Element,
    viewport_width: f32,
    mobile_breakpoint: Option<f32>,
    dynamic_styles: &[StyleRule],
) -> InlineStyle {
    let mut style = element
        .style
        .for_viewport(viewport_width, mobile_breakpoint);
    for rule in dynamic_styles {
        if dynamic_selector_matches(&rule.selector, element) {
            style.apply_overrides(&rule.style);
        }
    }
    if let Some(inline) = &element.style.inline {
        style.apply_overrides(inline);
    }
    style
}

fn dynamic_selector_matches(selector: &str, element: &Element) -> bool {
    selector.split(',').any(|selector| {
        let selector = selector.trim();
        if let Some(class) = selector.strip_prefix('.') {
            return element
                .attr("class")
                .is_some_and(|classes| classes.split_whitespace().any(|name| name == class));
        }
        if let Some(id) = selector.strip_prefix('#') {
            return element.attr("id") == Some(id);
        }
        if let Some((tag, class)) = selector.split_once('.') {
            return element.tag == tag
                && element
                    .attr("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|name| name == class));
        }
        if let Some((tag, id)) = selector.split_once('#') {
            return element.tag == tag && element.attr("id") == Some(id);
        }
        element.tag == selector
    })
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
    if let Some(column) = style.column {
        element = if column {
            element.flex_col()
        } else {
            element.flex_row()
        };
    }
    if let Some(flex_wrap) = style.flex_wrap {
        element = if flex_wrap {
            element.flex_wrap()
        } else {
            element.flex_nowrap()
        };
    }
    if let Some(flex_grow) = style.flex_grow {
        element = if flex_grow {
            element.flex_1()
        } else {
            element.flex_none()
        };
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
