# typeboard

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tokio](https://img.shields.io/badge/async-Tokio-black?logo=tokio&logoColor=white)](https://tokio.rs/)
[![Axum](https://img.shields.io/badge/http-Axum-4B275F?logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)
[![React](https://img.shields.io/badge/ui-React-61DAFB?logo=react&logoColor=black)](https://react.dev/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](./LICENSE)

**typeboard** aims to send and receive API signals in Rust, then visualize them on a dashboard.

The repo currently ships a minimal scaffold (Axum API + React UI). Over time it will grow into a practical playground for real-world Rust backend patterns—async I/O, HTTP stacks, CPU-bound parallelism, observability, and more.

---

## Goals

- Own **signal ingress/egress** in Rust (HTTP, events, and related I/O) with Axum / Tokio
- **Monitor and visualize** those signals on a frontend dashboard
- Move beyond toy demos: adopt production-minded patterns for persistence, caching, observability, auth, and deploy—step by step

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

```bash
cd frontend
npm install
npm run dev
```

Open http://localhost:5173 and use the button to fetch a message from the backend.

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
| Database | sqlx (+ PostgreSQL) | Durable storage |
| Cache / locks | Redis | Sessions, rate limits, short-lived queues |
| Realtime | WebSocket / SSE | Live dashboard updates |
| Observability | tracing, metrics | Logs, traces, metrics |
| Config | config / dotenv | Environment-specific settings |
| Auth | JWT (and related) | Protect APIs and the dashboard |
| Deploy | Docker Compose | Local and staging bring-up |

Study notes live under [`docs/`](./docs/):

- [Rust grammar](./docs/grammer/readme.md)
- [Tokio](./docs/tokio/readme.md)
- [reqwest / Axum request–response](./docs/reqwest/01.md)
- [MSA with Axum](./docs/msa/readme.md)
