# typeboard

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/desktop-Tauri%202-24C8DB?logo=tauri&logoColor=white)](https://tauri.app/)
[![TypeScript](https://img.shields.io/badge/lang-TypeScript-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Bun](https://img.shields.io/badge/runtime-Bun-fbf0df?logo=bun&logoColor=black)](https://bun.sh/)
[![xterm.js](https://img.shields.io/badge/term-xterm.js-000000?logo=xterm&logoColor=white)](https://xtermjs.org/)
[![portable-pty](https://img.shields.io/badge/pty-portable--pty-dea584?logo=rust&logoColor=white)](https://crates.io/crates/portable-pty)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)

**typeboard** is a native terminal emulator: a real PTY shell in Rust, rendered with xterm.js, wrapped as a desktop app by Tauri 2.

The frontend is Vite + TypeScript, installed and run with Bun. The backend owns the process: spawn `$SHELL` (login shell on macOS), shuttle bytes over Tauri IPC channels, and keep the PTY grid in sync with the window. Tabs, extra windows, find-in-buffer, and font zoom sit on top of that loop.

This is a working terminal, not a toy `exec` wrapper. Programs that need a TTY — vim, htop, colored prompts, OSC title sequences — behave as they would in a system terminal.

The same repo keeps a **study track** under [`docs/`](./docs/): Rust language, Tokio / Axum / hyper / reqwest / serde, Bun-native HTTP, MSA patterns, CS, security, and LLM notes. Those pages are not the app manual — they are how this project thinks about systems work next to the desktop shell.

---

## Goals

- Own **PTY I/O in Rust** (`portable-pty`): spawn, write, resize, teardown
- Render a **GPU-backed terminal** (xterm.js + WebGL) with a small custom chrome
- Support **multiple sessions per window** and **multiple app windows**
- Keep the UI native on macOS: overlay titlebar, Dock “New Window”, system menus
- Stay **Bun-native** for the frontend toolchain (`bun install`, Vite, Tauri CLI)
- Keep **written notes** in `docs/` for the Rust async / HTTP / Bun stack that sits beside the app

---

## Getting started

### Prerequisites

- [Bun](https://bun.sh) 1.4+
- [Rust](https://rustup.rs) (stable)
- macOS: Xcode Command Line Tools

### Dev

```bash
bun install
bun run tauri dev
```

Vite serves the UI on `http://localhost:1420`. Tauri loads that URL in a native window and talks to the Rust sidecar over IPC.

### Release bundle

```bash
bun run tauri build
```

The macOS app lands under `src-tauri/target/release/bundle/macos/` as `typeboard.app` (`com.async.typeboard`).

---

## Stack

| Layer | Choice | Notes |
|---|---|---|
| Desktop shell | [Tauri 2](https://tauri.app/) | Native window, menus, IPC, macOS overlay titlebar |
| PTY | [`portable-pty`](https://crates.io/crates/portable-pty) | Real shell session; `$SHELL -l` on macOS |
| Terminal UI | [xterm.js](https://xtermjs.org/) + WebGL | Grid, selection, links, search |
| Frontend | TypeScript · Vite | Vanilla TS (no React) |
| Package / lockfile | Bun 1.4 · `bun.lock` | `packageManager`: `bun@1.4.0` |
| Theme | Catppuccin Mocha | Default palette |
| Study notes | [`docs/`](./docs/) | Rust, async HTTP, Bun, MSA, CS — see below |

---

## Architecture

```
┌─────────────────────────────────────────┐
│  Vite / TypeScript  (src/)              │
│  xterm.js · Fit · Search · WebGL        │
│  tabs, find, zoom, drag / resize chrome │
└─────────────────┬───────────────────────┘
                  │ Tauri IPC (Channel)
┌─────────────────▼───────────────────────┐
│  Rust  (src-tauri/)                     │
│  pty.rs      spawn / write / resize     │
│  windows.rs  extra windows, app menu    │
│  macos_dock.rs  Dock context menu       │
└─────────────────────────────────────────┘
```

- **`pty.rs`** — session map, `portable-pty` master/writer, reader thread pushing bytes to a frontend `Channel`. `TERM_PROGRAM=typeboard`.
- **`src/main.ts`** — one xterm instance per tab; OSC titles; find (`⌘F`); font size in `localStorage` (`typeboard.fontSize`).
- **`windows.rs`** — `⌘N` / menu “New Window”; clone windows with the same webview.
- **`macos_dock.rs`** — NSApplication `dockMenu` via objc2 (“New Window”).

Capabilities live in `src-tauri/capabilities/default.json` (`windows: ["*"]`, drag / resize / title).

---

## Shortcuts

| Key | Action |
|---|---|
| typing | forwarded to the PTY |
| `⌘C` (with selection) | copy |
| `⌘V` | paste |
| `⌘T` / tab-bar **+** | new tab |
| `⌘N` / File → New Window | new window |
| Dock icon (right-click) | New Window |
| tab bar (right-click) | new tab / new window |
| `⌘W` | close tab (last tab closes the window) |
| `⇧⌘W` | close window |
| `⌘F` | find in terminal output |
| `⌘G` / `⇧⌘G` | next / previous match |
| titlebar drag | move window |
| window edges / corners | resize |
| `⌘+` / `⌘=` | larger font |
| `⌘-` | smaller font |
| `⌘0` | reset font |
| `Esc` | close find |
| any key after shell exit | restart session |

Font size is remembered per window. Chrome (tabs, titlebar) scales with it. Resizing the window refits the grid and the PTY.

---

## Layout

```
src/                 frontend (Vite + TypeScript)
src-tauri/src/       Rust: PTY, windows, Dock menu
src-tauri/tauri.conf.json
index.html           overlay titlebar + tab bar
docs/                study notes (not the app UI)
```

---

## Docs

[`docs/`](./docs/) is a dated notebook, not generated API docs. Filenames often look like `YYMMDD_HHMM_topic.md`. Start from a folder `readme.md` when one exists; otherwise open the latest file in that directory.

The terminal binary does not import these pages. They are the written half of the same workspace: how Rust async I/O, HTTP stacks, and Bun-native servers actually behave, so the desktop PTY work sits next to a systems vocabulary.

Typical entry:

```bash
# hot-reload a Bun.serve script from the notes
bun --hot path/to/server.ts
```

### Language & crates

| Path | What it is |
|---|---|
| [`docs/rust`](./docs/rust/) | Rust grammar roadmap (ownership through async). Start at [`readme.md`](./docs/rust/readme.md). |
| [`docs/serde`](./docs/serde/) | Serde / `serde_json`: attributes, enums, Axum·reqwest, `RawValue`. |
| [`docs/rusqlite`](./docs/rusqlite/) | SQLite from Rust: bindings, transactions, concurrency vs the C API. |

### Async & HTTP

| Path | What it is |
|---|---|
| [`docs/tokio`](./docs/tokio/) | Tokio runtime: tasks, `poll` / Waker, thread pool, timers / TCP. [`readme.md`](./docs/tokio/readme.md) then `01.md`–`06.md`. |
| [`docs/axum`](./docs/axum/) | Axum routing, extractors, JSON, middleware — concept → code → checkpoint. |
| [`docs/hyper`](./docs/hyper/) | Hyper 1.x under Axum / reqwest: the HTTP layer you rarely call directly. |
| [`docs/reqwest`](./docs/reqwest/) | Outbound HTTP from an Axum process (sender / receiver examples). |
| [`docs/msa`](./docs/msa/) | NestJS / Spring-scale MSA on Axum: workspace, sqlx, Redis, gateway, messaging, tracing, deploy. [`readme.md`](./docs/msa/readme.md). |

### Bun

[`docs/bun`](./docs/bun/) is the TypeScript / runtime track: `Bun.serve` without Express/Hono, native WebSocket, file I/O, CORS, benchmarks, Bun vs Go / Deno, Kafka wire encoding, and the Bun 1.4 report (`Bun.Image`, `Bun.markdown`, `Bun.cron`, experimental HTTP/3, `bun test --parallel`).

Bun 1.3.x–1.4 highlights that show up in those notes (not all wired into the Tauri app):

- **`Bun.serve`** — routing, cookies, Range, experimental HTTP/3 (QUIC)
- **Realtime** — native WebSocket, same-process HTTP upgrade
- **Data** — `Bun.SQL`, built-in Redis client
- **Tooling** — `bun test`, `bun build --compile`, `--metafile-md`
- **Runtime extras** — `Bun.markdown`, `Bun.Image`, `Bun.cron`

### Systems, security, models

| Path | What it is |
|---|---|
| [`docs/cs`](./docs/cs/) | CS / ops notes: locks, ACL vs SG/NACL, SRE golden signals, latency numbers, cross-build (Go/Rust/Bun), clean-room rewrites, transfer strategy. |
| [`docs/security`](./docs/security/) | PII encryption obligations and implementation (Korea-focused; not legal advice). |
| [`docs/llm`](./docs/llm/) | Model internals — GQA, Kimi K3 architecture / training / infra. |

---

## Status

Early but usable on macOS. Linux / Windows PTY paths from `portable-pty` are unproven here. Identifier is `com.async.typeboard`.

Licensed under [Apache 2.0](./LICENSE).
