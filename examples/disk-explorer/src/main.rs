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
    support::launch("Disk Explorer", |window, cx| {
        cx.new(|cx| generated::ui::Explorer::new(window, cx))
    });
}
