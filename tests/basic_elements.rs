#![cfg(feature = "compiler")]

use std::fs;

#[test]
fn compiles_basic_element_bindings() {
    let directory =
        std::env::temp_dir().join(format!("gpui-rsc-basic-elements-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("basic.rsx");
    let output = directory.join("basic.inter.rs");
    fs::write(
        &input,
        r#"
use gpui_rsc::runtime::signal;

pub fn App() -> gpui::AnyElement {
    let photo = signal("/tmp/photo.png");
    let date = signal("2026-10-02");
    let name = signal("Ada");
    let notes = signal("hello");
    let enabled = signal(true);
    let count = signal(0.5);
    <div>
        <img src={photo} alt="A photo" width="120" height="80" object-fit="cover" />
        <input id="day" type="date" value={date} />
        <input id="name" type="text" value={name} placeholder="Name" />
        <textarea id="notes" value={notes}></textarea>
        <input id="enabled" type="checkbox" checked={enabled} />
        <progress id="count" value={count} max="1"></progress>
    </div>
}
"#,
    )
    .unwrap();
    gpui_rsc::compile_file(&input, &output).unwrap();
    let generated = fs::read_to_string(&output).unwrap();
    syn::parse_file(&generated).unwrap();
    for expected in [
        "data-in",
        "data-rsc-value-binding",
        "render_image",
        "render_input",
        "render_textarea",
        "render_progress",
    ] {
        assert!(generated.contains(expected), "missing {expected}");
    }
    let _ = fs::remove_dir_all(directory);
}
