use gpui_kit::AppContext as _;
#[path = "../../support/mod.rs"]
mod support;
mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/rsc-build/generated.rs"
    ));
}
fn main() {
    support::launch("Video Editor", |window, cx| {
        cx.new(|cx| generated::ui::Editor::new(window, cx))
    });
}
