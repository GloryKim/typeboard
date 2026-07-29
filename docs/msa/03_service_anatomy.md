# 03. 서비스 해부 — Axum을 NestJS 모듈처럼

하나의 `user-service`를 NestJS의 `UserModule` + Spring의 `UserController/Service/Repository` 구조로 맞춥니다.

---

## 1. 레이어 역할

```
handlers/   ← Controller  (HTTP 입출력, DTO)
services/   ← Service     (유스케이스, 트랜잭션 경계)
repos/      ← Repository  (sqlx 쿼리)
state.rs    ← DI 컨테이너에 가까운 AppState
routes/     ← Router 조립 (@Module imports)
```

Axum에는 DI 컨테이너가 없으므로 **생성 시점에 조립**합니다.

```rust
let pool = PgPoolOptions::new().connect(&settings.database.url).await?;
let redis = redis::Client::open(settings.redis.url.as_str())?;
let users = UserRepo::new(pool.clone());
let user_svc = UserService::new(users, redis.clone());
let state = AppState { user_svc, settings };

let app = routes::router().with_state(state);
```

이게 Nest의 `providers: [UserService, UserRepo]` 와 같은 역할입니다.

---

## 2. 디렉터리

```
services/user-service/src/
├── main.rs
├── lib.rs              # router()를 pub로 — 통합 테스트용
├── config.rs
├── error.rs            # 서비스 고유 에러 → common::ApiError 매핑
├── state.rs
├── routes/
│   └── mod.rs
├── handlers/
│   ├── mod.rs
│   ├── health.rs
│   └── users.rs
├── services/
│   ├── mod.rs
│   └── user_service.rs
└── repos/
    ├── mod.rs
    └── user_repo.rs
```

---

## 3. `AppState` — DI 대체

```rust
// state.rs
use crate::services::user_service::UserService;
use crate::config::Settings;

#[derive(Clone)]
pub struct AppState {
    pub user_svc: UserService,
    pub settings: Settings,
}
```

핸들러:

```rust
use axum::{extract::State, Json};
use crate::state::AppState;
use common::ApiResult;

pub async fn get_me(
    State(state): State<AppState>,
    // 인증 추출기는 06에서
) -> ApiResult<Json<UserResponse>> {
    let user = state.user_svc.find_by_id(user_id).await?;
    Ok(Json(user.into()))
}
```

`Clone`이 필요하므로 내부는 `Arc`/`PgPool`(이미 Arc성) / 싼 핸들로 구성합니다.

---

## 4. Repository

```rust
// repos/user_repo.rs
use sqlx::PgPool;
use uuid::Uuid;
use crate::error::UserError;

#[derive(Clone)]
pub struct UserRepo {
    pool: PgPool,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl UserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>(
            r#"SELECT id, email, name, password_hash, created_at
               FROM users WHERE email = $1"#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn insert(
        &self,
        email: &str,
        name: &str,
        password_hash: &str,
    ) -> Result<UserRow, sqlx::Error> {
        sqlx::query_as::<_, UserRow>(
            r#"INSERT INTO users (email, name, password_hash)
               VALUES ($1, $2, $3)
               RETURNING id, email, name, password_hash, created_at"#,
        )
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
    }
}
```

---

## 5. Service (유스케이스)

```rust
// services/user_service.rs
use crate::repos::user_repo::{UserRepo, UserRow};
use crate::error::UserError;
use redis::aio::ConnectionManager;
use uuid::Uuid;

#[derive(Clone)]
pub struct UserService {
    repo: UserRepo,
    redis: ConnectionManager,
}

impl UserService {
    pub fn new(repo: UserRepo, redis: ConnectionManager) -> Self {
        Self { repo, redis }
    }

    pub async fn register(
        &self,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<UserRow, UserError> {
        if self.repo.find_by_email(email).await?.is_some() {
            return Err(UserError::EmailTaken);
        }
        let hash = hash_password(password)?;
        let user = self.repo.insert(email, name, &hash).await?;
        // 캐시 워밍은 선택
        Ok(user)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<UserRow, UserError> {
        // Redis 캐시 패턴은 05에서
        self.repo
            .find_by_id(id)
            .await?
            .ok_or(UserError::NotFound)
    }
}
```

트랜잭션이 필요하면 Service에서 `pool.begin()` 후 repo에 `&mut tx`를 넘깁니다 ([04_database](./04_database.md)).

---

## 6. Handler + Router

```rust
// handlers/users.rs
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use common::{ApiError, ApiResult};

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub email: String,
    pub name: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<Json<UserResponse>> {
    let user = state
        .user_svc
        .register(&body.email, &body.name, &body.password)
        .await
        .map_err(|e| match e {
            UserError::EmailTaken => ApiError::Conflict("email taken".into()),
            other => ApiError::Internal(other.into()),
        })?;

    Ok(Json(UserResponse {
        id: user.id,
        email: user.email,
        name: user.name,
    }))
}
```

```rust
// routes/mod.rs
use axum::{routing::{get, post}, Router};
use crate::{handlers, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/v1/users", post(handlers::users::register))
        .route("/v1/users/{id}", get(handlers::users::get_by_id))
}
```

```rust
// main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let settings = Settings::load()?;
    let app = user_service::build_app(settings).await?;
    let listener = tokio::net::TcpListener::bind(&app_addr).await?;
    tracing::info!("user-service listening on {app_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
```

`build_app`을 `lib.rs`에 두면 테스트에서 서버 없이 라우터를 붙일 수 있습니다.

---

## 7. Tower 미들웨어 = Nest Interceptor / Guard

```rust
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
    timeout::TimeoutLayer,
    compression::CompressionLayer,
};
use std::time::Duration;

let app = router()
    .layer(TraceLayer::new_for_http())
    .layer(CompressionLayer::new())
    .layer(TimeoutLayer::with_status_code(
        axum::http::StatusCode::REQUEST_TIMEOUT,
        Duration::from_secs(15),
    ))
    .layer(CorsLayer::permissive()) // prod에선 제한
    .with_state(state);
```

인증 Guard는 extractor로:

```rust
// 개념만 — 구현은 06
async fn create_order(
    claims: AuthUser,          // 없으면 401
    State(state): State<AppState>,
    Json(body): Json<CreateOrder>,
) { ... }
```

---

## 8. Validation

Nest의 `class-validator` 대응:

- `validator` + `garde` 크레이트
- 또는 핸들러에서 명시 체크 후 `ApiError::BadRequest`

```toml
validator = { version = "0.19", features = ["derive"] }
```

```rust
#[derive(Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}
```

---

## 9. 테스트 전략

| 종류 | 방법 |
|---|---|
| 유닛 | service에 mock repo trait |
| 통합 | `sqlx::test` + testcontainers / 로컬 PG |
| HTTP | `axum::http` + `oneshot` 또는 `reqwest` against `TestServer` |

초반에는 repo를 `trait UserRepoPort`로 두지 않아도 됩니다.  
테스트가 아파지면 그때 포트(인터페이스)를 추출하세요. **과한 추상화는 Nest에서 모듈만 늘리는 것과 같습니다.**

---

## 10. Spring/Nest 패키징과의 감각 차이

| 그곳 | 여기 |
|---|---|
| 리플렉션 DI | 컴파일 타임 조립 |
| 런타임 프록시 AOP | Tower Layer / extractor |
| 거대한 ApplicationContext | 작은 `AppState` |
| 핫 리로드 쉬움 | `cargo watch -x run` |

대신 **타입이 컴파일에 전부 드러나서** 대규모에서 회귀가 적습니다.

---

## 체크포인트

```
[ ] handler는 HTTP만, SQL은 repo에만 있다
[ ] AppState로 의존성을 조립한다
[ ] Router가 /health 와 버전 프리픽스(/v1)를 가진다
[ ] TraceLayer + TimeoutLayer를 기본으로 켠다
[ ] lib.rs에서 build_app을 노출해 테스트 가능하게 한다
```

다음: [04_database — PostgreSQL + sqlx 실전](./04_database.md)
