# typeboard

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/tokio-rs/topcoat/main/media/logo-dark.svg">
    <img src="https://raw.githubusercontent.com/tokio-rs/topcoat/main/media/logo-light.svg" alt="Topcoat" width="320">
  </picture>
</div>

<div align="center">

[![Rust](https://img.shields.io/badge/rust-1.98%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Topcoat](https://img.shields.io/crates/v/topcoat.svg)](https://github.com/tokio-rs/topcoat)
[![Tokio](https://img.shields.io/badge/async-Tokio-000000)](https://tokio.rs/)
[![Bun](https://img.shields.io/badge/tooling-Bun-fbf0df?logo=bun&logoColor=black)](https://bun.sh/)
[![Mermaid](https://img.shields.io/badge/diagrams-Mermaid-FF3670?logo=mermaid&logoColor=white)](https://mermaid.js.org/)
[![SQLite](https://img.shields.io/badge/search-SQLite%20FTS5-003B57?logo=sqlite&logoColor=white)](https://www.sqlite.org/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)

</div>

A server that turns study notes written as Markdown files into web pages. It is built with the Rust full-stack framework [Topcoat](https://github.com/tokio-rs/topcoat).

## Features

- Sidebar organized as section → category → document. Only the category you are reading stays expanded.
- The home page shows category cards and recently added documents.
- Live document search as you type (keyboard shortcut `/`)
- Syntax-colored code blocks (VS Code colors, automatic dark mode) with a copy button
- Mermaid diagrams, tables, and an on-page table of contents
- Saving a document refreshes open browser tabs automatically.
- If a document moves to another category, old URLs still resolve to the new location.

## Repository layout

```text
typeboard/
├── Cargo.toml / Cargo.lock   # Rust crate (binary name: bun-report)
├── package.json              # Bun scripts → Topcoat CLI / shell wrappers
├── LICENSE                   # Apache-2.0
├── README.md
├── src/
│   ├── main.rs               # Router, asset bundle, docs watcher
│   ├── app.rs                # Pages, shards, /static routes, live reload WS
│   ├── site.rs               # Env (PORT, SITE_TITLE), library load, watch
│   ├── docs.rs               # Section → category → doc tree from docs/
│   ├── markdown.rs           # comrak render + heading ids
│   ├── highlight.rs          # syntect / two-face → highlight.css
│   ├── search.rs             # In-memory SQLite FTS5
│   └── assets/               # Embedded CSS/JS served from /static/*
│       ├── style.rs
│       ├── app_js.rs
│       ├── livereload_js.rs
│       └── mermaid_init_js.rs
├── scripts/
│   ├── service.sh            # macOS LaunchAgent (login + KeepAlive)
│   └── prod.sh               # Linux-style nohup background start
└── docs/                     # Markdown library (auto-indexed)
    ├── _index.md
    ├── 01_Bun/
    ├── 02_Rust/
    ├── 03_시스템_설계/
    ├── 04_CS_기초/
    ├── 05_네트워크와_보안/
    ├── 06_AI/
    └── 07_기술_동향/
```

Build outputs (`target/`), logs (`logs/`), and the Topcoat asset bundle sit next to the binary and are gitignored.

## Run

Install the Topcoat CLI once:

```bash
cargo install topcoat-cli --version '~0.10' --locked
```


| Command                   | What it does                                                                            |
| ------------------------- | --------------------------------------------------------------------------------------- |
| `bun run dev`             | Dev mode. Rebuilds and refreshes the page when Rust (including embedded CSS/JS) changes |
| `bun run start`           | Bundle assets once and run (`http://localhost:3000`)                                    |
| `bun run typecheck`       | `cargo check`                                                                           |
| `bun run build`           | Release binary + asset bundle (`scripts/service.sh build`)                              |
| `bun run service:start`   | Register as a background service (starts on login)                                      |
| `bun run service:restart` | Rebuild and restart the service                                                         |
| `bun run service:stop`    | Stop the service                                                                        |
| `bun run service:status`  | Show LaunchAgent state                                                                  |
| `bun run service:logs`    | Tail the service log                                                                    |


Override the port with `PORT` and the site title with `SITE_TITLE`.

## Shell scripts

`package.json` mostly forwards to shell scripts. You can also call them directly.

### macOS — `scripts/service.sh`

Registers a LaunchAgent (`com.glory.bun-report`) so the **release** server starts at login and is restarted by `launchd` if it exits. `start` / `restart` rebuild with `cargo build --release` and `topcoat asset bundle --release` first. Logs go to `logs/server.log`.

```bash
sh scripts/service.sh build
sh scripts/service.sh start      # or: bun run service:start
sh scripts/service.sh restart
sh scripts/service.sh status
sh scripts/service.sh logs
sh scripts/service.sh stop
```

Env: `PORT` (default `3000`). The plist also sets `REPORT_ROOT` to the repo root.

### Linux / simple background — `scripts/prod.sh`

Builds if needed (or when passed `--build`), stops any previous binary, then starts with `nohup`.

```bash
sh scripts/prod.sh            # run existing release binary if present
sh scripts/prod.sh --build    # force release build + asset bundle first
```

Env: `HOST` (default `0.0.0.0`), `PORT` (default `3000`). Log: `logs/server.log`.

## Add documents

- Put a `.md` file in the right category folder. The first-line `# Title` becomes the title shown in the list.
- Name files `YYMMDD_HHMM_title.md` to attach a date and sort chronologically.
- For a new category, create a folder and an `_index.md` with `# Name` plus a one-line blurb. A numeric prefix such as `01_` on the folder name only controls order.
- Full rules are under “How to add documents” at the bottom of the site home page.

## Tech stack

### Rust (server)


| Piece                                                      | Role here                                                                                                                                 |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| **Topcoat 0.10** (`full`)                                  | App shell: router, HTML views (`view!` / pages / shards), asset bundling, browser runtime helpers, and the WebSocket used for live reload |
| **Tokio**                                                  | Async runtime behind `#[tokio::main]` and Topcoat’s server                                                                                |
| **comrak**                                                 | GitHub-flavored Markdown → HTML for doc pages and the search index                                                                        |
| **syntect** + **two-face**                                 | Server-side syntax highlighting for fenced code (VS Code–style themes; CSS emitted for light/dark)                                        |
| **rusqlite** (bundled SQLite)                              | In-memory FTS5 trigram search index over documents and sections                                                                           |
| **notify**                                                 | Watches `docs/` so edits rebuild search and push a browser refresh                                                                        |
| **serde** / **serde_json**                                 | Structured data for search APIs and site configuration                                                                                    |
| **regex**, **percent-encoding**, **unicode-normalization** | Slug/URL handling, heading ids, and NFC-safe paths for Korean filenames                                                                   |


The binary is a Cargo project (`Cargo.toml`). Site CSS/JS live as Rust string constants under `src/assets/` and are served from `/static/*` routes. After each build, Topcoat’s CLI still packs remote assets (Mermaid) next to the binary; `bun run start` runs `topcoat asset bundle` before `cargo run`.

### Bun (tooling)


| Piece                      | Role here                                                                                                                                          |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Bun**                    | Runs the npm-style scripts in `package.json` (`dev`, `start`, service helpers). It is the day-to-day CLI for this repo, not the HTTP server itself |
| `bun run dev`              | Delegates to `topcoat dev` for rebuild + hot refresh while you edit Rust / `src/assets`                                                            |
| `bun run start`            | `topcoat asset bundle && cargo run` — production-like local launch                                                                                 |
| **service / prod scripts** | `scripts/service.sh` (macOS LaunchAgent) and `scripts/prod.sh` (nohup), usually via `bun run service:`*                                            |


Bun is also the subject of much of the Markdown library under `docs/` (runtime notes and examples). The doc server that serves those pages is Rust/Topcoat; Bun is what you use to start that server and, separately, to run Bun-focused examples described in the notes.