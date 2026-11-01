//! Full-text search over the rendered docs, split into one entry per h2/h3 section.
//! SQLite FTS5 with the trigram tokenizer gives indexed substring matching, which suits Korean
//! (particles attach to words, so "패키지" must match "패키지를"). Trigrams can't match terms shorter
//! than three characters, so those are checked against lowercased copies of the sections kept in memory.

use std::{
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use regex::{Captures, Regex};
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::{docs::encode_component, highlight::escape};

pub struct SearchDoc {
    pub slug: String,
    pub title: String,
    /// "Bun › 설명서"
    pub place: String,
    pub html: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchHit {
    pub url: String,
    pub place: String,
    pub title: String,
    pub heading: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub query: String,
    pub total: usize,
    pub took_ms: f64,
    pub hits: Vec<SearchHit>,
}

struct Row {
    slug: String,
    place: String,
    anchor: String,
    title: String,
    heading: String,
    body: String,
    folded_title: String,
    folded_heading: String,
    folded_body: String,
    folded_all: String,
}

pub struct BuildStats {
    pub docs: usize,
    pub sections: usize,
    pub ms: f64,
}

const MAX_QUERY_LENGTH: usize = 200;
const MAX_TERMS: usize = 8;
const SNIPPET_LENGTH: usize = 140;

struct Inner {
    db: Connection,
    /// Section i is stored in the FTS table as rowid i + 1.
    rows: Vec<Row>,
}

pub struct SearchIndex {
    inner: Mutex<Inner>,
    stale: AtomicBool,
    load: Box<dyn Fn() -> Vec<SearchDoc> + Send + Sync>,
}

impl SearchIndex {
    pub fn new(load: impl Fn() -> Vec<SearchDoc> + Send + Sync + 'static) -> Self {
        let db = Connection::open_in_memory().expect("in-memory SQLite");
        db.execute_batch("CREATE VIRTUAL TABLE sections USING fts5(title, heading, body, tokenize = 'trigram')")
            .expect("FTS5 trigram table");
        Self { inner: Mutex::new(Inner { db, rows: Vec::new() }), stale: AtomicBool::new(true), load: Box::new(load) }
    }

    /// Marks the index out of date; the next search rebuilds it.
    pub fn invalidate(&self) {
        self.stale.store(true, Ordering::SeqCst);
    }

    pub fn build(&self) -> BuildStats {
        let mut inner = self.inner.lock().unwrap();
        self.build_locked(&mut inner)
    }

    fn build_locked(&self, inner: &mut Inner) -> BuildStats {
        let start = Instant::now();
        // Cleared before loading so a change that lands mid-build triggers another rebuild.
        self.stale.store(false, Ordering::SeqCst);
        let docs = (self.load)();
        let rows: Vec<Row> = docs.iter().flat_map(split_sections).collect();
        let tx = inner.db.transaction().expect("transaction");
        tx.execute("DELETE FROM sections", []).expect("clear index");
        {
            let mut insert = tx
                .prepare("INSERT INTO sections (rowid, title, heading, body) VALUES (?, ?, ?, ?)")
                .expect("prepare insert");
            for (i, r) in rows.iter().enumerate() {
                insert.execute(params![i as i64 + 1, r.title, r.heading, r.body]).expect("insert section");
            }
        }
        tx.commit().expect("commit index");
        let stats = BuildStats { docs: docs.len(), sections: rows.len(), ms: start.elapsed().as_secs_f64() * 1000.0 };
        inner.rows = rows;
        stats
    }

    pub fn search(&self, raw_query: &str, limit: usize) -> SearchResult {
        let mut inner = self.inner.lock().unwrap();
        if self.stale.load(Ordering::SeqCst) {
            self.build_locked(&mut inner);
        }

        let start = Instant::now();
        let query: String = crate::docs::nfc(raw_query).trim().chars().take(MAX_QUERY_LENGTH).collect();
        let mut terms: Vec<String> = Vec::new();
        for t in query.to_lowercase().split_whitespace() {
            if !terms.iter().any(|x| x == t) {
                terms.push(t.to_owned());
            }
        }
        terms.truncate(MAX_TERMS);
        if terms.is_empty() {
            return SearchResult { query, total: 0, took_ms: 0.0, hits: Vec::new() };
        }

        let long: Vec<&String> = terms.iter().filter(|t| t.chars().count() >= 3).collect();
        let short: Vec<&String> = terms.iter().filter(|t| t.chars().count() < 3).collect();

        let candidates: Vec<&Row> = if !long.is_empty() {
            let matcher = long.iter().map(|t| format!("\"{}\"", t.replace('"', "\"\""))).collect::<Vec<_>>().join(" ");
            let mut stmt = inner
                .db
                .prepare_cached("SELECT rowid FROM sections WHERE sections MATCH ? ORDER BY bm25(sections, 10.0, 5.0, 1.0)")
                .expect("prepare search");
            let ids: Vec<i64> = stmt
                .query_map([matcher], |r| r.get(0))
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default();
            ids.into_iter().filter_map(|id| inner.rows.get(id as usize - 1)).collect()
        } else {
            // Without a MATCH there is no bm25; rank by where the short terms appear and how often.
            let score = |r: &Row| {
                short.iter().fold(0usize, |sum, t| {
                    sum + if r.folded_title.contains(t.as_str()) { 10 } else { 0 }
                        + if r.folded_heading.contains(t.as_str()) { 5 } else { 0 }
                        + r.folded_body.matches(t.as_str()).count()
                })
            };
            let mut scored: Vec<(&Row, usize)> = inner.rows.iter().map(|r| (r, score(r))).filter(|(_, s)| *s > 0).collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            scored.into_iter().map(|(r, _)| r).collect()
        };
        let matched: Vec<&Row> = candidates.into_iter().filter(|r| short.iter().all(|t| r.folded_all.contains(t.as_str()))).collect();
        let total = matched.len();
        let alternation = terms.iter().map(|t| regex::escape(t)).collect::<Vec<_>>().join("|");
        let matcher = Regex::new(&format!("(?i){alternation}")).expect("term regex");

        let hits = matched
            .into_iter()
            .take(limit.clamp(1, 50))
            .map(|r| {
                let path: Vec<String> = r.slug.split('/').map(encode_component).collect();
                let anchor = if r.anchor.is_empty() { String::new() } else { format!("#{}", encode_component(&r.anchor)) };
                SearchHit {
                    place: escape(&r.place),
                    url: format!("/docs/{}?hl={}{anchor}", path.join("/"), encode_component(&query)),
                    title: mark(&r.title, &matcher),
                    heading: if r.heading == r.title { String::new() } else { mark(&r.heading, &matcher) },
                    snippet: mark(&excerpt(&r.body, &terms), &matcher),
                }
            })
            .collect();
        SearchResult { query, total, took_ms: start.elapsed().as_secs_f64() * 1000.0, hits }
    }
}

// ---------------------------------------------------------------------------------------------

static ANCHOR_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r##"<a class="anchor"[^>]*>#</a>"##).unwrap());
static MERMAID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)<pre><code class="language-mermaid">.*?</code></pre>"#).unwrap());
static SECTION_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)<h([23]) id="([^"]+)">(.*?)</h[23]>"#).unwrap());
static H1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<h1[^>]*>.*?</h1>").unwrap());
static BLOCK_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</?(p|li|td|th|tr|pre|div|br|ul|ol|table|thead|tbody|blockquote|h\d)\b[^>]*>").unwrap());
static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&(#x[0-9a-f]+|#\d+|[a-z]+);").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Splits a rendered page at its h2/h3 headings; text before the first heading belongs to the page title.
fn split_sections(doc: &SearchDoc) -> Vec<Row> {
    let html = ANCHOR_LINK.replace_all(&doc.html, "");
    // Diagram source is mostly arrow syntax, which makes poor search snippets.
    let html = MERMAID.replace_all(&html, "");
    let mut rows = Vec::new();
    let mut anchor = String::new();
    let mut title = doc.title.clone();
    let mut from = 0;
    let mut push = |to: usize, anchor: &str, title: &str, from: usize| {
        let body = to_text(&H1.replacen(&html[from..to], 1, ""));
        if body.is_empty() && anchor.is_empty() {
            return;
        }
        let folded_title = doc.title.to_lowercase();
        let folded_heading = title.to_lowercase();
        let folded_body = body.to_lowercase();
        let folded_all = format!("{folded_title} {folded_heading} {folded_body}");
        rows.push(Row {
            slug: doc.slug.clone(),
            place: doc.place.clone(),
            anchor: anchor.to_owned(),
            title: doc.title.clone(),
            heading: title.to_owned(),
            body,
            folded_title,
            folded_heading,
            folded_body,
            folded_all,
        });
    };
    for m in SECTION_HEADING.captures_iter(&html) {
        let whole = m.get(0).unwrap();
        push(whole.start(), &anchor, &title, from);
        anchor = m[2].to_owned();
        title = to_text(&m[3]);
        from = whole.end();
    }
    push(html.len(), &anchor, &title, from);
    rows
}

fn to_text(html: &str) -> String {
    let text = BLOCK_TAG.replace_all(html, " ");
    let text = remove_tags(&text);
    let text = ENTITY.replace_all(&text, |c: &Captures| {
        let code = &c[1];
        if let Some(num) = code.strip_prefix('#') {
            let value = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => num.parse().ok(),
            };
            return value.and_then(char::from_u32).map_or_else(|| c[0].to_owned(), String::from);
        }
        match code.to_lowercase().as_str() {
            "amp" => "&",
            "lt" => "<",
            "gt" => ">",
            "quot" => "\"",
            "apos" => "'",
            "nbsp" => " ",
            _ => return c[0].to_owned(),
        }
        .to_owned()
    });
    crate::docs::nfc(SPACES.replace_all(&text, " ").trim())
}

fn remove_tags(html: &str) -> String {
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
    TAG.replace_all(html, "").into_owned()
}

/// A window of body text around the first matching term (counted in characters).
fn excerpt(body: &str, terms: &[String]) -> String {
    let chars: Vec<char> = body.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let first = terms
        .iter()
        .filter_map(|t| {
            let needle: Vec<char> = t.chars().collect();
            if needle.is_empty() || needle.len() > lower.len() {
                return None;
            }
            lower.windows(needle.len()).position(|w| w == needle.as_slice())
        })
        .min()
        .unwrap_or(0);
    let start = first.saturating_sub(40);
    let end = (start + SNIPPET_LENGTH).min(chars.len());
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

/// Escapes text for HTML and wraps every term match in <mark>.
fn mark(text: &str, matcher: &Regex) -> String {
    let mut out = String::new();
    let mut last = 0;
    for m in matcher.find_iter(text) {
        out.push_str(&escape(&text[last..m.start()]));
        out.push_str("<mark>");
        out.push_str(&escape(m.as_str()));
        out.push_str("</mark>");
        last = m.end();
    }
    out.push_str(&escape(&text[last..]));
    out
}
