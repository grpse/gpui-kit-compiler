const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const test = require('node:test');
class Position { constructor(line, character) { Object.assign(this, { line, character }); } }
class Range { constructor(line, start, endLine, end) { Object.assign(this, { line, start, end }); } }
class TextEdit { constructor(range, newText) { Object.assign(this, { range, newText }); } }
let format;
const vscode = { Position, Range, TextEdit, languages: {
  registerDocumentFormattingEditProvider(_, provider) { format = provider.provideDocumentFormattingEdits; },
  registerCompletionItemProvider() {},
} };
const sandbox = { require: () => vscode, module: { exports: {} } };
vm.runInNewContext(fs.readFileSync(`${__dirname}/extension.js`, 'utf8'), sandbox);
sandbox.module.exports.activate({ subscriptions: [] });
function formatted(source, options = { tabSize: 2, insertSpaces: true }) {
  const lines = source.split('\n');
  const starts = lines.map((_, i) => lines.slice(0, i).reduce((n, line) => n + line.length + 1, 0));
  const document = { getText: () => source, offsetAt: p => starts[p.line] + p.character,
    positionAt: offset => new Position(starts.findLastIndex(start => start <= offset), 0) };
  for (const edit of format(document, options)) {
    lines[edit.range.line] = edit.newText + lines[edit.range.line].slice(edit.range.end);
  }
  return lines.join('\n');
}
test('nested conditional branches, multiline attributes, and closing delimiters', () => {
  const input = 'fn App() {\n  <div>\n{if ready {\n<Card\nvalue={value}\n/>\n} else if other {\n<div>\n{if nested {\n<Other />\n}}\n</div>\n} else {\n<Fallback />\n}}\n  </div>\n}';
  const expected = 'fn App() {\n  <div>\n    {if ready {\n      <Card\n        value={value}\n      />\n    } else if other {\n      <div>\n        {if nested {\n          <Other />\n        }}\n      </div>\n    } else {\n      <Fallback />\n    }}\n  </div>\n}';
  assert.equal(formatted(input), expected);
  assert.equal(formatted(expected), expected);
});
test('separate roots keep their own baseline; tabs and CRLF survive', () => {
  const input = '\t<div>\r\n<span />\r\n</div>\r\n\t\t<div>\r\n<br />\r\n</div>';
  const expected = '\t<div>\r\n\t\t<span />\r\n\t</div>\r\n\t\t<div>\r\n\t\t\t<br />\r\n\t\t</div>';
  assert.equal(formatted(input, { tabSize: 4, insertSpaces: false }), expected);
});
test('Rust strings, comments, comparisons and generics do not create markup', () => {
  const input = 'fn helper() {\n  let s = r#"\n<div>\n}"#;\n  // <div>\n  let x: Vec<String> = vec![];\n  if x.len() < 2 { return; }\n}\n<div>\n{if ok {\n// <div> {\n<Ready />\n}}\n</div>';
  assert.equal(formatted(input), input.replace('\n{if ok', '\n  {if ok').replace('\n<Ready', '\n    <Ready').replace('\n}}', '\n  }}'));
});
