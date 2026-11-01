//! The Markdown library in `docs/`, organized as section → category → document:
//!
//! ```text
//! docs/
//! ├── _index.md                  home page intro (optional)
//! └── 01_Bun/                    section "Bun"
//!     ├── 01_설명서/             category "설명서"
//!     │   ├── _index.md          category name (# heading) and intro (optional)
//!     │   ├── images/            files next to the docs, linked relatively
//!     │   └── 01_Bun_소개.md
//!     └── 260816_note.md         docs directly in a section form its own list
//! ```
//!
//! A leading `NN_` on a folder only sets the order; it is hidden from labels and URLs.
//! A `YYMMDD_HHMM_` prefix on a file name becomes the document's date.

use std::{cmp::Ordering, fs, path::Path, sync::LazyLock};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use crate::markdown::{first_heading, first_paragraph};

pub const INDEX_FILE: &str = "_index.md";
const DIRECT_LABEL: &str = "글 목록";

#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub slug: String,
    /// Path relative to the docs folder, as on disk.
    pub file: String,
    pub title: String,
    /// `2026-07-29` from a `260729_2343_` file name prefix.
    pub date: Option<String>,
    pub section_slug: String,
    pub section: String,
    pub category_slug: String,
    pub category: String,
}

impl Doc {
    /// "Bun › 설명서", or just the section for a section's own docs.
    pub fn place(&self) -> String {
        if self.category_slug == self.section_slug { self.section.clone() } else { format!("{} › {}", self.section, self.category) }
    }
}

#[derive(Clone, Debug)]
pub struct Category {
    pub slug: String,
    pub label: String,
    /// First paragraph of `_index.md`, shown on cards.
    pub summary: String,
    /// `_index.md` relative to the docs folder, when there is one.
    pub index_file: Option<String>,
    /// Docs placed directly in the section folder rather than in a category folder.
    pub direct: bool,
    pub docs: Vec<Doc>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub slug: String,
    pub label: String,
    pub categories: Vec<Category>,
}

#[derive(Clone, Debug, Default)]
pub struct Library {
    pub sections: Vec<Section>,
    /// `docs/_index.md`, the home page intro.
    pub home_index: Option<String>,
}

pub enum Lookup<'a> {
    Doc(&'a Doc),
    Category(&'a Section, &'a Category),
    Section(&'a Section),
    /// Found under another address (old or prefixed path); send the browser there.
    Redirect(String),
    Missing,
}

/// Same characters as JavaScript's `encodeURIComponent` leaves alone.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

pub fn encode_component(text: &str) -> String {
    utf8_percent_encode(text, COMPONENT).to_string()
}

fn encode_path(path: &str) -> String {
    path.split('/').map(encode_component).collect::<Vec<_>>().join("/")
}

pub fn doc_href(slug: &str) -> String {
    format!("/docs/{}", encode_path(slug))
}

/// Raw files (images and the like) inside docs/, served as they are.
pub fn file_href(rel: &str) -> String {
    format!("/files/{}", encode_path(rel))
}

/// macOS may store Korean file names decomposed (NFD); URLs use the composed (NFC) form.
pub fn nfc(text: &str) -> String {
    text.nfc().collect()
}

static ORDER_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+_").unwrap());
static DATE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{2})(\d{2})(\d{2})_\d{4}_").unwrap());

/// `01_시스템_설계` → `시스템_설계` (URL segment).
fn slug_segment(name: &str) -> String {
    let name = nfc(name);
    let stripped = ORDER_PREFIX.replace(&name, "");
    if stripped.is_empty() { name } else { stripped.into_owned() }
}

fn label_of(name: &str) -> String {
    slug_segment(name).replace('_', " ")
}

fn date_of(stem: &str) -> Option<String> {
    DATE_PREFIX.captures(stem).map(|c| format!("20{}-{}-{}", &c[1], &c[2], &c[3]))
}

fn is_markdown(name: &str) -> bool {
    name.to_lowercase().ends_with(".md") && name != INDEX_FILE
}

fn skip_dir(name: &str) -> bool {
    name.starts_with('.') || name.starts_with('_') || name == "images"
}

/// Entries of `dir` as (name, is_dir), in natural order.
fn entries(dir: &Path) -> Vec<(String, bool)> {
    let Ok(read) = fs::read_dir(dir) else { return Vec::new() };
    let mut list: Vec<(String, bool)> = read
        .flatten()
        .filter_map(|e| Some((e.file_name().to_str()?.to_owned(), e.file_type().ok()?.is_dir())))
        .filter(|(name, _)| !name.starts_with('.'))
        .collect();
    list.sort_by(|a, b| natural_cmp(&nfc(&a.0), &nfc(&b.0)));
    list
}

struct Walk<'a> {
    root: &'a Path,
}

impl Walk<'_> {
    fn read_title(&self, rel: &str, stem: &str) -> String {
        let text = fs::read_to_string(self.root.join(rel)).unwrap_or_default();
        first_heading(&text).unwrap_or_else(|| DATE_PREFIX.replace(stem, "").replace('_', " "))
    }

    /// Markdown files in `rel_dir` (and, for a category, its subfolders) as (file, slug, stem).
    fn docs_in(&self, rel_dir: &str, slug_dir: &str, recurse: bool, out: &mut Vec<(String, String, String)>) {
        for (name, is_dir) in entries(&self.root.join(rel_dir)) {
            let rel = if rel_dir.is_empty() { name.clone() } else { format!("{rel_dir}/{name}") };
            if is_dir {
                if recurse && !skip_dir(&name) {
                    self.docs_in(&rel, &format!("{slug_dir}/{}", slug_segment(&name)), true, out);
                }
            } else if is_markdown(&name) {
                let stem = nfc(&name[..name.len() - 3]);
                let slug = if slug_dir.is_empty() { stem.clone() } else { format!("{slug_dir}/{stem}") };
                out.push((rel, slug, stem));
            }
        }
    }

    fn make_docs(&self, found: Vec<(String, String, String)>, section: &Section, category_slug: &str, category: &str) -> Vec<Doc> {
        found
            .into_iter()
            .map(|(file, slug, stem)| Doc {
                title: self.read_title(&file, &stem),
                date: date_of(&stem),
                slug,
                file,
                section_slug: section.slug.clone(),
                section: section.label.clone(),
                category_slug: category_slug.to_owned(),
                category: category.to_owned(),
            })
            .collect()
    }

    /// `direct`: the section folder's own Markdown files, not a category folder.
    fn category(&self, section: &Section, rel_dir: &str, slug: String, default_label: String, direct: bool) -> Category {
        let index_rel = if rel_dir.is_empty() { INDEX_FILE.to_owned() } else { format!("{rel_dir}/{INDEX_FILE}") };
        let index_text = if direct { None } else { fs::read_to_string(self.root.join(&index_rel)).ok() };
        let label = index_text.as_deref().and_then(first_heading).unwrap_or(default_label);
        let summary = index_text.as_deref().map(first_paragraph).unwrap_or_default();
        let mut found = Vec::new();
        self.docs_in(rel_dir, &slug, !direct, &mut found);
        let docs = self.make_docs(found, section, &slug, &label);
        Category { slug, label, summary, index_file: index_text.map(|_| index_rel), direct, docs }
    }
}

pub fn load(docs_dir: &Path) -> Library {
    let walk = Walk { root: docs_dir };
    let mut sections = Vec::new();

    // Loose files at the top level form a section of their own.
    let top = Section { slug: String::new(), label: "문서".to_owned(), categories: Vec::new() };
    let loose = walk.category(&top, "", String::new(), DIRECT_LABEL.to_owned(), true);
    if !loose.docs.is_empty() {
        sections.push(Section { categories: vec![loose], ..top });
    }

    for (name, is_dir) in entries(docs_dir) {
        if !is_dir || skip_dir(&name) {
            continue;
        }
        let mut section = Section { slug: slug_segment(&name), label: label_of(&name), categories: Vec::new() };
        // A section's own _index.md can rename it and describes it.
        let section_index = fs::read_to_string(docs_dir.join(&name).join(INDEX_FILE)).ok();
        if let Some(label) = section_index.as_deref().and_then(first_heading) {
            section.label = label;
        }

        let mut categories = Vec::new();
        for (child, child_is_dir) in entries(&docs_dir.join(&name)) {
            if child_is_dir && !skip_dir(&child) {
                let slug = format!("{}/{}", section.slug, slug_segment(&child));
                categories.push(walk.category(&section, &format!("{name}/{child}"), slug, label_of(&child), false));
            }
        }
        let mut direct = walk.category(&section, &name, section.slug.clone(), DIRECT_LABEL.to_owned(), true);
        if !direct.docs.is_empty() {
            // A section with only its own docs shows them under the section's name.
            if categories.is_empty() {
                direct.label = section.label.clone();
                direct.summary = section_index.as_deref().map(first_paragraph).unwrap_or_default();
                direct.index_file = section_index.as_ref().map(|_| format!("{name}/{INDEX_FILE}"));
            }
            for doc in &mut direct.docs {
                doc.category = direct.label.clone();
            }
            categories.insert(0, direct);
        }
        if !categories.is_empty() {
            section.categories = categories;
            sections.push(section);
        }
    }

    let home_index = docs_dir.join(INDEX_FILE).is_file().then(|| INDEX_FILE.to_owned());
    Library { sections, home_index }
}

impl Library {
    pub fn docs(&self) -> impl Iterator<Item = &Doc> {
        self.sections.iter().flat_map(|s| &s.categories).flat_map(|c| &c.docs)
    }

    pub fn is_empty(&self) -> bool {
        self.docs().next().is_none()
    }

    fn exact(&self, slug: &str) -> Lookup<'_> {
        if let Some(doc) = self.docs().find(|d| d.slug == slug) {
            return Lookup::Doc(doc);
        }
        for section in self.sections.iter().filter(|s| !s.slug.is_empty()) {
            if section.slug == slug {
                return Lookup::Section(section);
            }
            if let Some(category) = section.categories.iter().find(|c| c.slug == slug) {
                return Lookup::Category(section, category);
            }
        }
        Lookup::Missing
    }

    /// Finds what `/docs/{path}` names. Paths written with folder prefixes (`01_Bun/…`, as in
    /// relative Markdown links) or pointing at a doc's old location redirect to its address.
    pub fn lookup(&self, path: &str) -> Lookup<'_> {
        let path = nfc(path.trim_matches('/'));
        let path = path.strip_suffix(".md").unwrap_or(&path).to_owned();
        if let found @ (Lookup::Doc(_) | Lookup::Category(..) | Lookup::Section(_)) = self.exact(&path) {
            return found;
        }
        let stripped = path.split('/').map(slug_segment).collect::<Vec<_>>().join("/");
        let slug = match self.exact(&stripped) {
            Lookup::Doc(d) => Some(d.slug.clone()),
            Lookup::Category(_, c) => Some(c.slug.clone()),
            Lookup::Section(s) => Some(s.slug.clone()),
            _ => None,
        };
        if let Some(slug) = slug {
            return Lookup::Redirect(slug);
        }
        // The same file name elsewhere: a doc that moved to another category.
        let last = loose_key(path.rsplit('/').next().unwrap_or(&path));
        let mut matches = self.docs().filter(|d| loose_key(d.slug.rsplit('/').next().unwrap_or("")) == last);
        match (matches.next(), matches.next()) {
            (Some(doc), None) if !last.is_empty() => Lookup::Redirect(doc.slug.clone()),
            _ => Lookup::Missing,
        }
    }

    /// Newest dated docs first.
    pub fn recent(&self, n: usize) -> Vec<&Doc> {
        let mut dated: Vec<&Doc> = self.docs().filter(|d| d.date.is_some()).collect();
        dated.sort_by(|a, b| b.slug.rsplit('/').next().cmp(&a.slug.rsplit('/').next()));
        dated.truncate(n);
        dated
    }
}

fn loose_key(name: &str) -> String {
    nfc(name).to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Compares digit runs by value, like `localeCompare(..., { numeric: true })`: 2_ before 10_.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut n = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        n.push(c);
                        it.next();
                    }
                    n
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}
