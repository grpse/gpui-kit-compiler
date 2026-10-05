#[path = "../../support/mod.rs"]
mod support;
use rsx_video_editor::{preprocess, state};
mod dock_skin;
mod editor;
mod graph_editor;
mod interactions;
mod workspace;
mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/rsc-build/generated.rs"
    ));
}
fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--prove-composition")
    {
        let result = arguments
            .get(index + 1)
            .ok_or_else(|| "Usage: --prove-composition NEW_OUTPUT_DIRECTORY".to_string())
            .and_then(|path| rsx_video_editor::proof::run_composition(std::path::Path::new(path)));
        match result {
            Ok(path) => println!("Composition proof: {}", path.display()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--prove-playback")
    {
        let result = arguments
            .get(index + 1)
            .zip(arguments.get(index + 2))
            .ok_or_else(|| "Usage: --prove-playback SOURCE OUTPUT_DIRECTORY".to_string())
            .and_then(|(source, output)| {
                rsx_video_editor::proof::run(
                    std::path::Path::new(source),
                    std::path::Path::new(output),
                )
            });
        match result {
            Ok(path) => println!("Playback proof: {}", path.display()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
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
