//! Pages, routes, and the shared page frame.

use std::{sync::Arc, time::Duration};

use topcoat::{
    Result,
    asset::{Asset, asset},
    context::{Cx, app_context},
    router::{
        Body, HeaderMap, HeaderName, HeaderValue, Next, StatusCode,
        content::{
            Css, Js, Json,
            websocket::{Message, WebSocketUpgrade},
        },
        error::{not_found, redirect},
        header, layer, page, path_param, query_params,
        request::{headers, uri},
        response::Response,
        route,
    },
    runtime::{Event, shard, signal},
    view::{Child, Unescaped, View, ViewExt, class, component, view},
};

use crate::{
    assets,
    docs::{self, Category, Doc, Library, Lookup, Section, doc_href},
    markdown::{self, Heading},
    site::Site,
};

// Downloaded once by the asset bundler and served from this site, so no CDN is contacted at runtime.
const MERMAID_JS: Asset = asset!("https://cdn.jsdelivr.net/npm/mermaid@12.2.0/dist/mermaid.min.js");

fn site(cx: &Cx) -> &Site {
    app_context::<std::sync::Arc<Site>>(cx)
}

// ---------------------------------------------------------------------------------------------
// Page frame

/// Sections with their categories; only the category being read is unfolded.
#[component]
async fn sidebar_nav(lib: Arc<Library>, current: Option<String>, current_category: Option<String>) -> Result<impl View> {
    let sections = lib.sections.clone();
    Ok(view! {
        #[key(section.slug.clone())]
        for section in sections {
            <div class="nav-section">
                if !section.slug.is_empty() {
                    <a class="nav-label" href=(doc_href(&section.slug))>(section.label)</a>
                }
                #[key(category.slug.clone())]
                for category in section.categories {
                    let here = current_category.as_deref() == Some(category.slug.as_str());
                    let count = category.docs.len();
                    <details class="nav-category" open=(here)>
                        <summary>
                            <a href=(doc_href(&category.slug)) class=(class!("nav-category-link", "current" if here && current.is_none()))>
                                // A section's own docs sit right under its name.
                                if category.direct { "글 목록" } else { (category.label) }
                            </a>
                            <span class="nav-count">(count)</span>
                        </summary>
                        <ul>
                            for doc in category.docs {
                                <li>
                                    <a
                                        href=(doc_href(&doc.slug))
                                        class=((current.as_deref() == Some(doc.slug.as_str())).then_some("active"))
                                    >
                                        (doc.title)
                                    </a>
                                </li>
                            }
                        </ul>
                    </details>
                }
            </div>
        }
    })
}

#[component]
async fn toc(headings: Vec<Heading>) -> Result<impl View> {
    Ok(view! {
        if !headings.is_empty() {
            <nav class="toc">
                <div class="toc-title">"이 페이지의 내용"</div>
                <ul>
                    for h in headings {
                        <li class=(format!("toc-h{}", h.level))>
                            <a href=(format!("#{}", h.id))>(h.text)</a>
                        </li>
                    }
                </ul>
            </nav>
        }
    })
}

/// Live results under the search box. Re-rendered on the server as the user types; the browser
/// runtime swaps in the new HTML and drops any response a newer keystroke has overtaken.
#[shard]
pub async fn search_panel(cx: &Cx, query: String) -> Result<impl View> {
    let query: String = query.chars().take(200).collect();
    let result = (!query.trim().is_empty()).then(|| site(cx).search.search(&query, 8));
    Ok(view! {
        <div class="search-panel" hidden=(result.is_none())>
            match result {
                Some(r) if !r.hits.is_empty() => {
                    for h in r.hits {
                        <a class="search-hit" href=(h.url)>
                            <span class="search-place">(Unescaped::new_unchecked(h.place))</span>
                            <span class="search-doc">(Unescaped::new_unchecked(h.title))</span>
                            if !h.heading.is_empty() {
                                <span class="search-heading">(Unescaped::new_unchecked(h.heading))</span>
                            }
                            <span class="search-snippet">(Unescaped::new_unchecked(h.snippet))</span>
                        </a>
                    }
                    <a class="search-all" href=(format!("/search?q={}", docs::encode_component(query.trim())))>
                        (r.total) "곳 전체 보기 · " (format!("{:.2}", r.took_ms)) "ms"
                    </a>
                },
                Some(_) => <div class="search-empty">"“" (query.trim()) "”에 맞는 내용이 없습니다"</div>,
                None => "",
            }
        </div>
    })
}

/// The `<html>` document around every page: top bar with search, sidebar, article, and table of contents.
#[component]
async fn frame(
    cx: &Cx,
    title: String,
    lib: Arc<Library>,
    #[default] current: Option<String>,
    #[default] current_category: Option<String>,
    #[default] headings: Vec<Heading>,
    #[default] query: String,
    #[default] mermaid: bool,
    child: Child<'_>,
) -> Result<impl View> {
    let site = site(cx);
    let page_title = if title == site.title { title } else { format!("{title} · {}", site.title) };
    let menu_open = signal(cx, || false);
    let live_query = signal(cx, String::new);

    Ok(view! {
        <!DOCTYPE html>
        <html lang="ko">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>(page_title)</title>
                <link rel="stylesheet" href="/static/style.css">
                <link rel="stylesheet" href="/static/highlight.css">
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body :class=$(if menu_open.get() { "menu-open" } else { "" })>
                <header class="topbar">
                    <button class="menu-toggle" type="button" aria-label="목차 열기" @click=$(|_e| menu_open.toggle())>"☰"</button>
                    <a class="brand" href="/">(site.title.as_str())</a>
                    <form class="search" action="/search" method="get" role="search">
                        <input
                            type="search"
                            name="q"
                            value=(query)
                            placeholder="문서 검색 ( / )"
                            aria-label="문서 검색"
                            autocomplete="off"
                            maxlength="200"
                            @input=$(|e: Event| live_query.set(e.target.value))
                        >
                        search_panel(query: $(live_query.get()))
                    </form>
                </header>
                <div class="layout">
                    <aside class="sidebar">sidebar_nav(lib: lib, current: current, current_category: current_category)</aside>
                    <main class="content"><article class="markdown">(child)</article></main>
                    toc(headings: headings)
                </div>
                <script src="/static/app.js" defer=""></script>
                if mermaid {
                    <script src=(MERMAID_JS) defer=""></script>
                    <script src="/static/mermaid-init.js" defer=""></script>
                }
                if site.live_reload {
                    <script src="/static/livereload.js" defer=""></script>
                }
            </body>
        </html>
    })
}

/// "Bun › 설명서" above a page, linking to the section and category pages.
#[component]
async fn breadcrumb(section: Option<(String, String)>, category: Option<(String, String)>) -> Result<impl View> {
    Ok(view! {
        <nav class="breadcrumb" aria-label="위치">
            <a href="/">"홈"</a>
            if let Some((slug, label)) = section {
                " › " <a href=(doc_href(&slug))>(label)</a>
            }
            if let Some((slug, label)) = category {
                " › " <a href=(doc_href(&slug))>(label)</a>
            }
        </nav>
    })
}

/// One card per category: name, number of docs, and the first line of its `_index.md`.
#[component]
async fn category_cards(categories: Vec<Category>) -> Result<impl View> {
    Ok(view! {
        <div class="cards">
            #[key(c.slug.clone())]
            for c in categories {
                let count = c.docs.len();
                <a class="card" href=(doc_href(&c.slug))>
                    <span class="card-title">(c.label) <span class="card-count">(count) "개"</span></span>
                    if !c.summary.is_empty() {
                        <span class="card-summary">(c.summary)</span>
                    }
                </a>
            }
        </div>
    })
}

#[component]
async fn doc_index(docs: Vec<Doc>, #[default] show_place: bool) -> Result<impl View> {
    Ok(view! {
        <ul class="doc-index">
            for d in docs {
                <li>
                    <a href=(doc_href(&d.slug))>(d.title.as_str())</a>
                    <span class="doc-meta">
                        if show_place {
                            (d.place())
                        }
                        if let Some(date) = d.date {
                            <span class="doc-date">(date)</span>
                        }
                    </span>
                </li>
            }
        </ul>
    })
}

/// An `_index.md` rendered for a page intro, without its `#` title (the page shows its own).
fn intro_html(site: &Site, rel: Option<&str>) -> Option<String> {
    let rel = rel?;
    let text = std::fs::read_to_string(site.docs_dir.join(rel)).ok()?;
    let html = markdown::render(&text, Some(&site.highlighter), None).html;
    let html = match (html.find("<h1"), html.find("</h1>")) {
        (Some(start), Some(end)) if start < end => format!("{}{}", &html[..start], &html[end + 5..]),
        _ => html,
    };
    (!html.trim().is_empty()).then_some(html)
}

// ---------------------------------------------------------------------------------------------
// Pages

#[page("/")]
pub async fn home(cx: &Cx) -> Result<impl View> {
    let site = site(cx);
    let lib = site.library();
    let title = site.title.clone();
    // The first paragraph greets readers; any `##` parts (like the writing guide) go at the end.
    let (intro, guide) = match intro_html(site, lib.home_index.as_deref()) {
        Some(html) => match html.find("<h2") {
            Some(i) => (Some(html[..i].to_owned()), Some(html[i..].to_owned())),
            None => (Some(html), None),
        },
        None => (None, None),
    };
    let sections = lib.sections.clone();
    let recent: Vec<Doc> = lib.recent(8).into_iter().cloned().collect();
    let empty = lib.is_empty();
    Ok(view! {
        frame(title: title.clone(), lib: lib,
            <h1>(title)</h1>
            if let Some(intro) = intro {
                <div class="intro">(Unescaped::new_unchecked(intro))</div>
            }
            if empty {
                <p>"아직 문서가 없습니다. " <code>"docs/섹션/카테고리/"</code> " 폴더에 " <code>".md"</code> " 파일을 넣으면 여기에 나타납니다."</p>
            }
            if !recent.is_empty() {
                <h2>"최근 추가된 문서"</h2>
                doc_index(docs: recent, show_place: true)
            }
            #[key(section.slug.clone())]
            for section in sections {
                <h2>
                    if section.slug.is_empty() {
                        (section.label)
                    } else {
                        <a href=(doc_href(&section.slug))>(section.label)</a>
                    }
                </h2>
                category_cards(categories: section.categories)
            }
            if let Some(guide) = guide {
                <div class="home-guide">(Unescaped::new_unchecked(guide))</div>
            }
        )
    })
}

mod doc_route {
    use super::*;

    path_param!(pub *doc_path);

    #[query_params]
    struct DocQuery {
        hl: Option<String>,
    }

    /// Docs, category pages (`/docs/Bun/설명서`), and section pages (`/docs/Bun`).
    #[page("/docs/{*doc_path}")]
    pub async fn doc_page(cx: &Cx) -> Result<impl View> {
        let site = site(cx);
        let lib = site.library();
        let path = path_param::<DocPath>(cx).map(|s| s.to_string()).collect::<Vec<String>>().join("/");

        let view = match lib.lookup(&path) {
            Lookup::Doc(doc) => {
                let doc = doc.clone();
                view! { doc_view(lib: lib.clone(), doc: doc) }.boxed()
            }
            Lookup::Category(section, category) => {
                let (section, category) = (section.clone(), category.clone());
                view! { category_view(lib: lib.clone(), section: section, category: category) }.boxed()
            }
            Lookup::Section(section) => {
                let section = section.clone();
                view! { section_view(lib: lib.clone(), section: section) }.boxed()
            }
            Lookup::Redirect(slug) => {
                let query = uri(cx).query().map(|q| format!("?{q}")).unwrap_or_default();
                return Err(redirect(format!("{}{query}", doc_href(&slug))).into());
            }
            Lookup::Missing => view! { not_found_frame(lib: lib.clone()) }.boxed(),
        };
        Ok(view)
    }

    #[component]
    async fn doc_view(cx: &Cx, lib: Arc<Library>, doc: Doc) -> Result<impl View> {
        let site = site(cx);
        let rendered = site.render_doc(&doc);
        let highlight = query_params::<DocQuery>(cx).ok().and_then(|q| q.hl.clone());
        let html = match highlight {
            Some(q) => markdown::mark_terms(&rendered.html, &q),
            None => rendered.html.clone(),
        };
        let mermaid = html.contains("<code class=\"language-mermaid\">");

        let siblings: Vec<Doc> = lib
            .sections
            .iter()
            .flat_map(|s| &s.categories)
            .find(|c| c.slug == doc.category_slug)
            .map(|c| c.docs.clone())
            .unwrap_or_default();
        let index = siblings.iter().position(|d| d.slug == doc.slug).unwrap_or(0);
        let prev = index.checked_sub(1).and_then(|i| siblings.get(i)).map(|d| (doc_href(&d.slug), d.title.clone()));
        let next = siblings.get(index + 1).map(|d| (doc_href(&d.slug), d.title.clone()));
        let crumbs_section = (!doc.section_slug.is_empty()).then(|| (doc.section_slug.clone(), doc.section.clone()));
        let crumbs_category = (doc.category_slug != doc.section_slug).then(|| (doc.category_slug.clone(), doc.category.clone()));

        Ok(view! {
            frame(
                title: doc.title.clone(),
                lib: lib,
                current: Some(doc.slug.clone()),
                current_category: Some(doc.category_slug.clone()),
                headings: rendered.headings.clone(),
                mermaid: mermaid,
                <div class="doc-head">
                    breadcrumb(section: crumbs_section, category: crumbs_category)
                    if let Some(date) = doc.date.clone() {
                        <span class="doc-date">(date)</span>
                    }
                </div>
                (Unescaped::new_unchecked(html))
                <nav class="pager">
                    match prev {
                        Some((href, title)) => <a class="prev" href=(href)>"← " (title)</a>,
                        None => <span></span>,
                    }
                    if let Some((href, title)) = next {
                        <a class="next" href=(href)>(title) " →"</a>
                    }
                </nav>
            )
        })
    }

    #[component]
    async fn category_view(cx: &Cx, lib: Arc<Library>, section: Section, category: Category) -> Result<impl View> {
        let site = site(cx);
        let intro = intro_html(site, category.index_file.as_deref());
        let count = category.docs.len();
        Ok(view! {
            frame(
                title: category.label.clone(),
                lib: lib,
                current_category: Some(category.slug.clone()),
                breadcrumb(section: Some((section.slug.clone(), section.label.clone())), category: None)
                <h1>(category.label.clone()) <span class="title-count">(count) "개"</span></h1>
                if let Some(intro) = intro {
                    <div class="intro">(Unescaped::new_unchecked(intro))</div>
                }
                doc_index(docs: category.docs)
            )
        })
    }

    #[component]
    async fn section_view(cx: &Cx, lib: Arc<Library>, section: Section) -> Result<impl View> {
        let site = site(cx);
        // Docs placed directly in the section folder are listed; category folders get cards.
        let (direct, folders): (Vec<Category>, Vec<Category>) = section.categories.iter().cloned().partition(|c| c.direct);
        let direct_docs: Vec<Doc> = direct.iter().flat_map(|c| c.docs.clone()).collect();
        let intro = intro_html(site, direct.first().and_then(|c| c.index_file.as_deref()));
        let current = direct.first().map(|c| c.slug.clone());
        Ok(view! {
            frame(
                title: section.label.clone(),
                lib: lib,
                current_category: current,
                breadcrumb(section: None, category: None)
                <h1>(section.label.clone())</h1>
                if let Some(intro) = intro {
                    <div class="intro">(Unescaped::new_unchecked(intro))</div>
                }
                if !folders.is_empty() {
                    category_cards(categories: folders)
                }
                if !direct_docs.is_empty() {
                    doc_index(docs: direct_docs)
                }
            )
        })
    }
}

#[query_params]
struct SearchQuery {
    q: Option<String>,
    limit: Option<String>,
}

#[page("/search")]
pub async fn search_page(cx: &Cx) -> Result<impl View> {
    let site = site(cx);
    let lib = site.library();
    let raw = query_params::<SearchQuery>(cx).ok().and_then(|q| q.q.clone()).unwrap_or_default();
    let result = site.search.search(&raw, 50);
    let query = result.query.clone();
    let title = if query.is_empty() { "검색".to_owned() } else { format!("검색: {query}") };
    let shown = result.hits.len();

    Ok(view! {
        frame(title: title, lib: lib, query: query.clone(),
            if query.is_empty() {
                <h1>"문서 검색"</h1>
                <p>"위의 검색창에 찾을 말을 입력하세요. 여러 단어를 띄어 쓰면 모두 들어 있는 곳을 찾습니다."</p>
            } else {
                <h1>"“" (query.as_str()) "” 검색 결과"</h1>
                <p class="search-summary">
                    (result.total) "곳 찾음"
                    if result.total > shown {
                        " (상위 " (shown) "곳 표시)"
                    }
                    " · " (format!("{:.2}", result.took_ms)) "ms"
                </p>
                if shown == 0 {
                    <p>"찾는 내용이 없습니다. 다른 단어나 더 짧은 단어로 찾아보세요."</p>
                } else {
                    <ol class="search-list">
                        for h in result.hits {
                            <li>
                                <span class="search-place">(Unescaped::new_unchecked(h.place))</span>
                                <a href=(h.url)>
                                    <span class="search-doc">(Unescaped::new_unchecked(h.title))</span>
                                    if !h.heading.is_empty() {
                                        " › " (Unescaped::new_unchecked(h.heading))
                                    }
                                </a>
                                <p>(Unescaped::new_unchecked(h.snippet))</p>
                            </li>
                        }
                    </ol>
                }
            }
        )
    })
}

#[component]
async fn not_found_body() -> Result<impl View> {
    Ok(view! {
        (StatusCode::NOT_FOUND)
        <h1>"페이지를 찾을 수 없습니다"</h1>
        <p><a href="/">"처음으로 돌아가기"</a></p>
    })
}

#[component]
async fn not_found_frame(lib: Arc<Library>) -> Result<impl View> {
    Ok(view! { frame(title: "404".to_owned(), lib: lib, not_found_body()) })
}

/// Everything no other page or route claims.
#[page("/{*missing}")]
pub async fn not_found_page(cx: &Cx) -> Result<impl View> {
    let lib = site(cx).library();
    Ok(view! { not_found_frame(lib: lib) })
}

// ---------------------------------------------------------------------------------------------
// Routes

#[route(GET "/api/search")]
pub async fn search_api(cx: &Cx) -> Result<(HeaderMap, Json<crate::search::SearchResult>)> {
    let params = query_params::<SearchQuery>(cx).ok();
    let q = params.and_then(|p| p.q.clone()).unwrap_or_default();
    let limit = params.and_then(|p| p.limit.as_deref()?.parse().ok()).filter(|&n: &usize| n > 0).unwrap_or(10);
    let result = site(cx).search.search(&q, limit);
    let mut extra = HeaderMap::new();
    extra.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    extra.insert(HeaderName::from_static("server-timing"), HeaderValue::from_str(&format!("search;dur={:.2}", result.took_ms))?);
    Ok((extra, Json(result)))
}

/// Site stylesheet embedded in `src/assets/style.rs`.
#[route(GET "/static/style.css")]
pub async fn style_css() -> Result<Css<&'static str>> {
    Ok(Css(assets::STYLE_CSS))
}

/// Token colors for server-highlighted code, light and dark.
#[route(GET "/static/highlight.css")]
pub async fn highlight_css(cx: &Cx) -> Result<Css<String>> {
    Ok(Css(site(cx).highlighter.css().to_owned()))
}

/// Page helpers embedded in `src/assets/app_js.rs`.
#[route(GET "/static/app.js")]
pub async fn app_js() -> Result<Js<&'static str>> {
    Ok(Js(assets::APP_JS))
}

/// Mermaid bootstrap embedded in `src/assets/mermaid_init_js.rs`.
#[route(GET "/static/mermaid-init.js")]
pub async fn mermaid_init_js() -> Result<Js<&'static str>> {
    Ok(Js(assets::MERMAID_INIT_JS))
}

/// Live-reload client embedded in `src/assets/livereload_js.rs`.
#[route(GET "/static/livereload.js")]
pub async fn livereload_js() -> Result<Js<&'static str>> {
    Ok(Js(assets::LIVERELOAD_JS))
}

/// Pages subscribe here and reload when docs/ changes. A closed socket followed by a successful
/// reconnect (server restarted) also reloads them; see `/static/livereload.js`.
#[route(GET "/__livereload")]
pub async fn livereload(cx: &Cx, upgrade: WebSocketUpgrade) -> Result<Response> {
    let site = site(cx);
    if !site.live_reload {
        return Err(not_found().into());
    }
    let mut changes = site.reload.subscribe();
    upgrade.on_upgrade(move |mut socket| async move {
        loop {
            tokio::select! {
                change = changes.recv() => match change {
                    Ok(()) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if socket.send(Message::text("reload")).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                incoming = socket.recv() => if !matches!(incoming, Some(Ok(_))) { break },
                () = tokio::time::sleep(Duration::from_secs(30)) => {
                    if socket.send(Message::Ping(Default::default())).await.is_err() {
                        break;
                    }
                }
            }
        }
    })
}

// ---------------------------------------------------------------------------------------------
// Security headers on every response

/// Topcoat's runtime compiles `$(...)` handlers with `new Function`, so scripts need
/// 'unsafe-eval'; inline scripts are still refused. Highlighted code uses classes, not inline styles.
#[layer("/")]
pub async fn security_headers(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let host = headers(cx)
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned)
        .or_else(|| uri(cx).authority().map(|a| a.to_string()))
        .unwrap_or_default();
    let mut response = next.run(cx, body).await?;
    let h = response.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    let csp = [
        "default-src 'self'".to_owned(),
        "script-src 'self' 'unsafe-eval'".to_owned(),
        "style-src 'self' 'unsafe-inline'".to_owned(),
        "img-src 'self' data: https:".to_owned(),
        format!("connect-src 'self' ws://{host} wss://{host}"),
        "object-src 'none'".to_owned(),
        "base-uri 'none'".to_owned(),
        "form-action 'self'".to_owned(),
    ]
    .join("; ");
    if let Ok(value) = HeaderValue::from_str(&csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    if !h.contains_key(header::CACHE_CONTROL)
        && h.get(header::CONTENT_TYPE).is_some_and(|t| t.as_bytes().starts_with(b"text/html"))
    {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    Ok(response)
}

pub use doc_route::doc_page;
