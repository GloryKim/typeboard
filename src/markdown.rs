//! Markdown → HTML for the doc pages and the search index (comrak, GitHub-flavored).

use std::{collections::HashMap, sync::LazyLock};

use comrak::{Options, markdown_to_html_with_plugins, options::Plugins};
use regex::{Captures, Regex};

use crate::highlight::{Adapter, Highlighter, escape};

#[derive(Clone, Debug)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

pub struct Rendered {
    pub html: String,
    pub headings: Vec<Heading>,
}

static ATX_H1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#[ \t]+(.+?)[ \t]*#*[ \t]*$").unwrap());
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&[#a-z0-9]+;").unwrap());
static NOT_SLUG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}\p{M}_ -]").unwrap());
static UNSAFE_HREF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)href="(?:javascript|vbscript|data):[^"]*""#).unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<h([1-4])>(.*?)</h[1-4]>").unwrap());
static TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<table>.*?</table>").unwrap());
static TH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<th[^>]*>(.*?)</th>").unwrap());
static TR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<tr>.*?</tr>").unwrap());
static TD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<td([\s>])").unwrap());
static MD_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="([^"]*?)\.md(#[^"]*)?""#).unwrap());
static RELATIVE_FILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r##"(src|href)="([^"#?]+\.[A-Za-z0-9]{1,5})""##).unwrap());
static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z][a-z0-9+.-]*:").unwrap());
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<pre[ >].*?</pre>|<code>.*?</code>").unwrap());
static ESCAPED_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&lt;br\s*/?&gt;").unwrap());
static MD_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!?\[([^\]]*)\]\([^)]*\)|[*_`]").unwrap());

/// Lines of `markdown` outside fenced code blocks.
fn prose_lines(markdown: &str) -> impl Iterator<Item = &str> {
    let mut fence = false;
    markdown.lines().filter(move |line| {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            return false;
        }
        !fence
    })
}

/// Korean prose uses `~` for ranges ("3~5개", "01~09"), which GFM would read as strikethrough.
/// A lone `~` outside code is escaped so only `~~double~~` strikes through.
fn escape_single_tildes(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len() + 16);
    let mut fence = false;
    for line in markdown.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
        }
        if fence || !line.contains('~') || t.starts_with("```") || t.starts_with("~~~") {
            out.push_str(line);
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut in_code = false;
        for (i, &c) in chars.iter().enumerate() {
            if c == '`' {
                in_code = !in_code;
            }
            let lone = c == '~'
                && !in_code
                && chars.get(i.wrapping_sub(1)) != Some(&'~')
                && chars.get(i + 1) != Some(&'~')
                && chars.get(i.wrapping_sub(1)) != Some(&'\\');
            if lone {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out
}

/// The text of the first `# Heading` outside code blocks, used as the page title.
pub fn first_heading(markdown: &str) -> Option<String> {
    prose_lines(markdown).find_map(|line| ATX_H1.captures(line).map(|c| MD_INLINE.replace_all(&c[1], "$1").trim().to_owned()))
}

/// The first paragraph as plain text, for category cards.
pub fn first_paragraph(markdown: &str) -> String {
    let mut out = String::new();
    for line in prose_lines(markdown) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with('|') || t.starts_with('>') || t.starts_with('<') {
            if out.is_empty() {
                continue;
            }
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&MD_INLINE.replace_all(t, "$1"));
    }
    out
}

pub fn strip_tags(html: &str) -> String {
    TAGS.replace_all(html, "").trim().to_owned()
}

/// GitHub-style heading ids (lowercase, punctuation dropped, each space a `-`), so the
/// tables of contents written by hand in the docs (`[개요](#1-개요)`) land on their headings.
fn slugify(html: &str, used: &mut HashMap<String, usize>) -> String {
    let text = ENTITY.replace_all(&strip_tags(html), "").to_lowercase();
    let base = NOT_SLUG.replace_all(&text, "").replace(' ', "-");
    let base = if base.is_empty() { "section".to_owned() } else { base };
    let count = used.entry(base.clone()).or_insert(0);
    let id = if *count == 0 { base } else { format!("{base}-{count}") };
    *count += 1;
    id
}

/// Copies each column header onto its cells so narrow screens can show rows as labeled cards.
fn label_table_cells(table: &str) -> String {
    let labels: Vec<String> = TH.captures_iter(table).map(|m| strip_tags(&m[1]).replace('"', "&quot;")).collect();
    TR.replace_all(table, |row: &Captures| {
        let mut i = 0;
        TD.replace_all(&row[0], |td: &Captures| {
            let label = labels.get(i).map(String::as_str).unwrap_or("");
            i += 1;
            format!("<td data-label=\"{label}\"{}", &td[1])
        })
        .into_owned()
    })
    .into_owned()
}

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.autolink = true;
    o.extension.tasklist = true;
    // Raw HTML in a doc is shown as text, never run.
    o.render.escape = true;
    o
}

/// `highlighter` colors code blocks; the search index passes `None` since it only needs text.
/// `file_base` (like `/files/01_Bun/01_설명서/`) is where relative images and other non-Markdown
/// links in the doc point, so they work at the doc's URL.
pub fn render(markdown: &str, highlighter: Option<&Highlighter>, file_base: Option<&str>) -> Rendered {
    let adapter = highlighter.map(Adapter);
    let mut plugins = Plugins::default();
    plugins.render.codefence_syntax_highlighter = adapter.as_ref().map(|a| a as _);
    let html = markdown_to_html_with_plugins(&escape_single_tildes(markdown), &options(), &plugins);

    let html = UNSAFE_HREF.replace_all(&html, "href=\"#\"");

    let mut headings = Vec::new();
    let mut used = HashMap::new();
    let html = HEADING.replace_all(&html, |c: &Captures| {
        let level: u8 = c[1].parse().unwrap();
        let inner = &c[2];
        let id = slugify(inner, &mut used);
        if level == 2 || level == 3 {
            headings.push(Heading { level, id: id.clone(), text: strip_tags(inner) });
        }
        format!("<h{level} id=\"{id}\">{inner}<a class=\"anchor\" href=\"#{id}\" aria-hidden=\"true\">#</a></h{level}>")
    });

    let html = TABLE.replace_all(&html, |c: &Captures| label_table_cells(&c[0]));

    // Lets docs link to each other by file name, e.g. [설치](02_설치.md#요구-사항).
    let html = MD_LINK.replace_all(&html, |c: &Captures| {
        let target = &c[1];
        if SCHEME.is_match(target) || target.starts_with('/') || target.starts_with('#') {
            return c[0].to_owned();
        }
        format!("href=\"{target}{}\"", c.get(2).map_or("", |m| m.as_str()))
    });

    let html = match file_base {
        Some(base) => RELATIVE_FILE.replace_all(&html, |c: &Captures| {
            let target = &c[2];
            if SCHEME.is_match(target) || target.starts_with('/') {
                return c[0].to_owned();
            }
            format!("{}=\"{base}{}\"", &c[1], target.trim_start_matches("./"))
        }),
        None => html,
    };

    Rendered { html: allow_line_breaks(&html), headings }
}

/// `<br>` is the one tag docs may use (for line breaks inside table cells); everything else
/// stays escaped. Code is left as written.
fn allow_line_breaks(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut last = 0;
    for m in CODE.find_iter(html) {
        out.push_str(&ESCAPED_BR.replace_all(&html[last..m.start()], "<br>"));
        out.push_str(m.as_str());
        last = m.end();
    }
    out.push_str(&ESCAPED_BR.replace_all(&html[last..], "<br>"));
    out
}

/// Wraps the searched words in `<mark>` when a page is opened from a search result (`?hl=`).
/// Only text outside tags is touched; code blocks and heading anchors are left alone.
pub fn mark_terms(html: &str, query: &str) -> String {
    let mut terms: Vec<String> = Vec::new();
    for t in query.to_lowercase().split_whitespace().take(8) {
        if !terms.iter().any(|x| x == t) {
            terms.push(t.to_owned());
        }
    }
    if terms.is_empty() {
        return html.to_owned();
    }
    // Entities are matched first so a term never cuts one in half.
    let alternation = terms.iter().map(|t| regex::escape(&escape(t))).collect::<Vec<_>>().join("|");
    let Ok(matcher) = Regex::new(&format!("(?i)(&[#a-z0-9]+;)|({alternation})")) else {
        return html.to_owned();
    };

    let mut out = String::with_capacity(html.len() + 64);
    let mut skip_depth = 0usize; // inside <pre> or <a class="anchor">
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(tag_end) = rest.starts_with('<').then(|| rest.find('>')).flatten() {
            let tag = &rest[..=tag_end];
            if tag.starts_with("<pre") || tag.starts_with("<a class=\"anchor\"") {
                skip_depth += 1;
            } else if (tag == "</pre>" || tag == "</a>") && skip_depth > 0 {
                skip_depth -= 1;
            }
            out.push_str(tag);
            rest = &rest[tag_end + 1..];
            continue;
        }
        // A stray `<` without a closing `>` is plain text.
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let text_end = rest[first..].find('<').map_or(rest.len(), |i| i + first);
        let text = &rest[..text_end];
        if skip_depth > 0 {
            out.push_str(text);
        } else {
            out.push_str(&matcher.replace_all(text, |c: &Captures| match c.get(2) {
                Some(m) => format!("<mark>{}</mark>", m.as_str()),
                None => c[0].to_owned(),
            }));
        }
        rest = &rest[text_end..];
    }
    out
}
