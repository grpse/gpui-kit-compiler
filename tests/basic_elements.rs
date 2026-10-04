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
    let clip = signal("/tmp/clip.mp4");
    let date = signal("2026-10-02");
    let name = signal("Ada");
    let notes = signal("hello");
    let enabled = signal(true);
    let count = signal(0.5);
    <div>
        <img src={photo} alt="A photo" width="120" height="80" object-fit="cover" />
        <video id="clip" src={clip} controls poster="/tmp/poster.png" width="320" height="180"></video>
        <video id="alternate" controls><source src="/tmp/alternate.mp4" /></video>
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
        "render_video",
        "render_input",
        "render_textarea",
        "render_progress",
    ] {
        assert!(generated.contains(expected), "missing {expected}");
    }
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn typography_and_inline_units_compile_to_native_gpui() {
    let directory =
        std::env::temp_dir().join(format!("gpui-rsc-typography-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("type.rsx");
    let output = directory.join("type.rs");
    fs::write(
        &input,
        r#"
pub fn App(dose: f32, grind: f32) -> gpui::AnyElement {
    let myStyles = styles({
        body: {
            font: gpui::font("sans-serif"),
            fontFamily: "sans-serif",
            fontSize: 14.0,
            fontWeight: gpui::FontWeight::MEDIUM,
            fontStyle: "italic",
            fontFeatures: gpui::FontFeatures::default(),
            lineHeight: gpui::relative(1.4)
        },
        caption: { fontWeight: 600, lineHeight: gpui::px(20.0) }
    });
    <div class={myStyles.body}>
        <div>{dose} g</div>
        <div>{grind}/10</div>
        <div>Amount: {dose} {grind} g</div>
        <div>Before <div class={myStyles.caption}>Nested</div> after</div>
    </div>
}
"#,
    )
    .unwrap();
    gpui_rsc::compile_file(&input, &output).unwrap();
    let generated =
        prettyplease::unparse(&syn::parse_file(&fs::read_to_string(output).unwrap()).unwrap());
    let compact = generated
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    for expected in [
        ".font(gpui::font(\"sans-serif\"))",
        ".font_family(\"sans-serif\")",
        ".font_weight(gpui::FontWeight::MEDIUM)",
        "font_style = Some(gpui::FontStyle::Italic)",
        ".font_features(gpui::FontFeatures::default())",
        ".line_height(gpui::relative(1.4))",
        ".line_height(gpui::px(20.0))",
        "view.inline_text(element, 0, 2)",
        "view.inline_text(element, 0, 5)",
        "view.inline_text(element, 2, 3)",
        "\" g\"",
        "\"/10\"",
        "\" \"",
    ] {
        let expected_compact = expected
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        assert!(
            if expected.starts_with('"') {
                generated.contains(expected)
            } else {
                compact.contains(&expected_compact)
            },
            "missing {expected}"
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn layout_styles_are_explicit_and_structural_wrappers_fill_the_parent() {
    let directory = std::env::temp_dir().join(format!("gpui-rsc-flow-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("flow.rsx");
    let output = directory.join("flow.rs");
    fs::write(
        &input,
        r#"
pub fn Child() -> gpui::AnyElement { <div>Child</div> }
pub fn App(section: &str) -> gpui::AnyElement {
    let myStyles = styles({
        page: { minHeight: context.viewport_height, flexShrink: 0, aspectRatio: 1.5 },
        label: { whiteSpace: "nowrap", textOverflow: "ellipsis", lineClamp: 2 },
        overlay: { position: "absolute", top: 8.0, left: 12.0 },
        flow: { left: 99.0, position: "static", top: 99.0 }
    });
    <div class={myStyles.page}>
        <button id="action" on-click={save()} class={myStyles.label}>Long button label</button>
        {if section == "child" { <Child /> } else { <div>No child</div> }}
        <div class={myStyles.overlay}>Overlay</div>
    </div>
}
fn save() {}
"#,
    )
    .unwrap();
    gpui_rsc::compile_file(&input, &output).unwrap();
    let generated = fs::read_to_string(&output).unwrap();
    syn::parse_file(&generated).unwrap();
    let compact = generated
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    for expected in [
        "gpui_rsc::runtime::view::element_container()",
        "container=container.w_full().flex().flex_col()",
        "viewport_height:view.content_height(window)",
        "bound_class_style.position==Some(gpui::Position::Absolute)",
        "container.style().max_size.width=None",
        ".min_h(gpui::px(context.viewport_height))",
        ".flex_shrink((0)asf32)",
        ".aspect_ratio(1.5)",
        "white_space=Some(gpui::WhiteSpace::Nowrap)",
        "text_overflow=Some(gpui::TextOverflow::Truncate",
        ".line_clamp(2)",
    ] {
        assert!(compact.contains(expected), "missing {expected}");
    }
    assert!(!compact.contains(".left(gpui::px(99.0))"));
    assert!(!compact.contains(".top(gpui::px(99.0))"));
    fs::write(
        &input,
        r#"
pub fn App() -> gpui::AnyElement {
    let myStyles = styles({ item: { position: "relative", left: 20.0 } });
    <div class={myStyles.item}>Offset</div>
}
"#,
    )
    .unwrap();
    let error = gpui_rsc::compile_file(&input, &output).unwrap_err();
    assert!(error.contains("requires position: \"absolute\""), "{error}");
    fs::remove_dir_all(directory).unwrap();
}
