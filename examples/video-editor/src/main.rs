#[path = "../../support/mod.rs"]
mod support;
use rsx_video_editor::state;
mod dock_skin;
mod editor;
mod workspace;
mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/rsc-build/generated.rs"
    ));
}
fn main() {
    if std::env::args().any(|arg| arg == "--validate") {
        return;
    }
    if std::env::args().any(|arg| arg == "--legacy") {
        support::launch("Video Editor", |window, cx| {
            cx.new(|cx| generated::legacy::Editor::new(window, cx))
        });
        return;
    }
    use gpui_kit::component::{Theme, ThemeMode};
    use gpui_kit::*;
    gpui_rsc::runtime::Lifecycle::new(|event| {
        if let gpui_rsc::runtime::LifecycleEvent::UnexpectedQuit { reason } = event {
            eprintln!("FlowCut quit unexpectedly: {reason}");
        }
    })
    .run(
        gpui_kit::application().with_assets(gpui_kit::assets::AllAssets),
        |cx, lifecycle| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            Theme::update(cx, |theme| {
                theme.colors.primary = rgb(0x7c4dff).into();
                theme.colors.slider_bar = rgb(0x7c4dff).into();
                theme.colors.slider_thumb = rgb(0xdeddf7).into();
            });
            lifecycle
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(1536.), px(980.)),
                            cx,
                        ))),
                        window_min_size: Some(size(px(1100.), px(740.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("FlowCut".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    cx,
                    generated::ui::entry,
                )
                .expect("open FlowCut");
        },
    );
}
