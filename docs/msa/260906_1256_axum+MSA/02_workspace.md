# 02. Cargo Workspace — NestJS 모노레포처럼 구성하기

NestJS의 `apps/` + `libs/` , Spring의 multi-module Gradle과 같은 자리를  
Rust에서는 **Cargo Workspace**가 맡습니다.

---

## 1. 목표 트리

```
platform/
├── Cargo.toml                      # [workspace]
├── rust-toolchain.toml             # (선택) 채널 고정
├── .env.example
├── docker-compose.yml
├── crates/
│   ├── common/                     # 에러, config, id, http helpers
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── auth-jwt/                   # JWT encode/decode 공유
│       ├── Cargo.toml
│       └── src/lib.rs
├── services/
│   ├── gateway/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── user-service/
│   │   ├── Cargo.toml
│   │   ├── migrations/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs              # 테스트용 라이브러리화 (권장)
│   │       ├── config.rs
│   │       ├── error.rs
│   │       ├── state.rs
│   │       ├── routes/
│   │       ├── handlers/
│   │       ├── services/           # 도메인 유스케이스
│   │       └── repos/              # sqlx 쿼리
│   ├── order-service/
│   ├── catalog-service/
│   └── notification-service/
└── scripts/
    └── migrate-all.sh
```

---

## 2. 루트 `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
  "crates/common",
  "crates/auth-jwt",
  "services/gateway",
  "services/user-service",
  "services/order-service",
  "services/catalog-service",
  "services/notification-service",
]

[workspace.package]
edition = "2021"
license = "MIT"
version = "0.1.0"

[workspace.dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip", "timeout"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["serde", "v4", "v7"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "migrate"] }
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }
jsonwebtoken = "9"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
config = "0.14"
dotenvy = "0.15"

# 내부 크레이트
common = { path = "crates/common" }
auth-jwt = { path = "crates/auth-jwt" }
```

버전을 workspace에서 한 번에 고정하면 NestJS의 `package.json` resolutions와 같은 효과를 냅니다.

서비스 `Cargo.toml` 예:

```toml
[package]
name = "user-service"
version.workspace = true
edition.workspace = true

[[bin]]
name = "user-service"
path = "src/main.rs"

[dependencies]
common = { workspace = true }
auth-jwt = { workspace = true }
axum = { workspace = true }
tokio = { workspace = true }
sqlx = { workspace = true }
redis = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tower-http = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
dotenvy = { workspace = true }
config = { workspace = true }
```

---

## 3. `crates/common` — 공유해도 되는 것

```rust
// crates/common/src/lib.rs
pub mod error;
pub mod config;
pub mod request_id;
pub mod time;

pub use error::{ApiError, ApiResult};
```

### 에러 타입 (Spring `ProblemDetail` / Nest `HttpException` 대응)

```rust
// crates/common/src/error.rs
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };

        let body = ErrorBody {
            error: code.into(),
            message: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
```

### Config 로더

```rust
// crates/common/src/config.rs
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct HttpConfig {
    pub addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}
```

서비스별로 `Settings`를 확장:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub http: HttpConfig,
    pub database: DbConfig,
    pub redis: RedisConfig,
    pub jwt_secret: String,
}
```

로딩은 `config` 크레이트 + env:

```rust
impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        let s = config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            // DATABASE__URL, HTTP__ADDR 형태
            .build()?;
        Ok(s.try_deserialize()?)
    }
}
```

또는 단순하게 `std::env::var`만 써도 초반에는 충분합니다.

---

## 4. 공유하면 안 되는 것

| 공유 OK | 공유 NG |
|---|---|
| `ApiError`, JWT claims, Money | `CreateOrder` 유스케이스 |
| request-id 미들웨어 | sqlx 모델 전체 dump |
| OpenAPI 공통 스키마 일부 | “편의상” 다른 서비스 repo 함수 |
| protobuf / 이벤트 envelope | DB connection을 다른 서비스에 넘김 |

공통 크레이트가 비대해지면 다시 모놀리스입니다.  
**타입이 필요하면 이벤트 스키마 크레이트만** 따로 두세요 (`crates/events`).

---

## 5. 빌드 · 실행 루틴

```bash
# 전체 체크
cargo check --workspace

# 특정 서비스만
cargo run -p user-service

# 릴리즈
cargo build -p gateway --release

# 테스트
cargo test -p user-service
cargo test --workspace
```

개발 시 여러 서비스 동시 실행:

```bash
# 예: cargo-watch 또는 just / Makefile
cargo run -p gateway &
cargo run -p user-service &
cargo run -p order-service &
```

로컬은 Docker Compose로 PG/Redis만 띄우고, 서비스는 호스트에서 `cargo run` 하는 조합이 디버깅에 좋습니다. ([09_deploy](./09_deploy.md))

---

## 6. NestJS libs 와의 매핑

| NestJS | Cargo |
|---|---|
| `apps/api` | `services/gateway` |
| `apps/user` | `services/user-service` |
| `libs/common` | `crates/common` |
| `libs/auth` | `crates/auth-jwt` |
| `tsconfig paths` | `workspace.dependencies` path |

---

## 체크포인트

```
[ ] workspace members에 서비스/크레이트가 등록됐다
[ ] 버전은 workspace.dependencies로 통일했다
[ ] common에는 인프라/타입만 있고 도메인 서비스가 없다
[ ] 각 서비스가 독립 바이너리로 cargo run 된다
```

다음: [03_service_anatomy — 한 서비스 내부를 Nest 모듈처럼 짜기](./03_service_anatomy.md)
