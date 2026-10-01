#[cfg(feature = "compiler")]
mod style_codegen;
// Compiler for single-file Rust components.
#[cfg(feature = "compiler")]
use scraper::{ElementRef, Html, Node as HtmlNode, Selector};
#[cfg(feature = "compiler")]
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "compiler")]
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
    let (html, select_option_refs) = extract_select_option_refs(html)?;
    let html = expand_template_expressions(&html)?;
    let html = expand_template_interpolations(&html)?;
    let html = normalize_component_tags(&html)?;
    let (html, mut class_bindings) = extract_class_bindings(&html)?;
    let document = Html::parse_document(&html);
    let (stylesheet, keyframes) = component_styles(&document, input)?;
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid filename")?;
    let body = document
        .select(&Selector::parse("body").unwrap())
        .next()
        .ok_or("component needs <body>")?;
    let mut gpui_functions = Vec::new();
    let mut next_style_id = 0;
    let tree = element_code(
        body,
        &stylesheet,
        &keyframes,
        &class_bindings,
        &mut next_style_id,
        &mut gpui_functions,
    )?;
    let script = expand_script_option_mappings(script)?;
    let script = expand_script_template_expressions(
        &script,
        &stylesheet,
        &keyframes,
        &mut class_bindings,
        &mut next_style_id,
        &mut gpui_functions,
    )?;
    let script = inject_select_options(&script, &select_option_refs)?;
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
        "// Generated Rust source from {}.\n{}\n{}\npub struct {};\nimpl gpui_rsc::CompiledComponent for {} {{\n    fn template() -> gpui_rsc::TemplateElement {{ {} }}\n}}\nimpl {} {{\n    pub fn definition() -> gpui_rsc::runtime::Definition {{ {} }}\n    pub fn template() -> gpui_rsc::TemplateElement {{ <Self as gpui_rsc::CompiledComponent>::template() }}\n}}\npub fn template() -> gpui_rsc::TemplateElement {{ {}::template() }}\npub fn title() -> &'static str {{ {:?} }}\n",
        input.display(),
        script,
        gpui_functions.join("\n"),
        struct_name,
        struct_name,
        tree,
        struct_name,
        "definition()",
        struct_name,
        title
    );
    let generated = style_codegen::translate(&generated)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if fs::read_to_string(output).ok().as_deref() != Some(&generated) {
        fs::write(output, generated).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(feature = "compiler")]
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
#[cfg(feature = "compiler")]
fn extract_select_option_refs(source: &str) -> Result<(String, Vec<(String, String)>), String> {
    let mut output = String::with_capacity(source.len());
    let mut refs = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("<select") {
        let start = cursor + relative;
        let Some(boundary) = source.as_bytes().get(start + "<select".len()) else {
            break;
        };
        if !boundary.is_ascii_whitespace() && *boundary != b'>' {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }
        let tag_end = start + html_tag_end(&source[start..])?;
        let open_tag = &source[start..tag_end];
        let close_start = source[tag_end..]
            .find("</select>")
            .map(|index| tag_end + index)
            .ok_or("<select> needs a closing tag")?;
        let close_end = close_start + "</select>".len();
        let children = &source[tag_end..close_start];
        let trimmed = children.trim();
        let (variable, expression_start, expression_end) =
            if trimmed.starts_with('{') && trimmed.ends_with('}') {
                let expression_start = children.find('{').unwrap();
                let expression_end = children.rfind('}').unwrap() + 1;
                let expression = trimmed[1..trimmed.len() - 1].trim();
                if is_simple_rust_identifier(expression) {
                    (
                        Some(expression.to_owned()),
                        expression_start,
                        expression_end,
                    )
                } else if expression.contains("=>") {
                    (
                        Some(expand_script_option_mappings(expression)?),
                        expression_start,
                        expression_end,
                    )
                } else {
                    (None, 0, 0)
                }
            } else {
                (None, 0, 0)
            };
        if let Some(variable) = variable {
            let id = html_attribute(open_tag, "id")
                .ok_or("a select with `{options}` needs an `id` attribute")?;
            refs.push((id, variable));
            output.push_str(&source[cursor..tag_end]);
            output.push_str(&children[..expression_start]);
            output.push_str(&children[expression_end..]);
            output.push_str(&source[close_start..close_end]);
        } else {
            output.push_str(&source[cursor..close_end]);
        }
        cursor = close_end;
    }
    output.push_str(&source[cursor..]);
    Ok((output, refs))
}

#[cfg(feature = "compiler")]
fn html_attribute(tag: &str, name: &str) -> Option<String> {
    let mut cursor = 1;
    while cursor < tag.len() {
        while tag
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        let key_start = cursor;
        while tag
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'=' && *byte != b'>')
        {
            cursor += 1;
        }
        if key_start == cursor {
            break;
        }
        let key = &tag[key_start..cursor];
        while tag
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        if tag.as_bytes().get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while tag
            .as_bytes()
            .get(cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            cursor += 1;
        }
        let quote = *tag.as_bytes().get(cursor)?;
        if quote != b'\'' && quote != b'"' {
            return None;
        }
        cursor += 1;
        let value_start = cursor;
        while tag
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| *byte != quote)
        {
            cursor += 1;
        }
        let value_end = cursor;
        cursor += 1;
        if key == name {
            return Some(tag[value_start..value_end].to_owned());
        }
    }
    None
}

#[cfg(feature = "compiler")]
fn is_simple_rust_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
}

#[cfg(feature = "compiler")]
fn normalize_component_tags(source: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("<component ") {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = html_tag_end(rest)?;
        let tag = &rest[..end];
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
        rest = &rest[end..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(feature = "compiler")]
fn expand_template_expressions(source: &str) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("{if") {
        let start = cursor + relative;
        let Some(next) = source.as_bytes().get(start + 3).copied() else {
            break;
        };
        if !next.is_ascii_whitespace() {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }
        let outer_end = rust_brace_end(source, start)?;
        let condition_start = start + 3;
        let body_start = rust_block_start(source, condition_start, outer_end)?;
        let condition = source[condition_start..body_start].trim();
        let (binding, expected) = parse_template_condition(condition)?;
        let body_end = rust_brace_end(source, body_start)?;
        let body = expand_template_expressions(&source[body_start + 1..body_end])?;

        let mut else_body = None;
        let mut expression_end = body_end;
        let after_body = skip_ascii_whitespace(source, body_end + 1);
        if source[after_body..outer_end].starts_with("else") {
            let else_word_end = after_body + "else".len();
            let else_open = skip_ascii_whitespace(source, else_word_end);
            if source.as_bytes().get(else_open) != Some(&b'{') {
                return Err("template `else` needs a braced body".into());
            }
            let else_close = rust_brace_end(source, else_open)?;
            if else_close > outer_end {
                return Err("template `else` extends outside its `{if ...}` expression".into());
            }
            else_body = Some(expand_template_expressions(
                &source[else_open + 1..else_close],
            )?);
            expression_end = else_close;
        }
        if expression_end != outer_end - 1 {
            let trailing = source[expression_end + 1..outer_end].trim();
            if !trailing.is_empty() {
                return Err(format!("unexpected content in template `if`: {trailing:?}"));
            }
        }

        output.push_str(&source[cursor..start]);
        output.push_str(&format!(
            "<rsc-if data-in=\"{}\" data-rsc-equals=\"{}\"><rsc-then>{}</rsc-then>",
            escape_html_attribute(&binding),
            escape_html_attribute(&expected),
            body,
        ));
        if let Some(else_body) = else_body {
            output.push_str(&format!("<rsc-else>{else_body}</rsc-else>"));
        }
        output.push_str("</rsc-if>");
        cursor = outer_end + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(feature = "compiler")]
fn expand_template_interpolations(source: &str) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    let mut in_tag = false;
    let mut quote = None;
    while index < source.len() {
        let character = source[index..].chars().next().unwrap();
        if let Some(delimiter) = quote {
            output.push(character);
            if character == delimiter {
                quote = None;
            }
            index += character.len_utf8();
            continue;
        }
        if in_tag {
            output.push(character);
            match character {
                '\'' | '"' => quote = Some(character),
                '>' => in_tag = false,
                _ => {}
            }
            index += character.len_utf8();
            continue;
        }
        if character == '<' {
            in_tag = true;
            output.push(character);
            index += character.len_utf8();
            continue;
        }
        if character == '{' {
            let end = rust_brace_end(source, index)?;
            let expression = source[index + 1..end].trim();
            if let Some(binding) = template_binding_path(expression) {
                output.push_str(&format!(
                    "<rsc-value data-in=\"{}\"></rsc-value>",
                    escape_html_attribute(&binding)
                ));
                index = end + 1;
                continue;
            }
        }
        output.push(character);
        index += character.len_utf8();
    }
    Ok(output)
}

#[cfg(feature = "compiler")]
fn expand_script_option_mappings(source: &str) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("=>") {
        let arrow = cursor + relative;
        let option_start = skip_ascii_whitespace(source, arrow + 2);
        if !source[option_start..].starts_with("<option") {
            output.push_str(&source[cursor..arrow + 2]);
            cursor = arrow + 2;
            continue;
        }
        let Some(map_start) = source[..arrow].rfind(".map(") else {
            return Err("an option template mapping needs `.map(|item| => ...)`".into());
        };
        let closure = source[map_start + ".map(".len()..arrow].trim();
        if !closure.starts_with('|') || !closure.ends_with('|') || closure.len() < 3 {
            return Err("an option template mapping needs a Rust closure pattern".into());
        }

        let option_tag_end = option_start + html_tag_end(&source[option_start..])?;
        let option_tag = &source[option_start..option_tag_end];
        let value_open = option_tag
            .find("value=")
            .map(|index| index + "value=".len())
            .ok_or("an option template needs `value=[expression]`")?;
        let value_open = skip_ascii_whitespace(option_tag, value_open);
        let (value_start, value_close) = match option_tag.as_bytes().get(value_open) {
            Some(b'[') => (
                value_open + 1,
                option_tag[value_open + 1..]
                    .find(']')
                    .map(|index| value_open + index + 1)
                    .ok_or("unclosed option value expression")?,
            ),
            Some(b'{') => (value_open + 1, rust_brace_end(option_tag, value_open)?),
            _ => {
                return Err(
                    "an option template needs `value=[expression]` or `value={expression}`".into(),
                );
            }
        };
        let value = option_tag[value_start..value_close].trim();
        if value.is_empty() {
            return Err("an option value expression cannot be empty".into());
        }

        let close_tag = "</option>";
        let close_start = source[option_tag_end..]
            .find(close_tag)
            .map(|index| option_tag_end + index)
            .ok_or("an option template needs `</option>`")?;
        let label_source = &source[option_tag_end..close_start];
        let label_open = label_source
            .find('{')
            .ok_or("an option template needs `{label}`")?;
        let label_end = rust_brace_end(label_source, label_open)?;
        if !label_source[..label_open].trim().is_empty()
            || !label_source[label_end + 1..].trim().is_empty()
        {
            return Err("an option template label must be a single `{expression}`".into());
        }
        let label = label_source[label_open + 1..label_end].trim();
        if label.is_empty() {
            return Err("an option label expression cannot be empty".into());
        }

        output.push_str(&source[cursor..arrow]);
        output.push_str(&format!(
            "gpui_rsc::runtime::SelectOption::new(({value}).to_string(), ({label}).to_string())"
        ));
        cursor = close_start + close_tag.len();
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(feature = "compiler")]
fn expand_script_template_expressions(
    source: &str,
    stylesheet: &[CssRule],
    keyframes: &HashMap<String, Keyframes>,
    class_bindings: &mut HashMap<usize, String>,
    next_style_id: &mut usize,
    gpui_functions: &mut Vec<String>,
) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("{if") {
        let start = cursor + relative;
        let Some(next) = source.as_bytes().get(start + 3).copied() else {
            break;
        };
        if !next.is_ascii_whitespace() {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }
        let Ok(outer_end) = rust_brace_end(source, start) else {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        };
        let condition_start = start + 3;
        let Ok(body_start) = rust_block_start(source, condition_start, outer_end) else {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        };
        if parse_template_condition(source[condition_start..body_start].trim()).is_err()
            || source[skip_ascii_whitespace(source, body_start + 1)..].starts_with('<') == false
        {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }

        let template = expand_template_expressions(&source[start..outer_end + 1])?;
        let template = expand_template_interpolations(&template)?;
        let template = normalize_component_tags(&template)?;
        let class_binding_offset = class_bindings.len();
        let (template, local_bindings) =
            extract_class_bindings_at(&template, class_binding_offset)?;
        class_bindings.extend(local_bindings);

        let document = Html::parse_document(&format!("<html><body>{template}</body></html>"));
        let conditional = document
            .select(&Selector::parse("body > rsc-if").unwrap())
            .next()
            .ok_or("script template conditional must have one root element")?;
        let expression = element_code(
            conditional,
            stylesheet,
            keyframes,
            class_bindings,
            next_style_id,
            gpui_functions,
        )?;
        output.push_str(&source[cursor..start]);
        output.push_str(&expression);
        cursor = outer_end + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(feature = "compiler")]
fn inject_select_options(source: &str, refs: &[(String, String)]) -> Result<String, String> {
    if refs.is_empty() {
        return Ok(source.to_owned());
    }
    let mut file = syn::parse_file(source)
        .map_err(|error| format!("cannot attach select option variables: {error}"))?;
    let definition = file.items.iter_mut().find_map(|item| match item {
        syn::Item::Fn(function) if function.sig.ident == "definition" => Some(function),
        _ => None,
    });
    let definition =
        definition.ok_or("select `{options}` needs a script `definition()` function")?;
    let tail = definition
        .block
        .stmts
        .last_mut()
        .ok_or("script `definition()` needs to return a Definition")?;
    let mut expression = match tail {
        syn::Stmt::Expr(expression, None) => expression.clone(),
        syn::Stmt::Macro(statement) if statement.semi_token.is_none() => {
            syn::Expr::Macro(syn::ExprMacro {
                attrs: statement.attrs.clone(),
                mac: statement.mac.clone(),
            })
        }
        _ => return Err("script `definition()` must end with a Definition expression".into()),
    };
    for (id, variable) in refs {
        let id = syn::parse_str::<syn::LitStr>(&format!("{id:?}"))
            .map_err(|error| format!("invalid select id: {error}"))?;
        let variable = syn::parse_str::<syn::Expr>(variable)
            .map_err(|error| format!("invalid select options variable: {error}"))?;
        expression = syn::parse_quote!((#expression).with_select_options(#id, #variable));
    }
    *tail = syn::Stmt::Expr(expression, None);
    Ok(quote::quote!(#file).to_string())
}

#[cfg(feature = "compiler")]
fn template_binding_path(expression: &str) -> Option<String> {
    let binding = expression
        .trim()
        .strip_prefix("data.")
        .unwrap_or(expression.trim());
    if binding.is_empty()
        || !binding
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || binding
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some(binding.to_owned())
}

#[cfg(feature = "compiler")]
fn rust_block_start(source: &str, start: usize, end: usize) -> Result<usize, String> {
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in source[start..end].char_indices() {
        let index = start + relative;
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == delimiter {
                quote = None;
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character == '{' {
            return Ok(index);
        }
    }
    Err("template `if` needs a braced body".into())
}

#[cfg(feature = "compiler")]
fn parse_template_condition(condition: &str) -> Result<(String, String), String> {
    let (binding, value) = condition
        .split_once("===")
        .or_else(|| condition.split_once("=="))
        .ok_or_else(|| format!("unsupported template condition {condition:?}"))?;
    let binding = binding
        .trim()
        .strip_prefix("data.")
        .unwrap_or(binding.trim());
    if binding.is_empty()
        || !binding
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || binding
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return Err(format!(
            "invalid template binding in condition {condition:?}"
        ));
    }
    let value = value.trim();
    let quote = value
        .chars()
        .next()
        .ok_or("template condition needs a value")?;
    if (quote != '"' && quote != '\'') || !value.ends_with(quote) || value.len() < 2 {
        return Err(format!(
            "template condition needs a quoted value: {condition:?}"
        ));
    }
    let value = &value[1..value.len() - 1];
    Ok((
        binding.to_owned(),
        value.replace("\\\"", "\"").replace("\\'", "'"),
    ))
}

#[cfg(feature = "compiler")]
fn skip_ascii_whitespace(source: &str, mut index: usize) -> usize {
    while source
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

#[cfg(feature = "compiler")]
fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(feature = "compiler")]
fn extract_class_bindings(source: &str) -> Result<(String, HashMap<usize, String>), String> {
    extract_class_bindings_at(source, 0)
}

#[cfg(feature = "compiler")]
fn extract_class_bindings_at(
    source: &str,
    id_offset: usize,
) -> Result<(String, HashMap<usize, String>), String> {
    let mut html = String::with_capacity(source.len());
    let mut bindings = HashMap::new();
    let mut remaining = source;
    while let Some(start) = remaining.find('<') {
        html.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        if remaining.starts_with("<!--") {
            let end = remaining.find("-->").ok_or("unterminated HTML comment")? + 3;
            html.push_str(&remaining[..end]);
            remaining = &remaining[end..];
            continue;
        }
        let end = html_tag_end(remaining)?;
        let tag = &remaining[..end];
        if tag.starts_with("</") || tag.starts_with("<!") || tag.starts_with("<?") {
            html.push_str(tag);
        } else {
            html.push_str(&replace_class_bindings(tag, &mut bindings, id_offset)?);
        }
        remaining = &remaining[end..];
    }
    html.push_str(remaining);
    Ok((html, bindings))
}

#[cfg(feature = "compiler")]
fn html_tag_end(tag: &str) -> Result<usize, String> {
    let mut quote = None;
    let mut escaped = false;
    let mut braces = 0usize;
    for (index, character) in tag.char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == delimiter {
                quote = None;
            }
        } else {
            match character {
                '\'' | '"' => quote = Some(character),
                '{' => braces += 1,
                '}' if braces > 0 => braces -= 1,
                '>' if braces == 0 => return Ok(index + 1),
                _ => {}
            }
        }
    }
    Err("unterminated HTML tag or Rust class expression".into())
}

#[cfg(feature = "compiler")]
fn replace_class_bindings(
    tag: &str,
    bindings: &mut HashMap<usize, String>,
    id_offset: usize,
) -> Result<String, String> {
    let mut result = String::with_capacity(tag.len());
    let bytes = tag.as_bytes();
    let mut cursor = 0;
    let mut copied = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut found = false;
    while cursor < bytes.len() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if bytes[cursor] == b'\\' {
                escaped = true;
            } else if bytes[cursor] == delimiter {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'\'' || bytes[cursor] == b'"' {
            quote = Some(bytes[cursor]);
            cursor += 1;
            continue;
        }
        if !bytes[cursor..].starts_with(b"class")
            || cursor == 0
            || !bytes[cursor - 1].is_ascii_whitespace()
        {
            cursor += 1;
            continue;
        }
        let mut value_start = cursor + 5;
        while tag
            .as_bytes()
            .get(value_start)
            .is_some_and(u8::is_ascii_whitespace)
        {
            value_start += 1;
        }
        if tag.as_bytes().get(value_start) != Some(&b'=') {
            cursor += 5;
            continue;
        }
        value_start += 1;
        while tag
            .as_bytes()
            .get(value_start)
            .is_some_and(u8::is_ascii_whitespace)
        {
            value_start += 1;
        }
        if tag.as_bytes().get(value_start) != Some(&b'{') {
            cursor += 5;
            continue;
        }
        if found {
            return Err("an element may have only one class={...} binding".into());
        }
        let close = rust_brace_end(tag, value_start)?;
        let expression = tag[value_start + 1..close].trim();
        if expression.is_empty() {
            return Err("class={...} needs a Rust style expression".into());
        }
        let id = id_offset + bindings.len();
        bindings.insert(id, expression.to_owned());
        result.push_str(&tag[copied..cursor]);
        result.push_str(&format!(" data-rsc-class-binding=\"{id}\""));
        cursor = close + 1;
        copied = cursor;
        found = true;
    }
    result.push_str(&tag[copied..]);
    Ok(result)
}

#[cfg(feature = "compiler")]
fn rust_brace_end(source: &str, open: usize) -> Result<usize, String> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in source[open..].char_indices() {
        let index = open + relative;
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == delimiter {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err("unclosed class={...} expression".into())
}

#[cfg(feature = "compiler")]
#[derive(Clone)]
struct CssRule {
    selector: Selector,
    max_width: Option<f32>,
    declarations: String,
}

#[cfg(feature = "compiler")]
#[derive(Clone, Copy, Default)]
struct AnimationFrame {
    top: Option<f32>,
    bottom: Option<f32>,
    opacity: Option<f32>,
}

#[cfg(feature = "compiler")]
#[derive(Clone, Copy)]
struct Keyframes {
    from: AnimationFrame,
    to: AnimationFrame,
}

#[cfg(feature = "compiler")]
fn component_styles(
    document: &Html,
    input: &Path,
) -> Result<(Vec<CssRule>, HashMap<String, Keyframes>), String> {
    let style_selector = Selector::parse("style").expect("static style selector is valid");
    let mut rules = Vec::new();
    let mut keyframes = HashMap::new();
    for style in document.select(&style_selector) {
        let source = style.text().collect::<String>();
        parse_stylesheet(&source, None, &mut rules, &mut keyframes).map_err(|error| {
            format!(
                "{} has an invalid component stylesheet: {error}",
                input.display()
            )
        })?;
    }
    Ok((rules, keyframes))
}

#[cfg(feature = "compiler")]
fn parse_stylesheet(
    source: &str,
    inherited_max_width: Option<f32>,
    rules: &mut Vec<CssRule>,
    keyframes: &mut HashMap<String, Keyframes>,
) -> Result<(), String> {
    let source = strip_css_comments(source)?;
    let mut remaining = source.as_str();
    loop {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }
        let Some(open) = remaining.find('{') else {
            if remaining.trim().is_empty() {
                break;
            }
            return Err(format!("expected '{{' after {}", remaining.trim()));
        };
        let header = remaining[..open].trim();
        let close = matching_brace(remaining, open)
            .ok_or_else(|| format!("missing closing '}}' for {header}"))?;
        let body = &remaining[open + 1..close];
        if let Some(condition) = header.strip_prefix("@media") {
            let media_width = parse_media_max_width(condition)?;
            let max_width = inherited_max_width
                .map(|parent| parent.min(media_width))
                .unwrap_or(media_width);
            parse_stylesheet(body, Some(max_width), rules, keyframes)?;
        } else if let Some(name) = header.strip_prefix("@keyframes") {
            let name = name.trim();
            if name.is_empty() {
                return Err("@keyframes needs a name".into());
            }
            keyframes.insert(name.to_owned(), parse_keyframes(body)?);
        } else if !header.starts_with('@') {
            let selector = Selector::parse(header)
                .map_err(|error| format!("invalid selector {header:?}: {error:?}"))?;
            gpui_style_calls(body)?;
            rules.push(CssRule {
                selector,
                max_width: inherited_max_width,
                declarations: body.trim().to_owned(),
            });
        }
        remaining = &remaining[close + 1..];
    }
    Ok(())
}

#[cfg(feature = "compiler")]
fn parse_keyframes(source: &str) -> Result<Keyframes, String> {
    let mut remaining = source;
    let mut from = None;
    let mut to = None;
    loop {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }
        let open = remaining
            .find('{')
            .ok_or("keyframe needs a declaration block")?;
        let selector = remaining[..open].trim();
        let close = matching_brace(remaining, open).ok_or("unclosed keyframe block")?;
        let frame = parse_animation_frame(&remaining[open + 1..close])?;
        match selector {
            "from" | "0%" => from = Some(frame),
            "to" | "100%" => to = Some(frame),
            _ => return Err(format!("unsupported keyframe stop {selector:?}")),
        }
        remaining = &remaining[close + 1..];
    }
    Ok(Keyframes {
        from: from.ok_or("@keyframes needs a from block")?,
        to: to.ok_or("@keyframes needs a to block")?,
    })
}

#[cfg(feature = "compiler")]
fn parse_animation_frame(source: &str) -> Result<AnimationFrame, String> {
    let mut frame = AnimationFrame::default();
    for declaration in source
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (name, value) = declaration
            .split_once(':')
            .ok_or_else(|| format!("invalid keyframe declaration {declaration:?}"))?;
        let value = value.trim();
        match name.trim() {
            "top" => {
                frame.top = Some(
                    css_number(value)?
                        .parse()
                        .map_err(|_| "invalid top keyframe")?,
                )
            }
            "bottom" => {
                frame.bottom = Some(
                    css_number(value)?
                        .parse()
                        .map_err(|_| "invalid bottom keyframe")?,
                )
            }
            "opacity" => {
                frame.opacity = Some(
                    css_opacity(value)?
                        .parse()
                        .map_err(|_| "invalid opacity keyframe")?,
                )
            }
            name => return Err(format!("unsupported keyframe property {name:?}")),
        }
    }
    Ok(frame)
}

#[cfg(feature = "compiler")]
fn strip_css_comments(source: &str) -> Result<String, String> {
    let mut result = String::with_capacity(source.len());
    let mut remaining = source;
    while let Some(start) = remaining.find("/*") {
        result.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("*/") else {
            return Err("unterminated CSS comment".into());
        };
        result.push(' ');
        remaining = &after_start[end + 2..];
    }
    result.push_str(remaining);
    Ok(result)
}

#[cfg(feature = "compiler")]
fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(feature = "compiler")]
fn parse_media_max_width(condition: &str) -> Result<f32, String> {
    let (_, value) = condition
        .split_once("max-width")
        .ok_or_else(|| format!("only max-width media queries are supported: {condition}"))?;
    let value = value
        .trim_start()
        .strip_prefix(':')
        .ok_or_else(|| format!("invalid max-width media query: {condition}"))?
        .trim()
        .split(')')
        .next()
        .unwrap_or("")
        .trim();
    let number = value
        .strip_suffix("px")
        .unwrap_or(value)
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid max-width value: {condition}"))?;
    if !number.is_finite() || number <= 0.0 {
        return Err(format!(
            "max-width must be a positive pixel width: {condition}"
        ));
    }
    Ok(number)
}

#[cfg(feature = "compiler")]
fn element_code(
    element: ElementRef<'_>,
    stylesheet: &[CssRule],
    keyframes: &HashMap<String, Keyframes>,
    class_bindings: &HashMap<usize, String>,
    next_style_id: &mut usize,
    gpui_functions: &mut Vec<String>,
) -> Result<String, String> {
    let style_id = *next_style_id;
    *next_style_id += 1;
    let element_responsive = stylesheet
        .iter()
        .filter(|rule| rule.max_width.is_some() && rule.selector.matches(&element))
        .collect::<Vec<_>>();

    let attrs = element
        .value()
        .attrs()
        .filter(|(key, _)| {
            *key != "style" && *key != "mobile-style" && *key != "data-rsc-class-binding"
        })
        .map(|(k, v)| format!("({:?}.into(), {:?}.into())", k, v))
        .collect::<Vec<_>>();
    let base_declarations = stylesheet
        .iter()
        .filter(|rule| rule.max_width.is_none() && rule.selector.matches(&element))
        .map(|rule| rule.declarations.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let attrs = attrs.join(",");
    let class_binding = element
        .value()
        .attr("data-rsc-class-binding")
        .map(|id| {
            let id = id
                .parse::<usize>()
                .map_err(|_| format!("invalid class binding identifier {id:?}"))?;
            class_bindings
                .get(&id)
                .map(String::as_str)
                .ok_or_else(|| format!("missing class binding {id}"))
        })
        .transpose()?;
    let base_gpui_calls = gpui_style_calls(&base_declarations)?;
    let mobile_style = element
        .value()
        .attr("mobile-style")
        .filter(|style| !style.trim().is_empty());
    let inline_style = element
        .value()
        .attr("style")
        .filter(|style| !style.trim().is_empty());
    if element_responsive
        .iter()
        .any(|rule| animation_declaration(&rule.declarations).is_some())
        || element
            .value()
            .attr("mobile-style")
            .and_then(animation_declaration)
            .is_some()
    {
        return Err("responsive animation is not supported yet".into());
    }
    let animation = element
        .value()
        .attr("style")
        .and_then(animation_declaration)
        .or_else(|| animation_declaration(&base_declarations));
    let animation_fn = if let Some(animation) = animation {
        let name = format!("__rsc_animate_{style_id}");
        let call = gpui_animation_call(animation, keyframes, &name)?;
        gpui_functions.push(format!(
            "fn {name}(element: gpui::Div) -> gpui::AnyElement {{\n    use gpui_kit::AnimationExt as _;\n    use gpui_kit::*;\n    use std::time::Duration;\n    element{call}.into_any_element()\n}}"
        ));
        Some(name)
    } else {
        None
    };
    let mut child_codes = Vec::new();
    let mut child_renders = Vec::new();
    for child in element.children() {
        match child.value() {
            HtmlNode::Text(text) if !text.trim().is_empty() => {
                let text = text.trim();
                child_codes.push(format!("gpui_rsc::TemplateNode::Text({text:?}.into())"));
                child_renders.push(format!("    container = container.child({text:?});\n"));
            }
            HtmlNode::Element(_) => {
                let Some(child_element) = ElementRef::wrap(child) else {
                    continue;
                };
                if child_element.value().name() == "style" {
                    continue;
                }
                let child_id = *next_style_id;
                let code = element_code(
                    child_element,
                    stylesheet,
                    keyframes,
                    class_bindings,
                    next_style_id,
                    gpui_functions,
                )?;
                let child_index = child_codes.len();
                child_codes.push(format!("gpui_rsc::TemplateNode::Element({code})"));
                child_renders.push(format!(
                    "    container = container.child(__rsc_render_{child_id}(view, view.child_element(element, {child_index}), viewport_width, cx));\n"
                ));
            }
            _ => {}
        }
    }
    if element.value().name() == "component" {
        child_renders = vec!["    container = container.child(view.render_node(&element.children[0], viewport_width, cx));\n".to_owned()];
    }
    let children = child_codes.join(",");
    let render_name = format!("__rsc_render_{style_id}");
    if animation_fn.is_some()
        && (matches!(
            element.value().name(),
            "output" | "input" | "select" | "option"
        ) || (element.value().name() == "button" && element.value().attr("data-out").is_some()))
    {
        return Err(format!(
            "animation on <{}> is not supported",
            element.value().name()
        ));
    }
    let mut render_body = format!("let mut container = div(){base_gpui_calls};\n");
    if let Some(mobile_style) = mobile_style {
        let calls = gpui_style_calls(mobile_style)?;
        render_body.push_str(&format!(
            "    if view.is_mobile(viewport_width) {{ container = container{calls}; }}\n"
        ));
    }
    for rule in &element_responsive {
        let max_width = rule.max_width.unwrap();
        let calls = gpui_style_calls(&rule.declarations)?;
        render_body.push_str(&format!(
            "    if viewport_width <= {max_width:?} {{ container = container{calls}; }}\n"
        ));
    }
    if let Some(expression) = class_binding {
        render_body.push_str(
            "    let style_context = gpui_rsc::runtime::StyleContext { viewport_width, snapshot: view.snapshot() };\n    let context = &style_context;\n",
        );
        if expression.contains("styles.") {
            render_body.push_str("    let styles = styles(context);\n");
        }
        render_body.push_str(&format!(
            "    let bound_class_style: gpui_rsc::runtime::Style = {expression};\n    gpui::Refineable::refine(container.style(), &bound_class_style);\n"
        ));
    }
    if let Some(inline_style) = inline_style {
        let calls = gpui_style_calls(inline_style)?;
        render_body.push_str(&format!("    container = container{calls};\n"));
    }
    let content = match element.value().name() {
        "rsc-if" => "view.render_if(element, container, viewport_width, cx)".to_owned(),
            "output" | "rsc-value" => "view.render_output(element, container)".to_owned(),
            "input" => "{ let control_style = container.style().clone(); view.render_slider(element, container, control_style) }".to_owned(),
            "select" => "view.render_select(element, container)".to_owned(),
            "button" if element.value().attr("data-out").is_some() => {
                "{ let control_style = container.style().clone(); view.render_button(element, container, control_style, cx) }".to_owned()
            }
            "option" => "container.into_any_element()".to_owned(),
            _ => {
                let mut body = String::new();
                for render in child_renders {
                    body.push_str(&render);
                }
                if let Some(name) = &animation_fn {
                    body.push_str(&format!("    {name}(container)"));
                } else {
                    body.push_str("    container.into_any_element()");
                }
                body
            }
    };
    render_body.push_str(&content);
    gpui_functions.push(format!(
        "#[allow(unused_imports, unused_variables, unused_mut)]\nfn {render_name}(view: &gpui_rsc::runtime::view::HtmlView, element: &gpui_rsc::runtime::binding::Element, viewport_width: f32, cx: &mut gpui::Context<gpui_rsc::runtime::view::HtmlView>) -> gpui::AnyElement {{\n    use gpui_kit::*;\n    use gpui_rsc::runtime::Style;\n    {render_body}\n}}"
    ));
    Ok(format!(
        "gpui_rsc::TemplateElement::new({:?}, vec![{}], vec![{}]).with_render({render_name})",
        element.value().name(),
        attrs,
        children
    ))
}

#[cfg(feature = "compiler")]
fn animation_declaration(source: &str) -> Option<&str> {
    source
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == "animation")
        .map(|(_, value)| value.trim())
        .last()
}

#[cfg(feature = "compiler")]
fn gpui_style_calls(source: &str) -> Result<String, String> {
    let mut calls = String::new();
    for declaration in source
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (name, value) = declaration
            .split_once(':')
            .ok_or_else(|| format!("invalid style declaration {declaration:?}"))?;
        let (name, value) = (name.trim(), value.trim());
        let call = match name {
            "display" if value == "flex" => ".flex()".to_owned(),
            "flex-direction" if value == "column" => ".flex_col()".to_owned(),
            "flex-direction" if value == "row" => ".flex_row()".to_owned(),
            "flex-wrap" if value == "wrap" => ".flex_wrap()".to_owned(),
            "flex-wrap" if value == "nowrap" => ".flex_nowrap()".to_owned(),
            "flex" if value == "1" => ".flex_1()".to_owned(),
            "flex" if value == "0" || value == "none" => ".flex_none()".to_owned(),
            "gap" => format!(".gap(px({}))", css_number(value)?),
            "padding" => {
                let [top, right, bottom, left] = css_box_values(value)?;
                format!(".pt(px({top})).pr(px({right})).pb(px({bottom})).pl(px({left}))")
            }
            "padding-top" => format!(".pt(px({}))", css_number(value)?),
            "padding-right" => format!(".pr(px({}))", css_number(value)?),
            "padding-bottom" => format!(".pb(px({}))", css_number(value)?),
            "padding-left" => format!(".pl(px({}))", css_number(value)?),
            "background" | "background-color" => {
                format!(".bg(rgb({}))", css_color(value)?)
            }
            "color" => format!(".text_color(rgb({}))", css_color(value)?),
            "font-size" => format!(".text_size(px({}))", css_number(value)?),
            "font-weight" => format!(".font_weight(FontWeight({:?}))", css_integer(value)? as f32),
            "border" => {
                let mut parts = value.split_whitespace();
                let width = parts.next().ok_or("border needs a width")?;
                let color = parts.last().ok_or("border needs a color")?;
                let width = css_number(width)?;
                format!(
                    ".border(px({width})).border_color(rgb({}))",
                    css_color(color)?
                )
            }
            "border-radius" => format!(".rounded(px({}))", css_number(value)?),
            "width" => format!(".w({})", css_gpui_length(value)?),
            "height" => format!(".h({})", css_gpui_length(value)?),
            "min-width" => format!(".min_w(px({}))", css_number(value)?),
            "max-width" => format!(".max_w(px({}))", css_number(value)?),
            "margin" if value == "auto" => ".mx_auto()".to_owned(),
            "justify-content" if value == "space-between" => ".justify_between()".to_owned(),
            "justify-content" if value == "center" => ".justify_center()".to_owned(),
            "justify-content" if value == "flex-end" => ".justify_end()".to_owned(),
            "justify-content" if value == "flex-start" => ".justify_start()".to_owned(),
            "align-items" if value == "center" => ".items_center()".to_owned(),
            "align-items" if value == "flex-start" => ".items_start()".to_owned(),
            "align-items" if value == "flex-end" => ".items_end()".to_owned(),
            "position" if value == "relative" => ".relative()".to_owned(),
            "position" if value == "absolute" => ".absolute()".to_owned(),
            "top" => format!(".top(px({}))", css_number(value)?),
            "right" => format!(".right(px({}))", css_number(value)?),
            "bottom" => format!(".bottom(px({}))", css_number(value)?),
            "left" => format!(".left(px({}))", css_number(value)?),
            "overflow" if value == "hidden" => ".overflow_hidden()".to_owned(),
            "opacity" => format!(".opacity({})", css_opacity(value)?),
            "animation" => String::new(),
            // The generated document container owns GPUI scrolling.
            "overflow-y" if value == "auto" || value == "scroll" => String::new(),
            "overflow-y" if value == "hidden" => ".overflow_y_hidden()".to_owned(),
            "overflow-y" if value == "visible" => String::new(),
            _ => return Err(format!("unsupported GPUI style {name}:{value}")),
        };
        calls.push_str(&call);
    }
    Ok(calls)
}

#[cfg(feature = "compiler")]
fn gpui_animation_call(
    value: &str,
    keyframes: &HashMap<String, Keyframes>,
    function_name: &str,
) -> Result<String, String> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let [name, duration, "infinite"] = parts.as_slice() else {
        return Err(format!(
            "animation needs a name, duration in ms, and infinite: {value:?}"
        ));
    };
    let duration = duration
        .strip_suffix("ms")
        .ok_or_else(|| format!("animation duration must use ms: {value:?}"))?
        .parse::<u64>()
        .map_err(|_| format!("invalid animation duration: {value:?}"))?;
    if duration == 0 {
        return Err("animation duration must be positive".into());
    }
    let frames = keyframes
        .get(*name)
        .ok_or_else(|| format!("unknown @keyframes {name:?}"))?;
    let mut calls = String::new();
    for (property, from, to) in [
        ("top", frames.from.top, frames.to.top),
        ("bottom", frames.from.bottom, frames.to.bottom),
        ("opacity", frames.from.opacity, frames.to.opacity),
    ] {
        match (from, to) {
            (Some(from), Some(to)) => {
                let value = format!("{from:?} + delta * {:?}", to - from);
                calls.push_str(&match property {
                    "top" => format!(".top(px({value}))"),
                    "bottom" => format!(".bottom(px({value}))"),
                    _ => format!(".opacity({value})"),
                });
            }
            (None, None) => {}
            _ => return Err(format!("{property} must be set in both animation stops")),
        }
    }
    if calls.is_empty() {
        return Err(format!("@keyframes {name:?} has no supported properties"));
    }
    Ok(format!(
        ".with_animation({:?}, Animation::new(Duration::from_millis({duration})).repeat_synced(), |this, delta| this{calls})",
        format!("{function_name}-{name}")
    ))
}

#[cfg(feature = "compiler")]
fn css_gpui_length(value: &str) -> Result<String, String> {
    if let Some(percent) = value.strip_suffix('%') {
        let number = percent
            .parse::<f32>()
            .map_err(|_| format!("invalid CSS percentage {value:?}"))?;
        if !number.is_finite() {
            return Err(format!("invalid CSS percentage {value:?}"));
        }
        Ok(format!("relative({:?})", number / 100.0))
    } else {
        Ok(format!("px({})", css_number(value)?))
    }
}

#[cfg(feature = "compiler")]
fn css_opacity(value: &str) -> Result<String, String> {
    let number = value
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid CSS opacity {value:?}"))?;
    if !number.is_finite() || !(0.0..=1.0).contains(&number) {
        return Err(format!("invalid CSS opacity {value:?}"));
    }
    Ok(format!("{number:?}"))
}

#[cfg(feature = "compiler")]
fn css_number(value: &str) -> Result<String, String> {
    let number = value.trim().strip_suffix("px").unwrap_or(value.trim());
    let parsed = number
        .parse::<f32>()
        .map_err(|_| format!("invalid CSS length {value:?}"))?;
    if !parsed.is_finite() {
        return Err(format!("invalid CSS length {value:?}"));
    }
    Ok(format!("{parsed:?}"))
}

#[cfg(feature = "compiler")]
fn css_integer(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("invalid CSS integer {value:?}"))
}

#[cfg(feature = "compiler")]
fn css_color(value: &str) -> Result<String, String> {
    let hex = value
        .strip_prefix('#')
        .ok_or_else(|| format!("unsupported CSS color {value:?}"))?;
    let color = u32::from_str_radix(hex, 16).map_err(|_| format!("invalid CSS color {value:?}"))?;
    if hex.len() != 3 && hex.len() != 6 {
        return Err(format!("unsupported CSS color {value:?}"));
    }
    let color = if hex.len() == 3 {
        let r = (color >> 8) & 0xf;
        let g = (color >> 4) & 0xf;
        let b = color & 0xf;
        (r * 0x11 << 16) | (g * 0x11 << 8) | (b * 0x11)
    } else {
        color
    };
    Ok(format!("0x{color:06x}"))
}

#[cfg(feature = "compiler")]
fn css_box_values(value: &str) -> Result<[String; 4], String> {
    let values = value
        .split_whitespace()
        .map(css_number)
        .collect::<Result<Vec<_>, _>>()?;
    match values.as_slice() {
        [all] => Ok([all.clone(), all.clone(), all.clone(), all.clone()]),
        [vertical, horizontal] => Ok([
            vertical.clone(),
            horizontal.clone(),
            vertical.clone(),
            horizontal.clone(),
        ]),
        [top, horizontal, bottom] => Ok([
            top.clone(),
            horizontal.clone(),
            bottom.clone(),
            horizontal.clone(),
        ]),
        [top, right, bottom, left] => {
            Ok([top.clone(), right.clone(), bottom.clone(), left.clone()])
        }
        _ => Err(format!("invalid CSS padding {value:?}")),
    }
}
