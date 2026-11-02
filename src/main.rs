//! typeboard docs server: renders the Markdown library in ./docs (section → category → doc).
//! Built on Topcoat (router, views, asset bundle, browser runtime, WebSocket).

mod app;
mod assets;
mod docs;
mod highlight;
mod markdown;
mod search;
mod site;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::{Router, RouterBuilderDirectoryExt},
    runtime::RouterBuilderRuntimeExt,
};

#[tokio::main]
async fn main() {
    // The bundle is made by the Topcoat CLI after each build, next to the binary.
    let assets = match AssetBundle::load() {
        Ok(assets) => assets,
        Err(error) => {
            eprintln!("에셋 번들이 없습니다 ({error}).");
            eprintln!("`bun run start`(번들 후 실행)나 `bun run dev`(topcoat dev)로 실행하세요.");
            std::process::exit(1);
        }
    };

    let site = std::sync::Arc::new(site::Site::from_env());

    let stats = site.search.build();
    println!("[search] 색인 완료: 문서 {}개, 구역 {}개 ({:.1}ms)", stats.docs, stats.sections, stats.ms);
    let _watcher = site::watch_docs(site.clone())
        .map_err(|e| eprintln!("[watch] docs/ 감시 실패: {e}"))
        .ok();

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_owned());
    println!(
        "{}: http://localhost:{port}  (문서 폴더: {}, 자동 새로고침 {})",
        site.title,
        site.docs_dir.display(),
        if site.live_reload { "켜짐" } else if std::env::var_os("TOPCOAT_DEV_URL").is_some() { "topcoat dev" } else { "꺼짐" },
    );

    // Code coloring is the slow part of a page; do it for every doc now so first visits are fast.
    let warm = site.clone();
    tokio::task::spawn_blocking(move || {
        for doc in warm.library().docs() {
            warm.render_doc(doc);
        }
    });

    let files = site.docs_dir.clone();
    let router: Router = Router::builder()
        .layer(app::security_headers)
        .page(app::home)
        .page(app::doc_page)
        .page(app::search_page)
        .page(app::not_found_page)
        .route(app::search_api)
        .route(app::style_css)
        .route(app::highlight_css)
        .route(app::app_js)
        .route(app::mermaid_init_js)
        .route(app::livereload_js)
        .route(app::livereload)
        .route(app::search_panel)
        // Images and other files that sit next to the docs.
        .serve_dir("/files/{*file}", files)
        .app_context(site.clone())
        .assets(assets)
        .runtime()
        .build();

    topcoat::start(router).await.expect("server failed");
}
