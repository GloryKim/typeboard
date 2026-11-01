//! Colors fenced code blocks on the server (syntect).
//!
//! Tokens get `hl-*` classes; `/static/highlight.css` colors only the kinds VS Code's Dark+ and
//! Light+ themes tell apart (keywords, strings, comments, numbers, functions, types, variables),
//! leaving brackets, operators, and punctuation in the normal text color. The page follows the
//! system color scheme without running any script.

use std::{borrow::Cow, collections::HashMap, fmt};

use comrak::adapters::SyntaxHighlighterAdapter;
use syntect::{
    html::{ClassStyle, line_tokens_to_classed_spans},
    parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};

/// The Rust grammar files `let`/`const` and `u32`/`bool` under the same scope; these words are
/// moved to the type scope so they color like types, as rust-analyzer shows them in VS Code.
static RUST_PRIMITIVE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r#"<span class="hl-storage hl-type hl-rust">(bool|char|str|[ui](?:8|16|32|64|128|size)|f32|f64|Self)</span>"#).unwrap()
});

const STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

/// Fences shown as plain text: program output and diagrams.
const PLAIN: &[&str] = &["text", "txt", "plain", "output", "console", "mermaid"];

/// Commands get no line numbers (they read like a terminal); source code and config files do.
const UNNUMBERED: &[&str] = &["bash", "sh", "shell", "zsh", "powershell", "ps1", "cmd", "bat"];

/// VS Code Dark+ / Light+ token colors. Selectors are scope atoms, so `.hl-string` covers every
/// kind of string; later, more specific rules win.
const CSS: &str = r#"
:root {
  --hl-comment: #008000;
  --hl-keyword: #0000ff;
  --hl-control: #af00db;
  --hl-string: #a31515;
  --hl-number: #098658;
  --hl-function: #795e26;
  --hl-type: #267f99;
  --hl-variable: #001080;
  --hl-constant: #0070c1;
  --hl-tag: #800000;
  --hl-attribute: #e50000;
  --hl-escape: #ee0000;
  --hl-regexp: #811f3f;
}
@media (prefers-color-scheme: dark) {
  :root {
    --hl-comment: #6a9955;
    --hl-keyword: #569cd6;
    --hl-control: #c586c0;
    --hl-string: #ce9178;
    --hl-number: #b5cea8;
    --hl-function: #dcdcaa;
    --hl-type: #4ec9b0;
    --hl-variable: #9cdcfe;
    --hl-constant: #4fc1ff;
    --hl-tag: #569cd6;
    --hl-attribute: #9cdcfe;
    --hl-escape: #d7ba7d;
    --hl-regexp: #d16969;
  }
}

/* variables, parameters, properties, object keys */
.hl-variable, .hl-meta.hl-object-literal.hl-key, .hl-support.hl-variable.hl-property,
.hl-variable.hl-other.hl-member, .hl-entity.hl-name.hl-label { color: var(--hl-variable); }
.hl-variable.hl-other.hl-constant, .hl-constant.hl-other, .hl-support.hl-constant,
.hl-entity.hl-name.hl-constant { color: var(--hl-constant); }
.hl-variable.hl-language { color: var(--hl-keyword); }

/* keywords and declarations: blue; control flow (if, return, import, match): purple */
.hl-keyword, .hl-storage, .hl-constant.hl-language, .hl-keyword.hl-operator.hl-new,
.hl-keyword.hl-operator.hl-expression, .hl-keyword.hl-operator.hl-word,
.hl-punctuation.hl-definition.hl-template-expression { color: var(--hl-keyword); }
.hl-keyword.hl-control, .hl-keyword.hl-other.hl-import, .hl-keyword.hl-import { color: var(--hl-control); }
/* operators stay plain, as in VS Code */
.hl-keyword.hl-operator { color: inherit; }
.hl-keyword.hl-operator.hl-new, .hl-keyword.hl-operator.hl-expression,
.hl-keyword.hl-operator.hl-word, .hl-keyword.hl-operator.hl-logical.hl-python { color: var(--hl-keyword); }

/* functions and macros */
.hl-entity.hl-name.hl-function, .hl-support.hl-function, .hl-variable.hl-function,
.hl-meta.hl-function-call .hl-variable.hl-function, .hl-support.hl-macro,
.hl-entity.hl-name.hl-macro, .hl-meta.hl-macro { color: var(--hl-function); }

/* types, classes, modules */
.hl-entity.hl-name.hl-type, .hl-entity.hl-name.hl-class, .hl-entity.hl-name.hl-struct,
.hl-entity.hl-name.hl-enum, .hl-entity.hl-name.hl-trait, .hl-entity.hl-name.hl-interface,
.hl-entity.hl-other.hl-inherited-class, .hl-support.hl-type, .hl-support.hl-class,
.hl-entity.hl-name.hl-namespace, .hl-entity.hl-name.hl-module, .hl-storage.hl-type.hl-primitive, .hl-meta.hl-type-annotation .hl-entity.hl-name { color: var(--hl-type); }
/* declaration words that the grammars file under storage.type */
.hl-storage.hl-type.hl-function,
.hl-storage.hl-type.hl-struct, .hl-storage.hl-type.hl-enum, .hl-storage.hl-type.hl-trait,
.hl-storage.hl-type.hl-impl, .hl-storage.hl-type.hl-module, .hl-storage.hl-type.hl-type,
.hl-storage.hl-type.hl-ts, .hl-storage.hl-type.hl-js, .hl-storage.hl-type.hl-python,
.hl-storage.hl-type.hl-go, .hl-storage.hl-type.hl-java { color: var(--hl-keyword); }

/* literals */
.hl-string, .hl-punctuation.hl-definition.hl-string { color: var(--hl-string); }
.hl-string.hl-regexp { color: var(--hl-regexp); }
.hl-constant.hl-character.hl-escape { color: var(--hl-escape); }
.hl-constant.hl-numeric { color: var(--hl-number); }
/* `${...}` inside a template string is code again */
.hl-meta.hl-template.hl-expression, .hl-meta.hl-embedded.hl-line { color: var(--fg); }

/* comments */
.hl-comment, .hl-punctuation.hl-definition.hl-comment { color: var(--hl-comment); }

/* markup and config keys (HTML tags, YAML/TOML/JSON keys) */
.hl-entity.hl-name.hl-tag { color: var(--hl-tag); }
.hl-entity.hl-other.hl-attribute-name { color: var(--hl-attribute); }
.hl-support.hl-type.hl-property-name, .hl-meta.hl-mapping.hl-key .hl-string { color: var(--hl-variable); }

/* Rust attributes (#[derive]) and lifetimes */
.hl-meta.hl-annotation, .hl-meta.hl-attribute { color: inherit; }
.hl-storage.hl-modifier.hl-lifetime, .hl-entity.hl-name.hl-lifetime { color: var(--hl-keyword); }
"#;

pub struct Highlighter {
    syntaxes: SyntaxSet,
}

impl Highlighter {
    pub fn new() -> Self {
        Self { syntaxes: two_face::syntax::extra_newlines() }
    }

    /// The stylesheet served at `/static/highlight.css`.
    pub fn css(&self) -> &str {
        CSS
    }

    fn syntax(&self, lang: &str) -> Option<&SyntaxReference> {
        let lang = lang.to_ascii_lowercase();
        if PLAIN.contains(&lang.as_str()) {
            return None;
        }
        let token = match lang.as_str() {
            "shell" | "zsh" => "bash",
            "javascript" => "js",
            "typescript" => "ts",
            "yml" => "yaml",
            "golang" => "go",
            "py" => "python",
            "cypher" => "cql",
            other => other,
        };
        self.syntaxes
            .find_syntax_by_token(token)
            .or_else(|| self.syntaxes.find_syntax_by_name(&lang))
            .filter(|s| s.name != "Plain Text")
    }

    /// Each source line becomes a self-contained `<span class="line">`, reopening any scope that
    /// spans lines (block comments, template strings) so the line-number CSS can make it a block.
    fn highlight(&self, syntax: &SyntaxReference, code: &str, numbered: bool) -> Option<String> {
        let mut state = ParseState::new(syntax);
        let mut stack = ScopeStack::new();
        let mut out = String::with_capacity(code.len() * 3);
        let code = code.strip_suffix('\n').unwrap_or(code);
        for (i, line) in LinesWithEndings::from(code).enumerate() {
            if i > 0 && !numbered {
                out.push('\n');
            }
            out.push_str("<span class=\"line\">");
            let open = stack.as_slice().len();
            for scope in stack.as_slice() {
                out.push_str("<span class=\"");
                push_classes(&mut out, &scope.build_string());
                out.push_str("\">");
            }
            let ops = state.parse_line(line, &self.syntaxes).ok()?;
            let (html, delta) = line_tokens_to_classed_spans(line, &ops, STYLE, &mut stack).ok()?;
            out.push_str(&html.replace('\n', ""));
            let still_open = open as isize + delta;
            for _ in 0..still_open.max(0) {
                out.push_str("</span>");
            }
            out.push_str("</span>");
        }
        if syntax.name == "Rust" {
            out = RUST_PRIMITIVE.replace_all(&out, r#"<span class="hl-storage hl-type hl-primitive hl-rust">$1</span>"#).into_owned();
        }
        Some(out)
    }
}

fn push_classes(out: &mut String, scope: &str) {
    for (i, atom) in scope.split('.').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str("hl-");
        out.push_str(atom);
    }
}

/// Plugs [`Highlighter`] into comrak. comrak writes the `<pre>` tag before it hands over the
/// language, so the `<pre>` is written together with `<code>` once the language is known.
pub struct Adapter<'a>(pub &'a Highlighter);

impl SyntaxHighlighterAdapter for Adapter<'_> {
    fn write_highlighted(&self, output: &mut dyn fmt::Write, lang: Option<&str>, code: &str) -> fmt::Result {
        if let Some(lang) = lang
            && let Some(syntax) = self.0.syntax(lang)
            && let Some(html) = self.0.highlight(syntax, code, numbered(lang))
        {
            return output.write_str(&html);
        }
        output.write_str(&escape(code))
    }

    fn write_pre_tag(&self, _output: &mut dyn fmt::Write, _attributes: HashMap<&'static str, Cow<'_, str>>) -> fmt::Result {
        Ok(())
    }

    fn write_code_tag(&self, output: &mut dyn fmt::Write, attributes: HashMap<&'static str, Cow<'_, str>>) -> fmt::Result {
        let lang = attributes
            .get("class")
            .and_then(|c| c.strip_prefix("language-"))
            .map(|l| l.split_whitespace().next().unwrap_or(""))
            .unwrap_or("");
        let highlighted = !lang.is_empty() && self.0.syntax(lang).is_some();
        let pre_class = match (highlighted, numbered(lang)) {
            (true, true) => " class=\"highlighted has-line-numbers\"",
            (true, false) => " class=\"highlighted\"",
            _ => "",
        };
        if lang.is_empty() {
            write!(output, "<pre{pre_class}><code>")
        } else {
            write!(output, "<pre{pre_class}><code class=\"language-{}\">", escape(lang))
        }
    }
}

fn numbered(lang: &str) -> bool {
    !UNNUMBERED.contains(&lang.to_ascii_lowercase().as_str())
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}
