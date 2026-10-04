const vscode = require("vscode");

const genericTags = ["div"];

const styleProperties = [
  "display", "flexDirection", "flexWrap", "flex", "gap", "padding",
  "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
  "background", "backgroundColor", "color", "textColor", "fontSize", "fontWeight",
  "font", "fontFamily", "fontStyle", "fontFeatures", "lineHeight", "whiteSpace", "textOverflow", "lineClamp",
  "flexShrink", "flexBasis", "minHeight", "maxHeight", "aspectRatio",
  "border", "borderColor", "borderRadius", "width", "height", "minWidth",
  "maxWidth", "margin", "marginAuto", "justifyContent", "alignItems",
  "position", "top", "right", "bottom", "left", "overflow", "overflowY",
  "opacity",
];

const voidTags = new Set(["input", "img", "br", "hr"]);

function parseMarkupTag(source, start) {
  const match = source.slice(start).match(/^<(\/)?([A-Za-z][A-Za-z0-9:-]*)/);
  if (!match) return undefined;

  let quote;
  let escaped = false;
  let braces = 0;
  for (let index = start + match[0].length; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = undefined;
      continue;
    }
    if (character === "'" || character === '"') quote = character;
    else if (character === "{") braces += 1;
    else if (character === "}" && braces > 0) braces -= 1;
    else if (character === ">" && braces === 0) {
      const tagName = match[2];
      const closing = Boolean(match[1]);
      const selfClosing = /\/\s*>$/.test(source.slice(start, index + 1));
      return {
        name: tagName,
        closing,
        selfClosing,
        end: index + 1,
        component: /^[A-Z]/.test(tagName),
      };
    }
  }
  return undefined;
}

function provideFormattingEdits(document, options) {
  const source = document.getText();
  const lines = source.split(/\r\n|\n|\r/);
  const lineStarts = lines.map((_, index) => document.offsetAt(new vscode.Position(index, 0)));

  const indentSize = Number.isInteger(options.tabSize) && options.tabSize > 0 ? options.tabSize : 4;
  const indentUnit = options.insertSpaces ? " ".repeat(indentSize) : "\t";
  const stack = [];
  const lineIndents = new Map();
  let baseline;
  let cursor = 0;
  const depth = () => stack.reduce((total, frame) => total + frame.indent, 0);
  const record = (line, level) => {
    if (!lineIndents.has(line)) lineIndents.set(line, baseline + indentUnit.repeat(level));
  };

  while (cursor < source.length) {
    const lineNumber = document.positionAt(cursor).line;
    const lineStart = lineStarts[lineNumber] ?? 0;
    const beginsLine = /^\s*$/.test(source.slice(lineStart, cursor));
    const activeMarkup = stack.length > 0;
    const character = source[cursor];

    // Rust strings and comments may contain fake tags or unmatched braces.
    // Plain markup text remains text, including apostrophes and quotes.
    const inRust = !activeMarkup || stack.at(-1).kind === "brace";
    if (inRust) {
      const rest = source.slice(cursor);
      const raw = rest.match(/^(?:b)?r(#+)?"/);
      if (raw) {
        const end = source.indexOf('"' + (raw[1] || ""), cursor + raw[0].length);
        cursor = end < 0 ? source.length : end + 1 + (raw[1] || "").length;
        continue;
      }
      if (rest.startsWith("//")) {
        cursor = source.indexOf("\n", cursor);
        if (cursor < 0) break;
        continue;
      }
      if (rest.startsWith("/*")) {
        let comments = 1;
        cursor += 2;
        while (cursor < source.length && comments) {
          if (source.startsWith("/*", cursor)) { comments += 1; cursor += 2; }
          else if (source.startsWith("*/", cursor)) { comments -= 1; cursor += 2; }
          else cursor += 1;
        }
        continue;
      }
      if (character === '"' || (character === "'" && /^'(?:\\.|[^'\\])'/.test(rest))) {
        const quote = character;
        cursor += 1;
        while (cursor < source.length) {
          if (source[cursor] === "\\") cursor += 2;
          else if (source[cursor++] === quote) break;
        }
        continue;
      }
    }

    if (activeMarkup && character === "{") {
      if (beginsLine) record(lineNumber, depth());
      // The interpolation wrapper adds no level: the if body supplies it.
      const wrapper = /^\{\s*if\b/.test(source.slice(cursor));
      stack.push({ kind: "brace", indent: wrapper ? 0 : 1 });
      cursor += 1;
      continue;
    }
    if (activeMarkup && character === "}" && stack.at(-1).kind === "brace") {
      stack.pop();
      if (beginsLine) record(lineNumber, depth());
      cursor += 1;
      continue;
    }

    if (character !== "<" || (!activeMarkup && !beginsLine)) {
      if (activeMarkup && beginsLine && !/\s/.test(character)) record(lineNumber, depth());
      cursor += 1;
      continue;
    }
    const tag = parseMarkupTag(source, cursor);
    if (!tag) { cursor += 1; continue; }
    // Imported components are self-closing; other uppercase names are Rust generics.
    if (tag.component && (!tag.selfClosing || tag.closing)) {
      cursor = tag.end;
      continue;
    }
    if (!activeMarkup) baseline = (lines[lineNumber] ?? "").match(/^\s*/)[0];
    if (tag.closing) {
      const stackIndex = stack.findLastIndex((frame) => frame.kind === "tag" && frame.name === tag.name);
      if (stackIndex >= 0) stack.length = stackIndex;
    }
    if (beginsLine) record(lineNumber, depth());
    const endLine = document.positionAt(tag.end - 1).line;
    for (let line = lineNumber + 1; line <= endLine; line += 1) {
      const closingDelimiter = /^\s*\/?>\s*$/.test(lines[line]);
      record(line, depth() + (closingDelimiter ? 0 : 1));
    }
    if (!tag.closing && !tag.selfClosing && !tag.component && !voidTags.has(tag.name.toLowerCase())) {
      stack.push({ kind: "tag", name: tag.name, indent: 1 });
    }
    cursor = tag.end;
  }

  const edits = [];
  for (const [lineNumber, desiredIndent] of lineIndents) {
    const line = lines[lineNumber] ?? "";
    const currentIndent = line.match(/^\s*/)?.[0] ?? "";
    if (currentIndent === desiredIndent) continue;
    edits.push(
      new vscode.TextEdit(
        new vscode.Range(lineNumber, 0, lineNumber, currentIndent.length),
        desiredIndent,
      ),
    );
  }
  return edits;
}

function completion(label, detail, insertText = label, kind = vscode.CompletionItemKind.Property) {
  const item = new vscode.CompletionItem(label, kind);
  item.detail = detail;
  item.insertText = /\$(?:\d|\{\d+:)/.test(insertText) ? new vscode.SnippetString(insertText) : insertText;
  return item;
}

function rustStyleCompletions(document, position) {
  const source = document.getText().slice(0, document.offsetAt(position));
  const start = source.lastIndexOf("styles(");
  if (start < 0) return undefined;
  const objectStart = source.indexOf("{", start);
  if (objectStart < 0) return undefined;

  let depth = 0;
  let quoted = false;
  let escaped = false;
  for (let i = objectStart; i < source.length; i += 1) {
    const character = source[i];
    if (quoted) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') quoted = false;
      continue;
    }
    if (character === '"') quoted = true;
    else if (character === "{") depth += 1;
    else if (character === "}") depth -= 1;
  }
  if (depth !== 2) return undefined;

  const fragmentStart = Math.max(source.lastIndexOf("{"), source.lastIndexOf(",")) + 1;
  const fragment = source.slice(fragmentStart).trim();
  if (fragment.includes(":")) return undefined;
  return styleProperties.map((name) =>
    completion(name, "Rust GPUI style property", `${name}: `),
  );
}

function tagCompletions() {
  const items = genericTags.map((tag) =>
    completion(tag, "GPUI div element", `${tag}>$0</${tag}>`, vscode.CompletionItemKind.Class),
  );
  items.push(
    completion("button", "GPUI button; run a Rust expression when clicked", 'button on-click={$1}>$0</button>', vscode.CompletionItemKind.Class),
    completion("input", "GPUI range slider bound to a signal or mutable value", 'input id="${1:value}" type="range" min="0" max="100" step="1" value={${2:value}} />', vscode.CompletionItemKind.Class),
    completion("img", "GPUI image from a file path or URL", 'img src="${1:path/to/image.png}" alt="${2:Description}" />', vscode.CompletionItemKind.Class),
    completion("textarea", "GPUI multiline text input", 'textarea id="${1:notes}" value={${2:notes}}></textarea>', vscode.CompletionItemKind.Class),
    completion("progress", "GPUI progress indicator", 'progress value={${1:progress}} max="${2:1}"></progress>', vscode.CompletionItemKind.Class),
    completion("select", "GPUI selection control with Rust-mapped options", 'select id="${1:method}" value={${2:method}}>{${3:methods}.iter().map(|&${4:method}| => <option value={${4:method}}>{${4:method}}</option>)}</select>', vscode.CompletionItemKind.Class),
    completion("option", "Option for a GPUI select", 'option value="$1">$0</option>', vscode.CompletionItemKind.Class),
    completion("output", "Formatted or bar output; plain values can use {name} in a div", 'output data-in="$1"></output>', vscode.CompletionItemKind.Class),
    completion("ComponentName", "Instantiate an imported Rust component function", 'ComponentName $0/>', vscode.CompletionItemKind.Class),
  );
  return items;
}

function attributeCompletions(tag, used) {
  const attrs = [
    ["class", "Bind a Rust style, for example {myStyles.card}", "class={myStyles.$1}"],
    ["id", "Element id; also identifies select options"],
  ];
  if (tag === "button") attrs.push(["on-click", "Run a Rust expression when the button is clicked", "on-click={$1}"], ["data-out", "Legacy binding to a Rust action"], ["data-args", "Comma-separated Rust action arguments"]);
  if (tag === "input") attrs.push(["type", "Supported control type", 'type="range"'], ["min", "Range lower bound"], ["max", "Range upper bound"], ["step", "Range increment"], ["value", "Two-way binding to a signal or mutable Rust value", 'value={$1}'], ["checked", "Two-way checkbox binding", 'checked={$1}'], ["placeholder", "Input placeholder"]);
  if (tag === "textarea") attrs.push(["value", "Two-way text binding", 'value={$1}'], ["placeholder", "Input placeholder"]);
  if (tag === "img") attrs.push(["src", "File path, URL, or readable binding", 'src="$1"'], ["alt", "Description for fallback"], ["width", "Width in pixels"], ["height", "Height in pixels"], ["object-fit", "Image fitting mode", 'object-fit="contain"']);
  if (tag === "progress") attrs.push(["value", "Current value or readable binding", 'value={$1}'], ["max", "Maximum value", 'max="1"']);
  if (tag === "select") attrs.push(["value", "Two-way binding to a signal or mutable Rust value", 'value={$1}']);
  if (tag === "output") attrs.push(["data-in", "Read-only Rust binding"], ["data-render", "Output presentation", 'data-render="bar"'], ["data-format", "Custom output formatter key"]);
  if (tag === "option") attrs.push(["value", "Value written by the select"], ["selected", "Use this option as the initial value"]);
  return attrs
    .filter(([name]) => !used.has(name))
    .map(([name, detail, snippet]) => completion(name, detail, snippet || `${name}="$1"`, vscode.CompletionItemKind.Field));
}

function provideCompletions(document, position) {
  const config = vscode.workspace.getConfiguration("gpuiRsc");
  if (!config.get("completions.enabled", true)) return undefined;

  const prefix = document.lineAt(position).text.slice(0, position.character);
  const tagStart = prefix.match(/<([A-Za-z][A-Za-z0-9:-]*)?$/);
  if (tagStart) return tagCompletions();

  const styleCompletions = rustStyleCompletions(document, position);
  if (styleCompletions) return styleCompletions;

  const openingTag = prefix.match(/<([A-Za-z][A-Za-z0-9:-]*)\b([^<>]*)$/);
  if (openingTag) {
    const tag = openingTag[1].toLowerCase();
    const used = new Set([...openingTag[2].matchAll(/([A-Za-z_:][\w:.-]*)\s*=/g)].map((item) => item[1]));
    const value = openingTag[2].match(/([A-Za-z_:][\w:.-]*)\s*=\s*(["'][^"']*)$/);
    if (value && value[1] === "type" && tag === "input") {
      return ["range", "text", "email", "password", "search", "url", "tel", "date", "checkbox"].map((type) => completion(type, "Supported GPUI input type", type, vscode.CompletionItemKind.Value));
    }
    return attributeCompletions(tag, used);
  }

  const beforeCursor = document.getText().slice(0, document.offsetAt(position));
  const interpolation = prefix.match(/\{[^{}]*$/);
  const lastSelect = beforeCursor.toLowerCase().lastIndexOf("<select");
  const lastSelectClose = beforeCursor.toLowerCase().lastIndexOf("</select>");
  if (interpolation && lastSelect > lastSelectClose) {
    return [completion("options map", "Create GPUI select options from a Rust iterator", 'methods.iter().map(|&method| => <option value={method}>{method}</option>)', vscode.CompletionItemKind.Snippet)];
  }
  return undefined;
}

function activate(context) {
  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(
      { language: "gpui-rsc", scheme: "file" },
      { provideDocumentFormattingEdits: provideFormattingEdits },
    ),
    vscode.languages.registerCompletionItemProvider(
      { language: "gpui-rsc", scheme: "file" },
      { provideCompletionItems: provideCompletions },
      "<",
      "{",
      ";",
      ",",
      ":",
    ),
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
