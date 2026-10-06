use gpui_kit::*;
use gpui_rsc::runtime::{Lifecycle, LifecycleEvent, LifecycleHandler};

#[allow(dead_code)]
pub fn bytes(size: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut number = size as f64;
    let mut unit = 0;
    while number >= 1024. && unit < UNITS.len() - 1 {
        number /= 1024.;
        unit += 1;
    }
    if unit == 0 {
        format!("{size} B")
    } else {
        format!("{number:.1} {}", UNITS[unit])
    }
}

#[allow(dead_code)]
pub fn initial_directory() -> std::path::PathBuf {
    std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()))
}

pub fn launch<V: Render>(
    title: &'static str,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V> + 'static,
) {
    launch_with_lifecycle(title, handle_lifecycle, build);
}

/// Customize this function or pass your own handler to `launch_with_lifecycle`.
fn handle_lifecycle(event: LifecycleEvent) {
    if let LifecycleEvent::UnexpectedQuit { reason } = event {
        eprintln!("application quit unexpectedly: {reason}");
    }
}

pub fn launch_with_lifecycle<V: Render>(
    title: &'static str,
    handler: LifecycleHandler,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V> + 'static,
) {
    // Native RSX is checked by Rust; validation must not initialize AppKit.
    if std::env::args().nth(1).as_deref() == Some("--validate") {
        return;
    }
    Lifecycle::new(handler).run(
        gpui_kit::application().with_assets(gpui_kit::assets::Assets),
        move |cx, lifecycle| {
            gpui_kit::init(cx);
            lifecycle
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(1220.), px(820.)),
                            cx,
                        ))),
                        window_min_size: Some(size(px(820.), px(560.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some(title.into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    cx,
                    build,
                )
                .expect("open example window");
        },
    );
}
