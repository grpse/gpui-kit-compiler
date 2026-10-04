use gpui_kit::*;

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
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            gpui_kit::open_window(
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
        });
}
