mod generated {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/rsc-build/generated.rs"
    ));
}

use gpui_rsc::runtime::{self, StartupConfig};

fn main() {
    runtime::run_with_config(
        generated::app::App(),
        StartupConfig {
            width: 1120.0,
            height: 780.0,
            min_width: Some(520.0),
            min_height: Some(480.0),
            ..StartupConfig::default()
        },
    );
}
