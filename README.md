# typeboard

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tokio](https://img.shields.io/badge/async-Tokio-black?logo=tokio&logoColor=white)](https://tokio.rs/)
[![Axum](https://img.shields.io/badge/http-Axum-4B275F?logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)
[![React](https://img.shields.io/badge/ui-React-61DAFB?logo=react&logoColor=black)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/lang-TypeScript-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Bun](https://img.shields.io/badge/runtime-Bun-fbf0df?logo=bun&logoColor=black)](https://bun.sh/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)

**typeboard** aims to send and receive API signals in Rust, then visualize them on a dashboard.

The repo currently ships a minimal scaffold (Axum API + React/TypeScript UI). Over time it will grow into a practical playground for real-world Rust backend patterns—async I/O, HTTP stacks, CPU-bound parallelism, observability, and more.

Bun is the frontend toolchain today, and a second study track in [`docs/bun`](./docs/bun/): native `Bun.serve` HTTP/WebSocket (no Express/Hono), zero-dependency TS servers, and comparisons against Go/Deno. Recent Bun releases (1.3.x → Rust rewrite toward 1.4) make that track especially useful next to the Axum/Tokio work.

---

## Goals

- Own **signal ingress/egress** in Rust (HTTP, events, and related I/O) with Axum / Tokio
- **Monitor and visualize** those signals on a frontend dashboard
- Move beyond toy demos: adopt production-minded patterns for persistence, caching, observability, auth, and deploy—step by step
- Keep a **Bun-native** reference path (`Bun.serve`, built-in WS/SQL/Redis notes) for fast TS prototypes and closed-network / dependency-zero experiments

---

## Getting started

### API (`api/`, port 3002)

```bash
cd api
cargo run
```

### Starter backend (`backend/`, port 3001)

```bash
cd backend
cargo run
```

### Frontend (`frontend/`, port 5173)

Vite + React + **TypeScript**, installed and run with **Bun**.

```bash
cd frontend

bun install
bun run dev
```

Production build:

```bash
bun run build
```

Open http://localhost:5173 and use the button to fetch a message from the backend (`http://127.0.0.1:3001/api/hello`).

### Bun notes (optional)

Study / experiment with Bun alone (no Vite app required)—see [`docs/bun`](./docs/bun/). Typical entry points:

```bash
# hot-reload a Bun.serve script
bun --hot path/to/server.ts

# package manager / scripts (frontend)
bun install && bun run dev
```

---

## Stack notes

| Layer | Choice | Notes |
|---|---|---|
| Signal / HTTP (Rust) | Axum · Tokio · hyper | Primary path for API signals |
| Dashboard UI | React · TypeScript · Vite | Served via Bun install / scripts |
| TS / realtime experiments | Bun (`Bun.serve`, native WebSocket) | Routing, pub/sub WS, SSE, HTML import, `--hot` |
| Package / lockfile | `bun.lock` | Text lockfile; isolated linker available in recent Bun |

Bun 1.3.x highlights that matter for this repo’s direction (not all wired into the scaffold yet):

- **`Bun.serve`** — built-in routing, cookies, metrics, Range requests, experimental **HTTP/3 (QUIC)**
- **Realtime** — native WebSocket (+ client `ws+unix://`), same-process HTTP upgrade
- **Data** — unified `Bun.SQL` (Postgres / MySQL / SQLite), built-in Redis client
- **Tooling** — `bun test` (`--parallel` / `--isolate` / `--shard`), `bun build --compile` (incl. browser HTML), `--metafile-md` for LLM-friendly bundle graphs
- **Runtime extras** — `Bun.markdown`, `Bun.Image`, `Bun.cron`, native REPL
- **Implementation** — Bun is moving from Zig → **Rust** (v1.4 canary); useful context while studying Rust HTTP stacks here

---

## Planned Rust ecosystem

Priorities may shift, but the direction is to layer in mature crates from the Rust async and web ecosystem.

### Core async & HTTP

| Area | Crate / project | Role |
|---|---|---|
| Async runtime | [Tokio](https://github.com/tokio-rs/tokio) | Tasks, timers, networking, concurrency |
| Futures primitives | [futures-rs](https://github.com/rust-lang/futures-rs) | Combinators, streams, and `Future` utilities beyond Tokio’s surface |
| Async traits | [async-trait](https://github.com/dtolnay/async-trait) | `async fn` in traits for service boundaries and handlers |
| Low-level HTTP | [hyper](https://github.com/hyperium/hyper) | HTTP/1–2 client/server foundation under Axum / reqwest |
| HTTP routing | [Axum](https://github.com/tokio-rs/axum) + [Tower](https://github.com/tower-rs/tower) | API surface, middleware, backpressure |
| HTTP client | [reqwest](https://github.com/seanmonstar/reqwest) | Outbound calls to internal/external services |
| Serialization | [serde](https://github.com/serde-rs/serde) | Request/response JSON and config |

### Parallelism & UI direction

| Area | Crate / project | Role |
|---|---|---|
| Data-parallel CPU work | [rayon](https://github.com/rayon-rs/rayon) | Parallel iterators / thread-pool work off the async runtime |
| Full-stack reactive UI (explore) | [Topcoat](https://github.com/tokio-rs/topcoat) | Server-rendered reactive web apps in Rust (complementary to the React dashboard) |

### Ops & product surface

| Area | Candidates | Role |
|---|---|---|
| Database | sqlx (+ PostgreSQL), rusqlite / SQLite; compare with `Bun.SQL` / `bun:sqlite` notes | Durable storage |
| Cache / locks | Redis (Rust client); compare with Bun’s built-in Redis | Sessions, rate limits, short-lived queues |
| Realtime | WebSocket / SSE (Axum · `Bun.serve`) | Live dashboard updates |
| Observability | tracing, metrics | Logs, traces, metrics |
| Config | config / dotenv | Environment-specific settings |
| Auth | JWT (and related) | Protect APIs and the dashboard |
| Deploy | Docker Compose | Local and staging bring-up |

---

## Docs

Study notes live under [`docs/`](./docs/):

| Topic | Path |
|---|---|
| Rust | [`docs/rust`](./docs/rust/) |
| Tokio | [`docs/tokio/readme.md`](./docs/tokio/readme.md) |
| Axum | [`docs/axum`](./docs/axum/) |
| Hyper | [`docs/hyper`](./docs/hyper/) |
| reqwest | [`docs/reqwest`](./docs/reqwest/) |
| Serde | [`docs/serde`](./docs/serde/) |
| rusqlite | [`docs/rusqlite`](./docs/rusqlite/) |
| Bun | [`docs/bun`](./docs/bun/) |
| MSA with Axum | [`docs/msa/readme.md`](./docs/msa/readme.md) |
| CS notes | [`docs/cs`](./docs/cs/) |
| LLM notes | [`docs/llm`](./docs/llm/) |
