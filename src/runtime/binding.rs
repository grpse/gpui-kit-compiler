//! Binds a compiler-generated element tree to application state and GPUI Kit.
use crate::runtime::{
    Binding, ComponentProps, Definition, Direction, OutputFormatter, SelectOption, Snapshot, Value,
};

use crate::{RenderFn, TemplateElement, TemplateNode};
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
    pub render: Option<RenderFn>,
    pub children: Vec<Node>,
    pub binding: Option<Binding>,
    pub args: Vec<Binding>,
    pub control_id: Option<String>,
    pub component: Option<ComponentInstance>,
}

#[derive(Clone)]
pub struct ComponentInstance {
    pub id: String,
    pub page: Box<Page>,
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
        default: Option<f32>,
    },
    Select {
        id: String,
        binding: Binding,
        options: Vec<SelectOption>,
        default: String,
    },
    Text {
        id: String,
        binding: Binding,
        default: String,
        multiline: bool,
        placeholder: String,
        masked: bool,
    },
    Date {
        id: String,
        binding: Binding,
        default: Option<String>,
    },
    Checkbox {
        id: String,
        binding: Binding,
        default: bool,
    },
}
impl Control {
    pub fn id(&self) -> &str {
        match self {
            Self::Range { id, .. }
            | Self::Select { id, .. }
            | Self::Text { id, .. }
            | Self::Date { id, .. }
            | Self::Checkbox { id, .. } => id,
        }
    }
    pub fn binding(&self) -> &Binding {
        match self {
            Self::Range { binding, .. }
            | Self::Select { binding, .. }
            | Self::Text { binding, .. }
            | Self::Date { binding, .. }
            | Self::Checkbox { binding, .. } => binding,
        }
    }
    pub fn default_value(&self) -> Option<Value> {
        match self {
            Self::Range {
                default: Some(default),
                ..
            } => Some(Value::Number(*default)),
            Self::Range { default: None, .. } => None,
            Self::Select { default, .. } => Some(Value::Text(default.clone())),
            Self::Text { default, .. } => Some(Value::Text(default.clone())),
            Self::Date { default, .. } => default.clone().map(Value::Text),
            Self::Checkbox { default, .. } => Some(Value::from(*default)),
        }
    }
}
#[derive(Clone)]
pub struct Page {
    pub root: Element,
    pub controls: Vec<Control>,
    pub defaults: HashMap<String, Value>,
    pub signals: Vec<(String, crate::runtime::Signal)>,
    pub mobile_breakpoint: Option<f32>,
    pub output_formatter: Option<OutputFormatter>,
    pub inputs: Vec<Binding>,
    pub renderer: crate::ComponentRenderFn,
}

impl Page {
    pub fn bind_signals(&self, engine: &crate::runtime::Engine) {
        for (key, signal) in &self.signals {
            signal.bind_engine(key.clone(), engine.clone());
        }
    }

    pub fn props(&self, snapshot: &Snapshot) -> ComponentProps {
        ComponentProps::from_values(
            self.inputs
                .iter()
                .filter_map(|binding| binding.get(snapshot).map(|value| (binding.name, value)))
                .collect(),
        )
    }
}

pub fn compile(def: &Definition) -> Result<Page, String> {
    let scope = def
        .bindings
        .iter()
        .map(|b| (b.name.to_owned(), b.clone()))
        .collect();
    compile_component(def, &scope, def.name)
}
fn compile_component(
    def: &Definition,
    scope: &HashMap<String, Binding>,
    path: &str,
) -> Result<Page, String> {
    let mut scope_with_signals = scope.clone();
    let mut component_signals = Vec::new();
    for (name, signal) in &def.signals {
        if scope_with_signals.contains_key(*name) {
            return Err(format!("signal {name} conflicts with a component binding"));
        }
        let key = component_signal_key(path, def.name, name);
        scope_with_signals.insert(
            (*name).to_owned(),
            Binding::signal_at(*name, key.clone(), signal.clone()),
        );
        component_signals.push((key, signal.clone()));
    }
    for (name, prototype) in &def.local_signals {
        if scope_with_signals.contains_key(*name) {
            return Err(format!("signal {name} conflicts with a component binding"));
        }
        let signal = prototype.clone();
        let key = component_signal_key(path, def.name, name);
        scope_with_signals.insert(
            (*name).to_owned(),
            Binding::signal_at(*name, key.clone(), signal.clone()),
        );
        component_signals.push((key, signal));
    }
    let scope = &scope_with_signals;
    let mut controls = Vec::new();
    let mut seen = HashSet::new();
    let root = compile_element(&def.template, def, scope, path, &mut controls, &mut seen)?;
    let mut defaults = controls
        .iter()
        .filter_map(|control| {
            control
                .binding()
                .data_key
                .zip(control.default_value())
                .map(|(key, value)| (key.to_owned(), value))
        })
        .collect::<HashMap<_, _>>();
    collect_nested_defaults(&root, &mut defaults);
    defaults.extend(def.initial_values.clone());
    for (key, signal) in &component_signals {
        defaults.insert(key.clone(), signal.get());
    }
    let readable = scope
        .iter()
        .filter(|(_, binding)| binding.direction.reads())
        .map(|(name, binding)| (name.as_str(), binding.clone()))
        .collect::<HashMap<_, _>>();
    let mut used_inputs = HashSet::new();
    collect_template_inputs(&root, &readable, &mut used_inputs);
    used_inputs.extend(def.view_inputs.iter().map(|name| (*name).to_owned()));
    used_inputs.extend(
        def.signals
            .iter()
            .chain(def.local_signals.iter())
            .map(|(name, _)| (*name).to_owned()),
    );
    let mut inputs = readable
        .iter()
        .filter(|(name, _)| used_inputs.contains(**name))
        .map(|(_, binding)| binding.clone())
        .collect::<Vec<_>>();
    inputs.sort_by_key(|binding| binding.name);
    Ok(Page {
        root,
        controls,
        defaults,
        signals: component_signals,
        mobile_breakpoint: def.mobile_breakpoint,
        output_formatter: def.output_formatter,
        inputs,
        renderer: def.renderer,
    })
}

fn component_signal_key(path: &str, component_name: &str, signal_name: &str) -> String {
    if path == component_name {
        signal_name.to_owned()
    } else {
        format!("{path}/{signal_name}")
    }
}

fn collect_template_inputs(
    element: &Element,
    readable: &HashMap<&str, Binding>,
    inputs: &mut HashSet<String>,
) {
    for name in ["data-in", "data-in-out", "data-rsc-value-binding"] {
        if let Some(binding) = element.attr(name)
            && readable.contains_key(binding)
        {
            inputs.insert(binding.to_owned());
        }
    }
    if let Some(arguments) = element.attr("data-args") {
        for token in arguments
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .filter(|token| !token.is_empty())
        {
            if readable.contains_key(token) {
                inputs.insert(token.to_owned());
            }
        }
    }
    for child in &element.children {
        if let Node::Element(child) = child {
            collect_template_inputs(child, readable, inputs);
        }
    }
}

fn collect_nested_defaults(element: &Element, defaults: &mut HashMap<String, Value>) {
    if let Some(component) = &element.component {
        defaults.extend(component.page.defaults.clone());
    }
    for child in &element.children {
        if let Node::Element(child) = child {
            collect_nested_defaults(child, defaults);
        }
    }
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
        .ok_or_else(|| {
            format!(
                "component property {target} needs a braced Rust expression, such as `{target}={{{target}}}`"
            )
        })?
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
    if tag == "video" {
        return Err("<video> is not supported by GPUI Kit 0.7; use a native component backed by a video renderer".into());
    }
    if !matches!(
        tag.as_str(),
        "div"
            | "body"
            | "component"
            | "output"
            | "rsc-value"
            | "input"
            | "textarea"
            | "img"
            | "progress"
            | "select"
            | "button"
            | "option"
            | "rsc-if"
            | "rsc-then"
            | "rsc-else"
    ) {
        return Err(format!("<{tag}> is not a supported GPUI element"));
    }
    let attrs: HashMap<String, String> = element.attrs.iter().cloned().collect();
    let attr = |k: &str| attrs.get(k).map(String::as_str);
    if tag == "img" && attr("src").is_none() && attr("data-in").is_none() {
        return Err("<img> needs a src attribute".into());
    }
    if tag == "img" {
        if let Some(fit) = attr("object-fit") {
            if !matches!(fit, "contain" | "cover" | "fill" | "scale-down" | "none") {
                return Err(format!("<img> has unsupported object-fit {fit:?}"));
            }
        }
        for name in ["width", "height"] {
            if let Some(value) = attr(name) {
                if !value
                    .parse::<f32>()
                    .is_ok_and(|value| value.is_finite() && value > 0.0)
                {
                    return Err(format!("<img> needs a positive numeric {name}"));
                }
            }
        }
    }
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
        seen.insert(format!("{instance}/component"));
        let child_page = compile_component(child, &child_scope, &instance)?;
        return Ok(Element {
            tag: "component".into(),
            render: element.render,
            attrs,
            children: vec![],
            binding: None,
            args: vec![],
            control_id: None,
            component: Some(ComponentInstance {
                id: instance,
                page: Box::new(child_page),
            }),
        });
    }
    let bindings = ["data-in", "data-out", "data-in-out"]
        .iter()
        .filter_map(|k| attr(k).map(|v| (*k, v)))
        .collect::<Vec<_>>();
    if bindings.len() > 1 {
        return Err(format!("<{tag}> has multiple binding directions"));
    }
    if attr("data-rsc-value-binding").is_some() && !bindings.is_empty() {
        return Err(format!(
            "<{tag}> value={{...}} already declares its two-way binding; remove data-in-out"
        ));
    }
    let declared_binding = bindings
        .first()
        .copied()
        .or_else(|| attr("data-rsc-value-binding").map(|name| ("value", name)));
    let mut binding = None;
    let mut control_id = None;
    let mut args = Vec::new();
    if let Some((kind, name)) = declared_binding {
        if kind == "data-in"
            && !matches!(
                tag.as_str(),
                "output" | "rsc-value" | "rsc-if" | "img" | "progress"
            )
        {
            return Err(format!("data-in on <{tag}> requires an output element"));
        }
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
            let control_name = attr("id").unwrap_or(name);
            let id = format!("{path}/{control_name}");
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
                    let (min, max, step) = (parse("min")?, parse("max")?, parse("step")?);
                    let default = match attr("value") {
                        Some(_) => Some(parse("value")?),
                        None if kind == "value" => None,
                        None => return Err(format!("{id} needs value={{binding}}")),
                    };
                    if !(min < max
                        && step > 0.0
                        && default.is_none_or(|default| (min..=max).contains(&default)))
                    {
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
                    let options_key = attr("id").unwrap_or(&id);
                    let options =
                        def.select_options
                            .get(options_key)
                            .cloned()
                            .unwrap_or_else(|| {
                                element
                                    .children
                                    .iter()
                                    .filter_map(|child| match child {
                                        TemplateNode::Element(e) if e.tag == "option" => {
                                            let label = e
                                                .children
                                                .iter()
                                                .find_map(|node| match node {
                                                    TemplateNode::Text(text) => Some(text.clone()),
                                                    _ => None,
                                                })
                                                .unwrap_or_default();
                                            Some(SelectOption {
                                                value: e
                                                    .attr("value")
                                                    .map(str::to_owned)
                                                    .unwrap_or_else(|| label.clone()),
                                                label,
                                                selected: e.attr("selected").is_some(),
                                            })
                                        }
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                            });
                    let default = options
                        .iter()
                        .find(|option| option.selected)
                        .or_else(|| options.first())
                        .map(|option| option.value.clone())
                        .ok_or_else(|| {
                            format!("{id} needs <option> children or with_select_options")
                        })?;
                    controls.push(Control::Select {
                        id: id.clone(),
                        binding: source.clone(),
                        options,
                        default,
                    });
                }
                "input"
                    if matches!(
                        attr("type").unwrap_or("text"),
                        "text" | "email" | "password" | "search" | "url" | "tel"
                    ) =>
                {
                    controls.push(Control::Text {
                        id: id.clone(),
                        binding: source.clone(),
                        default: attr("value").unwrap_or("").to_owned(),
                        multiline: false,
                        placeholder: attr("placeholder").unwrap_or("").to_owned(),
                        masked: attr("type") == Some("password"),
                    });
                }
                "textarea" => {
                    controls.push(Control::Text {
                        id: id.clone(),
                        binding: source.clone(),
                        default: element
                            .children
                            .iter()
                            .filter_map(|child| match child {
                                TemplateNode::Text(text) => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<String>(),
                        multiline: true,
                        placeholder: attr("placeholder").unwrap_or("").to_owned(),
                        masked: false,
                    });
                }
                "input" if attr("type") == Some("date") => {
                    let default = attr("value").map(str::to_owned);
                    if default.as_deref().is_some_and(|value| {
                        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err()
                    }) {
                        return Err(format!("{id} needs a valid YYYY-MM-DD date"));
                    }
                    controls.push(Control::Date {
                        id: id.clone(),
                        binding: source.clone(),
                        default,
                    });
                }
                "input" if attr("type") == Some("checkbox") => {
                    controls.push(Control::Checkbox {
                        id: id.clone(),
                        binding: source.clone(),
                        default: attr("checked").is_some(),
                    });
                }
                _ => {
                    return Err(format!(
                        "{kind} on <{tag}> requires a supported input, textarea, or select"
                    ));
                }
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
    if tag == "input" && control_id.is_none() {
        return Err("<input> needs a supported type and value={binding} or data-in-out".into());
    }
    if matches!(tag.as_str(), "select" | "textarea") && control_id.is_none() {
        return Err("<select> needs value={binding} or data-in-out".into());
    }
    if tag == "button"
        && binding
            .as_ref()
            .is_none_or(|binding| !binding.direction.writes())
    {
        return Err("<button> needs data-out".into());
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
        render: element.render,
        attrs,
        children,
        binding,
        args,
        control_id,
        component: None,
    })
}
pub use gpui::{Length, StyleRefinement as InlineStyle};
