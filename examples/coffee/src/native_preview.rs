//! A native GPUI component embedded in RSX through an uppercase component tag.
//! The equipment art is kept as SVG data, while recipe state and motion stay in Rust.
use std::{
    sync::{Arc, LazyLock},
    time::Duration,
};

use gpui::*;
use gpui_rsc::runtime::{ComponentProps, Definition, Value, view::HtmlView};

struct MethodArt {
    title: &'static str,
    process: &'static str,
    description: &'static str,
    equipment: &'static str,
    image: Arc<Image>,
    drop_x: f32,
    drop_y: f32,
    drop_travel: f32,
}

static V60_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("art/v60.svg").to_vec(),
    ))
});
static FRENCH_PRESS_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("art/french-press.svg").to_vec(),
    ))
});
static AEROPRESS_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("art/aeropress.svg").to_vec(),
    ))
});
static ESPRESSO_IMAGE: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("art/espresso.svg").to_vec(),
    ))
});

fn art_for(method: &str) -> MethodArt {
    match method {
        "French press" => MethodArt {
            title: "French press",
            process: "IMMERSION / METAL FILTER",
            description: "A mesh plunger separates the brewed coffee from its grounds.",
            equipment: "BEAKER  ·  MESH  ·  PLUNGER",
            image: FRENCH_PRESS_IMAGE.clone(),
            drop_x: 170.0,
            drop_y: 85.0,
            drop_travel: 45.0,
        },
        "AeroPress" => MethodArt {
            title: "AeroPress",
            process: "PRESSURE / PAPER FILTER",
            description: "A sealed plunger presses coffee through a filter into the mug.",
            equipment: "PLUNGER  ·  CHAMBER  ·  FILTER CAP",
            image: AEROPRESS_IMAGE.clone(),
            drop_x: 179.0,
            drop_y: 177.0,
            drop_travel: 34.0,
        },
        "Espresso" => MethodArt {
            title: "Espresso",
            process: "PRESSURE / FINE GRIND",
            description: "The group head drives water through a puck in the portafilter.",
            equipment: "GROUP HEAD  ·  PORTAFILTER  ·  CUP",
            image: ESPRESSO_IMAGE.clone(),
            drop_x: 177.0,
            drop_y: 158.0,
            drop_travel: 45.0,
        },
        _ => MethodArt {
            title: "V60 pour-over",
            process: "GRAVITY / PAPER FILTER",
            description: "Water crosses the ribbed cone and drips into the glass server.",
            equipment: "KETTLE  ·  CONE  ·  SERVER",
            image: V60_IMAGE.clone(),
            drop_x: 179.0,
            drop_y: 143.0,
            drop_travel: 34.0,
        },
    }
}

fn number(props: &ComponentProps, key: &str, default: f32) -> f32 {
    props.get(key).and_then(Value::number).unwrap_or(default)
}

fn render(
    _view: &mut HtmlView,
    props: &ComponentProps,
    _viewport_width: f32,
    _window: &mut Window,
    _cx: &mut Context<HtmlView>,
) -> AnyElement {
    let method = match props.get("method") {
        Some(Value::Text(value)) => value.as_str(),
        _ => "V60",
    };
    let art = art_for(method);
    let water = number(props, "water", 300.0);
    let intensity = number(props, "intensity", 50.0);
    let bitterness = number(props, "bitterness", 50.0);
    let body = number(props, "body", 50.0);
    let strength = ((intensity * 0.5 + bitterness * 0.3 + body * 0.2) / 100.0).clamp(0.0, 1.0);
    let brew_color = if strength > 0.65 {
        rgb(0x754024)
    } else if strength > 0.40 {
        rgb(0x9a5b32)
    } else {
        rgb(0xc17b42)
    };
    let water_fraction = ((water - 30.0) / 570.0).clamp(0.05, 1.0);

    div()
        .flex()
        .flex_col()
        .w_full()
        .min_w_0()
        .max_w(px(450.0))
        .flex_shrink_0()
        .p(px(20.0))
        .gap(px(16.0))
        .rounded(px(18.0))
        .border_1()
        .border_color(rgb(0x483a30))
        .bg(rgb(0x211c18))
        .text_color(rgb(0xf4ece1))
        .child(
            div()
                .flex()
                .justify_between()
                .flex_wrap()
                .items_start()
                .gap(px(12.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .max_w_full()
                        .flex_1()
                        .gap(px(5.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(0xd9a46c))
                                .child(art.process),
                        )
                        .child(
                            div()
                                .text_size(px(22.0))
                                .font_weight(FontWeight::BOLD)
                                .child(art.title),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(0xb9a89a))
                                .child(art.description),
                        ),
                )
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(6.0))
                        .rounded_full()
                        .bg(rgb(0x3b3028))
                        .text_size(px(10.0))
                        .text_color(rgb(0xe4b883))
                        .child("LIVE VIEW"),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex_shrink_0()
                .p(px(6.0))
                .rounded(px(14.0))
                .border_1()
                .border_color(rgb(0xd9c8b3))
                .bg(rgb(0xf3e8d7))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .relative()
                        .w_full()
                        .max_w(px(360.0))
                        .aspect_ratio(360.0 / 260.0)
                        .flex_shrink_0()
                        .child(img(art.image).absolute().size_full().object_fit(ObjectFit::Contain))
                        .child(
                            div()
                                .absolute()
                                .left(relative(art.drop_x / 360.0))
                                .top(relative(art.drop_y / 260.0))
                                .w(relative(6.0 / 360.0))
                                .h(relative(10.0 / 260.0))
                                .rounded_full()
                                .bg(brew_color)
                                .with_animation(
                                    "extraction-flow",
                                    Animation::new(Duration::from_millis(1250)).repeat(),
                                    move |dot, phase| {
                                        dot.top(relative((art.drop_y + phase * art.drop_travel) / 260.0))
                                            .opacity((1.0 - phase).max(0.08))
                                    },
                                ),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xb7a79a))
                        .child(art.equipment),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(11.0))
                        .child(div().text_color(rgb(0xdcc7b1)).child("RECIPE WATER"))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("{water:.0} ml")),
                        ),
                )
                .child(
                    div()
                        .h(px(7.0))
                        .w_full()
                        .rounded_full()
                        .bg(rgb(0x46372d))
                        .child(
                            div().h_full().rounded_full().bg(brew_color).with_spring(
                                "water-level",
                                SpringAnimation::new(SpringConfig::new(160.0, 20.0, 1.0))
                                    .to(water_fraction),
                                |bar, fraction| bar.w(relative(fraction)),
                            ),
                        ),
                ),
        )
        .into_any_element()
}

#[allow(non_snake_case)]
pub fn ExtractionIllustration() -> Definition {
    Definition::native(
        "extraction-illustration",
        vec![
            gpui_rsc::in_param!("method"),
            gpui_rsc::in_param!("water"),
            gpui_rsc::in_param!("intensity"),
            gpui_rsc::in_param!("bitterness"),
            gpui_rsc::in_param!("body"),
        ],
        &["method", "water", "intensity", "bitterness", "body"],
        render,
    )
}
