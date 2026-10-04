//! Lowers the existing Rust style API to native GPUI builders at compile time.
use quote::{ToTokens, quote};
use syn::{
    Expr,
    visit_mut::{self, VisitMut},
};

pub fn translate(source: &str) -> Result<String, String> {
    let mut file = syn::parse_file(source).map_err(|e| format!("invalid component Rust: {e}"))?;
    let mut translator = Translator {
        error: None,
        float_closures: Vec::new(),
    };
    translator.visit_file_mut(&mut file);
    if let Some(error) = translator.error {
        return Err(error);
    }
    let output = file.into_token_stream().to_string();
    Ok(rewrite_float_closure_arguments(
        output,
        &translator.float_closures,
    ))
}

fn rewrite_float_closure_arguments(mut source: String, names: &[String]) -> String {
    let mut names = names.to_vec();
    for (offset, _) in source.match_indices("= | ").collect::<Vec<_>>() {
        let Some(end) = source[offset + 4..].find('|').map(|end| offset + 4 + end) else {
            continue;
        };
        if !source[offset + 4..end].contains("f32") {
            continue;
        }
        if let Some(name) = source[..offset]
            .split_whitespace()
            .last()
            .map(str::to_owned)
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    for name in &names {
        let needle = format!("{name} (");
        let mut cursor = 0;
        while let Some(offset) = source[cursor..].find(&needle) {
            let start = cursor + offset + needle.len();
            let Some(end) = matching_paren(&source, start - 1) else {
                break;
            };
            let arguments = source[start..end].to_owned();
            let rewritten = arguments
                .split(',')
                .map(|argument| {
                    let argument = argument.trim();
                    if argument.starts_with("0x")
                        || argument.bytes().all(|byte| byte.is_ascii_digit())
                    {
                        format!("({argument}) as f32")
                    } else {
                        argument.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(" , ");
            source.replace_range(start..end, &rewritten);
            cursor = start + rewritten.len() + 1;
        }
    }
    source
}

fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    for (index, character) in source.char_indices().skip_while(|(index, _)| *index < open) {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

struct Translator {
    error: Option<String>,
    float_closures: Vec<String>,
}

fn style_chain(expression: &Expr) -> Option<Vec<(String, Vec<Expr>)>> {
    match expression {
        Expr::MethodCall(call) => {
            let mut chain = style_chain(&call.receiver)?;
            chain.push((call.method.to_string(), call.args.iter().cloned().collect()));
            Some(chain)
        }
        Expr::Call(call) if call.args.is_empty() => {
            let Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let parts: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            (parts.len() >= 2
                && parts.last().unwrap() == "new"
                && matches!(parts[parts.len() - 2].as_str(), "Style" | "InlineStyle"))
            .then(Vec::new)
        }
        _ => None,
    }
}

impl VisitMut for Translator {
    fn visit_expr_closure_mut(&mut self, closure: &mut syn::ExprClosure) {
        if let Some(name) = closure.inputs.first().and_then(|input| match input {
            syn::Pat::Type(typed) if is_f32(&typed.ty) => match typed.pat.as_ref() {
                syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            },
            _ => None,
        }) {
            self.float_closures.push(name);
        }
        visit_mut::visit_expr_closure_mut(self, closure);
    }

    fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
        visit_mut::visit_expr_call_mut(self, call);
        let name = match call.func.as_ref() {
            Expr::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        };
        if name.is_some_and(|name| {
            self.float_closures
                .iter()
                .any(|candidate| candidate == &name)
        }) {
            for argument in &mut call.args {
                if matches!(
                    argument,
                    Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(_),
                        ..
                    })
                ) {
                    let value = argument.clone();
                    *argument = syn::parse2(quote!((#value) as f32)).unwrap();
                }
            }
        }
    }

    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Some(chain) = style_chain(expression) {
            match lower_chain(chain, self) {
                Ok(lowered) => *expression = lowered,
                Err(error) => self.error = Some(error),
            }
            return;
        }
        visit_mut::visit_expr_mut(self, expression);
        if let Expr::Call(call) = expression {
            if let Expr::Path(path) = call.func.as_ref() {
                let parts: Vec<_> = path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                if parts.len() >= 2 && parts[parts.len() - 2] == "Length" && call.args.len() == 1 {
                    let value = &call.args[0];
                    let translated = match parts.last().unwrap().as_str() {
                        "Px" => Some(quote!(gpui::Length::from(gpui::px(#value)))),
                        "Percent" => Some(quote!(gpui::Length::from(gpui::relative(#value)))),
                        _ => None,
                    };
                    if let Some(tokens) = translated {
                        *expression = syn::parse2(tokens).unwrap();
                    }
                }
            }
        }
    }
}

fn is_f32(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path) if path.path.is_ident("f32"))
}

fn lower_chain(
    chain: Vec<(String, Vec<Expr>)>,
    translator: &mut Translator,
) -> Result<Expr, String> {
    let position = chain
        .iter()
        .rev()
        .find_map(|(name, _)| match name.as_str() {
            "position_static" | "position_relative" | "position_absolute" => Some(name.as_str()),
            _ => None,
        });
    let is_static = position == Some("position_static");
    let is_absolute = position == Some("position_absolute");
    let mut statements = Vec::new();
    for (name, mut args) in chain {
        if matches!(name.as_str(), "top" | "right" | "bottom" | "left") {
            if is_static {
                continue;
            }
            if !is_absolute {
                return Err(format!(
                    "style offset {name} requires position: \"absolute\"; normal-flow elements remain inside their parent"
                ));
            }
        }
        for arg in &mut args {
            translator.visit_expr_mut(arg);
        }
        let args = args.as_slice();
        let calls = match (name.as_str(), args) {
            ("flex", []) => quote!(__style.flex()),
            ("display_flex", [v]) => quote!(if #v { __style.flex() } else { __style.block() }),
            ("flex_direction", [v]) => {
                quote!(if #v { __style.flex_col() } else { __style.flex_row() })
            }
            ("flex_wrap", [v]) => {
                quote!(if #v { __style.flex_wrap() } else { __style.flex_nowrap() })
            }
            ("flex_shrink", [v]) => quote!(__style.flex_shrink((#v) as f32)),
            ("flex_basis", [v]) => quote!(__style.flex_basis(#v)),
            ("flex_grow", [v]) => quote!(if #v { __style.flex_1() } else { __style.flex_none() }),
            ("gap", [v]) => quote!(__style.gap(gpui::px(#v))),
            ("padding", [t, r, b, l]) => {
                quote!(__style.pt(gpui::px(#t)).pr(gpui::px(#r)).pb(gpui::px(#b)).pl(gpui::px(#l)))
            }
            ("padding_top", [v]) => quote!(__style.pt(gpui::px(#v))),
            ("padding_right", [v]) => quote!(__style.pr(gpui::px(#v))),
            ("padding_bottom", [v]) => quote!(__style.pb(gpui::px(#v))),
            ("padding_left", [v]) => quote!(__style.pl(gpui::px(#v))),
            ("background_color", [v]) if is_rgba_expression(v) => quote!(__style.bg(#v)),
            ("text_color", [v]) if is_rgba_expression(v) => quote!(__style.text_color(#v)),
            ("border_color", [v]) if is_rgba_expression(v) => quote!(__style.border_color(#v)),
            ("background_color", [v]) => quote!(__style.bg(gpui::rgb(#v))),
            ("text_color", [v]) => quote!(__style.text_color(gpui::rgb(#v))),
            ("border_color", [v]) => quote!(__style.border_color(gpui::rgb(#v))),
            ("width", [v]) => quote!(__style.w(#v)),
            ("height", [v]) => quote!(__style.h(#v)),
            ("min_width", [v]) => quote!(__style.min_w(gpui::px(#v))),
            ("min_height", [v]) => quote!(__style.min_h(gpui::px(#v))),
            ("max_height", [v]) => quote!(__style.max_h(gpui::px(#v))),
            ("aspect_ratio", [v]) => quote!(__style.aspect_ratio(#v)),
            ("white_space", [v]) => quote!({
                __style.text_style().white_space = Some(#v);
                __style
            }),
            ("text_overflow", [v]) => quote!({
                __style.text_style().text_overflow = #v;
                __style
            }),
            ("line_clamp", [v]) => quote!(__style.line_clamp(#v)),
            ("max_width", [v]) => quote!(__style.max_w(gpui::px(#v))),
            ("border_radius", [v]) => quote!(__style.rounded(gpui::px(#v))),
            ("font", [v]) => quote!(__style.font(#v)),
            ("font_family", [v]) => quote!(__style.font_family(#v)),
            ("font_features", [v]) => quote!(__style.font_features(#v)),
            ("font_style", [v]) => quote!({
                __style.text_style().font_style = Some(#v);
                __style
            }),
            ("line_height", [v]) => quote!(__style.line_height(#v)),
            ("font_weight", [Expr::Path(v)])
                if v.path.segments.iter().any(|s| s.ident == "FontWeight") =>
            {
                quote!(__style.font_weight(#v))
            }
            ("font_size", [v]) => quote!(__style.text_size(gpui::px(#v))),
            ("font_weight", [v]) => quote!(__style.font_weight(gpui::FontWeight((#v) as f32))),
            ("border", [w, c]) if is_rgba_expression(c) => {
                quote!(__style.border(gpui::px(#w)).border_color(#c))
            }
            ("border", [w, c]) => quote!(__style.border(gpui::px(#w)).border_color(gpui::rgb(#c))),
            ("position_static", []) => quote!({
                __style.style().position = Some(gpui::Position::Relative);
                __style.style().inset = Default::default();
                __style
            }),
            ("position_relative", []) => quote!(__style.relative()),
            ("position_absolute", []) => quote!(__style.absolute()),
            ("top", [v]) => quote!(__style.top(gpui::px(#v))),
            ("right", [v]) => quote!(__style.right(gpui::px(#v))),
            ("bottom", [v]) => quote!(__style.bottom(gpui::px(#v))),
            ("left", [v]) => quote!(__style.left(gpui::px(#v))),
            ("overflow_hidden", []) => quote!(__style.overflow_hidden()),
            ("overflow_y_hidden", []) => quote!(__style.overflow_y_hidden()),
            ("opacity", [v]) => quote!(__style.opacity(#v)),
            ("margin_auto", []) => quote!(__style.mx_auto()),
            ("margin_auto_enabled", [v]) => quote!(if #v { __style.mx_auto() } else { __style }),
            ("overflow_y", [v]) => {
                quote!({ __style.style().overflow.y = Some(if #v { gpui::Overflow::Scroll } else { gpui::Overflow::Visible }); __style })
            }
            ("justify_content", [v]) => {
                quote!(match (#v).as_ref() { "space-between" => __style.justify_between(), "center" => __style.justify_center(), "flex-end" => __style.justify_end(), _ => __style.justify_start() })
            }
            ("align_items", [v]) => {
                quote!(match (#v).as_ref() { "center" => __style.items_center(), "flex-start" => __style.items_start(), "flex-end" => __style.items_end(), _ => __style.items_stretch() })
            }
            _ => {
                return Err(format!(
                    "unsupported Rust style builder {name} with {} arguments",
                    args.len()
                ));
            }
        };
        statements.push(quote!(__style = #calls;));
    }
    syn::parse2(quote!({
        use gpui::Styled as _;
        let mut __style = gpui::div();
        #(#statements)*
        __style.style().clone()
    }))
    .map_err(|e| e.to_string())
}

fn is_rgba_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Struct(expression) => expression
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Rgba"),
        _ => false,
    }
}
