#[cfg(feature = "compiler")]
mod style_codegen;
// Compiler for single-file Rust JSX components.
#[cfg(feature = "compiler")]
use scraper::{ElementRef, Html, Node as HtmlNode, Selector};
#[cfg(feature = "compiler")]
use quote::ToTokens;
#[cfg(feature = "compiler")]
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(feature = "compiler")]
pub fn compile_file(input: &Path, output: &Path) -> Result<(), String> {
    if input.extension().and_then(|s| s.to_str()) != Some("rsx") {
        return Err(format!("{} is not .rsx", input.display()));
    }
    let source = fs::read_to_string(input).map_err(|e| e.to_string())?;
    if !source.contains("<script>") {
        if let Some((script, components)) = rust_component_functions(&source, input)? {
            return compile_rust_components(input, output, &script, &components);
        }
    }
    let sections = component_sections(&source, input)?;
    let (template, select_option_refs) = extract_select_option_refs(&sections.template)?;
    let template = expand_template_expressions(&template)?;
    let template = expand_template_interpolations(&template)?;
    let component_functions = component_function_tags(&template)?;
    let template = normalize_component_tags(&template)?;
    let (template, mut class_bindings) = extract_class_bindings(&template)?;
    let document_source = ensure_html_document(&template);
    let document = Html::parse_document(&document_source);
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid filename")?;
    let component_function = component_function_name(stem);
    let imports = component_functions
        .iter()
        .map(|name| format!("{name}()"))
        .collect::<Vec<_>>();
    let definition_expression = if imports.is_empty() {
        "definition()".to_owned()
    } else {
        format!("definition().with_imports(vec![{}])", imports.join(", "))
    };
    let body = document
        .select(&Selector::parse("body").unwrap())
        .next()
        .ok_or("component needs <body>")?;
    let mut gpui_functions = Vec::new();
    let mut next_style_id = 0;
    let tree = element_code(
        body,
        &class_bindings,
        &HashMap::new(),
        "",
        &mut next_style_id,
        &mut gpui_functions,
    )?;
    let script = expand_script_option_mappings(&sections.script)?;
    let script = expand_script_template_expressions(
        &script,
        &mut class_bindings,
        &mut next_style_id,
        &mut gpui_functions,
    )?;
    let script = inject_select_options(&script, &select_option_refs)?;
    let struct_name = format!("{}Component", component_function_name(stem));
    let view_inputs = component_view_inputs(&sections.script);
    let view_inputs_fn = format!(
        "#[allow(dead_code)]\nfn __rsc_generated_view_inputs() -> &'static [&'static str] {{ &[{}] }}\n",
        view_inputs
            .iter()
            .map(|name| format!("{name:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let default_title = if script.contains("fn title(") {
        String::new()
    } else {
        format!("pub fn title() -> &'static str {{ {stem:?} }}\n")
    };
    // Script is inserted verbatim. It can contain any Rust items, functions, and methods.
    let generated = format!(
        "// Generated Rust source from {}.\n{}\n{}\n{}\n{}\npub struct {};\nimpl gpui_rsc::CompiledComponent for {} {{\n    fn template() -> gpui_rsc::TemplateElement {{ {} }}\n}}\nimpl {} {{\n    pub fn definition() -> gpui_rsc::runtime::Definition {{ {} }}\n    pub fn template() -> gpui_rsc::TemplateElement {{ <Self as gpui_rsc::CompiledComponent>::template() }}\n}}\npub fn template() -> gpui_rsc::TemplateElement {{ {}::template() }}\n#[allow(non_snake_case)]\npub fn {}() -> gpui_rsc::runtime::Definition {{ {}::definition() }}\n#[allow(unused_variables)]\npub fn __rsc_render_component(view: &mut gpui_rsc::runtime::view::HtmlView, props: &gpui_rsc::runtime::ComponentProps, viewport_width: f32, window: &mut gpui::Window, cx: &mut gpui::Context<gpui_rsc::runtime::view::HtmlView>) -> gpui::AnyElement {{ view.render_root(props, viewport_width, window, cx) }}\n",
        input.display(),
        script,
        default_title,
        view_inputs_fn,
        gpui_functions.join("\n"),
        struct_name,
        struct_name,
        tree,
        struct_name,
        definition_expression,
        struct_name,
        component_function,
        struct_name,
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
struct RustComponentFunction {
    name: String,
    inputs: Vec<RustComponentInput>,
    prelude: String,
    template: String,
}

#[cfg(feature = "compiler")]
struct RustComponentInput {
    name: String,
    ty: String,
    mutable: bool,
    direction: &'static str,
}

#[cfg(feature = "compiler")]
fn rust_component_functions(
    source: &str,
    input: &Path,
) -> Result<Option<(String, Vec<RustComponentFunction>)>, String> {
    let source = source.to_owned();
    let mut components = Vec::new();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("pub fn ") {
        let start = cursor + relative;
        let Some(args_open_rel) = source[start..].find('(') else {
            break;
        };
        let args_open = start + args_open_rel;
        let args_close = matching_delimiter(&source, args_open, b'(', b')').ok_or_else(|| {
            format!(
                "{} has an unclosed function parameter list",
                input.display()
            )
        })?;
        let Some(body_open_rel) = source[args_close + 1..].find('{') else {
            break;
        };
        let body_open = args_close + 1 + body_open_rel;
        let body_close = rust_brace_end(&source, body_open).map_err(|error| {
            format!(
                "{} has an invalid component function: {error}",
                input.display()
            )
        })?;
        let body = &source[body_open + 1..body_close];
        let Some(markup_start) = find_component_markup_start(body) else {
            cursor = body_close + 1;
            continue;
        };
        let header = format!("{}{{}}", &source[start..body_open]);
        let item = syn::parse_str::<syn::ItemFn>(&header).map_err(|error| {
            format!(
                "{} has an invalid component function signature: {error}",
                input.display()
            )
        })?;
        let mut inputs = Vec::new();
        for argument in &item.sig.inputs {
            let syn::FnArg::Typed(argument) = argument else {
                return Err("component functions cannot use a `self` parameter".into());
            };
            let syn::Pat::Ident(name) = argument.pat.as_ref() else {
                return Err("component function inputs must be named parameters".into());
            };
            let direction = if argument
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("out"))
            {
                "out_param"
            } else if matches!(argument.ty.as_ref(), syn::Type::Reference(reference) if reference.mutability.is_some())
            {
                "in_out_param"
            } else {
                "in_param"
            };
            inputs.push(RustComponentInput {
                name: name.ident.to_string(),
                ty: argument.ty.to_token_stream().to_string(),
                mutable: name.mutability.is_some()
                    || matches!(argument.ty.as_ref(), syn::Type::Reference(reference) if reference.mutability.is_some()),
                direction,
            });
        }
        components.push(RustComponentFunction {
            name: item.sig.ident.to_string(),
            inputs,
            prelude: body[..markup_start].trim().to_owned(),
            template: body[markup_start..].to_owned(),
        });
        spans.push((start, body_close + 1));
        cursor = body_close + 1;
    }
    if components.is_empty() {
        return Ok(None);
    }
    let mut script = String::with_capacity(source.len());
    cursor = 0;
    for (start, end) in spans {
        script.push_str(&source[cursor..start]);
        script.push('\n');
        cursor = end;
    }
    script.push_str(&source[cursor..]);
    Ok(Some((script, components)))
}

#[cfg(feature = "compiler")]
fn find_component_markup_start(source: &str) -> Option<usize> {
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
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
        if character == '"' {
            quote = Some(character);
            continue;
        }
        if parens == 0 && brackets == 0 && braces == 0 {
            let rest = &source[index..];
            if (character == '<' && rest.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic))
                || rest.starts_with("{if ")
            {
                return Some(index);
            }
        }
        match character {
            '(' => parens += 1,
            ')' => parens = parens.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ => {}
        }
    }
    None
}

#[cfg(feature = "compiler")]
fn matching_delimiter(source: &str, open: usize, left: u8, right: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in source[open..].char_indices() {
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
        if character == '"'
            || (character == '\''
                && source[open + relative + 1..]
                    .chars()
                    .nth(1)
                    .is_some_and(|next| next == '\''))
        {
            quote = Some(character);
        } else if character as u32 == left as u32 {
            depth += 1;
        } else if character as u32 == right as u32 {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(open + relative);
            }
        }
    }
    None
}

#[cfg(feature = "compiler")]
fn compile_rust_components(
    input: &Path,
    output: &Path,
    script: &str,
    components: &[RustComponentFunction],
) -> Result<(), String> {
    let mut generated_components = Vec::new();
    let has_custom_definition = script.contains("fn definition(");
    let root_component = components.iter().find(|component| component.name == "App");
    let root_helpers = if has_custom_definition && root_component.is_some() {
        let title = if script.contains("fn title(") {
            String::new()
        } else {
            format!(
                "fn title() -> &'static str {{ {:?} }}\n",
                input.file_stem().unwrap().to_string_lossy()
            )
        };
        let template = if script.contains("fn template(") {
            String::new()
        } else {
            "fn template() -> gpui_rsc::TemplateElement { <AppComponent as gpui_rsc::CompiledComponent>::template() }\n".to_owned()
        };
        let view_inputs = component_view_inputs(script)
            .iter()
            .map(|name| format!("{name:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{title}{template}fn __rsc_generated_view_inputs() -> &'static [&'static str] {{ &[{view_inputs}] }}\n"
        )
    } else {
        String::new()
    };
    let mut next_style_id = 0;

    for component in components {
        let component_template = component.template.as_str();
        let (template, select_option_refs) = extract_select_option_refs(&component_template)?;
        let prelude = expand_rust_style_bundles(&component.prelude)?;
        let prelude = expand_script_option_mappings(&prelude)?;
        let template = expand_template_expressions(&template)?;
        let template = expand_template_interpolations(&template)?;
        let component_functions = component_function_tags(&template)?;
        let template = normalize_component_tags(&template)?;
        let (template, class_bindings) = extract_class_bindings(&template)?;
        let (prelude, style_variables) =
            extract_style_variables(&prelude, class_bindings.values())?;
        let document = Html::parse_document(&ensure_html_document(&template));
        let body = document
            .select(&Selector::parse("body").unwrap())
            .next()
            .ok_or_else(|| format!("{}: component needs markup", input.display()))?;
        let input_locals = component_input_locals(&component.inputs)?;
        let mut gpui_functions = Vec::new();
        let tree = element_code(
            body,
            &class_bindings,
            &style_variables,
            &input_locals,
            &mut next_style_id,
            &mut gpui_functions,
        )?;
        let name = component_tag_stem(&component.name);
        let struct_name = format!("{}Component", component.name);
        let imports = component_functions
            .iter()
            .map(|name| format!("{name}()"))
            .collect::<Vec<_>>();
        let view_inputs = component
            .inputs
            .iter()
            .filter(|input| input.direction != "out_param")
            .map(|input| format!("{:?}", input.name))
            .collect::<Vec<_>>()
            .join(", ");
        let definition = if component.name == "App" && has_custom_definition {
            "definition()".to_owned()
        } else {
            let bindings = component
                .inputs
                .iter()
                .map(|input| format!("gpui_rsc::{}!({:?})", input.direction, input.name))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "gpui_rsc::runtime::Definition::new({name:?}, {name:?}, <{struct_name} as gpui_rsc::CompiledComponent>::template(), vec![{bindings}], __rsc_render_component)"
            )
        };
        let inputs_fn = format!("__{}_view_inputs", component.name);
        let definition = format!("({definition}).with_view_inputs({inputs_fn}())");
        let definition = if imports.is_empty() {
            definition
        } else {
            format!("({definition}).with_imports(vec![{}])", imports.join(", "))
        };
        let definition = select_option_refs
            .iter()
            .fold(definition, |definition, (id, options)| {
                format!("({definition}).with_select_options({id:?}, {options})")
            });
        generated_components.push(format!(
            "pub struct {struct_name};\nimpl gpui_rsc::CompiledComponent for {struct_name} {{ fn template() -> gpui_rsc::TemplateElement {{ {tree} }} }}\nimpl {struct_name} {{ pub fn definition() -> gpui_rsc::runtime::Definition {{ {prelude} {definition} }} }}\n#[allow(non_snake_case)] pub fn {}() -> gpui_rsc::runtime::Definition {{ {struct_name}::definition() }}\n#[allow(dead_code, non_snake_case)] fn __{}_view_inputs() -> &'static [&'static str] {{ &[{view_inputs}] }}\n{}",
            component.name,
            component.name,
            gpui_functions.join("\n"),
        ));
    }

    let generated = format!(
        "// Generated Rust component functions from {}.\n{}\n{}\n{}\n{}\n#[allow(unused_variables)] pub fn __rsc_render_component(view: &mut gpui_rsc::runtime::view::HtmlView, props: &gpui_rsc::runtime::ComponentProps, viewport_width: f32, window: &mut gpui::Window, cx: &mut gpui::Context<gpui_rsc::runtime::view::HtmlView>) -> gpui::AnyElement {{ view.render_root(props, viewport_width, window, cx) }}\n",
        input.display(),
        script,
        generated_components.join("\n"),
        root_helpers,
        "",
    );
    let generated = style_codegen::translate(&generated)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if fs::read_to_string(output).ok().as_deref() != Some(&generated) {
        fs::write(output, generated).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(feature = "compiler")]
fn component_input_locals(inputs: &[RustComponentInput]) -> Result<String, String> {
    let mut locals = String::new();
    for input in inputs {
        if input.direction == "out_param" {
            continue;
        }
        let ty = syn::parse_str::<syn::Type>(&input.ty)
            .map_err(|error| format!("invalid type for component parameter {}: {error}", input.name))?;
        let (value_ty, by_reference) = match &ty {
            syn::Type::Reference(reference) => (reference.elem.as_ref(), true),
            other => (other, false),
        };
        let syn::Type::Path(path) = value_ty else {
            return Err(format!(
                "component parameter {} has unsupported type {}",
                input.name, input.ty
            ));
        };
        let Some(type_name) = path.path.segments.last().map(|segment| segment.ident.to_string()) else {
            return Err(format!(
                "component parameter {} has unsupported type {}",
                input.name, input.ty
            ));
        };
        let mutable = if input.mutable { "mut " } else { "" };
        let key = format!("{:?}", input.name);
        let declaration = match type_name.as_str() {
            "f32" => format!(
                "let {mutable}{}: f32 = props.get({key}).and_then(|value| value.number()).unwrap_or_default();\n",
                input.name
            ),
            "f64" | "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32"
            | "u64" | "usize" => format!(
                "let {mutable}{}: {type_name} = props.get({key}).and_then(|value| value.number()).map(|value| value as {type_name}).unwrap_or_default();\n",
                input.name
            ),
            "str" if by_reference => format!(
                "let {}: &str = props.get({key}).and_then(|value| match value {{ gpui_rsc::runtime::Value::Text(text) => Some(text.as_str()), _ => None }}).unwrap_or(\"\");\n",
                input.name
            ),
            "String" => format!(
                "let {mutable}{}: String = props.get({key}).and_then(|value| match value {{ gpui_rsc::runtime::Value::Text(text) => Some(text.clone()), _ => None }}).unwrap_or_default();\n",
                input.name
            ),
            "bool" => format!(
                "let {mutable}{}: bool = props.get({key}).and_then(|value| match value {{ gpui_rsc::runtime::Value::Text(text) => Some(text == \"true\"), gpui_rsc::runtime::Value::Number(number) => Some(*number != 0.0), gpui_rsc::runtime::Value::Arguments(_) => None }}).unwrap_or_default();\n",
                input.name
            ),
            "Value" if !by_reference => format!(
                "let {mutable}{} = props.get({key}).cloned().unwrap_or_else(|| gpui_rsc::runtime::Value::Text(String::new()));\n",
                input.name
            ),
            _ => {
                return Err(format!(
                    "component parameter {} has unsupported attribute input type {}",
                    input.name, input.ty
                ));
            }
        };
        locals.push_str(&declaration);
    }
    Ok(locals)
}

#[cfg(feature = "compiler")]
fn component_function_name(stem: &str) -> String {
    stem.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(feature = "compiler")]
struct ComponentSections {
    script: String,
    template: String,
}

#[cfg(feature = "compiler")]
fn component_view_inputs(script: &str) -> Vec<String> {
    if !script.contains("StyleContext") {
        return Vec::new();
    }
    let Some(component_start) = script.find("component!") else {
        return Vec::new();
    };
    let component = &script[component_start..];
    let Some(bindings_offset) = component.find("bindings") else {
        return Vec::new();
    };
    let Some(open_relative) = component[bindings_offset..].find('[') else {
        return Vec::new();
    };
    let open = bindings_offset + open_relative;
    let Some(close) = matching_square_bracket(component, open) else {
        return Vec::new();
    };
    let binding_block = &component[open + 1..close];
    let mut parameter_names = Vec::new();
    let mut cursor = 0;
    let markers = [
        "in_out_binding!",
        "in_out_param!",
        "in_binding!",
        "in_param!",
    ];
    while cursor < binding_block.len() {
        let Some((offset, marker)) = markers
            .iter()
            .filter_map(|marker| {
                binding_block[cursor..]
                    .find(marker)
                    .map(|offset| (cursor + offset, *marker))
            })
            .min_by_key(|(offset, _)| *offset)
        else {
            break;
        };
        let after_marker = offset + marker.len();
        let Some(quote_offset) = binding_block[after_marker..].find('"') else {
            break;
        };
        let value_start = after_marker + quote_offset + 1;
        let Some(value_end) = binding_block[value_start..].find('"') else {
            break;
        };
        let name = &binding_block[value_start..value_start + value_end];
        if !parameter_names.iter().any(|parameter| parameter == name) {
            parameter_names.push(name.to_owned());
        }
        cursor = value_start + value_end + 1;
    }
    if parameter_names.is_empty() {
        return Vec::new();
    }

    let script_without_bindings = format!(
        "{}{}",
        &script[..component_start + open],
        &script[component_start + close + 1..]
    );
    let style_code = component_style_code(&script_without_bindings);
    let mut inputs = parameter_names
        .into_iter()
        .filter(|name| script_contains_string(&style_code, name))
        .collect::<Vec<_>>();
    inputs.sort();
    inputs
}

#[cfg(feature = "compiler")]
fn component_style_code(source: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("fn ") {
        let start = cursor + relative;
        let Some(open_relative) = source[start..].find('{') else {
            break;
        };
        let open = start + open_relative;
        let signature = &source[start..open];
        let Ok(close) = rust_brace_end(source, open) else {
            cursor = open + 1;
            continue;
        };
        if signature.contains("StyleContext") {
            output.push_str(&source[start..=close]);
        }
        cursor = close + 1;
    }
    output
}

#[cfg(feature = "compiler")]
fn matching_square_bracket(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for (relative, character) in source[open..].char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }
        match character {
            '"' => quoted = true,
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + relative);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(feature = "compiler")]
fn script_contains_string(source: &str, expected: &str) -> bool {
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, character) in source.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                if &source[start..index] == expected {
                    return true;
                }
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
            start = index + 1;
        }
    }
    false
}

#[cfg(feature = "compiler")]
fn component_sections(source: &str, input: &Path) -> Result<ComponentSections, String> {
    let (script, rest) = take_component_section(source.trim_start(), "script", true)?;
    let (template, rest) = take_component_section(&rest, "template", false)?;
    if !rest.trim().is_empty() {
        return Err(format!(
            "{} has content outside <script> and <template>; component styles belong in a Rust styles(...) bundle",
            input.display()
        ));
    }
    Ok(ComponentSections {
        script: script.ok_or_else(|| format!("{} needs a <script> section", input.display()))?,
        template: template
            .ok_or_else(|| format!("{} needs a <template> section", input.display()))?,
    })
}

#[cfg(feature = "compiler")]
fn take_component_section(
    source: &str,
    name: &str,
    closing_tag_on_own_line: bool,
) -> Result<(Option<String>, String), String> {
    let open_tag = format!("<{name}>");
    let close_tag = format!("</{name}>");
    let Some(open) = source.find(&open_tag) else {
        return Ok((None, source.to_owned()));
    };
    let content_start = open + open_tag.len();
    if source[content_start..].contains(&open_tag) {
        return Err(format!("multiple <{name}> sections are not supported"));
    }
    let close = source[content_start..]
        .match_indices(&close_tag)
        .find(|(relative, tag)| {
            if !closing_tag_on_own_line {
                return true;
            }
            let index = content_start + *relative;
            let before = &source[..index];
            let after = &source[index + tag.len()..];
            before.rsplit('\n').next().unwrap_or("").trim().is_empty()
                && after.split('\n').next().unwrap_or("").trim().is_empty()
        })
        .map(|(relative, _)| content_start + relative)
        .ok_or_else(|| {
            if closing_tag_on_own_line {
                format!("<{name}> needs {close_tag} on its own line")
            } else {
                format!("<{name}> needs a closing tag")
            }
        })?;
    let content = source[content_start..close].to_owned();
    let after_close = close + close_tag.len();
    let mut rest = String::with_capacity(source.len() - (after_close - open));
    rest.push_str(&source[..open]);
    rest.push_str(&source[after_close..]);
    Ok((Some(content), rest))
}

#[cfg(feature = "compiler")]
fn ensure_html_document(source: &str) -> String {
    let lower = source.to_ascii_lowercase();
    if lower.contains("<html") || lower.contains("<body") {
        source.to_owned()
    } else {
        format!("<!doctype html><html><body>{source}</body></html>")
    }
}

#[cfg(feature = "compiler")]
pub fn compile_directory(directory: &Path, output: &Path) -> Result<Vec<PathBuf>, String> {
    let mut inputs = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("rsx"))
        .collect::<Vec<_>>();
    inputs.sort();
    if inputs.is_empty() {
        return Err(format!("no .rsx files in {}", directory.display()));
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
fn component_function_tags(source: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('<') {
        let start = cursor + relative;
        let name_start = start + 1;
        let name_end = source[name_start..]
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = &source[name_start..name_end];
        if name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
        {
            let tag_end = html_tag_end(&source[start..])? + start;
            let tag = &source[start..tag_end];
            if !tag.trim_end_matches('>').trim_end().ends_with('/') {
                return Err(format!(
                    "<{name}> component tags must be self-closing; pass values as properties"
                ));
            }
            if !names.iter().any(|existing| existing == name) {
                names.push(name.to_owned());
            }
            cursor = tag_end;
        } else {
            cursor = name_end.max(start + 1);
        }
    }
    Ok(names)
}

#[cfg(feature = "compiler")]
fn component_tag_stem(name: &str) -> String {
    let characters = name.chars().collect::<Vec<_>>();
    let mut stem = String::new();
    for (index, character) in characters.iter().copied().enumerate() {
        if character == '_' {
            if !stem.ends_with('-') {
                stem.push('-');
            }
            continue;
        }
        if character.is_ascii_uppercase() && index > 0 {
            let previous = characters[index - 1];
            let next_is_lower = characters
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_lowercase());
            if (previous.is_ascii_lowercase() || previous.is_ascii_digit())
                || (previous.is_ascii_uppercase() && next_is_lower)
            {
                stem.push('-');
            }
        }
        stem.push(character.to_ascii_lowercase());
    }
    stem
}

#[cfg(feature = "compiler")]
fn normalize_component_tags(source: &str) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('<') {
        let start = cursor + relative;
        let name_start = start + 1;
        let name_end = source[name_start..]
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = &source[name_start..name_end];
        if name == "component" {
            return Err(
                "use an imported uppercase component function tag, such as <CoffeeProfile />"
                    .into(),
            );
        }
        let is_function_component = name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase());
        if !is_function_component {
            output.push_str(&source[cursor..start + 1]);
            cursor = start + 1;
            continue;
        }

        let end = html_tag_end(&source[start..])? + start;
        let tag = &source[start..end];
        output.push_str(&source[cursor..start]);
        if !tag.trim_end_matches('>').trim_end().ends_with('/') {
            return Err(format!(
                "<{name}> component tags must be self-closing; pass values as properties"
            ));
        }
        let attributes = component_attributes(&source[name_end..end - 1])?;
        output.push_str(&format!(
            "<component name=\"{}\"{}>",
            component_tag_stem(name),
            attributes.trim_end().trim_end_matches('/').trim_end()
        ));
        output.push_str("</component>");
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(feature = "compiler")]
fn component_attributes(source: &str) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find("={") {
        let open = cursor + relative + 1;
        let close = rust_brace_end(source, open)?;
        output.push_str(&source[cursor..open]);
        output.push('[');
        output.push_str(&source[open + 1..close]);
        output.push(']');
        cursor = close + 1;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
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
            class_bindings,
            &HashMap::new(),
            "",
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
        let (open, close) = match tag.as_bytes().get(value_start) {
            Some(b'{') => (b'{', b'}'),
            Some(b'[') => (b'[', b']'),
            _ => {
                cursor += 5;
                continue;
            }
        };
        if found {
            return Err("an element may have only one class={...} binding".into());
        }
        let close_at = matching_delimiter(tag, value_start, open, close)
            .ok_or("unclosed Rust class style expression")?;
        let expression = tag[value_start + 1..close_at].trim();
        if expression.is_empty() {
            return Err("class={...} needs a Rust style expression".into());
        }
        let id = id_offset + bindings.len();
        bindings.insert(id, expression.to_owned());
        result.push_str(&tag[copied..cursor]);
        result.push_str(&format!(" data-rsc-class-binding=\"{id}\""));
        cursor = close_at + 1;
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
fn element_code(
    element: ElementRef<'_>,
    class_bindings: &HashMap<usize, String>,
    style_variables: &HashMap<String, String>,
    input_locals: &str,
    next_style_id: &mut usize,
    gpui_functions: &mut Vec<String>,
) -> Result<String, String> {
    if element.value().name() == "style" {
        return Err("<style> tags are not supported; put styles in a Rust styles({...}) bundle".into());
    }
    let style_id = *next_style_id;
    *next_style_id += 1;
    if element.value().attr("style").is_some() || element.value().attr("mobile-style").is_some() {
        return Err("CSS style attributes are not supported; use a Rust styles({...}) bundle".into());
    }
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
    if let Some(class_name) = element.value().attr("class")
        && !class_name.trim().is_empty()
    {
        return Err(format!(
            "static class={class_name:?} has no Rust style binding; use class={{myStyles.someStyle}}"
        ));
    }
    let attrs = element
        .value()
        .attrs()
        .filter(|(key, _)| *key != "data-rsc-class-binding")
        .map(|(k, v)| format!("({:?}.into(), {:?}.into())", k, v))
        .collect::<Vec<_>>()
        .join(",");
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
                let child_id = *next_style_id;
                let code = element_code(
                    child_element,
                    class_bindings,
                    style_variables,
                    input_locals,
                    next_style_id,
                    gpui_functions,
                )?;
                let child_index = child_codes.len();
                child_codes.push(format!("gpui_rsc::TemplateNode::Element({code})"));
                child_renders.push(format!(
                    "    container = container.child(__rsc_render_{child_id}(view, view.child_element(element, {child_index}), props, viewport_width, window, cx));\n"
                ));
            }
            _ => {}
        }
    }
    let children = child_codes.join(",");
    let render_name = format!("__rsc_render_{style_id}");
    let mut render_body = "let mut container = div();\n".to_owned();
    render_body.push_str(input_locals);
    if let Some(expression) = class_binding {
        render_body.push_str(
            "    let style_context = gpui_rsc::runtime::StyleContext { viewport_width, props };\n    let context = &style_context;\n",
        );
        if let Some(root) = style_expression_root(expression) {
            if let Some(initializer) = style_variables.get(&root) {
                render_body.push_str(&format!("    let {root} = {initializer};\n"));
            } else if expression.contains('.') {
                render_body.push_str(&format!("    let {root} = {root}(context);\n"));
            }
        }
        render_body.push_str(&format!(
            "    let bound_class_style: gpui_rsc::runtime::Style = {expression};\n    gpui::Refineable::refine(container.style(), &bound_class_style);\n"
        ));
    }
    let content = match element.value().name() {
        "component" => "container.child(view.render_component(element, viewport_width, window, cx)).into_any_element()".to_owned(),
        "rsc-if" => "view.render_if(element, props, container, viewport_width, window, cx)".to_owned(),
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
                body.push_str("    container.into_any_element()");
                body
            }
    };
    render_body.push_str(&content);
    gpui_functions.push(format!(
        "#[allow(unused_imports, unused_variables, unused_mut, non_snake_case)]\nfn {render_name}(view: &mut gpui_rsc::runtime::view::HtmlView, element: &gpui_rsc::runtime::binding::Element, props: &gpui_rsc::runtime::ComponentProps, viewport_width: f32, window: &mut gpui::Window, cx: &mut gpui::Context<gpui_rsc::runtime::view::HtmlView>) -> gpui::AnyElement {{\n    use gpui_kit::*;\n    use gpui_rsc::runtime::Style;\n    {render_body}\n}}"
    ));
    Ok(format!(
        "gpui_rsc::TemplateElement::new({:?}, vec![{}], vec![{}]).with_render({render_name})",
        element.value().name(),
        attrs,
        children
    ))
}

#[cfg(feature = "compiler")]
fn extract_style_variables<'a>(
    prelude: &str,
    class_expressions: impl Iterator<Item = &'a String>,
) -> Result<(String, HashMap<String, String>), String> {
    let referenced = class_expressions
        .filter_map(|expression| style_expression_root(expression))
        .collect::<std::collections::HashSet<_>>();
    if referenced.is_empty() {
        return Ok((prelude.to_owned(), HashMap::new()));
    }

    let block = syn::parse_str::<syn::Block>(&format!("{{{prelude}}}"))
        .map_err(|error| format!("invalid component prelude: {error}"))?;
    let mut remaining = Vec::new();
    let mut style_variables = HashMap::new();
    for statement in block.stmts {
        let extracted = if let syn::Stmt::Local(local) = &statement {
            let syn::Pat::Ident(pattern) = &local.pat else {
                remaining.push(statement);
                continue;
            };
            if referenced.contains(&pattern.ident.to_string()) {
                if let Some(initializer) = &local.init {
                    let expression = &initializer.expr;
                    style_variables.insert(
                        pattern.ident.to_string(),
                        quote::quote!(#expression).to_string(),
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if !extracted {
            remaining.push(statement);
        }
    }

    let remaining = remaining
        .into_iter()
        .map(|statement| quote::quote!(#statement).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    Ok((remaining, style_variables))
}

#[cfg(feature = "compiler")]
fn style_expression_root(expression: &str) -> Option<String> {
    fn root(expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Field(field) => root(&field.base),
            syn::Expr::Path(path) if path.qself.is_none() => path
                .path
                .segments
                .first()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        }
    }

    let expression = syn::parse_str::<syn::Expr>(expression).ok()?;
    root(&expression)
}

#[cfg(feature = "compiler")]
fn expand_rust_style_bundles(source: &str) -> Result<String, String> {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    let mut scan = 0;
    let mut bundle_id = 0usize;
    while let Some(relative) = source[scan..].find("styles") {
        let start = scan + relative;
        let before_is_ident = source[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        let after_name = start + "styles".len();
        let open = skip_ascii_whitespace(source, after_name);
        if before_is_ident || source.as_bytes().get(open) != Some(&b'(') {
            scan = after_name;
            continue;
        }
        let call_close =
            matching_delimiter(source, open, b'(', b')').ok_or("unclosed styles(...) call")?;
        let argument_open = skip_ascii_whitespace(source, open + 1);
        if source.as_bytes().get(argument_open) != Some(&b'{') {
            scan = after_name;
            continue;
        }
        let close = matching_delimiter(source, argument_open, b'{', b'}')
            .ok_or("unclosed style bundle object")?;
        if close >= call_close || !source[close + 1..call_close].trim().is_empty() {
            return Err("styles(...) accepts one style bundle object".into());
        }
        let fields = parse_style_bundle_fields(&source[argument_open + 1..close])?;
        if fields.is_empty() {
            return Err("styles(...) needs at least one named style".into());
        }
        let type_name = format!("__RscStyleBundle{bundle_id}");
        bundle_id += 1;
        let struct_fields = fields
            .iter()
            .map(|(name, _)| format!("{name}: gpui_rsc::runtime::Style"))
            .collect::<Vec<_>>()
            .join(",");
        let values = fields
            .iter()
            .map(|(name, style)| format!("{name}: {style}"))
            .collect::<Vec<_>>()
            .join(",");
        output.push_str(&source[cursor..start]);
        output.push_str(&format!(
            "{{ #[allow(dead_code)] struct {type_name} {{ {struct_fields} }} {type_name} {{ {values} }} }}"
        ));
        cursor = call_close + 1;
        scan = cursor;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

#[cfg(feature = "compiler")]
fn parse_style_bundle_fields(source: &str) -> Result<Vec<(String, String)>, String> {
    let mut fields = Vec::new();
    for field in split_rust_top_level(source, ',') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        let (name, value) = field
            .split_once(':')
            .ok_or_else(|| format!("style bundle field needs `name: {{ ... }}`: {field}"))?;
        let name = name.trim();
        if !is_simple_rust_identifier(name) {
            return Err(format!("invalid style bundle name {name:?}"));
        }
        let value = value.trim();
        if !value.starts_with('{')
            || matching_delimiter(value, 0, b'{', b'}') != Some(value.len() - 1)
        {
            return Err(format!("style `{name}` needs a property object"));
        }
        fields.push((
            name.to_owned(),
            parse_style_properties(&value[1..value.len() - 1])?,
        ));
    }
    Ok(fields)
}

#[cfg(feature = "compiler")]
fn parse_style_properties(source: &str) -> Result<String, String> {
    let mut style = "gpui_rsc::runtime::Style::new()".to_owned();
    for property in split_rust_top_level(source, ',') {
        let property = property.trim();
        if property.is_empty() {
            continue;
        }
        let (name, value) = property
            .split_once(':')
            .ok_or_else(|| format!("style property needs `name: value`: {property}"))?;
        style.push_str(&style_property_call(name.trim(), value.trim())?);
    }
    Ok(style)
}

#[cfg(feature = "compiler")]
fn style_property_call(name: &str, value: &str) -> Result<String, String> {
    let property = camel_to_snake(name);
    let mut method = property.as_str();
    let mut arguments = value.to_owned();
    let mut tuple_args = None;
    match property.as_str() {
        "display" => {
            method = "display_flex";
            arguments = match parse_string_literal(value)?.as_str() {
                "flex" => "true".into(),
                "block" => "false".into(),
                other => return Err(format!("unsupported display value {other:?}")),
            };
        }
        "flex_direction" => {
            method = "flex_direction";
            arguments = match parse_string_literal(value) {
                Ok(value) => match value.as_str() {
                    "column" => "true".into(),
                    "row" => "false".into(),
                    other => return Err(format!("unsupported flexDirection value {other:?}")),
                },
                Err(_) => {
                    syn::parse_str::<syn::Expr>(value)
                        .map_err(|error| format!("invalid flexDirection expression: {error}"))?;
                    format!("({value}) == \"column\"")
                }
            };
        }
        "flex_wrap" => {
            arguments = match parse_string_literal(value) {
                Ok(value) => match value.as_str() {
                    "wrap" => "true".into(),
                    "nowrap" => "false".into(),
                    other => return Err(format!("unsupported flexWrap value {other:?}")),
                },
                Err(_) => {
                    syn::parse_str::<syn::Expr>(value)
                        .map_err(|error| format!("invalid flexWrap expression: {error}"))?;
                    format!("({value}) == \"wrap\"")
                }
            };
        }
        "flex" => {
            method = "flex_grow";
            arguments = match value {
                "1" | "true" => "true".into(),
                "0" | "false" => "false".into(),
                _ => value.into(),
            };
        }
        "flex_grow" => {}
        "padding" => {
            let values = if value.starts_with('(') {
                parse_style_tuple(value)?
            } else {
                vec![value.to_owned()]
            };
            if values.is_empty() || values.len() > 4 {
                return Err("padding needs one to four values".into());
            }
            let expanded = match values.as_slice() {
                [all] => vec![all.clone(), all.clone(), all.clone(), all.clone()],
                [vertical, horizontal] => vec![
                    vertical.clone(),
                    horizontal.clone(),
                    vertical.clone(),
                    horizontal.clone(),
                ],
                [top, horizontal, bottom] => vec![
                    top.clone(),
                    horizontal.clone(),
                    bottom.clone(),
                    horizontal.clone(),
                ],
                [top, right, bottom, left] => {
                    vec![top.clone(), right.clone(), bottom.clone(), left.clone()]
                }
                _ => unreachable!(),
            };
            tuple_args = Some(expanded);
        }
        "border" => {
            tuple_args = Some(parse_style_tuple(value)?);
        }
        "background_color" | "text_color" => {
            arguments = expand_style_color(value)?;
        }
        "border_color" => {
            arguments = expand_style_color(value)?;
        }
        "position" => {
            let position = parse_string_literal(value)?;
            method = match position.as_str() {
                "relative" => "position_relative",
                "absolute" => "position_absolute",
                other => return Err(format!("unsupported position value {other:?}")),
            };
            arguments.clear();
        }
        "overflow" => {
            method = match parse_string_literal(value)?.as_str() {
                "hidden" => "overflow_hidden",
                other => return Err(format!("unsupported overflow value {other:?}")),
            };
            arguments.clear();
        }
        "overflow_y" => {
            arguments = match parse_string_literal(value)?.as_str() {
                "auto" | "scroll" => "true".into(),
                "hidden" => {
                    method = "overflow_y_hidden";
                    arguments.clear();
                    String::new()
                }
                "visible" => "false".into(),
                other => return Err(format!("unsupported overflowY value {other:?}")),
            };
        }
        "margin" => {
            if parse_string_literal(value)? != "auto" {
                return Err("margin currently supports only `auto`".into());
            }
            method = "margin_auto";
            arguments.clear();
        }
        "margin_auto" => {
            method = "margin_auto_enabled";
        }
        "justify_content" | "align_items" => {
            if let Ok(value) = parse_string_literal(value) {
                let accepted = if property == "justify_content" {
                    ["center", "space-between", "flex-end", "flex-start"]
                } else {
                    ["center", "stretch", "flex-start", "flex-end"]
                };
                if !accepted.contains(&value.as_str()) {
                    return Err(format!("unsupported {name} value {value:?}"));
                }
                arguments = format!("{value:?}");
            } else {
                syn::parse_str::<syn::Expr>(value)
                    .map_err(|error| format!("invalid {name} expression: {error}"))?;
            }
        }
        "background" | "color" => {
            method = if property == "background" {
                "background_color"
            } else {
                "text_color"
            };
            arguments = expand_style_color(value)?;
        }
        "animation" => {
            return Err("animation is not currently supported in styles({...})".into());
        }
        _ => {}
    }

    let argument_list = if let Some(mut arguments) = tuple_args {
        let expected = if property == "padding" { 4 } else { 2 };
        if arguments.len() != expected {
            return Err(format!(
                "style `{name}` needs {expected} tuple value(s), got {}",
                arguments.len()
            ));
        }
        if property == "border" {
            arguments[1] = expand_style_color(&arguments[1])?;
        }
        for argument in &arguments {
            syn::parse_str::<syn::Expr>(argument)
                .map_err(|error| format!("invalid value for style `{name}`: {error}"))?;
        }
        arguments.join(",")
    } else {
        if !arguments.is_empty() {
            syn::parse_str::<syn::Expr>(&arguments)
                .map_err(|error| format!("invalid value for style `{name}`: {error}"))?;
            arguments
        } else {
            String::new()
        }
    };
    Ok(format!(".{method}({argument_list})"))
}

#[cfg(feature = "compiler")]
fn parse_string_literal(value: &str) -> Result<String, String> {
    let literal = syn::parse_str::<syn::LitStr>(value)
        .map_err(|_| format!("expected a quoted string style value, got {value:?}"))?;
    Ok(literal.value())
}

#[cfg(feature = "compiler")]
fn parse_style_tuple(value: &str) -> Result<Vec<String>, String> {
    let expression = syn::parse_str::<syn::Expr>(value)
        .map_err(|error| format!("style shorthand must be a tuple: {error}"))?;
    let syn::Expr::Tuple(tuple) = expression else {
        return Err("padding and border values must be tuples".into());
    };
    if tuple.elems.is_empty() {
        return Err("style tuple cannot be empty".into());
    }
    Ok(tuple
        .elems
        .into_iter()
        .map(|expression| quote::quote!(#expression).to_string())
        .collect())
}

#[cfg(feature = "compiler")]
fn expand_style_color(value: &str) -> Result<String, String> {
    let Some(open) = value.find('(') else {
        return Ok(value.to_owned());
    };
    let function = value[..open].trim();
    if !matches!(function, "rgba" | "rgb") {
        return Ok(value.to_owned());
    }
    let close = matching_delimiter(value, open, b'(', b')')
        .ok_or_else(|| format!("unclosed {function}(...) color"))?;
    if close + 1 != value.len() {
        return Ok(value.to_owned());
    }
    let channels = split_rust_top_level(&value[open + 1..close], ',');
    if (function == "rgb" && channels.len() != 3)
        || (function == "rgba" && !matches!(channels.len(), 3 | 4))
    {
        return Err(format!(
            "{function} color needs three channels and optional alpha"
        ));
    }
    let alpha = channels.get(3).copied().unwrap_or("1.0").trim();
    Ok(format!(
        "gpui::Rgba {{ r: {}, g: {}, b: {}, a: {} }}",
        channels[0].trim(),
        channels[1].trim(),
        channels[2].trim(),
        alpha
    ))
}

#[cfg(feature = "compiler")]
fn split_rust_top_level(source: &str, separator: char) -> Vec<&str> {
    let mut output = Vec::new();
    let mut start = 0;
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
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
            '"' => quote = Some(character),
            '(' => parens += 1,
            ')' => parens = parens.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ if character == separator && parens == 0 && brackets == 0 && braces == 0 => {
                output.push(&source[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    output.push(&source[start..]);
    output
}

#[cfg(feature = "compiler")]
fn camel_to_snake(name: &str) -> String {
    let mut output = String::new();
    for character in name.chars() {
        if character.is_ascii_uppercase() {
            output.push('_');
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }
    output
}
