#![cfg(feature = "compiler")]

use std::{fs, process::Command};

#[test]
fn converted_functions_compile_and_run_as_regular_rust() {
    let source = r##"
#[derive(Debug)]
struct Node { text: String }
impl Node {
    fn new(_id: &str, text: &str) -> Self { Self { text: text.into() } }
    fn child(mut self, child: impl std::fmt::Display) -> Self {
        self.text.push_str(&child.to_string()); self
    }
    fn flex(self) -> Self { self }
    fn gap_2(self) -> Self { self }
    fn children(mut self, children: impl Iterator<Item = Node>) -> Self {
        for child in children { self.text.push_str(&child.text); } self
    }
}
impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.text.fmt(f) }
}
mod kit { pub(super) fn div() -> super::Node { super::Node::new("", "") } }
fn badge<T: std::fmt::Display>(value: T) -> Node {
    // <Broken tags inside comments must not be interpreted.
    /* <Broken /* nested */ tags here too. */
    let example = r#"<Fake args={bad}>not markup</Fake>"#;
    assert!(example.starts_with("<Fake"));
    let _ = Vec::<u8>::new();
    let _ = <std::string::String>::new();
    let _ = <Node as ToString>::to_string(&kit::div());
    let text = if 1 < 2 { "ok" } else { "no" };
    <kit::div flex gap-2 children={(0..2).map(|n| <Node args={("row", "")} >{n}</Node>)}>
        <Node args={("label", "hello")} />
        <Node ctor={Node::new("custom", " world as text")} />
        {if text == "ok" { <kit::div>{value}</kit::div> } else { <kit::div /> }}
        "literal <tag> {braces}"
    </kit::div>
}
struct View;
impl View {
    fn render(&mut self) -> Node { <kit::div>{badge(7)}</kit::div> }
}
fn main() {
    assert_eq!(View.render().text, "01hello world as text7literal <tag> {braces}");
}
"##;
    let converted = gpui_rsc::convert_source(source).unwrap();
    assert!(converted.contains("fn badge<T: std::fmt::Display>(value: T) -> Node"));
    assert!(!converted.contains("gpui_rsc::"));
    let directory = std::env::temp_dir().join(format!("rsc-native-rustc-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let output = directory.join("converted.rs");
    let binary = directory.join("converted");
    fs::write(&output, converted).unwrap();
    let result = Command::new("rustc")
        .arg("--edition=2024")
        .arg(&output)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(Command::new(&binary).status().unwrap().success());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn marked_functions_can_mix_with_stateful_components() {
    let directory = std::env::temp_dir().join(format!("rsc-native-mixed-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("mixed.rsx");
    let output = directory.join("mixed.rs");
    fs::write(
        &input,
        r#"
#[gpui]
pub fn badge(message: &str) -> impl gpui::IntoElement {
    <gpui::div>{message.to_owned()}</gpui::div>
}
pub fn App() -> gpui::AnyElement { <div>Stateful</div> }
"#,
    )
    .unwrap();
    gpui_rsc::compile_file(&input, &output).unwrap();
    let converted = fs::read_to_string(&output).unwrap();
    let converted = prettyplease::unparse(&syn::parse_file(&converted).unwrap());
    assert!(converted.contains("pub fn badge(message: &str) -> impl gpui::IntoElement"));
    assert!(converted.contains("pub fn App() -> gpui_rsc::runtime::Definition"));
    assert!(!converted.contains("#[gpui]"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_only_project_files_keep_signatures() {
    let directory = std::env::temp_dir().join(format!("rsc-native-only-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("native.rsx");
    let output = directory.join("native.rs");
    fs::write(&input, "#[gpui]\nfn badge<T: ToString>(value: T) -> impl gpui::IntoElement { <gpui::div>{value.to_string()}</gpui::div> }").unwrap();
    gpui_rsc::compile_file(&input, &output).unwrap();
    let converted = fs::read_to_string(&output).unwrap();
    assert!(converted.contains("fn badge<T: ToString>(value: T) -> impl gpui::IntoElement"));
    assert!(!converted.contains("Definition"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_tags_produce_conversion_errors() {
    for (source, expected) in [
        ("fn f() { <div></span> }", "expected </div>"),
        ("fn f() { <div args={1} ctor={div()} /> }", "at most one"),
        ("fn f() { <div padding=8 /> }", "braces or a quoted string"),
        ("#[gpui] struct Broken;", "needs a body"),
    ] {
        let error = gpui_rsc::convert_source(source).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}
