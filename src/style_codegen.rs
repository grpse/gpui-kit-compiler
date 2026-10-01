//! Lowers the existing Rust style API to native GPUI builders at compile time.
use quote::{quote, ToTokens};
use syn::{visit_mut::{self, VisitMut}, Expr};

pub fn translate(source: &str) -> Result<String, String> {
    let mut file = syn::parse_file(source).map_err(|e| format!("invalid component Rust: {e}"))?;
    let mut translator = Translator { error: None };
    translator.visit_file_mut(&mut file);
    if let Some(error) = translator.error { return Err(error); }
    Ok(file.into_token_stream().to_string())
}

struct Translator { error: Option<String> }

fn style_chain(expression: &Expr) -> Option<Vec<(String, Vec<Expr>)>> {
    match expression {
        Expr::MethodCall(call) => {
            let mut chain = style_chain(&call.receiver)?;
            chain.push((call.method.to_string(), call.args.iter().cloned().collect()));
            Some(chain)
        }
        Expr::Call(call) if call.args.is_empty() => {
            let Expr::Path(path) = call.func.as_ref() else { return None };
            let parts: Vec<_> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
            (parts.len() >= 2 && parts.last().unwrap() == "new"
                && matches!(parts[parts.len()-2].as_str(), "Style" | "InlineStyle"))
                .then(Vec::new)
        }
        _ => None,
    }
}

impl VisitMut for Translator {
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
                let parts: Vec<_> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
                if parts.len() >= 2 && parts[parts.len()-2] == "Length" && call.args.len() == 1 {
                    let value = &call.args[0];
                    let translated = match parts.last().unwrap().as_str() {
                        "Px" => Some(quote!(gpui::Length::from(gpui::px(#value)))),
                        "Percent" => Some(quote!(gpui::Length::from(gpui::relative(#value)))),
                        _ => None,
                    };
                    if let Some(tokens) = translated { *expression = syn::parse2(tokens).unwrap(); }
                }
            }
        }
    }
}

fn lower_chain(chain: Vec<(String, Vec<Expr>)>, translator: &mut Translator) -> Result<Expr, String> {
    let mut statements = Vec::new();
    for (name, mut args) in chain {
        for arg in &mut args { translator.visit_expr_mut(arg); }
        let args = args.as_slice();
        let calls = match (name.as_str(), args) {
            ("flex", []) => quote!(__style.flex()),
            ("display_flex", [v]) => quote!(if #v { __style.flex() } else { __style.block() }),
            ("flex_direction", [v]) => quote!(if #v { __style.flex_col() } else { __style.flex_row() }),
            ("flex_wrap", [v]) => quote!(if #v { __style.flex_wrap() } else { __style.flex_nowrap() }),
            ("flex_grow", [v]) => quote!(if #v { __style.flex_1() } else { __style.flex_none() }),
            ("gap", [v]) => quote!(__style.gap(gpui::px(#v))),
            ("padding", [t,r,b,l]) => quote!(__style.pt(gpui::px(#t)).pr(gpui::px(#r)).pb(gpui::px(#b)).pl(gpui::px(#l))),
            ("background_color", [v]) => quote!(__style.bg(gpui::rgb(#v))),
            ("text_color", [v]) => quote!(__style.text_color(gpui::rgb(#v))),
            ("width", [v]) => quote!(__style.w(#v)),
            ("height", [v]) => quote!(__style.h(#v)),
            ("min_width", [v]) => quote!(__style.min_w(gpui::px(#v))),
            ("max_width", [v]) => quote!(__style.max_w(gpui::px(#v))),
            ("border_radius", [v]) => quote!(__style.rounded(gpui::px(#v))),
            ("font_size", [v]) => quote!(__style.text_size(gpui::px(#v))),
            ("font_weight", [v]) => quote!(__style.font_weight(gpui::FontWeight((#v) as f32))),
            ("border", [w,c]) => quote!(__style.border(gpui::px(#w)).border_color(gpui::rgb(#c))),
            ("margin_auto", []) => quote!(__style.mx_auto()),
            ("margin_auto_enabled", [v]) => quote!(if #v { __style.mx_auto() } else { __style }),
            ("overflow_y", [v]) => quote!({ __style.style().overflow.y = Some(if #v { gpui::Overflow::Scroll } else { gpui::Overflow::Visible }); __style }),
            ("justify_content", [v]) => quote!(match (#v).as_ref() { "space-between" => __style.justify_between(), "center" => __style.justify_center(), "flex-end" => __style.justify_end(), _ => __style.justify_start() }),
            ("align_items", [v]) => quote!(match (#v).as_ref() { "center" => __style.items_center(), "flex-start" => __style.items_start(), "flex-end" => __style.items_end(), _ => __style.items_stretch() }),
            _ => return Err(format!("unsupported Rust style builder {name} with {} arguments", args.len())),
        };
        statements.push(quote!(__style = #calls;));
    }
    syn::parse2(quote!({
        use gpui::Styled as _;
        let mut __style = gpui::div();
        #(#statements)*
        __style.style().clone()
    })).map_err(|e| e.to_string())
}
