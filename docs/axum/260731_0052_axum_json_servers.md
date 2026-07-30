# Axum JSON 송신·수신 서버 상세 교재

> 이 문서는 Axum으로 **JSON을 주는(송신) 서버**와 **JSON을 받는(수신) 서버** 두 프로세스를 만들고,  
> **비동기 수신**, **필드명(serde)**, **랜덤 페이로드 송신**, **수신 JSON 파일 저장**,  
> **Protobuf와 페이로드 크기 비교**까지 코드와 함께 정리합니다.
>
> 양식: **개념 → 코드 → 설명 → 체크포인트**  
> 기준: **Axum 0.8**. 이 저장소 `api`는 0.7이므로 path param만 `:id`로 바꾸면 됩니다.  
> 관련: [260729_2343_axum.md](./260729_2343_axum.md), [docs/reqwest](../reqwest/)

---

## 목차

1. [전체 그림](#1-전체-그림)
2. [프로젝트 구조](#2-프로젝트-구조)
3. [JSON의 핵심 — Serialize / Deserialize](#3-json의-핵심--serialize--deserialize)
4. [필드명 — 가장 많이 막히는 지점](#4-필드명--가장-많이-막히는-지점)
5. [수신 서버 (Receiver) — 메모리 + JSON 파일 저장](#5-수신-서버-receiver--메모리--json-파일-저장)
6. [송신 서버 (Sender) — 수동 전송 + 랜덤 전송](#6-송신-서버-sender--수동-전송--랜덤-전송)
7. [비동기 수신 — 왜 async인가](#7-비동기-수신--왜-async인가)
8. [비동기 수신 패턴 심화](#8-비동기-수신-패턴-심화)
9. [에러·상태 코드·Content-Type](#9-에러상태-코드content-type)
10. [동시에 여러 JSON 받기](#10-동시에-여러-json-받기)
11. [Protobuf — 같은 데이터, 더 작은 페이로드](#11-protobuf--같은-데이터-더-작은-페이로드)
12. [실전 실행 순서](#12-실전-실행-순서)
13. [curl / 프론트 검증](#13-curl--프론트-검증)
14. [코드 검토 메모](#14-코드-검토-메모)
15. [자주 하는 실수](#15-자주-하는-실수)
16. [체크리스트](#16-체크리스트)

---

## 1. 전체 그림

### 개념

두 대의 독립 프로세스를 둡니다.

| 역할 | 포트 | 하는 일 |
|---|---|---|
| **Receiver** | `:3000` | `POST /ingest` 로 JSON을 받고, **메모리 + JSON 파일**에 저장 |
| **Sender** | `:4000` | `POST /send` (본문 지정) 또는 `POST /send/random` (임의 값 생성) 후 Receiver로 전달 |

```text
클라이언트
   │  POST /send          또는  POST /send/random
   ▼
┌──────────────────┐   reqwest async POST /ingest   ┌──────────────────┐
│ Sender :4000     │ ─────────────────────────────▶ │ Receiver :3000    │
│ (랜덤 OrderEvent  │         application/json       │ Json 파싱·검증    │
│  생성 가능)       │ ◀───────────────────────────── │ 메모리 Vec 보관   │
└──────────────────┘            ack JSON             │ data/*.jsonl 저장 │
                                                     └──────────────────┘
```

- Sender → Receiver 호출은 Axum이 아니라 **`reqwest`** 다.
- 같은 계약을 나중에 **Protobuf**로 바꾸면 필드 이름을 wire에 안 실어 페이로드가 주는 경우가 많다 (11절).

### 체크포인트

- [ ] Sender / Receiver / reqwest 역할 구분
- [ ] `/send` 와 `/send/random` 차이를 한 줄로

---

## 2. 프로젝트 구조

```text
axum-json-lab/
├── Cargo.toml
├── receiver/
│   ├── Cargo.toml
│   ├── src/main.rs
│   └── data/                 # 실행 시 생성 — events.jsonl
└── sender/
    ├── Cargo.toml
    └── src/main.rs
```

```toml
# axum-json-lab/Cargo.toml
[workspace]
members = ["receiver", "sender"]
resolver = "2"
```

### Receiver `Cargo.toml`

```toml
[package]
name = "receiver"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
# Protobuf 실습(11절)용 — JSON만 쓰면 생략 가능
axum-extra = { version = "0.12", features = ["protobuf"] }
prost = "0.13"
```

### Sender `Cargo.toml`

```toml
[package]
name = "sender"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
rand = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
# Protobuf 실습(11절)용
prost = "0.13"
bytes = "1"
```

`reqwest`의 `json` feature가 있어야 `.json(&payload)` / `.json::<T>()` 가 동작한다.

### 체크포인트

- [ ] `cargo check -p receiver` / `cargo check -p sender` 통과
- [ ] sender에 `rand`·`reqwest`가 있는 이유 설명

---

## 3. JSON의 핵심 — Serialize / Deserialize

### 개념

| 방향 | Rust 트레이트 | 누가 쓰나 |
|---|---|---|
| JSON → Rust 구조체 | `Deserialize` | **수신** (`Json<T>` extractor) |
| Rust 구조체 → JSON | `Serialize` | **송신** (응답 `Json(T)`, reqwest `.json(&t)`, 파일 저장) |

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderEvent {
    order_id: String,
    product_name: String,
    quantity: u32,
    note: Option<String>,
}
```

- 필드 타입이 JSON과 안 맞으면 Axum `Json` extractor는 기본적으로 **400** 을 낸다.
- 파일에 남길 때도 같은 `Serialize`를 쓴다.

### `Json<T>` (수신)

1. body를 async로 모은다  
2. `serde_json`으로 `T`에 역직렬화  
3. 실패 시 rejection → 기본 400  
4. 클라이언트는 `Content-Type: application/json`을 넣는 것이 안전하다

### `Json(T)` (응답)

1. `Serialize` → JSON 바이트  
2. `content-type: application/json` 설정

### 체크포인트

- [ ] Deserialize = 받기, Serialize = 주기·저장
- [ ] 양쪽이면 `Serialize, Deserialize` 둘 다

---

## 4. 필드명 — 가장 많이 막히는 지점

### 개념

serde 기본은 **Rust 필드명 = JSON 키**.  
Rust는 `snake_case`, 프론트/다수 API는 `camelCase` → 불일치 시 400으로 보인다.

### 권장 — `rename_all`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderEvent {
    order_id: String,      // JSON: orderId
    product_name: String,  // JSON: productName
    quantity: u32,
    note: Option<String>,
}
```

Sender와 Receiver가 **같은 rename 규칙**을 써야 한다. 가능하면 `shared` 크레이트에 DTO 한 곳.

### 자주 쓰는 어트리뷰트

| 어트리뷰트 | 효과 |
|---|---|
| `rename = "orderId"` | 필드 하나만 키 지정 |
| `rename_all = "camelCase"` | 구조체 전체 |
| `default` / `default = "fn"` | 없을 때 기본값 |
| `skip_serializing_if = "Option::is_none"` | None이면 키 생략 |
| `deny_unknown_fields` | 여분 키면 실패 |

### 체크포인트

- [ ] camelCase JSON으로 성공, snake_case 키로 400 확인

---

## 5. 수신 서버 (Receiver) — 메모리 + JSON 파일 저장

### 개념

수신 후:

1. 도메인 검증  
2. `StoredEvent` 생성 (id, 시각)  
3. 메모리 `Vec`에 push (조회용)  
4. **`data/events.jsonl`에 한 줄 JSON append** (영속·감사 로그용)

JSON Lines(`.jsonl`): 파일 한 줄 = JSON 객체 하나. 추가만 하면 되고, 파싱도 줄 단위로 쉽다.

**중요:** `std::sync::Mutex`를 잡은 채 `.await` 하지 않는다.  
메모리 반영은 lock 안에서 짧게, 파일 쓰기는 lock 밖에서 `tokio::fs`로 await 한다.

### 코드

```rust
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    events: Arc<Mutex<Vec<StoredEvent>>>,
    /// 예: ./data/events.jsonl
    journal_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEvent {
    id: Uuid,
    received_at: DateTime<Utc>,
    event: OrderEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderEvent {
    order_id: String,
    product_name: String,
    quantity: u32,
    note: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestAck {
    status: &'static str,
    id: Uuid,
    received_at: DateTime<Utc>,
    /// 디스크에도 남겼는지
    persisted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiError {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'static str>,
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "role": "receiver" }))
}

async fn ingest(
    State(state): State<AppState>,
    Json(body): Json<OrderEvent>,
) -> impl IntoResponse {
    if body.order_id.trim().is_empty() {
        return error(StatusCode::BAD_REQUEST, "orderId required", Some("orderId"));
    }
    if body.product_name.trim().is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "productName required",
            Some("productName"),
        );
    }
    if body.quantity == 0 {
        return error(StatusCode::BAD_REQUEST, "quantity must be > 0", Some("quantity"));
    }

    let stored = StoredEvent {
        id: Uuid::new_v4(),
        received_at: Utc::now(),
        event: body,
    };

    // 1) 메모리 — lock 짧게
    {
        let mut guard = state.events.lock().unwrap();
        guard.push(stored.clone());
        tracing::info!(count = guard.len(), id = %stored.id, "ingested in memory");
    }

    // 2) 파일 — lock 밖에서 await
    let persisted = match append_jsonl(&state.journal_path, &stored).await {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(error = %e, "failed to persist jsonl");
            false
        }
    };

    // 디스크 실패해도 메모리는 있는 상태. 운영에선 정책에 따라 500으로 바꿀 수 있다.
    (
        StatusCode::CREATED,
        Json(IngestAck {
            status: "accepted",
            id: stored.id,
            received_at: stored.received_at,
            persisted,
        }),
    )
        .into_response()
}

async fn list(State(state): State<AppState>) -> Json<Vec<StoredEvent>> {
    Json(state.events.lock().unwrap().clone())
}

async fn append_jsonl(path: &Path, event: &StoredEvent) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let line = serde_json::to_string(event)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    file.flush().await?;
    Ok(())
}

fn error(status: StatusCode, msg: &str, field: Option<&'static str>) -> axum::response::Response {
    (
        status,
        Json(ApiError {
            error: msg.to_string(),
            field,
        }),
    )
        .into_response()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .init();

    let journal_path = PathBuf::from(
        std::env::var("JOURNAL_PATH").unwrap_or_else(|_| "data/events.jsonl".into()),
    );

    let state = AppState {
        events: Arc::new(Mutex::new(Vec::new())),
        journal_path,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ingest", post(ingest))
        .route("/events", get(list))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::info!("receiver on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
```

### 설명

| 항목 | 내용 |
|---|---|
| `events.jsonl` | 수신마다 한 줄 append. 재시작 후에도 파일은 남음 |
| `persisted` | 파일 쓰기 성공 여부. 실패해도 201 + `persisted: false` (학습용 정책) |
| `create_dir_all` | `data/` 없으면 생성 |
| lock / await 분리 | 런타임 안전 |

운영에서 “파일까지 성공해야 201”이면 `persisted == false`일 때 500을 주고 메모리 rollback을 검토한다.

### 체크포인트

- [ ] `/ingest` 후 `data/events.jsonl`에 줄이 생기는지
- [ ] `GET /events`와 파일 내용이 대응하는지

---

## 6. 송신 서버 (Sender) — 수동 전송 + 랜덤 전송

### 개념

| 엔드포인트 | 동작 |
|---|---|
| `POST /send` | 클라이언트가 준 JSON을 그대로 Receiver로 전달 |
| `POST /send/random` | **서버가 임의 OrderEvent를 생성**해 Receiver로 전달 |

랜덤 필드는 재현 가능한 범위를 둔다 (상품 후보 목록 + `rand`).

### 코드

```rust
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    http: reqwest::Client,
    receiver_base: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest {
    order_id: String,
    product_name: String,
    quantity: u32,
    note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderEvent {
    order_id: String,
    product_name: String,
    quantity: u32,
    note: Option<String>,
}

/// Receiver IngestAck — wire는 문자열이므로 String으로 받아도 된다.
/// (Uuid/DateTime JSON 문자열). 타입을 맞추려면 sender에도 uuid/chrono를 넣으면 된다.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IngestAck {
    status: String,
    id: String,
    received_at: String,
    #[serde(default)]
    persisted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendResponse {
    ok: bool,
    upstream_status: u16,
    /// 실제로 보낸 페이로드 (랜덤일 때 클라이언트가 확인용으로 봄)
    sent: Option<OrderEvent>,
    ack: Option<IngestAck>,
    error: Option<String>,
}

const PRODUCTS: &[&str] = &[
    "typeboard-sticker",
    "mechanical-switch",
    "deskmat-xl",
    "keycap-set",
    "usb-c-cable",
];

const NOTES: &[&str] = &[
    "async json demo",
    "rush shipping",
    "gift wrap",
    "no note",
];

fn random_order_event() -> OrderEvent {
    let mut rng = rand::thread_rng();
    let product = PRODUCTS.choose(&mut rng).copied().unwrap_or("unknown");
    let note_raw = NOTES.choose(&mut rng).copied().unwrap_or("no note");
    let note = if note_raw == "no note" {
        None
    } else {
        Some(note_raw.to_string())
    };

    OrderEvent {
        order_id: format!("RND-{:08}", rng.gen_range(0..100_000_000u32)),
        product_name: product.to_string(),
        quantity: rng.gen_range(1..=20),
        note,
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "role": "sender" }))
}

async fn send(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> impl IntoResponse {
    let payload = OrderEvent {
        order_id: req.order_id,
        product_name: req.product_name,
        quantity: req.quantity,
        note: req.note,
    };
    forward_to_receiver(&state, payload).await
}

/// 본문 없이 호출 — 서버가 랜덤 JSON을 만들어 송신
async fn send_random(State(state): State<AppState>) -> impl IntoResponse {
    let payload = random_order_event();
    tracing::info!(?payload, "generated random order");
    forward_to_receiver(&state, payload).await
}

async fn forward_to_receiver(state: &AppState, payload: OrderEvent) -> axum::response::Response {
    let url = format!("{}/ingest", state.receiver_base.trim_end_matches('/'));

    // .json()이 Content-Type: application/json 을 설정한다. 중복 header는 불필요.
    let result = state.http.post(&url).json(&payload).send().await;

    let response = match result {
        Err(e) => {
            tracing::error!(error = %e, "upstream request failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(SendResponse {
                    ok: false,
                    upstream_status: 0,
                    sent: Some(payload),
                    ack: None,
                    error: Some(format!("receiver unreachable: {e}")),
                }),
            )
                .into_response();
        }
        Ok(res) => res,
    };

    let upstream_status = response.status().as_u16();
    let status = response.status();

    if status.is_success() {
        match response.json::<IngestAck>().await {
            Ok(ack) => (
                StatusCode::OK,
                Json(SendResponse {
                    ok: true,
                    upstream_status,
                    sent: Some(payload),
                    ack: Some(ack),
                    error: None,
                }),
            )
                .into_response(),
            Err(e) => (
                StatusCode::BAD_GATEWAY,
                Json(SendResponse {
                    ok: false,
                    upstream_status,
                    sent: Some(payload),
                    ack: None,
                    error: Some(format!("invalid ack json: {e}")),
                }),
            )
                .into_response(),
        }
    } else {
        let body = response.text().await.unwrap_or_default();
        (
            StatusCode::BAD_GATEWAY,
            Json(SendResponse {
                ok: false,
                upstream_status,
                sent: Some(payload),
                ack: None,
                error: Some(body),
            }),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("info"))
        .init();

    let state = AppState {
        http: reqwest::Client::new(),
        receiver_base: std::env::var("RECEIVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3000".into()),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/send", post(send))
        .route("/send/random", post(send_random))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .unwrap();
    tracing::info!("sender on http://127.0.0.1:4000");
    axum::serve(listener, app).await.unwrap();
}
```

### 설명

| 코드 | 의미 |
|---|---|
| `random_order_event` | orderId / 상품 / 수량 / note를 임의 생성 |
| `forward_to_receiver` | `/send`와 `/send/random` 공통 송신 경로 |
| `sent` 필드 | 랜덤값을 클라이언트가 응답에서 확인 |
| `.json(&payload)`만 사용 | Content-Type 중복 설정 제거 |

### 체크포인트

- [ ] `POST /send/random` 여러 번 → 매번 다른 `sent` / `orderId`
- [ ] Receiver `events.jsonl`에 줄이 쌓이는지

---

## 7. 비동기 수신 — 왜 async인가

### 개념

`async` 핸들러에서 body 읽기·reqwest·파일 append의 `.await` 동안 Tokio는 **다른 연결 핸들러를 스케줄**할 수 있다.

| 상황 | 비동기 이득 |
|---|---|
| JSON body, reqwest, `tokio::fs` | 큼 |
| 핸들러 안 동기 대량 CPU | 워커 점유 → `spawn_blocking` 검토 |
| `std::sync::Mutex` lock 중 `.await` | **금지**. lock은 짧게 |

`Json` extractor 자체도 body를 async로 모은 뒤 serde 한다. “JSON 수신 = 이미 비동기 I/O 경로”다.

### 체크포인트

- [ ] 파일 쓰기를 lock 밖으로 뺀 이유 설명

---

## 8. 비동기 수신 패턴 심화

### 패턴 A — 요청-응답 (5절)

받음 → 검증 → 메모리+파일 → 201. 클라이언트가 같은 왕복에서 성공을 안다.

### 패턴 B — 202 + 큐

```rust
// try_send 성공 → 202 Accepted (처리 완료가 아님)
// 큐 full → 503
```

파일/DB가 느리면 HTTP 밖에서 워커가 `recv().await` 후 저장한다.

### 패턴 C — `tokio::spawn` fire-and-forget

간단하지만 실패 추적·백프레셔·종료 시 유실에 약하다. 데모 이상이면 `mpsc`/외부 큐.

### 체크포인트

- [ ] 201 vs 202 구분

---

## 9. 에러·상태 코드·Content-Type

| 상황 | 코드 |
|---|---|
| 수신·생성 성공 | 201 |
| Sender가 업스트림 성공을 중계 | 200 |
| 큐만 접수 | 202 |
| JSON/필드 검증 실패 | 400 |
| Receiver 다운·잘못된 ack | **502** (Sender 관점) |
| 큐 full | 503 |

```bash
# Content-Type 필수에 가깝다
curl -X POST ... -H 'content-type: application/json' -d '...'
```

### 체크포인트

- [ ] Content-Type 없이 POST → 실패 확인
- [ ] Receiver 종료 후 `/send/random` → 502

---

## 10. 동시에 여러 JSON 받기

```bash
for i in $(seq 1 20); do
  curl -s -X POST http://127.0.0.1:4000/send/random &
done
wait
curl -s http://127.0.0.1:3000/events | head
wc -l receiver/data/events.jsonl
```

`reqwest::Client`는 State에 하나. 동시 `.send().await`에 안전하다(핸들 공유).

파일 append는 OS/파일시스템 수준에서 줄 단위가 깨질 수 있으므로, 강한 보장이 필요하면 **한 워커만 쓰기** 또는 DB를 쓴다. 학습용 jsonl에는 보통 충분하다.

---

## 11. Protobuf — 같은 데이터, 더 작은 페이로드

### 개념

JSON은 **필드 이름을 문자열로 반복**한다.

```json
{"orderId":"RND-00001234","productName":"deskmat-xl","quantity":7,"note":"gift wrap"}
```

Protobuf(wire format)는:

- 필드 이름 대신 **숫자 태그** (`order_id = 1` …)
- 정수는 **varint** (작은 수는 1바이트에 가깝게)
- 스키마(`.proto` / `prost` 정의)가 양쪽 코드에 있어야 디코딩 가능
- 사람이 읽을 수 없음 → 디버깅은 JSON이 유리

그래서 **같은 의미의 메시지**를 Protobuf로 바꾸면 페이로드가 주는 경우가 많다.  
다만 메시지가 극히 작거나, 필드가 거의 없고 긴 문자열만 있으면 이득이 작다. gzip을 JSON에 씌워도 이름은 어느 정도 줄어들지만, Protobuf는 **압축 전 wire**에서도 짧다.

### 크기 감각 (예시)

대략 위와 비슷한 한 건:

| 형식 | 대략 크기 | 비고 |
|---|---|---|
| JSON (camelCase, 공백 없음) | ~90–110 바이트 | 키 문자열이 반복 |
| Protobuf (prost) | ~40–60 바이트 | 태그+길이+값 |
| JSON + gzip | JSON보다 작아질 수 있음 | CPU·버퍼 비용, 작은 메시지에선 오버헤드 |

정확한 수치는 문자열 길이에 좌우된다. 아래 코드로 **같은 값을 두 형식으로 encode해 `len()`을 비교**하면 된다.

### prost 메시지 정의 (빌드 스크립트 없이 학습용)

```rust
// sender / receiver 공용으로 쓸 메시지 (학습용 — 태그 번호를 양쪽에서 동일하게)
#[derive(Clone, PartialEq, prost::Message)]
pub struct OrderEventProto {
    #[prost(string, tag = "1")]
    pub order_id: String,
    #[prost(string, tag = "2")]
    pub product_name: String,
    #[prost(uint32, tag = "3")]
    pub quantity: u32,
    #[prost(string, optional, tag = "4")]
    pub note: Option<String>,
}
```

실무에서는 `.proto` + `prost-build` / `tonic-build`로 생성한다. 태그 번호가 계약이므로 **한번 쓴 번호를 바꾸면 깨진다**.

### 크기 비교 유틸 (Sender나 바이너리 테스트)

```rust
use prost::Message;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderEventJson {
    order_id: String,
    product_name: String,
    quantity: u32,
    note: Option<String>,
}

fn compare_sizes() {
    let json_model = OrderEventJson {
        order_id: "RND-00001234".into(),
        product_name: "deskmat-xl".into(),
        quantity: 7,
        note: Some("gift wrap".into()),
    };
    let proto_model = OrderEventProto {
        order_id: json_model.order_id.clone(),
        product_name: json_model.product_name.clone(),
        quantity: json_model.quantity,
        note: json_model.note.clone(),
    };

    let json_bytes = serde_json::to_vec(&json_model).unwrap();
    let proto_bytes = proto_model.encode_to_vec();

    println!("json  len = {}", json_bytes.len());
    println!("proto len = {}", proto_bytes.len());
    println!(
        "proto is {:.1}% of json",
        100.0 * proto_bytes.len() as f64 / json_bytes.len() as f64
    );
}
```

### Receiver — Protobuf 수신 (axum-extra)

```rust
use axum::routing::post;
use axum_extra::protobuf::Protobuf;
use prost::Message;

// OrderEventProto 는 위와 동일 정의

async fn ingest_protobuf(
    State(state): State<AppState>,
    Protobuf(body): Protobuf<OrderEventProto>,
) -> impl IntoResponse {
    // proto → 기존 JSON 도메인 타입으로 변환해 저장 경로 재사용
    let event = OrderEvent {
        order_id: body.order_id,
        product_name: body.product_name,
        quantity: body.quantity,
        note: body.note,
    };
    // 검증·메모리·jsonl 저장은 ingest와 동일 함수로 빼서 호출하면 된다
    ingest_domain(state, event).await
}
```

라우트:

```rust
.route("/ingest", post(ingest))                 // JSON
.route("/ingest/protobuf", post(ingest_protobuf)) // Protobuf
```

참고 (팩트):

- `axum_extra::protobuf::Protobuf`는 body를 `prost::Message`로 디코딩한다.
- **요청 Content-Type을 강제하지 않는다** (문서 기준).
- 응답으로 `Protobuf(T)`를 주면 기본 `Content-Type`은 **`application/octet-stream`** 이다.
- 관례적으로 클라이언트가 `application/protobuf` 또는 `application/x-protobuf`를 쓰기도 하나, **단일 RFC 표준 MIME은 없다**.

### Sender — Protobuf 송신 (reqwest)

```rust
use bytes::Bytes;
use prost::Message;

async fn forward_protobuf(state: &AppState, event: &OrderEvent) -> reqwest::Result<reqwest::Response> {
    let proto = OrderEventProto {
        order_id: event.order_id.clone(),
        product_name: event.product_name.clone(),
        quantity: event.quantity,
        note: event.note.clone(),
    };
    let wire = proto.encode_to_vec();
    let url = format!("{}/ingest/protobuf", state.receiver_base.trim_end_matches('/'));

    state
        .http
        .post(url)
        .header("content-type", "application/x-protobuf")
        .body(Bytes::from(wire))
        .send()
        .await
}
```

JSON 경로의 `.json()` 대신 **encode된 바이트**를 body로 넣는다.  
응답 ack를 JSON으로 유지하면, 페이로드 절감은 **업스트림 요청 body**에만 적용된다. ack까지 proto로 바꾸면 더 줄어든다.

### JSON vs Protobuf 선택

| | JSON | Protobuf |
|---|---|---|
| 가독성 | 좋음 (curl·로그) | 나쁨 |
| 스키마 | 느슨 / serde | 태그·타입 고정 |
| 브라우저 앱 | 기본 친화 | 디코딩 라이브러리 필요 |
| 서비스 간 고빈도 RPC | 가능 | 흔히 유리 (gRPC/tonic) |
| 필드명 wire 비용 | 있음 | 없음(숫자 태그) |

이 저장소처럼 브라우저 ↔ Axum이면 JSON이 기본이다.  
**Sender↔Receiver 같은 내부 링크**에서 트래픽·CPU가 보이면 Protobuf(또는 gRPC)를 검토한다.

### 체크포인트

- [ ] 같은 필드로 json len / proto len 출력해 보기
- [ ] `/ingest/protobuf`로 바이너리 POST 후 jsonl에 남는지
- [ ] MIME이 비표준 관례임을 알기

---

## 12. 실전 실행 순서

```bash
# 터미널 1 — receiver 크레이트 루트에서
cargo run
# data/events.jsonl 생성됨

# 터미널 2
cargo run
# sender

# 터미널 3
curl -s http://127.0.0.1:3000/health
curl -s http://127.0.0.1:4000/health

# 수동 JSON
curl -s -X POST http://127.0.0.1:4000/send \
  -H 'content-type: application/json' \
  -d '{
    "orderId": "ORD-100",
    "productName": "typeboard-sticker",
    "quantity": 3,
    "note": "manual"
  }'

# 랜덤 여러 번
curl -s -X POST http://127.0.0.1:4000/send/random
curl -s -X POST http://127.0.0.1:4000/send/random

curl -s http://127.0.0.1:3000/events
cat data/events.jsonl
```

---

## 13. curl / 프론트 검증

### 필드명 실패

```bash
curl -i -X POST http://127.0.0.1:3000/ingest \
  -H 'content-type: application/json' \
  -d '{"order_id":"X","product_name":"y","quantity":1}'
# rename_all=camelCase 이면 400
```

### fetch

```javascript
const res = await fetch("http://127.0.0.1:4000/send/random", { method: "POST" });
const data = await res.json();
console.log(data.sent, data.ack);
```

다른 origin이면 CORS 필요. curl만 쓰면 불필요.

---

## 14. 코드 검토 메모

이전에 있던 초안 대비 **수정·주의**한 점:

| 항목 | 내용 |
|---|---|
| EnvFilter | `EnvFilter::new("info")`로 명시 (문자열 From은 되지만 의도를 분명히) |
| Content-Type 중복 | reqwest `.json()`만 사용. 수동 header 제거 |
| IngestAck | Receiver가 추가한 `persisted`를 Sender가 `#[serde(default)]`로 수용 |
| 파일 저장 | lock과 `tokio::fs` await 분리 |
| 랜덤 송신 | `/send/random` + 응답의 `sent`로 값 확인 |
| Protobuf | 크기 이득은 “항상”이 아니라 스키마·필드 구성에 달림. MIME은 비표준 관례 |

컴파일·실행 전제: workspace에서 `receiver` 작업 디렉터리 기준으로 `data/`가 생긴다. sender만 다른 cwd에서 실행해도 Receiver 쪽 경로에 쓰인다.

---

## 15. 자주 하는 실수

1. 다른 서버로 JSON 보내는데 reqwest 없이 Axum만 사용  
2. Serialize / Deserialize 한쪽만 derive  
3. camelCase / snake_case 불일치  
4. Content-Type 누락  
5. Mutex lock 중 await (특히 파일 쓰기)  
6. 요청마다 `Client::new()`  
7. 202를 처리 완료로 오해  
8. 업스트림 실패를 무조건 500으로 뭉갬 → Sender는 502가 맞을 때가 많음  
9. Protobuf로 바꿨는데 태그 번호를 Sender/Receiver에서 다르게 줌  
10. JSON과 Protobuf를 같은 경로·같은 extractor로 혼용  

---

## 16. 체크리스트

- [ ] Receiver: `/ingest`, `/events`, jsonl 저장, `persisted`
- [ ] Sender: `/send`, `/send/random`, 응답 `sent`·`ack`
- [ ] `rename_all = "camelCase"`
- [ ] 랜덤 여러 번 → 파일 줄 수 증가
- [ ] (선택) proto encode 길이 < json 길이 확인
- [ ] (선택) `/ingest/protobuf` 경로 동작

---

## 부록 A. 이 저장소 `api`와

`api`의 `GET /api/hello`는 JSON을 **주는** 쪽에 가깝다. 연습:

1. `POST /api/orders` 수신 + 파일/메모리 저장  
2. 작은 sender에서 랜덤 POST  
3. 내부 링크만 Protobuf로 바꿔 크기 비교  

---

## 부록 B. 학습 순서

1. Receiver JSON + jsonl (5절)  
2. Sender 수동 `/send` (6절)  
3. `/send/random` + 파일 확인 (6·12절)  
4. 비동기·202 (7–8절)  
5. Protobuf 크기 비교·선택적 경로 (11절)  

---

끝. 랜덤 송신으로 계약을 반복 검증하고, 수신 측 jsonl로 증거를 남기며, 같은 필드를 Protobuf로 바꿔 **왜·얼마나** 줄어드는지까지 한 흐름으로 보면 Axum JSON 서버의 실무 감각이 잡힌다.
