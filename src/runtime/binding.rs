//! Binds a compiler-generated element tree to application state and GPUI Kit.
use crate::runtime::{
    Binding, Definition, Direction, OutputFormatter, Snapshot, StyleSheet, Value,
};

use crate::{RenderFn, TemplateElement, TemplateNode};
use gpui::{Div, FontWeight, Styled as _, px, relative, rgb};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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
    pub mobile_style: InlineStyle,
    pub inline_style: InlineStyle,
    pub render: Option<RenderFn>,
    pub children: Vec<Node>,
    pub binding: Option<Binding>,
    pub args: Vec<Binding>,
    pub control_id: Option<String>,
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
    pub mobile_breakpoint: Option<f32>,
    pub style_sheets: Vec<StyleSheet>,
    pub responsive_style_sheets: Vec<StyleSheet>,
    pub output_formatter: Option<OutputFormatter>,
}

pub fn compile(def: &Definition) -> Result<Page, String> {
    let mut controls = Vec::new();
    let mut seen = HashSet::new();
    let scope = def
        .bindings
        .iter()
        .map(|b| (b.name.to_owned(), b.clone()))
        .collect();
    let root = compile_component(def, &scope, def.name, &mut controls, &mut seen)?;
    let defaults = controls
        .iter()
        .filter_map(|c| {
            c.binding()
                .data_key
                .map(|k| (k.to_owned(), c.default_value()))
        })
        .collect::<HashMap<_, _>>();
    fn collect_style_sheets(
        definition: &Definition,
        into: &mut Vec<StyleSheet>,
        responsive: &mut Vec<StyleSheet>,
    ) {
        into.extend(definition.style_sheets.iter().copied());
        responsive.extend(definition.responsive_style_sheets.iter().copied());
        for import in &definition.imports {
            collect_style_sheets(import, into, responsive);
        }
    }
    let mut style_sheets = Vec::new();
    let mut responsive_style_sheets = Vec::new();
    collect_style_sheets(def, &mut style_sheets, &mut responsive_style_sheets);
    Ok(Page {
        root,
        controls,
        defaults,
        mobile_breakpoint: def.mobile_breakpoint,
        style_sheets,
        responsive_style_sheets,
        output_formatter: def.output_formatter,
    })
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
            if key != "name"
                && key != "style"
                && key != "mobile-style"
                && key != "class"
                && key != "id"
                && !child_scope.contains_key(key)
            {
                return Err(format!("unknown parameter {key} on {name}"));
            }
        }
        let instance = format!("{path}/{name}-{}", seen.len());
        let child_root = compile_component(child, &child_scope, &instance, controls, seen)?;
        return Ok(Element {
            tag: "div".into(),
            style: element.style.clone(),
            mobile_style: element.mobile_style.clone(),
            inline_style: element.inline_style.clone(),
            render: element.render,
            attrs,
            children: vec![Node::Element(child_root)],
            binding: None,
            args: vec![],
            control_id: None,
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
        style: element.style.clone(),
        mobile_style: element.mobile_style.clone(),
        inline_style: element.inline_style.clone(),
        render: element.render,
        attrs,
        children,
        binding,
        args,
        control_id,
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

type GpuiStyleOperation = Arc<dyn Fn(Div) -> Div + Send + Sync>;

#[derive(Clone, Default)]
pub struct InlineStyle {
    gpui_operations: Vec<GpuiStyleOperation>,
    pub display_flex: Option<bool>,
    pub column: Option<bool>,
    pub flex_wrap: Option<bool>,
    pub flex_grow: Option<bool>,
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
    pub margin_auto: Option<bool>,
    pub justify: Option<String>,
    pub align: Option<String>,
    pub scroll_y: Option<bool>,
}

impl InlineStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn flex(mut self) -> Self {
        self.display_flex = Some(true);
        self.gpui_operations
            .push(Arc::new(|element| element.flex()));
        self
    }

    pub fn display_flex(mut self, enabled: bool) -> Self {
        self.display_flex = Some(enabled);
        if enabled {
            self.gpui_operations
                .push(Arc::new(|element| element.flex()));
        }
        self
    }

    pub fn flex_direction(mut self, column: bool) -> Self {
        self.column = Some(column);
        self.gpui_operations.push(Arc::new(move |element| {
            if column {
                element.flex_col()
            } else {
                element.flex_row()
            }
        }));
        self
    }

    pub fn flex_wrap(mut self, wrap: bool) -> Self {
        self.flex_wrap = Some(wrap);
        self.gpui_operations.push(Arc::new(move |element| {
            if wrap {
                element.flex_wrap()
            } else {
                element.flex_nowrap()
            }
        }));
        self
    }

    pub fn flex_grow(mut self, grow: bool) -> Self {
        self.flex_grow = Some(grow);
        self.gpui_operations.push(Arc::new(move |element| {
            if grow {
                element.flex_1()
            } else {
                element.flex_none()
            }
        }));
        self
    }

    pub fn gap(mut self, pixels: f32) -> Self {
        self.gap = Some(pixels);
        self.gpui_operations
            .push(Arc::new(move |element| element.gap(px(pixels))));
        self
    }

    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.padding = Some(BoxValues {
            top,
            right,
            bottom,
            left,
        });
        self.gpui_operations.push(Arc::new(move |element| {
            element
                .pt(px(top))
                .pr(px(right))
                .pb(px(bottom))
                .pl(px(left))
        }));
        self
    }

    pub fn background_color(mut self, rgb: u32) -> Self {
        self.background = Some(rgb & 0x00ff_ffff);
        let color = self.background.unwrap();
        self.gpui_operations
            .push(Arc::new(move |element| element.bg(gpui::rgb(color))));
        self
    }

    pub fn text_color(mut self, rgb: u32) -> Self {
        self.color = Some(rgb & 0x00ff_ffff);
        let color = self.color.unwrap();
        self.gpui_operations.push(Arc::new(move |element| {
            element.text_color(gpui::rgb(color))
        }));
        self
    }

    pub fn width(mut self, width: Length) -> Self {
        self.width = Some(width);
        self.gpui_operations
            .push(Arc::new(move |element| match width {
                Length::Px(value) => element.w(px(value)),
                Length::Percent(value) => element.w(relative(value)),
            }));
        self
    }

    pub fn height(mut self, height: Length) -> Self {
        self.height = Some(height);
        self.gpui_operations
            .push(Arc::new(move |element| match height {
                Length::Px(value) => element.h(px(value)),
                Length::Percent(value) => element.h(relative(value)),
            }));
        self
    }

    pub fn min_width(mut self, pixels: f32) -> Self {
        self.min_width = Some(pixels);
        self.gpui_operations
            .push(Arc::new(move |element| element.min_w(px(pixels))));
        self
    }

    pub fn max_width(mut self, pixels: f32) -> Self {
        self.max_width = Some(pixels);
        self.gpui_operations
            .push(Arc::new(move |element| element.max_w(px(pixels))));
        self
    }

    pub fn border_radius(mut self, pixels: f32) -> Self {
        self.border_radius = Some(pixels);
        self.gpui_operations
            .push(Arc::new(move |element| element.rounded(px(pixels))));
        self
    }

    pub fn font_size(mut self, pixels: f32) -> Self {
        self.font_size = Some(pixels);
        self.gpui_operations
            .push(Arc::new(move |element| element.text_size(px(pixels))));
        self
    }

    pub fn font_weight(mut self, weight: u16) -> Self {
        self.font_weight = Some(weight);
        self.gpui_operations.push(Arc::new(move |element| {
            element.font_weight(FontWeight(weight as f32))
        }));
        self
    }

    pub fn border(mut self, pixels: f32, color: u32) -> Self {
        self.border_width = Some(pixels);
        self.border_color = Some(color & 0x00ff_ffff);
        let color = self.border_color.unwrap();
        self.gpui_operations.push(Arc::new(move |element| {
            element.border(px(pixels)).border_color(rgb(color))
        }));
        self
    }

    pub fn margin_auto(mut self) -> Self {
        self.margin_auto = Some(true);
        self.gpui_operations
            .push(Arc::new(|element| element.mx_auto()));
        self
    }

    pub fn margin_auto_enabled(mut self, enabled: bool) -> Self {
        self.margin_auto = Some(enabled);
        if enabled {
            self.gpui_operations
                .push(Arc::new(|element| element.mx_auto()));
        }
        self
    }

    pub fn overflow_y(mut self, scroll: bool) -> Self {
        self.scroll_y = Some(scroll);
        self
    }

    pub fn justify_content(mut self, value: impl Into<String>) -> Self {
        self.justify = Some(value.into());
        match self.justify.as_deref() {
            Some("space-between") => self
                .gpui_operations
                .push(Arc::new(|element| element.justify_between())),
            Some("center") => self
                .gpui_operations
                .push(Arc::new(|element| element.justify_center())),
            Some("flex-end") => self
                .gpui_operations
                .push(Arc::new(|element| element.justify_end())),
            _ => {}
        }
        self
    }

    pub fn align_items(mut self, value: impl Into<String>) -> Self {
        self.align = Some(value.into());
        match self.align.as_deref() {
            Some("center") => self
                .gpui_operations
                .push(Arc::new(|element| element.items_center())),
            Some("flex-start") => self
                .gpui_operations
                .push(Arc::new(|element| element.items_start())),
            Some("flex-end") => self
                .gpui_operations
                .push(Arc::new(|element| element.items_end())),
            _ => {}
        }
        self
    }

    pub(crate) fn apply_to_gpui(&self, element: Div) -> Div {
        self.gpui_operations
            .iter()
            .fold(element, |element, apply| apply(element))
    }

    pub(crate) fn apply_overrides(&mut self, overrides: &InlineStyle) {
        self.gpui_operations
            .extend(overrides.gpui_operations.iter().cloned());
        self.display_flex = overrides.display_flex.or(self.display_flex);
        self.column = overrides.column.or(self.column);
        self.flex_wrap = overrides.flex_wrap.or(self.flex_wrap);
        self.flex_grow = overrides.flex_grow.or(self.flex_grow);
        self.gap = overrides.gap.or(self.gap);
        self.padding = overrides.padding.or(self.padding);
        self.background = overrides.background.or(self.background);
        self.color = overrides.color.or(self.color);
        self.font_size = overrides.font_size.or(self.font_size);
        self.font_weight = overrides.font_weight.or(self.font_weight);
        self.border_color = overrides.border_color.or(self.border_color);
        self.border_width = overrides.border_width.or(self.border_width);
        self.border_radius = overrides.border_radius.or(self.border_radius);
        self.width = overrides.width.or(self.width);
        self.height = overrides.height.or(self.height);
        self.min_width = overrides.min_width.or(self.min_width);
        self.max_width = overrides.max_width.or(self.max_width);
        self.margin_auto = overrides.margin_auto.or(self.margin_auto);
        self.justify = overrides.justify.clone().or(self.justify.clone());
        self.align = overrides.align.clone().or(self.align.clone());
        self.scroll_y = overrides.scroll_y.or(self.scroll_y);
    }
}
