//! Binds a compiler-generated element tree to application state and GPUI Kit.
use crate::runtime::{Binding, Definition, Direction, Snapshot, Value};

use crate::{TemplateElement, TemplateNode};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub enum Node {
    Text(String),
    Element(Element),
}
#[derive(Clone)]
pub struct Element {
    pub tag: String,
    pub attrs: HashMap<String, String>,
    pub style: InlineStyle,
    pub children: Vec<Node>,
    pub binding: Option<Binding>,
    pub args: Vec<Binding>,
    pub control_id: Option<String>,
    pub output_id: Option<String>,
}
impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }
    pub fn text_content(&self) -> String {
        fn walk(nodes: &[Node], out: &mut String) {
            for node in nodes {
                match node {
                    Node::Text(v) => out.push_str(v),
                    Node::Element(e) => walk(&e.children, out),
                }
            }
        }
        let mut out = String::new();
        walk(&self.children, &mut out);
        out.trim().to_owned()
    }
}
#[derive(Clone)]
pub enum Control {
    Range {
        id: String,
        binding: Binding,
        min: f32,
        max: f32,
        step: f32,
        default: f32,
    },
    Select {
        id: String,
        binding: Binding,
        options: Vec<String>,
        default: String,
    },
}
impl Control {
    pub fn id(&self) -> &str {
        match self {
            Self::Range { id, .. } | Self::Select { id, .. } => id,
        }
    }
    pub fn binding(&self) -> &Binding {
        match self {
            Self::Range { binding, .. } | Self::Select { binding, .. } => binding,
        }
    }
    pub fn default_value(&self) -> Value {
        match self {
            Self::Range { default, .. } => Value::Number(*default),
            Self::Select { default, .. } => Value::Text(default.clone()),
        }
    }
}
#[derive(Clone)]
pub struct Page {
    pub root: Element,
    pub controls: Vec<Control>,
    pub defaults: HashMap<String, Value>,
}

pub fn compile(def: &Definition) -> Result<Page, String> {
    let mut controls = Vec::new();
    let mut seen = HashSet::new();
    let scope = def
        .bindings
        .iter()
        .map(|b| (b.name.to_owned(), b.clone()))
        .collect();
    let mut root = compile_component(def, &scope, def.name, &mut controls, &mut seen)?;
    assign_output_ids(&mut root, &mut 0);
    let defaults = controls
        .iter()
        .filter_map(|c| {
            c.binding()
                .data_key
                .map(|k| (k.to_owned(), c.default_value()))
        })
        .collect::<HashMap<_, _>>();
    Ok(Page {
        root,
        controls,
        defaults,
    })
}
fn assign_output_ids(element: &mut Element, next: &mut usize) {
    if element.attr("data-in").is_some() {
        element.output_id = Some(format!("output-{next}"));
        *next += 1;
    }
    for child in &mut element.children {
        if let Node::Element(child) = child {
            assign_output_ids(child, next);
        }
    }
}
fn compile_component(
    def: &Definition,
    scope: &HashMap<String, Binding>,
    path: &str,
    controls: &mut Vec<Control>,
    seen: &mut HashSet<String>,
) -> Result<Element, String> {
    compile_element(&def.template, def, scope, path, controls, seen)
}
fn resolve_expr(
    expr: &str,
    target: &'static str,
    direction: Direction,
    scope: &HashMap<String, Binding>,
) -> Result<Binding, String> {
    let text = expr.trim();
    let inner = text
        .strip_prefix('[')
        .and_then(|x| x.strip_suffix(']'))
        .ok_or_else(|| format!("parameter {target} needs a [value expression]"))?
        .trim();
    if let Some(binding) = scope.get(inner) {
        return binding.alias(target, direction);
    }
    if direction.writes() {
        return Err(format!(
            "writable parameter {target} must reference a writable parent binding; got {expr}"
        ));
    }
    if let Some(s) = inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Ok(Binding::constant(target, Value::Text(s.to_owned())));
    } else if let Some(s) = inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Ok(Binding::constant(target, Value::Text(s.to_owned())));
    }
    let mut parser = ExpressionParser {
        input: inner.as_bytes(),
        pos: 0,
        scope,
    };
    let expression = parser.sum()?;
    parser.skip_space();
    if parser.pos != parser.input.len() {
        return Err(format!("invalid expression {expr}"));
    }
    Ok(Binding::read_with(target, move |snapshot| {
        expression.eval(snapshot).map(Value::Number)
    }))
}

#[derive(Clone)]
enum NumericExpr {
    Number(f32),
    Source(Binding),
    Binary(char, Box<NumericExpr>, Box<NumericExpr>),
}
impl NumericExpr {
    fn eval(&self, snapshot: &Snapshot) -> Option<f32> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Source(binding) => binding.get(snapshot)?.number(),
            Self::Binary(op, left, right) => {
                let (a, b) = (left.eval(snapshot)?, right.eval(snapshot)?);
                let value = match op {
                    '+' => a + b,
                    '-' => a - b,
                    '*' => a * b,
                    '/' if b != 0.0 => a / b,
                    _ => return None,
                };
                value.is_finite().then_some(value)
            }
        }
    }
}
struct ExpressionParser<'a> {
    input: &'a [u8],
    pos: usize,
    scope: &'a HashMap<String, Binding>,
}
impl ExpressionParser<'_> {
    fn skip_space(&mut self) {
        while self
            .input
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.pos += 1;
        }
    }
    fn take(&mut self, value: u8) -> bool {
        self.skip_space();
        if self.input.get(self.pos) == Some(&value) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn sum(&mut self) -> Result<NumericExpr, String> {
        let mut expr = self.product()?;
        loop {
            let op = if self.take(b'+') {
                '+'
            } else if self.take(b'-') {
                '-'
            } else {
                break;
            };
            expr = NumericExpr::Binary(op, Box::new(expr), Box::new(self.product()?));
        }
        Ok(expr)
    }
    fn product(&mut self) -> Result<NumericExpr, String> {
        let mut expr = self.atom()?;
        loop {
            let op = if self.take(b'*') {
                '*'
            } else if self.take(b'/') {
                '/'
            } else {
                break;
            };
            expr = NumericExpr::Binary(op, Box::new(expr), Box::new(self.atom()?));
        }
        Ok(expr)
    }
    fn atom(&mut self) -> Result<NumericExpr, String> {
        self.skip_space();
        if self.take(b'-') {
            return Ok(NumericExpr::Binary(
                '-',
                Box::new(NumericExpr::Number(0.0)),
                Box::new(self.atom()?),
            ));
        }
        if self.take(b'(') {
            let expr = self.sum()?;
            if !self.take(b')') {
                return Err("missing closing parenthesis".into());
            }
            return Ok(expr);
        }
        let start = self.pos;
        while self
            .input
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'.')
        {
            self.pos += 1;
        }
        if start == self.pos {
            return Err("expected a number or readable binding".into());
        }
        let token = std::str::from_utf8(&self.input[start..self.pos]).map_err(|e| e.to_string())?;
        if let Ok(n) = token.parse() {
            return Ok(NumericExpr::Number(n));
        }
        let binding = self
            .scope
            .get(token)
            .ok_or_else(|| format!("unknown binding {token}"))?
            .alias("expression", Direction::In)?;
        Ok(NumericExpr::Source(binding))
    }
}
fn compile_element(
    element: &TemplateElement,
    def: &Definition,
    scope: &HashMap<String, Binding>,
    path: &str,
    controls: &mut Vec<Control>,
    seen: &mut HashSet<String>,
) -> Result<Element, String> {
    let tag = element.tag.clone();
    let attrs: HashMap<String, String> = element.attrs.iter().cloned().collect();
    let attr = |k: &str| attrs.get(k).map(String::as_str);
    if tag == "component" {
        let name = attr("name").ok_or("<component> needs name")?;
        let child = def
            .imports
            .iter()
            .find(|d| d.name == name)
            .ok_or_else(|| format!("{name} is not imported by the {} .rsc script", def.name))?;
        let mut child_scope = HashMap::new();
        for parameter in &child.bindings {
            let expr = attr(parameter.name)
                .ok_or_else(|| format!("<component name=\"{name}\"> needs {}", parameter.name))?;
            child_scope.insert(
                parameter.name.to_owned(),
                resolve_expr(expr, parameter.name, parameter.direction, scope)?,
            );
        }
        for key in attrs.keys() {
            if key != "name" && key != "style" && !child_scope.contains_key(key) {
                return Err(format!("unknown parameter {key} on {name}"));
            }
        }
        let instance = format!("{path}/{name}-{}", seen.len());
        let child_root = compile_component(child, &child_scope, &instance, controls, seen)?;
        return Ok(Element {
            tag: "div".into(),
            style: InlineStyle::parse(attr("style").unwrap_or("")),
            attrs,
            children: vec![Node::Element(child_root)],
            binding: None,
            args: vec![],
            control_id: None,
            output_id: None,
        });
    }
    let bindings = ["data-in", "data-out", "data-in-out"]
        .iter()
        .filter_map(|k| attr(k).map(|v| (*k, v)))
        .collect::<Vec<_>>();
    if bindings.len() > 1 {
        return Err(format!("<{tag}> has multiple binding directions"));
    }
    let mut binding = None;
    let mut control_id = None;
    let mut args = Vec::new();
    if let Some((kind, name)) = bindings.first().copied() {
        let source = scope
            .get(name)
            .ok_or_else(|| format!("unknown {kind}=\"{name}\" in {}", def.name))?;
        let direction = match kind {
            "data-in" => Direction::In,
            "data-out" => Direction::Out,
            _ => Direction::InOut,
        };
        let source = source.alias(source.name, direction)?;
        if direction == Direction::InOut || (direction == Direction::Out && tag != "button") {
            let id = format!("{path}/{name}");
            if !seen.insert(id.clone()) {
                return Err(format!("duplicate control {id}"));
            }
            match tag.as_str() {
                "input" if attr("type") == Some("range") => {
                    let parse = |key: &str| -> Result<f32, String> {
                        attr(key)
                            .ok_or_else(|| format!("{id} needs {key}"))?
                            .parse()
                            .map_err(|_| format!("{id} has invalid {key}"))
                    };
                    let (min, max, step, default) = (
                        parse("min")?,
                        parse("max")?,
                        parse("step")?,
                        parse("value")?,
                    );
                    if !(min < max && step > 0.0 && (min..=max).contains(&default)) {
                        return Err(format!("{id} has invalid range/default"));
                    }
                    controls.push(Control::Range {
                        id: id.clone(),
                        binding: source.clone(),
                        min,
                        max,
                        step,
                        default,
                    });
                }
                "select" => {
                    let options = element
                        .children
                        .iter()
                        .filter_map(|child| match child {
                            TemplateNode::Element(e) if e.tag == "option" => Some((
                                e.attr("value").map(str::to_owned).unwrap_or_else(|| {
                                    e.children
                                        .iter()
                                        .find_map(|node| match node {
                                            TemplateNode::Text(t) => Some(t.clone()),
                                            _ => None,
                                        })
                                        .unwrap_or_default()
                                }),
                                e.attr("selected").is_some(),
                            )),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let default = options
                        .iter()
                        .find(|(_, s)| *s)
                        .or_else(|| options.first())
                        .map(|(v, _)| v.clone())
                        .ok_or_else(|| format!("{id} needs <option>"))?;
                    controls.push(Control::Select {
                        id: id.clone(),
                        binding: source.clone(),
                        options: options.into_iter().map(|(v, _)| v).collect(),
                        default,
                    });
                }
                _ => return Err(format!("{kind} on <{tag}> requires range input or select")),
            }
            control_id = Some(id);
        } else if direction == Direction::Out {
            if tag != "button" {
                return Err(format!("data-out on <{tag}> requires a button"));
            }
            if let Some(list) = attr("data-args") {
                for arg in list.split(',').map(str::trim).filter(|x| !x.is_empty()) {
                    args.push(resolve_expr(arg, "argument", Direction::In, scope)?);
                }
            }
        }
        binding = Some(source);
    }
    let mut children = Vec::new();
    for child in &element.children {
        match child {
            TemplateNode::Text(text) if !text.trim().is_empty() => {
                children.push(Node::Text(text.clone()))
            }
            TemplateNode::Element(child) => children.push(Node::Element(compile_element(
                child, def, scope, path, controls, seen,
            )?)),
            _ => {}
        }
    }
    Ok(Element {
        tag,
        style: InlineStyle::parse(attr("style").unwrap_or("")),
        attrs,
        children,
        binding,
        args,
        control_id,
        output_id: None,
    })
}
#[derive(Clone, Copy, Debug)]
pub enum Length {
    Px(f32),
    Percent(f32),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BoxValues {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Clone, Debug, Default)]
pub struct InlineStyle {
    pub display_flex: bool,
    pub column: bool,
    pub flex_wrap: bool,
    pub flex_grow: bool,
    pub gap: Option<f32>,
    pub padding: Option<BoxValues>,
    pub background: Option<u32>,
    pub color: Option<u32>,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub border_color: Option<u32>,
    pub border_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub margin_auto: bool,
    pub justify: Option<String>,
    pub align: Option<String>,
    pub scroll_y: bool,
}

impl InlineStyle {
    fn parse(source: &str) -> Self {
        let mut style = Self::default();
        for declaration in source.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            match name {
                "display" => style.display_flex = value == "flex",
                "flex-direction" => style.column = value == "column",
                "flex-wrap" => style.flex_wrap = value == "wrap",
                "flex" => style.flex_grow = value == "1",
                "gap" => style.gap = px_value(value),
                "padding" => style.padding = box_values(value),
                "background" | "background-color" => style.background = color(value),
                "color" => style.color = color(value),
                "font-size" => style.font_size = px_value(value),
                "font-weight" => style.font_weight = value.parse().ok(),
                "border" => {
                    style.border_width = value.split_whitespace().next().and_then(px_value);
                    style.border_color = value.split_whitespace().last().and_then(color);
                }
                "border-radius" => style.border_radius = px_value(value),
                "width" => style.width = length(value),
                "height" => style.height = length(value),
                "min-width" => style.min_width = px_value(value),
                "max-width" => style.max_width = px_value(value),
                "margin" => style.margin_auto = value == "auto",
                "justify-content" => style.justify = Some(value.to_owned()),
                "align-items" => style.align = Some(value.to_owned()),
                "overflow-y" => style.scroll_y = value == "auto" || value == "scroll",
                _ => {}
            }
        }
        style
    }
}

fn px_value(value: &str) -> Option<f32> {
    value.trim_end_matches("px").parse().ok()
}
fn length(value: &str) -> Option<Length> {
    if let Some(percent) = value.strip_suffix('%') {
        percent
            .parse::<f32>()
            .ok()
            .map(|v| Length::Percent(v / 100.0))
    } else {
        px_value(value).map(Length::Px)
    }
}
fn color(value: &str) -> Option<u32> {
    u32::from_str_radix(value.strip_prefix('#')?, 16).ok()
}
fn box_values(value: &str) -> Option<BoxValues> {
    let numbers: Vec<f32> = value
        .split_whitespace()
        .map(px_value)
        .collect::<Option<_>>()?;
    match numbers.as_slice() {
        [all] => Some(BoxValues {
            top: *all,
            right: *all,
            bottom: *all,
            left: *all,
        }),
        [vertical, horizontal] => Some(BoxValues {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        }),
        [top, horizontal, bottom] => Some(BoxValues {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        }),
        [top, right, bottom, left] => Some(BoxValues {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        }),
        _ => None,
    }
}
