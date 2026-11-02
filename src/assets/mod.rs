//! Static CSS/JS formerly under `public/`, embedded as Rust string constants.

mod app_js;
mod livereload_js;
mod mermaid_init_js;
mod style;

pub use app_js::JS as APP_JS;
pub use livereload_js::JS as LIVERELOAD_JS;
pub use mermaid_init_js::JS as MERMAID_INIT_JS;
pub use style::CSS as STYLE_CSS;
