use gpui_kit::{prelude::*, *};
use gpui_kit::component::label::Label;
use gpui_kit::base::Disableable;
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

#[allow(non_snake_case)]
pub fn VideoPanel() -> Definition {
    Definition::native("video-panel", vec![], &[], render_video_panel)
}

fn render_video_panel(
    view: &mut HtmlView,
    _props: &ComponentProps,
    _viewport_width: f32,
    window: &mut Window,
    cx: &mut Context<HtmlView>,
) -> AnyElement {
    use gpui_rsc::runtime::binding::Element;
    use gpui_kit::component::button::Button;
    const ID: &str = "local-video";
    let source = view.video_source(ID, cx);
    let filename = std::path::Path::new(&source).file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "No video selected".into());
    let element = Element {
        tag: "video".into(),
        attrs: [("id", ID), ("src", source.as_str()), ("width", "960"), ("height", "360")]
            .into_iter().map(|(key, value)| (key.into(), value.into())).collect(),
        render: None, children: vec![], binding: None, args: vec![],
        control_id: Some(ID.into()), component: None,
    };
    let mut style = gpui_rsc::runtime::InlineStyle::default();
    style.size.width = Some(gpui::relative(1.0).into());
    let video = view.render_video(&element, style, window, cx);
    let choose = Button::new("choose-video").label("Choose video…")
        .on_click(cx.listener(|_, _, _, cx| {
            let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true, directories: false, multiple: false,
                prompt: Some("Choose a video file".into()),
            });
            cx.spawn(async move |this, cx| {
                match paths.await {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            let source = path.to_string_lossy().into_owned();
                            let _ = this.update(cx, |view, cx| view.set_video_source(ID, source, cx));
                        }
                    }
                    Ok(Err(error)) => {
                        let _ = this.update(cx, |_, cx| {
                            cx.notify();
                            eprintln!("Unable to choose video: {error}");
                        });
                    }
                    _ => {}
                }
            }).detach();
        }));
    let mut controls = div().flex().flex_wrap().gap_2().child(choose);
    for (action, label) in [("play", "Play / Resume"), ("pause", "Pause"), ("stop", "Stop")] {
        controls = controls.child(Button::new(action).label(label).disabled(source.is_empty())
            .on_click(cx.listener(move |view, _, window, cx| view.video_playback(ID, action, window, cx))));
    }
    div().flex().flex_col().gap_3().w_full()
        .child(video).child(div().w_full().min_w_0().truncate().child(filename)).child(controls).into_any_element()
}
