//! Direct RSX lowering. Constructors and methods are checked by Rust, not a tag registry.
use quote::ToTokens;

pub fn convert(source: &str) -> Result<String, String> {
    let (source, _) = convert_marked(source)?;
    let converted = convert_code(&source)?;
    format_rust(&converted)
}

pub fn format_rust(source: &str) -> Result<String, String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("invalid Rust after RSX conversion: {error}"))?;
    Ok(prettyplease::unparse(&file))
}

/// Convert only functions carrying the compiler directive `#[gpui]`.
pub fn convert_marked(source: &str) -> Result<(String, bool), String> {
    if !source.contains("#[gpui]") {
        return Ok((source.to_owned(), false));
    }
    let mut output = String::new();
    let mut copied = 0;
    let mut position = 0;
    let mut changed = false;
    while position < source.len() {
        if let Some(end) = skip_literal_or_comment(source, position)? {
            position = end;
            continue;
        }
        if source[position..].starts_with("#[gpui]") {
            let signature_start = position + "#[gpui]".len();
            let body_start = function_body_start(source, signature_start)?;
            let signature = &source[signature_start..body_start];
            syn::parse_str::<syn::ItemFn>(&format!("{signature}{{}}"))
                .map_err(|error| format!("#[gpui] must annotate a Rust function: {error}"))?;
            let body_end = balanced_end(source, body_start, '{', '}')?;
            let body = convert_code(&source[body_start + 1..body_end - 1])?;
            output.push_str(&source[copied..position]);
            output.push_str(signature);
            output.push('{');
            output.push_str(&body);
            output.push('}');
            position = body_end;
            copied = position;
            changed = true;
        } else {
            position += source[position..].chars().next().unwrap().len_utf8();
        }
    }
    output.push_str(&source[copied..]);
    Ok((output, changed))
}

fn function_body_start(source: &str, mut position: usize) -> Result<usize, String> {
    let mut angle_depth = 0usize;
    while position < source.len() {
        if let Some(end) = skip_literal_or_comment(source, position)? {
            position = end;
            continue;
        }
        match source.as_bytes()[position] {
            b'(' => position = balanced_end(source, position, '(', ')')?,
            b'[' => position = balanced_end(source, position, '[', ']')?,
            b'<' => {
                angle_depth += 1;
                position += 1;
            }
            b'>' => {
                angle_depth = angle_depth.saturating_sub(1);
                position += 1;
            }
            b'{' if angle_depth > 0 => position = balanced_end(source, position, '{', '}')?,
            b'{' => return Ok(position),
            b';' => return Err("#[gpui] function needs a body".into()),
            _ => position += source[position..].chars().next().unwrap().len_utf8(),
        }
    }
    Err("#[gpui] function needs a body".into())
}

fn convert_code(source: &str) -> Result<String, String> {
    let mut output = String::new();
    let mut position = 0;
    let mut copied = 0;
    while position < source.len() {
        if let Some(end) = skip_literal_or_comment(source, position)? {
            position = end;
            continue;
        }
        if is_element_start(source, position) {
            let mut parser = ElementParser { source, position };
            let expression = parser.element()?;
            output.push_str(&source[copied..position]);
            output.push_str(&expression);
            position = parser.position;
            copied = position;
        } else {
            position += source[position..].chars().next().unwrap().len_utf8();
        }
    }
    output.push_str(&source[copied..]);
    Ok(output)
}

fn is_element_start(source: &str, position: usize) -> bool {
    let rest = &source[position..];
    if !rest.starts_with('<')
        || !rest[1..]
            .strip_prefix("::")
            .unwrap_or(&rest[1..])
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
    {
        return false;
    }
    let before = source[..position].trim_end();
    if before.ends_with("::") {
        return false;
    }
    // Qualified Rust calls such as <T as Trait>::method() are not elements.
    let name_end = 1 + rest[1..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | ':'))
        .map(char::len_utf8)
        .sum::<usize>();
    if rest[name_end..].trim_start().starts_with("as ") {
        return false;
    }
    if let Some(end) = rest.find('>') {
        let after = rest[end + 1..].trim_start();
        if after.starts_with("::") || after.starts_with("as ") {
            return false;
        }
    }
    let previous = before.chars().next_back();
    // A block can precede a new expression statement without a semicolon.
    // Require a tag delimiter or builder attribute so `{value}<limit` remains
    // an ordinary Rust comparison.
    let after_name = rest[name_end..].trim_start();
    let after_block = previous == Some('}')
        && (after_name.starts_with('>')
            || after_name.starts_with("/>")
            || after_name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_'));
    previous.is_none()
        || after_block
        || previous
            .is_some_and(|c| matches!(c, '=' | '{' | '(' | '[' | ',' | ';' | ':' | '>' | '|'))
        || before.strip_suffix("return").is_some_and(|prefix| {
            prefix
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_')
        })
}

struct ElementParser<'a> {
    source: &'a str,
    position: usize,
}

impl ElementParser<'_> {
    fn element(&mut self) -> Result<String, String> {
        self.expect("<")?;
        let name = self.name()?;
        syn::parse_str::<syn::Path>(&name)
            .map_err(|error| format!("invalid Rust tag <{name}>: {error}"))?;
        let mut attributes = Vec::new();
        let self_closing;
        loop {
            self.whitespace();
            if self.consume("/>") {
                self_closing = true;
                break;
            }
            if self.consume(">") {
                self_closing = false;
                break;
            }
            if self.position == self.source.len() {
                return Err(format!("unclosed opening tag <{name}>"));
            }
            let key = self.attribute_name()?;
            self.whitespace();
            let value = if self.consume("=") {
                self.whitespace();
                let value_position = self.position;
                Some(self.attribute_value().map_err(|error| {
                    format!("<{name}> attribute {key} at byte {value_position}: {error}")
                })?)
            } else {
                None
            };
            attributes.push((key, value));
        }
        let args = attributes
            .iter()
            .filter(|(key, _)| key == "args")
            .collect::<Vec<_>>();
        let constructors = attributes
            .iter()
            .filter(|(key, _)| key == "ctor")
            .collect::<Vec<_>>();
        if args.len() > 1
            || constructors.len() > 1
            || (!args.is_empty() && !constructors.is_empty())
        {
            return Err(format!(
                "<{name}> needs at most one of args={{...}} or ctor={{...}}"
            ));
        }
        let mut expression = if let Some((_, value)) = constructors.first() {
            let value = value.as_deref().ok_or("ctor needs a Rust expression")?;
            match syn::parse_str::<syn::Expr>(value).map_err(|e| e.to_string())? {
                syn::Expr::Path(_)
                | syn::Expr::Call(_)
                | syn::Expr::MethodCall(_)
                | syn::Expr::Paren(_)
                | syn::Expr::Field(_)
                | syn::Expr::Index(_) => value.to_owned(),
                _ => format!("({value})"),
            }
        } else {
            let args = args
                .first()
                .map(|(_, value)| {
                    constructor_args(value.as_deref().ok_or("args needs a Rust expression")?)
                })
                .transpose()?
                .unwrap_or_default();
            let last = name.rsplit("::").next().unwrap();
            if last.chars().next().is_some_and(char::is_uppercase) {
                format!("{name}::new({args})")
            } else {
                format!("{name}({args})")
            }
        };
        for (key, value) in attributes {
            if matches!(key.as_str(), "args" | "ctor") {
                continue;
            }
            let (method, multiple_args) = match key.strip_suffix(":args") {
                Some(method) => (method, true),
                None => (key.as_str(), false),
            };
            let method = method.replace('-', "_");
            syn::parse_str::<syn::Ident>(&method)
                .map_err(|_| format!("<{name}> attribute {key:?} is not a Rust method name"))?;
            let arguments =
                if multiple_args {
                    constructor_args(value.as_deref().ok_or_else(|| {
                        format!("<{name}> {key} needs a Rust argument expression")
                    })?)?
                } else {
                    value.unwrap_or_default()
                };
            expression.push_str(&format!(".{method}({arguments})"));
        }
        if self_closing {
            return Ok(expression);
        }
        loop {
            self.whitespace();
            if self.position == self.source.len() {
                return Err(format!("<{name}> needs a matching </{name}>"));
            }
            if self.consume("</") {
                let closing = self.name()?;
                self.whitespace();
                self.expect(">")?;
                if closing != name {
                    return Err(format!(
                        "<{name}> closes with </{closing}>; expected </{name}>"
                    ));
                }
                return Ok(expression);
            }
            if self.consume("<!--") {
                let end = self.source[self.position..]
                    .find("-->")
                    .ok_or("unclosed RSX comment")?;
                self.position += end + 3;
                continue;
            }
            let child = if self.source[self.position..].starts_with('"') {
                let start = self.position;
                self.position =
                    skip_literal_or_comment(self.source, start)?.ok_or("unclosed string child")?;
                self.source[start..self.position].to_owned()
            } else if self.source[self.position..].starts_with('<') {
                self.element()?
            } else if self.source[self.position..].starts_with('{') {
                let end = balanced_end(self.source, self.position, '{', '}')?;
                let body = convert_code(&self.source[self.position + 1..end - 1])?;
                self.position = end;
                if syn::parse_str::<syn::Expr>(&body).is_ok() {
                    body
                } else {
                    format!("{{{body}}}")
                }
            } else {
                let start = self.position;
                while self.position < self.source.len()
                    && !matches!(self.source.as_bytes()[self.position], b'<' | b'{')
                {
                    self.position += self.source[self.position..]
                        .chars()
                        .next()
                        .unwrap()
                        .len_utf8();
                }
                let text = self.source[start..self.position]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if text.is_empty() {
                    continue;
                }
                if let Ok(literal) = syn::parse_str::<syn::LitStr>(&text) {
                    literal.to_token_stream().to_string()
                } else {
                    format!("{text:?}")
                }
            };
            expression.push_str(&format!(".child({child})"));
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let start = self.position;
        while let Some(character) = self.source[self.position..].chars().next() {
            if character.is_alphanumeric() || matches!(character, '_' | ':') {
                self.position += character.len_utf8();
            } else {
                break;
            }
        }
        if self.position == start {
            return Err(format!("expected Rust tag name at byte {}", self.position));
        }
        Ok(self.source[start..self.position].to_owned())
    }

    fn attribute_name(&mut self) -> Result<String, String> {
        let start = self.position;
        while let Some(character) = self.source[self.position..].chars().next() {
            if character.is_alphanumeric() || matches!(character, '_' | '-' | ':') {
                self.position += character.len_utf8();
            } else {
                break;
            }
        }
        if self.position == start {
            return Err(format!("expected builder method at byte {}", self.position));
        }
        Ok(self.source[start..self.position].to_owned())
    }

    fn attribute_value(&mut self) -> Result<String, String> {
        if self.source[self.position..].starts_with('{') {
            let end = balanced_end(self.source, self.position, '{', '}')?;
            let body = convert_code(&self.source[self.position + 1..end - 1])?;
            self.position = end;
            // Keep statement blocks valid, while simple expressions stay available
            // to constructor_args for tuple expansion.
            if syn::parse_str::<syn::Expr>(&body).is_ok() {
                Ok(body)
            } else {
                let block = format!("{{{body}}}");
                syn::parse_str::<syn::Expr>(&block)
                    .map_err(|error| format!("invalid attribute expression: {error}"))?;
                Ok(block)
            }
        } else if self.source[self.position..].starts_with('"') {
            let start = self.position;
            self.position =
                skip_literal_or_comment(self.source, start)?.ok_or("unclosed string attribute")?;
            Ok(self.source[start..self.position].to_owned())
        } else {
            Err("builder attribute values need braces or a quoted string, such as padding={px(8.0)}".into())
        }
    }

    fn whitespace(&mut self) {
        while self.source[self.position..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            self.position += self.source[self.position..]
                .chars()
                .next()
                .unwrap()
                .len_utf8();
        }
    }

    fn consume(&mut self, value: &str) -> bool {
        if self.source[self.position..].starts_with(value) {
            self.position += value.len();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, value: &str) -> Result<(), String> {
        if self.consume(value) {
            Ok(())
        } else {
            Err(format!("expected {value:?} at byte {}", self.position))
        }
    }
}

fn constructor_args(source: &str) -> Result<String, String> {
    let expression = syn::parse_str::<syn::Expr>(source)
        .map_err(|error| format!("invalid constructor arguments: {error}"))?;
    Ok(match expression {
        syn::Expr::Tuple(tuple) => tuple
            .elems
            .iter()
            .map(|expr| expr.to_token_stream().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        _ => source.to_owned(),
    })
}

fn balanced_end(source: &str, start: usize, left: char, right: char) -> Result<usize, String> {
    let mut depth = 0;
    let mut position = start;
    while position < source.len() {
        // Markup text is not Rust: a URL can contain //, and apostrophes or
        // quotes need not delimit literals. Let the RSX parser skip the whole
        // element, including its own Rust attribute/child blocks.
        if is_element_start(source, position) {
            let mut parser = ElementParser { source, position };
            parser.element()?;
            position = parser.position;
            continue;
        }
        if let Some(end) = skip_literal_or_comment(source, position)? {
            position = end;
            continue;
        }
        let character = source[position..].chars().next().unwrap();
        position += character.len_utf8();
        if character == left {
            depth += 1;
        } else if character == right {
            depth -= 1;
            if depth == 0 {
                return Ok(position);
            }
        }
    }
    Err(format!("unclosed {left} starting at byte {start}"))
}

/// Skip Rust strings, chars, raw strings, and nested comments without reading RSX
/// examples embedded inside them as actual markup. Lifetimes remain ordinary code.
fn skip_literal_or_comment(source: &str, position: usize) -> Result<Option<usize>, String> {
    let rest = &source[position..];
    if rest.starts_with("//") {
        return Ok(Some(position + rest.find('\n').unwrap_or(rest.len())));
    }
    if rest.starts_with("/*") {
        let mut depth = 1;
        let mut cursor = position + 2;
        while cursor < source.len() {
            if source[cursor..].starts_with("/*") {
                depth += 1;
                cursor += 2;
            } else if source[cursor..].starts_with("*/") {
                depth -= 1;
                cursor += 2;
                if depth == 0 {
                    return Ok(Some(cursor));
                }
            } else {
                cursor += source[cursor..].chars().next().unwrap().len_utf8();
            }
        }
        return Err("unclosed Rust block comment".into());
    }
    let raw_start = if rest.starts_with("br") || rest.starts_with("cr") {
        Some(2)
    } else if rest.starts_with('r') {
        Some(1)
    } else {
        None
    };
    if let Some(mut cursor) = raw_start {
        let hash_start = cursor;
        while rest.as_bytes().get(cursor) == Some(&b'#') {
            cursor += 1;
        }
        if rest.as_bytes().get(cursor) == Some(&b'"') {
            let closing = format!("\"{}", "#".repeat(cursor - hash_start));
            let end = rest[cursor + 1..]
                .find(&closing)
                .ok_or("unclosed Rust raw string")?;
            return Ok(Some(position + cursor + 1 + end + closing.len()));
        }
    }
    let quote_start =
        if rest.starts_with("b\"") || rest.starts_with("c\"") || rest.starts_with("b'") {
            1
        } else {
            0
        };
    let delimiter = rest.as_bytes().get(quote_start).copied();
    if !matches!(delimiter, Some(b'"' | b'\'')) {
        return Ok(None);
    }
    let delimiter = delimiter.unwrap();
    if delimiter == b'\'' {
        // A char consists of one scalar (or escape) followed by a quote.
        let after = &rest[quote_start + 1..];
        if !after.starts_with('\\') {
            let len = after.chars().next().map(char::len_utf8).unwrap_or(0);
            if after.as_bytes().get(len) != Some(&b'\'') {
                return Ok(None);
            }
        }
    }
    let mut escaped = false;
    for (offset, character) in rest[quote_start + 1..].char_indices() {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character as u32 == delimiter as u32 {
            return Ok(Some(position + quote_start + 1 + offset + 1));
        }
    }
    Err("unclosed Rust literal".into())
}
