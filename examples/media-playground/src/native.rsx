use gpui_kit::{prelude::*, *};
use gpui_kit::component::label::Label;
use gpui_rsc::runtime::{ComponentProps, Definition, view::HtmlView};

// This directive keeps the signature and lowers tags to ordinary Rust calls.
#[gpui]
pub fn native_badge(message: &str) -> impl IntoElement {
    <gpui_kit::div flex items-center gap-2 p-3 rounded-lg bg={rgb(0x19222e)}>
        <Label args={message.to_owned()} />
        <gpui_kit::div text-sm text-color={rgb(0x60d3c0)}>
            Constructors + builder methods
        </gpui_kit::div>
    </gpui_kit::div>
}

#[allow(non_snake_case)]
pub fn NativeBadge() -> Definition {
    Definition::native("native-badge", vec![], &[], render_badge)
}

fn render_badge(
    _view: &mut HtmlView,
    _props: &ComponentProps,
    _viewport_width: f32,
    _window: &mut Window,
    _cx: &mut Context<HtmlView>,
) -> AnyElement {
    native_badge("Native GPUI Kit element").into_any_element()
}
