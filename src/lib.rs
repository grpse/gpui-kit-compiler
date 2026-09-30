//! Compiler for single-file Rust components. `.inter.rs` files contain Rust tokens
//! and are consumed by `include!` in a normal Cargo build.
pub mod runtime;
use scraper::{ElementRef, Html, Node as HtmlNode, Selector};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub enum TemplateNode {
    Text(String),
    Element(TemplateElement),
}
#[derive(Clone)]
pub struct TemplateElement {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<TemplateNode>,
}
impl TemplateElement {
    pub fn new(
        tag: impl Into<String>,
        attrs: Vec<(String, String)>,
        children: Vec<TemplateNode>,
    ) -> Self {
        Self {
            tag: tag.into(),
            attrs,
            children,
        }
    }
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}
pub trait CompiledComponent {
    fn template() -> TemplateElement;
}

pub fn compile_file(input: &Path, output: &Path) -> Result<(), String> {
    if input.extension().and_then(|s| s.to_str()) != Some("rsc") {
        return Err(format!("{} is not .rsc", input.display()));
    }
    let source = fs::read_to_string(input).map_err(|e| e.to_string())?;
    let source = source.trim_start();
    let script_start = source
        .strip_prefix("<script>")
        .ok_or_else(|| format!("{} must start with <script>", input.display()))?;
    // Treat the closing tag as a whole line so Rust strings containing
    // `</script>` are copied unchanged.
    let close = script_start
        .match_indices("</script>")
        .find(|(index, tag)| {
            let before = &script_start[..*index];
            let after = &script_start[*index + tag.len()..];
            before.rsplit('\n').next().unwrap_or("").trim().is_empty()
                && after.split('\n').next().unwrap_or("").trim().is_empty()
        })
        .map(|(index, _)| index)
        .ok_or_else(|| format!("{} needs </script> on its own line", input.display()))?;
    let (script, html) = (
        &script_start[..close],
        &script_start[close + "</script>".len()..],
    );
    if html.trim().is_empty() {
        return Err(format!("{} has no HTML", input.display()));
    }
    let html = normalize_component_tags(html);
    let document = Html::parse_document(&html);
    let body = document
        .select(&Selector::parse("body").unwrap())
        .next()
        .ok_or("component needs <body>")?;
    let tree = element_code(body);
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid filename")?;
    let title = document
        .select(&Selector::parse("title").unwrap())
        .next()
        .map(|element| element.text().collect::<String>())
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| stem.to_owned());
    let struct_name = format!(
        "{}Component",
        stem.split('-')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            })
            .collect::<String>()
    );
    // Script is inserted verbatim. It can contain any Rust items, functions, and methods.
    let generated = format!(
        "// Generated Rust source from {}.\n{}\n\npub struct {};\nimpl gpui_rsc::CompiledComponent for {} {{\n    fn template() -> gpui_rsc::TemplateElement {{ {} }}\n}}\nimpl {} {{\n    pub fn definition() -> gpui_rsc::runtime::Definition {{ definition() }}\n    pub fn template() -> gpui_rsc::TemplateElement {{ <Self as gpui_rsc::CompiledComponent>::template() }}\n}}\npub fn template() -> gpui_rsc::TemplateElement {{ {}::template() }}\npub fn title() -> &'static str {{ {:?} }}\n",
        input.display(),
        script,
        struct_name,
        struct_name,
        tree,
        struct_name,
        struct_name,
        title
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if fs::read_to_string(output).ok().as_deref() != Some(&generated) {
        fs::write(output, generated).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn compile_directory(directory: &Path, output: &Path) -> Result<Vec<PathBuf>, String> {
    let mut inputs = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("rsc"))
        .collect::<Vec<_>>();
    inputs.sort();
    if inputs.is_empty() {
        return Err(format!("no .rsc files in {}", directory.display()));
    }
    let mut outputs = Vec::new();
    for input in inputs {
        let output_file = output.join(format!(
            "{}.inter.rs",
            input.file_stem().unwrap().to_string_lossy()
        ));
        compile_file(&input, &output_file)?;
        outputs.push(output_file);
    }
    Ok(outputs)
}
fn normalize_component_tags(source: &str) -> String {
    let mut out = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("<component ") {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('>') else {
            break;
        };
        let tag = &rest[..=end];
        if tag.trim_end_matches('>').trim_end().ends_with('/') {
            out.push_str(
                tag.trim_end_matches('>')
                    .trim_end()
                    .trim_end_matches('/')
                    .trim_end(),
            );
            out.push_str("></component>");
        } else {
            out.push_str(tag);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}
fn element_code(element: ElementRef<'_>) -> String {
    let attrs = element
        .value()
        .attrs()
        .map(|(k, v)| format!("({:?}.into(), {:?}.into())", k, v))
        .collect::<Vec<_>>()
        .join(",");
    let children = element
        .children()
        .filter_map(|child| match child.value() {
            HtmlNode::Text(t) if !t.trim().is_empty() => Some(format!(
                "gpui_rsc::TemplateNode::Text({:?}.into())",
                t.trim()
            )),
            HtmlNode::Element(_) => ElementRef::wrap(child)
                .map(|e| format!("gpui_rsc::TemplateNode::Element({})", element_code(e))),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "gpui_rsc::TemplateElement::new({:?}, vec![{}], vec![{}])",
        element.value().name(),
        attrs,
        children
    )
}
