//! Values shared by every request (Topcoat app context) and the docs/ watcher.

use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime},
};

use notify::{RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc};

use crate::{
    docs,
    highlight::Highlighter,
    markdown,
    search::{SearchDoc, SearchIndex},
};

pub struct Site {
    pub title: String,
    pub docs_dir: PathBuf,
    /// Our own reload socket, used when the server runs on its own (`cargo run`, the background
    /// service). Under `topcoat dev`, Topcoat's dev script refreshes pages instead.
    pub live_reload: bool,
    pub highlighter: Arc<Highlighter>,
    pub search: Arc<SearchIndex>,
    pub reload: broadcast::Sender<()>,
    /// Rendered pages keyed by file, reused until the file's modification time changes.
    rendered: Mutex<HashMap<PathBuf, (SystemTime, Arc<markdown::Rendered>)>>,
    /// The section/category tree, read once and dropped when docs/ changes.
    library: RwLock<Option<Arc<docs::Library>>>,
}

impl Site {
    pub fn from_env() -> Self {
        let root = env::var_os("REPORT_ROOT").map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from);
        let docs_dir = root.join("docs");
        let _ = std::fs::create_dir_all(&docs_dir);
        let under_topcoat_dev = env::var_os("TOPCOAT_DEV_URL").is_some();
        let highlighter = Arc::new(Highlighter::new());

        let search = {
            let docs_dir = docs_dir.clone();
            Arc::new(SearchIndex::new(move || {
                let library = docs::load(&docs_dir);
                library
                    .docs()
                    .map(|d| SearchDoc {
                        html: markdown::render(&fs::read_to_string(docs_dir.join(&d.file)).unwrap_or_default(), None, None).html,
                        slug: d.slug.clone(),
                        title: d.title.clone(),
                        place: d.place(),
                    })
                    .collect()
            }))
        };

        Self {
            title: env::var("SITE_TITLE").unwrap_or_else(|_| "typeboard".to_owned()),
            docs_dir,
            live_reload: env::var("LIVE_RELOAD").as_deref() != Ok("0") && !under_topcoat_dev,
            highlighter,
            search,
            reload: broadcast::channel(16).0,
            rendered: Mutex::new(HashMap::new()),
            library: RwLock::new(None),
        }
    }

    pub fn library(&self) -> Arc<docs::Library> {
        if let Some(library) = self.library.read().unwrap().as_ref() {
            return library.clone();
        }
        let library = Arc::new(docs::load(&self.docs_dir));
        *self.library.write().unwrap() = Some(library.clone());
        library
    }

    /// Called when anything under docs/ changes.
    fn docs_changed(&self) {
        *self.library.write().unwrap() = None;
        self.search.invalidate();
    }

    /// The page HTML for `doc`, highlighted. Code coloring is the slow part of a request, so the
    /// result is cached and only redone after the file is saved again.
    pub fn render_doc(&self, doc: &docs::Doc) -> Arc<markdown::Rendered> {
        let path = self.docs_dir.join(&doc.file);
        let modified = fs::metadata(&path).and_then(|m| m.modified()).ok();
        if let Some(modified) = modified
            && let Some((at, rendered)) = self.rendered.lock().unwrap().get(&path)
            && *at == modified
        {
            return rendered.clone();
        }
        let text = fs::read_to_string(&path).unwrap_or_default();
        let dir = doc.file.rsplit_once('/').map_or("", |(dir, _)| dir);
        let base = if dir.is_empty() { docs::file_href("").to_owned() } else { format!("{}/", docs::file_href(dir)) };
        let rendered = Arc::new(markdown::render(&text, Some(&self.highlighter), Some(&base)));
        if let Some(modified) = modified {
            self.rendered.lock().unwrap().insert(path, (modified, rendered.clone()));
        }
        rendered
    }
}

/// Watches docs/: the doc list and search index are rebuilt on next use, and open pages reload.
/// Editors often write a file in several steps; one reload per burst is enough.
pub fn watch_docs(site: Arc<Site>) -> notify::Result<notify::RecommendedWatcher> {
    let reload = site.live_reload.then(|| site.reload.clone());
    let (tx, mut rx) = mpsc::unbounded_channel::<Option<PathBuf>>();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event
            && !event.kind.is_access()
        {
            let _ = tx.send(event.paths.into_iter().next());
        }
    })?;
    watcher.watch(&site.docs_dir, RecursiveMode::Recursive)?;

    tokio::spawn(async move {
        while let Some(mut file) = rx.recv().await {
            // Collect the rest of the burst.
            while let Ok(Some(next)) = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await {
                file = next.or(file);
            }
            site.docs_changed();
            if let Some(reload) = &reload {
                let name = file.as_deref().and_then(Path::file_name).map_or("?".into(), |n| n.to_string_lossy());
                println!("[livereload] 변경 감지: {name} → 브라우저 새로고침");
                let _ = reload.send(());
            }
        }
    });
    Ok(watcher)
}
