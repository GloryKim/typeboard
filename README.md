# typeboard

A server that turns study notes written as Markdown files into web pages. It is built with the Rust full-stack framework [Topcoat](https://github.com/tokio-rs/topcoat).

Drop `.md` files into the `docs` folder and they show up on the site without a separate content build. The library currently covers Bun, Rust, system design, networks and security, and AI notes.

## Features

- Sidebar organized as section → category → document. Only the category you are reading stays expanded.
- The home page shows category cards and recently added documents.
- Live document search as you type (keyboard shortcut `/`)
- Syntax-colored code blocks (VS Code colors, automatic dark mode) with a copy button
- Mermaid diagrams, tables, and an on-page table of contents
- Saving a document refreshes open browser tabs automatically.
- If a document moves to another category, old URLs still resolve to the new location.

## Run

Install the Topcoat CLI once:

```bash
cargo install topcoat-cli --version '~0.10' --locked
```

| Command | What it does |
|---|---|
| `bun run dev` | Dev mode. Rebuilds and refreshes the page when Rust (including embedded CSS/JS) changes |
| `bun run start` | Bundle assets once and run (`http://localhost:3000`) |
| `bun run service:start` | Register as a background service (starts on login) |
| `bun run service:restart` | Rebuild and restart the service |
| `bun run service:stop` | Stop the service |
| `bun run service:logs` | Tail the service log |

Override the port with `PORT` and the site title with `SITE_TITLE`.

## Add documents

- Put a `.md` file in the right category folder. The first-line `# Title` becomes the title shown in the list.
- Name files `YYMMDD_HHMM_title.md` to attach a date and sort chronologically.
- For a new category, create a folder and an `_index.md` with `# Name` plus a one-line blurb. A numeric prefix such as `01_` on the folder name only controls order.
- Full rules are under “How to add documents” at the bottom of the site home page.

## Tech stack

### Rust (server)

| Piece | Role here |
|---|---|
| **Topcoat 0.10** (`full`) | App shell: router, HTML views (`view!` / pages / shards), asset bundling, browser runtime helpers, and the WebSocket used for live reload |
| **Tokio** | Async runtime behind `#[tokio::main]` and Topcoat’s server |
| **comrak** | GitHub-flavored Markdown → HTML for doc pages and the search index |
| **syntect** + **two-face** | Server-side syntax highlighting for fenced code (VS Code–style themes; CSS emitted for light/dark) |
| **rusqlite** (bundled SQLite) | In-memory FTS5 trigram search index over documents and sections |
| **notify** | Watches `docs/` so edits rebuild search and push a browser refresh |
| **serde** / **serde_json** | Structured data for search APIs and site configuration |
| **regex**, **percent-encoding**, **unicode-normalization** | Slug/URL handling, heading ids, and NFC-safe paths for Korean filenames |

The binary is a Cargo project (`Cargo.toml`). Site CSS/JS live as Rust string constants under `src/assets/` and are served from `/static/*` routes. After each build, Topcoat’s CLI still packs remote assets (Mermaid) next to the binary; `bun run start` runs `topcoat asset bundle` before `cargo run`.

### Bun (tooling)

| Piece | Role here |
|---|---|
| **Bun** | Runs the npm-style scripts in `package.json` (`dev`, `start`, service helpers). It is the day-to-day CLI for this repo, not the HTTP server itself |
| **`bun run dev`** | Delegates to `topcoat dev` for rebuild + hot refresh while you edit Rust/`public` |
| **`bun run start`** | `topcoat asset bundle && cargo run` — production-like local launch |
| **service scripts** | Thin wrappers around `scripts/service.sh` (macOS LaunchAgent) invoked via Bun |

Bun is also the subject of much of the Markdown library under `docs/` (runtime notes and examples). The doc server that serves those pages is Rust/Topcoat; Bun is what you use to start that server and, separately, to run Bun-focused examples described in the notes.
