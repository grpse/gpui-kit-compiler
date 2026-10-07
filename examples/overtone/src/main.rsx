use gpui_kit::component::TitleBar;
use gpui_kit::*;
pub use rsx_overtone::{model, persistence, storage};
mod studio {
    include!(concat!(env!("OUT_DIR"), "/studio.rs"));
}
mod primitives {
    include!(concat!(env!("OUT_DIR"), "/primitives.rs"));
}
mod panels {
    include!(concat!(env!("OUT_DIR"), "/panels.rs"));
}
mod settings {
    include!(concat!(env!("OUT_DIR"), "/settings.rs"));
}
mod workspace {
    include!(concat!(env!("OUT_DIR"), "/workspace.rs"));
}
#[cfg(all(test, feature = "ui-tests"))]
mod ui_tests {
    include!(concat!(env!("OUT_DIR"), "/ui_tests.rs"));
}

fn main() {
    if std::env::args().any(|arg| arg == "--validate") {
        return;
    }
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    gpui_rsc::runtime::Lifecycle::new(|event| {
        if let gpui_rsc::runtime::LifecycleEvent::UnexpectedQuit { reason } = event {
            eprintln!("Overtone: {reason}");
        }
    })
    .run(
        gpui_kit::application().with_assets(gpui_kit::assets::Assets),
        move |cx, lifecycle| {
            gpui_kit::init(cx);
            cx.set_menus(workspace::application_menus());
            lifecycle
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(1480.), px(960.)),
                            cx,
                        ))),
                        window_min_size: Some(size(px(1000.), px(680.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Overtone · Composition studio".into()),
                            ..TitleBar::title_bar_options()
                        }),
                        window_decorations: Some(WindowDecorations::Client),
                        is_resizable: true,
                        is_minimizable: true,
                        ..TitleBar::window_options()
                    },
                    cx,
                    |window, cx| cx.new(|cx| studio::Studio::new(path, window, cx)),
                )
                .expect("open Overtone");
        },
    );
}
