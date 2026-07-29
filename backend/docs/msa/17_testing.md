# 17. 테스트 — 통합 · 계약 · 부하 · 장애 주입

[03 §9](./03_service_anatomy.md)의 테스트 전략은 표 3줄입니다.

| 유닛 | service에 mock repo trait |
| 통합 | `sqlx::test` + testcontainers / 로컬 PG |
| HTTP | `axum::http` + `oneshot` 또는 `reqwest` |

방향은 맞습니다. **실제로 굴러가는 코드**를 채웁니다.

그리고 03의 이 조언은 계속 유효합니다:

> 초반에는 repo를 `trait UserRepoPort`로 두지 않아도 됩니다.
> 테스트가 아파지면 그때 포트를 추출하세요.

---

## 1. 무엇을 테스트할 것인가

### 1-1. MSA에서 피라미드가 바뀝니다

모놀리스의 테스트 피라미드(유닛 多 → E2E 少)를 그대로 가져오면 **틀린 것을 많이 테스트하게 됩니다.**

```
모놀리스           MSA
─────────         ─────────
E2E    ▲          E2E       ▲    ← 여전히 적게
통합  ███         계약     ████   ← 새로 추가, 중요
유닛 █████        통합    █████   ← 비중 증가 (DB/HTTP 경계가 위험)
                  유닛    ████
```

**이유:** MSA의 버그는 대부분 **경계**에서 납니다.

| 버그 위치 | 유닛 테스트가 잡나 |
|---|---|
| 도메인 계산 로직 | ✅ |
| SQL 쿼리 오류 | ❌ (모의 repo는 실제 SQL을 안 돌림) |
| 트랜잭션 경계 | ❌ |
| 직렬화 불일치 | ❌ |
| 서비스 간 계약 위반 | ❌ |
| 타임아웃/재시도 동작 | ❌ |

**모의(mock)를 많이 쓸수록 "모의를 테스트"하게 됩니다.**

### 1-2. 우선순위

```
1. 통합 테스트 (실제 DB)     ← 가장 높은 투자 대비 효과
2. 계약 테스트               ← 16 §6
3. 유닛 테스트 (순수 로직)
4. 부하 테스트               ← 배포 전 1회
5. 장애 주입                 ← 분기 1회
6. E2E                       ← 최소한만 (느리고 불안정)
```

---

## 2. 통합 테스트 — 실제 DB로

### 2-1. `sqlx::test`가 대부분을 해줍니다

`sqlx::test` 매크로는 **테스트마다 독립 DB를 만들고, 마이그레이션을 돌리고, 끝나면 지웁니다.**

```rust
// tests/user_repo.rs
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn insert_and_find_by_email(pool: PgPool) -> sqlx::Result<()> {
    let repo = UserRepo::new(pool.clone());

    let created = repo.insert("ada@example.com", "Ada", "hash").await?;
    assert_eq!(created.email, "ada@example.com");

    let found = repo.find_by_email("ada@example.com").await?;
    assert_eq!(found.unwrap().id, created.id);

    let missing = repo.find_by_email("nobody@example.com").await?;
    assert!(missing.is_none());
    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_email_violates_unique(pool: PgPool) -> sqlx::Result<()> {
    let repo = UserRepo::new(pool);
    repo.insert("ada@example.com", "Ada", "h").await?;

    let err = repo.insert("ada@example.com", "Ada2", "h").await.unwrap_err();

    // 04 §7의 에러 매핑이 실제로 동작하는지 검증
    match err {
        sqlx::Error::Database(db) => {
            assert_eq!(db.constraint(), Some("users_email_key"));
        }
        other => panic!("expected unique violation, got {other:?}"),
    }
    Ok(())
}
```

**두 번째 테스트가 특히 가치 있습니다.**
[04 §7](./04_database.md)의 `db.constraint() == Some("users_email_key")` 는 **제약 이름 문자열에 의존**합니다.
마이그레이션에서 제약 이름이 바뀌면 조용히 깨지고, 409 대신 500이 나갑니다.
이 테스트가 그걸 잡습니다.

### 2-2. 실행 환경

```bash
# 테스트용 PG (별도 포트로 개발용과 분리)
docker run -d --name test-pg -p 5433:5432 \
  -e POSTGRES_PASSWORD=test -e POSTGRES_USER=test -e POSTGRES_DB=test \
  postgres:16-alpine

export DATABASE_URL=postgres://test:test@localhost:5433/test
cargo test
```

`sqlx::test`가 `DATABASE_URL`의 서버에 임시 DB를 만들고 지웁니다.
**테스트가 서로 격리**되므로 병렬 실행이 안전합니다.

### 2-3. testcontainers — CI에서 더 깔끔

```toml
[dev-dependencies]
testcontainers = "0.23"
testcontainers-modules = { version = "0.11", features = ["postgres", "redis"] }
```

```rust
// tests/common/mod.rs
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::{postgres::Postgres, redis::Redis};

pub struct TestEnv {
    pub pool: PgPool,
    pub redis: ConnectionManager,
    // 컨테이너 핸들을 살려둬야 테스트 중 종료되지 않음
    _pg: ContainerAsync<Postgres>,
    _redis: ContainerAsync<Redis>,
}

impl TestEnv {
    pub async fn new() -> anyhow::Result<Self> {
        let pg = Postgres::default().start().await?;
        let url = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            pg.get_host_port_ipv4(5432).await?
        );
        let pool = PgPoolOptions::new().max_connections(5).connect(&url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        let redis_c = Redis::default().start().await?;
        let redis_url = format!("redis://127.0.0.1:{}", redis_c.get_host_port_ipv4(6379).await?);
        let redis = ConnectionManager::new(redis::Client::open(redis_url)?).await?;

        Ok(Self { pool, redis, _pg: pg, _redis: redis_c })
    }
}
```

컨테이너 시작에 2~5초가 걸리므로 **테스트 전체에서 한 번만** 만드세요.

```rust
use tokio::sync::OnceCell;
static ENV: OnceCell<TestEnv> = OnceCell::const_new();

pub async fn shared_env() -> &'static TestEnv {
    ENV.get_or_init(|| async { TestEnv::new().await.unwrap() }).await
}
```

### 2-4. 테스트 간 격리

공유 DB를 쓰면 격리가 필요합니다. 두 방법:

```rust
// (A) 트랜잭션 롤백 — 빠르지만 커밋 동작을 테스트 못 함
#[tokio::test]
async fn test_with_rollback() {
    let env = shared_env().await;
    let mut tx = env.pool.begin().await.unwrap();

    UserRepo::insert(&mut *tx, "a@b.com", "A", "h").await.unwrap();
    // 커밋하지 않음 → 자동 롤백

    // ⚠️ 서비스 코드가 내부에서 begin/commit 하면 이 방식은 못 씀
}

// (B) 테스트마다 스키마 분리 — 느리지만 완전
async fn isolated_schema(pool: &PgPool) -> String {
    let schema = format!("test_{}", Uuid::now_v7().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}")).execute(pool).await.unwrap();
    sqlx::query(&format!("SET search_path TO {schema}")).execute(pool).await.unwrap();
    schema
}
```

**(A)를 기본으로, 트랜잭션 로직 자체를 테스트할 때만 (B).**

### 2-5. 픽스처는 빌더로

```rust
pub struct UserFixture {
    email: String,
    name: String,
    roles: Vec<String>,
}

impl Default for UserFixture {
    fn default() -> Self {
        Self {
            // 유니크 제약 충돌 방지
            email: format!("user-{}@test.local", Uuid::now_v7().simple()),
            name: "Test User".into(),
            roles: vec!["user".into()],
        }
    }
}

impl UserFixture {
    pub fn admin(mut self) -> Self { self.roles = vec!["admin".into()]; self }
    pub fn email(mut self, e: &str) -> Self { self.email = e.into(); self }

    pub async fn create(self, pool: &PgPool) -> UserRow {
        UserRepo::new(pool.clone())
            .insert(&self.email, &self.name, "$argon2id$dummy")
            .await.unwrap()
    }
}

// 사용 — 의도가 드러납니다
let admin = UserFixture::default().admin().create(&pool).await;
```

**랜덤 이메일이 핵심입니다.** 고정값을 쓰면 병렬 테스트가 유니크 제약으로 충돌합니다.

---

## 3. HTTP 레벨 테스트

### 3-1. `lib.rs`의 `build_app`을 실제로 활용

[03 §6](./03_service_anatomy.md)이 `build_app`을 `lib.rs`에 두라고 한 이유가 여기서 나옵니다.

```toml
[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

```rust
// tests/api.rs
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;   // oneshot

async fn test_app() -> axum::Router {
    let env = shared_env().await;
    user_service::build_app_with(env.pool.clone(), env.redis.clone(), test_settings()).await.unwrap()
}

#[tokio::test]
async fn register_returns_201_with_user_body() {
    let app = test_app().await;

    let res = app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/v1/users")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email":"ada@test.local","name":"Ada","password":"correct-horse-battery"}"#))
            .unwrap()
    ).await.unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);

    let bytes = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["email"], "ada@test.local");
    assert!(body.get("password_hash").is_none(), "해시가 응답에 노출됨");
}
```

마지막 단언이 중요합니다. **응답에 민감 필드가 없는지**를 테스트로 고정하세요.
DTO를 리팩터링하다 `UserRow`를 그대로 반환하는 실수를 잡습니다.

### 3-2. 에러 규약 테스트

[10_errata §3](./10_errata.md), [§12-7](./10_errata.md)이 만든 규약을 지키게 합니다.

```rust
#[tokio::test]
async fn errors_always_use_json_envelope() {
    let app = test_app().await;

    let cases = [
        // (요청, 기대 상태, 기대 error 코드)
        (json_req("POST", "/v1/users", r#"{"broken"#), 400, "bad_request"),   // 파싱 실패
        (get_req("/v1/users/not-a-uuid"), 400, "bad_request"),                 // Path 거절
        (get_req("/v1/users/018f4a2b-0000-7000-8000-000000000000"), 404, "not_found"),
        (get_req("/v1/admin/stats"), 401, "unauthorized"),
    ];

    for (req, expected_status, expected_code) in cases {
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status().as_u16(), expected_status);

        // content-type이 반드시 JSON
        assert_eq!(
            res.headers().get("content-type").unwrap(),
            "application/json",
            "에러 응답이 JSON이 아님 — 커스텀 extractor 누락?"
        );

        let body: serde_json::Value = read_json(res).await;
        assert_eq!(body["error"], expected_code);
        assert!(body["trace_id"].is_string());
    }
}
```

**첫 번째와 두 번째 케이스가 [10_errata §12-7](./10_errata.md)을 검증합니다.**
커스텀 extractor 없이 기본 `Json`/`Path`를 쓰면 `text/plain`이 나와 실패합니다.

### 3-3. 5xx가 내부 정보를 흘리지 않는지

```rust
#[tokio::test]
async fn internal_errors_do_not_leak_details() {
    // DB를 일부러 끊는다
    let app = app_with_broken_db().await;

    let res = app.oneshot(get_req("/v1/users/018f4a2b-0000-7000-8000-000000000000"))
        .await.unwrap();

    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body: serde_json::Value = read_json(res).await;
    let msg = body["message"].as_str().unwrap();

    // 10_errata §3
    assert_eq!(msg, "internal server error");
    for leak in ["postgres", "relation", "connection", "sqlx", "password"] {
        assert!(!msg.to_lowercase().contains(leak), "내부 정보 유출: {msg}");
    }
}
```

이런 테스트가 있으면 **보안 규약이 코드 변경에도 유지됩니다.**

### 3-4. 인증 테스트

```rust
fn auth_header(user_id: Uuid, roles: &[&str]) -> String {
    let jwt = test_jwt_issuer();
    format!("Bearer {}", jwt.issue(user_id, "t@test.local",
        roles.iter().map(|s| s.to_string()).collect(), "user-service").unwrap())
}

#[tokio::test]
async fn cannot_read_other_users_order() {
    let app = test_app().await;
    let (alice, bob) = (Uuid::now_v7(), Uuid::now_v7());
    let order = create_order_for(alice).await;

    let res = app.oneshot(
        Request::builder()
            .uri(format!("/v1/orders/{}", order.id))
            .header("authorization", auth_header(bob, &["user"]))
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    // 06 §7의 "리소스 소유권은 서비스 내부에서"
    // 403보다 404가 나은 경우가 많다 — 리소스 존재 자체를 숨김
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn spoofed_internal_header_is_ignored() {
    let app = test_app().await;

    let res = app.oneshot(
        Request::builder()
            .uri("/v1/users/me")
            .header("x-internal-user-id", Uuid::now_v7().to_string())   // 토큰 없이 헤더만
            .body(Body::empty()).unwrap()
    ).await.unwrap();

    // 12 §1 — 헤더가 아니라 토큰을 믿어야 한다
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
```

**마지막 테스트를 반드시 넣으세요.** [10_errata §1](./10_errata.md)의 치명적 취약점에 대한 회귀 방지입니다.

---

## 4. 외부 의존성 모킹

### 4-1. wiremock

```toml
[dev-dependencies]
wiremock = "0.6"
```

```rust
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::{method, path}};

#[tokio::test]
async fn order_creation_fails_when_user_not_found() {
    let mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/users/018f4a2b-0000-7000-8000-000000000000"))
        .respond_with(ResponseTemplate::new(404)
            .set_body_json(json!({"error":"not_found","message":"user not found"})))
        .mount(&mock).await;

    let client = UserClient::new(mock.uri());
    let svc = OrderService::new(pool, client);

    let err = svc.create(cmd).await.unwrap_err();
    assert!(matches!(err, OrderError::UserNotFound));
}
```

### 4-2. 회복탄력성 동작 테스트

[11 §5~6](./11_resilience.md)의 코드가 실제로 동작하는지 확인합니다.
**이걸 테스트하지 않으면 장애 때 처음 알게 됩니다.**

```rust
#[tokio::test]
async fn client_times_out_and_does_not_hang() {
    let mock = MockServer::start().await;
    Mock::given(method("GET")).and(path_regex(r"^/v1/users/.*"))
        .respond_with(ResponseTemplate::new(200)
            .set_delay(Duration::from_secs(10)))   // 느린 응답
        .mount(&mock).await;

    let client = UserClient::new(mock.uri());   // 2초 타임아웃

    let start = Instant::now();
    let err = client.get_user(Uuid::now_v7(), Deadline::in_secs(5)).await.unwrap_err();

    assert!(matches!(err, ClientError::Timeout));
    assert!(start.elapsed() < Duration::from_secs(3), "타임아웃이 동작하지 않음");
}

#[tokio::test]
async fn circuit_breaker_opens_after_threshold() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock).await;

    let client = UserClient::with_breaker(mock.uri(), 3);   // 3회 실패 시 open

    for _ in 0..3 {
        let _ = client.get_user(Uuid::now_v7(), Deadline::in_secs(5)).await;
    }

    // 4번째는 네트워크를 타지 않고 즉시 실패해야 함
    let start = Instant::now();
    let err = client.get_user(Uuid::now_v7(), Deadline::in_secs(5)).await.unwrap_err();

    assert!(matches!(err, ClientError::CircuitOpen));
    assert!(start.elapsed() < Duration::from_millis(10), "서킷이 열리지 않음");
    assert_eq!(mock.received_requests().await.unwrap().len(), 3, "열린 뒤에도 호출됨");
}

#[tokio::test]
async fn post_is_not_retried() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)                          // 정확히 1번만
        .mount(&mock).await;

    let _ = client.create_something().await;
    // 07 §2 — POST는 재시도 금지. drop 시 expect가 검증됨
}
```

### 4-3. 멱등성 테스트

```rust
#[tokio::test]
async fn same_idempotency_key_creates_one_order() {
    let app = test_app().await;
    let key = Uuid::now_v7().to_string();

    let make_req = || Request::builder()
        .method("POST").uri("/v1/orders")
        .header("authorization", auth_header(user_id, &["user"]))
        .header("idempotency-key", &key)
        .header("content-type", "application/json")
        .body(Body::from(order_json())).unwrap();

    let r1 = app.clone().oneshot(make_req()).await.unwrap();
    let r2 = app.clone().oneshot(make_req()).await.unwrap();

    assert_eq!(r1.status(), StatusCode::CREATED);
    assert_eq!(r2.status(), StatusCode::CREATED);   // 재생(replay)

    let b1: Value = read_json(r1).await;
    let b2: Value = read_json(r2).await;
    assert_eq!(b1["id"], b2["id"], "같은 키인데 다른 주문이 생성됨");

    assert_eq!(count_orders(&pool, user_id).await, 1);
}

#[tokio::test]
async fn concurrent_requests_with_same_key_create_one_order() {
    // 05 §7의 레이스 (10_errata §8)를 실제로 검증
    let app = test_app().await;
    let key = Uuid::now_v7().to_string();

    let (r1, r2) = tokio::join!(
        app.clone().oneshot(idem_req(&key)),
        app.clone().oneshot(idem_req(&key)),
    );

    let statuses = [r1.unwrap().status(), r2.unwrap().status()];
    // 하나는 201, 다른 하나는 201(replay) 또는 409(in-flight)
    assert!(statuses.contains(&StatusCode::CREATED));
    assert_eq!(count_orders(&pool, user_id).await, 1, "동시 요청으로 중복 생성됨");
}
```

**두 번째 테스트가 [10_errata §8](./10_errata.md)의 버그를 직접 잡습니다.**
`SET NX` 없이 구현하면 여기서 2건이 나옵니다.

---

## 5. 메시징 테스트

### 5-1. outbox

```rust
#[sqlx::test(migrations = "./migrations")]
async fn order_creation_writes_outbox_in_same_transaction(pool: PgPool) {
    let svc = OrderService::new(pool.clone(), stub_user_client());

    let order = svc.create(valid_cmd()).await.unwrap();

    let events: Vec<OutboxRow> = sqlx::query_as(
        "SELECT * FROM outbox WHERE aggregate_id = $1"
    ).bind(order.id).fetch_all(&pool).await.unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].topic, "order.created");
    assert!(events[0].published_at.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_order_writes_no_outbox_event(pool: PgPool) {
    let svc = OrderService::new(pool.clone(), failing_user_client());

    let _ = svc.create(valid_cmd()).await.unwrap_err();

    // 롤백됐으므로 주문도 이벤트도 없어야 함
    assert_eq!(count_rows(&pool, "orders").await, 0);
    assert_eq!(count_rows(&pool, "outbox").await, 0);
}
```

두 번째가 **outbox 패턴의 핵심 보장**을 검증합니다.

### 5-2. 워커 동작

```rust
#[sqlx::test(migrations = "./migrations")]
async fn worker_retries_with_backoff_then_marks_dead(pool: PgPool) {
    let publisher = AlwaysFailingPublisher::new();
    let worker = OutboxWorker::new(pool.clone(), Arc::new(publisher.clone()),
        OutboxConfig { max_attempts: 3, ..Default::default() });

    insert_outbox_event(&pool, "order.created").await;

    for _ in 0..3 {
        worker.process_batch().await.unwrap();
        // 백오프를 건너뛰기 위해 시간을 앞당김
        sqlx::query("UPDATE outbox SET next_attempt_at = now()")
            .execute(&pool).await.unwrap();
    }

    let row: OutboxRow = fetch_one_outbox(&pool).await;
    assert_eq!(row.attempts, 3);
    assert!(row.dead, "max_attempts 초과 시 dead 처리되어야 함");
    assert!(row.last_error.is_some());

    // dead 이벤트가 다음 배치를 막지 않는지
    insert_outbox_event(&pool, "order.cancelled").await;
    let processed = worker.process_batch().await.unwrap();
    assert_eq!(processed, 1, "dead 이벤트가 뒤를 막고 있음");
}
```

마지막 단언이 [14 §4-1](./14_messaging_ops.md)의 head-of-line blocking을 검증합니다.

### 5-3. 소비자 멱등성

```rust
#[sqlx::test(migrations = "./migrations")]
async fn duplicate_event_is_processed_once(pool: PgPool) {
    let event = order_created_envelope();

    let r1 = handle_order_created(&pool, &event).await.unwrap();
    let r2 = handle_order_created(&pool, &event).await.unwrap();

    assert_eq!(r1, ProcessOutcome::Processed);
    assert_eq!(r2, ProcessOutcome::AlreadyProcessed);
    assert_eq!(count_rows(&pool, "notification_log").await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_duplicate_events_processed_once(pool: PgPool) {
    let event = order_created_envelope();

    let (a, b) = tokio::join!(
        handle_order_created(&pool, &event),
        handle_order_created(&pool, &event),
    );

    assert!(a.is_ok() && b.is_ok());
    assert_eq!(count_rows(&pool, "notification_log").await, 1);
}
```

### 5-4. 이벤트 직렬화 호환

```rust
#[test]
fn old_event_payloads_still_deserialize() {
    // 프로덕션에서 실제로 발행됐던 형태를 고정
    let v1 = r#"{"order_id":"018f4a2b-0000-7000-8000-000000000000",
                 "user_id":"018f4a2c-0000-7000-8000-000000000000",
                 "total_cents":12000,"currency":"KRW"}"#;

    // coupon_code 필드가 나중에 추가됐지만 옛 payload도 읽혀야 함
    let ev: OrderCreated = serde_json::from_str(v1).unwrap();
    assert_eq!(ev.total_cents, 12000);
    assert!(ev.coupon_code.is_none());
}

#[test]
fn unknown_fields_are_ignored() {
    // 생산자가 새 필드를 추가해도 옛 소비자가 깨지지 않아야 함
    let future = r#"{"order_id":"018f...","user_id":"018f...",
                     "total_cents":1,"currency":"KRW","brand_new_field":"x"}"#;
    assert!(serde_json::from_str::<OrderCreated>(future).is_ok());
}
```

**두 번째가 [14 §8-2](./14_messaging_ops.md)의 `deny_unknown_fields` 금지를 강제합니다.**

---

## 6. 마이그레이션 테스트

[13](./13_data_evolution.md)의 규칙을 CI로 옮깁니다.

```rust
#[sqlx::test]
async fn migrations_are_idempotent(pool: PgPool) {
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();   // 두 번 돌려도 OK
}

#[test]
fn migration_files_follow_naming_convention() {
    for entry in std::fs::read_dir("./migrations").unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(name.ends_with(".sql"), "SQL이 아닌 파일: {name}");
        assert!(
            name.chars().take(14).all(|c| c.is_ascii_digit()),
            "타임스탬프 접두사가 없음: {name}"
        );
    }
}

#[test]
fn no_dangerous_statements_without_marker() {
    // 위험 구문에는 리뷰 표시를 강제
    let dangerous = ["DROP COLUMN", "DROP TABLE", "ALTER COLUMN", "SET NOT NULL"];

    for entry in std::fs::read_dir("./migrations").unwrap() {
        let path = entry.unwrap().path();
        let sql = std::fs::read_to_string(&path).unwrap().to_uppercase();

        for stmt in dangerous {
            if sql.contains(stmt) && !sql.contains("-- REVIEWED: BREAKING") {
                panic!(
                    "{:?}: '{}' 는 파괴적 변경입니다.\n\
                     13_data_evolution §2의 expand/contract를 확인하고,\n\
                     의도한 것이면 '-- REVIEWED: BREAKING <이유>' 주석을 추가하세요.",
                    path, stmt
                );
            }
        }
        // 인덱스는 CONCURRENTLY 확인
        if sql.contains("CREATE INDEX") && !sql.contains("CONCURRENTLY")
            && !sql.contains("-- REVIEWED: SMALL TABLE")
        {
            panic!("{:?}: CREATE INDEX에 CONCURRENTLY가 없습니다.", path);
        }
    }
}
```

**이런 린트가 리뷰어의 기억력보다 훨씬 믿을 만합니다.**

### 6-1. 마이그레이션 소요 시간 측정

```rust
#[tokio::test]
#[ignore]   // cargo test -- --ignored 로 수동 실행
async fn migration_completes_within_budget_on_production_sized_data() {
    let pool = connect_to_staging_clone().await;   // 프로덕션 크기 복제본

    let start = Instant::now();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_secs(300),
        "마이그레이션이 {}초 걸림 — 배포 타임아웃 위험", elapsed.as_secs());
}
```

[13 §7-4](./13_data_evolution.md)의 *"프로덕션 크기 데이터로 테스트했는가"* 를 자동화한 것입니다.

---

## 7. 부하 테스트

### 7-1. 무엇을 알아내려는가

목적 없이 돌리면 숫자만 나옵니다.

| 질문 | 방법 |
|---|---|
| 최대 처리량은? | 부하를 올리며 지연이 꺾이는 지점 찾기 |
| `ConcurrencyLimit`을 얼마로? | [11 §4-3](./11_resilience.md) |
| 커넥션 풀이 충분한가? | 부하 중 `db_pool_connections{state="idle"}` 관측 |
| 과부하 시 우아하게 실패하나? | 한계 초과 부하 → 503이 나오나 OOM이 나나 |
| 메모리 누수가 있나? | 30분 지속 부하 → RSS 추이 |

### 7-2. 도구

```bash
# oha — Rust 제작, 설치 간단, 로컬 확인용
oha -z 60s -c 50 --latency-correction \
  -H "authorization: Bearer $TOKEN" \
  http://localhost:8080/v1/orders

# k6 — 시나리오 작성, CI 통합
k6 run loadtest.js
```

```javascript
// loadtest.js
import http from 'k6/http';
import { check } from 'k6';

export const options = {
  stages: [
    { duration: '30s', target: 50 },    // 워밍업
    { duration: '2m',  target: 200 },   // 정상 부하
    { duration: '1m',  target: 500 },   // 스파이크
    { duration: '30s', target: 0 },     // 정리
  ],
  thresholds: {
    // SLO를 그대로 임계치로 (15 §9-1)
    'http_req_duration{expected_response:true}': ['p(99)<300'],
    'http_req_failed': ['rate<0.01'],
  },
};

export default function () {
  const res = http.get(`${__ENV.BASE_URL}/v1/orders`, {
    headers: { Authorization: `Bearer ${__ENV.TOKEN}` },
  });
  check(res, {
    'status is 200 or 503': (r) => r.status === 200 || r.status === 503,
    'no 500': (r) => r.status !== 500,
  });
}
```

**`503은 허용, 500은 불허`** 가 좋은 기준입니다.
503은 [11 §4](./11_resilience.md)의 load shedding이 의도대로 동작한 것이고, 500은 버그입니다.

### 7-3. 부하 중에 봐야 할 것

부하 테스트의 가치는 **숫자가 아니라 그때의 대시보드**에 있습니다.

```
[ ] p50/p95/p99 지연 곡선의 꺾이는 지점
[ ] http_requests_in_flight vs ConcurrencyLimit
[ ] load_shed_total 이 언제부터 증가하나
[ ] db_pool_connections{state="idle"} 이 0에 붙나
[ ] 메모리 RSS 추이 (누수 여부)
[ ] CPU throttling (11 §7)
[ ] 부하 종료 후 회복 시간
```

마지막이 특히 중요합니다. **부하가 끝났는데 회복이 안 되면** 큐가 쌓였거나 리소스가 샜다는 뜻입니다.

### 7-4. 언제 돌리나

```
- 주요 릴리스 전 (필수)
- 아키텍처 변경 후
- 정기 (월 1회) — 회귀 감지
- CI 매 PR ❌ (느리고 불안정, 환경 편차가 큼)
```

CI에는 **스모크 수준의 짧은 부하**만 두고, 본격 테스트는 스테이징에서 별도로 하세요.

---

## 8. 장애 주입

[01 §7](./01_architecture.md)이 실패 모드를 표로 설계했습니다. **그 표를 검증해야 합니다.**
설계만 하고 검증하지 않은 실패 모드는 대부분 실제로는 다르게 동작합니다.

### 8-1. 로컬에서 (docker compose)

```bash
#!/usr/bin/env bash
# scripts/chaos.sh
set -euo pipefail
BASE=http://127.0.0.1:8080

expect_status() {
  local desc="$1" expected="$2" url="$3"
  local actual
  actual=$(curl -s -o /dev/null -w '%{http_code}' "$url" || echo "000")
  if [[ "$actual" == "$expected" ]]; then
    echo "✅ $desc → $actual"
  else
    echo "❌ $desc → 기대 $expected, 실제 $actual"; exit 1
  fi
}

echo "=== 1. Redis 다운 → DB 폴백 (01 §7) ==="
docker compose stop redis
expect_status "캐시 없이 조회 성공" 200 "$BASE/v1/users/$USER_ID"
docker compose start redis && sleep 3

echo "=== 2. catalog 다운 → 정책대로 (01 §7) ==="
docker compose stop catalog-service
# 정책이 "거절"이면 422, "미검증 통과"면 201
expect_status "주문 생성" 422 "$BASE/v1/orders"
docker compose start catalog-service && sleep 3

echo "=== 3. user DB 다운 → /ready 실패, /health 유지 (11 §1-6) ==="
docker compose stop postgres-user
sleep 6
expect_status "liveness 유지" 200 "http://127.0.0.1:3001/health"
expect_status "readiness 실패" 503 "http://127.0.0.1:3001/ready"
docker compose start postgres-user && sleep 5

echo "=== 4. notification 다운 → 주문은 성공, 이벤트는 적재 (01 §7) ==="
docker compose stop notification-service
create_order
BEFORE=$(pending_outbox_count)
docker compose start notification-service && sleep 10
AFTER=$(pending_outbox_count)
[[ "$AFTER" -lt "$BEFORE" ]] && echo "✅ 복구 후 이벤트 소진" || { echo "❌ 이벤트가 밀려 있음"; exit 1; }

echo "=== 5. graceful shutdown 중 요청 유실 없음 (11 §1) ==="
oha -z 20s -c 20 "$BASE/v1/users/$USER_ID" > /tmp/load.txt &
sleep 5
docker compose restart user-service
wait
grep -q "\[500\]" /tmp/load.txt && { echo "❌ 재시작 중 5xx 발생"; exit 1; }
echo "✅ 무중단 재시작"

echo "모든 카오스 시나리오 통과"
```

**5번이 [11 §1](./11_resilience.md) 전체를 검증합니다.** 이 한 줄이 통과하면 graceful shutdown이 실제로 동작하는 겁니다.

### 8-2. 네트워크 지연/손실 주입

```bash
# toxiproxy로 지연 주입
toxiproxy-cli create -l 0.0.0.0:5433 -u postgres:5432 pg-proxy
toxiproxy-cli toxic add pg-proxy -t latency -a latency=2000

# DATABASE_URL을 프록시로 바꾸고 관찰:
# - statement_timeout이 동작하나 (13 §4-3)
# - /ready가 실패로 전환되나
# - 요청이 무한 대기하지 않나
```

```bash
# 패킷 손실 (Linux)
tc qdisc add dev eth0 root netem loss 10%
```

### 8-3. 정기 실행

```
- 로컬 카오스 스크립트: CI의 통합 잡에서 매 PR (빠름)
- 스테이징 장애 주입: 월 1회, 사람이 관찰
- 프로덕션 게임데이: 분기 1회, 계획된 시간에
```

**"장애 대응 절차가 문서대로 동작하는가"** 도 같이 확인하세요.
런북([15 §9-3](./15_observability_deep.md))이 실제로 쓸 만한지는 써봐야 압니다.

---

## 9. 테스트 인프라

### 9-1. cargo-nextest

```bash
cargo install cargo-nextest --locked
cargo nextest run --workspace
```

| 이점 | 설명 |
|---|---|
| 프로세스 격리 | 한 테스트의 패닉이 다른 테스트를 오염시키지 않음 |
| 병렬 실행 | 기본 `cargo test`보다 빠름 |
| 테스트별 타임아웃 | 멈춘 테스트가 CI를 영원히 붙들지 않음 |
| 재시도 | 불안정한 테스트를 격리해 파악 |
| 출력 | 실패한 테스트만 요약 |

```toml
# .config/nextest.toml
[profile.default]
retries = 0
slow-timeout = { period = "30s", terminate-after = 4 }   # 2분 후 강제 종료
failure-output = "immediate-final"

[profile.ci]
retries = { backoff = "exponential", count = 2, delay = "1s" }
fail-fast = false          # 전부 돌려서 실패 목록을 한 번에
```

**`terminate-after`가 중요합니다.** 데드락 난 테스트가 CI를 6시간 붙드는 사고를 막습니다.

### 9-2. 커버리지

```bash
cargo install cargo-llvm-cov --locked
cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
```

**커버리지 목표를 강제하지 마세요.** 숫자를 채우려고 의미 없는 테스트가 늘어납니다.
추이를 보는 용도로 쓰세요 — **급격히 떨어지면** 왜인지 물어보는 정도.

### 9-3. 불안정한(flaky) 테스트

```
불안정한 테스트 1개 = 팀 전체가 CI 실패를 무시하기 시작하는 시작점
```

발견 즉시:

```rust
#[ignore = "flaky: 타이밍 의존, ISSUE-123"]
```

로 격리하고 이슈를 만드세요. **고칠 때까지 CI를 빨갛게 두면 안 됩니다.**

흔한 원인:

| 원인 | 해결 |
|---|---|
| `sleep`으로 타이밍 맞추기 | 조건 폴링 또는 채널 대기 |
| 고정 포트 | `:0` 바인딩 후 실제 포트 조회 |
| 고정 데이터 (이메일 등) | 랜덤 (§2-5) |
| 테스트 간 순서 의존 | 격리 (§2-4) |
| 실제 시각 의존 | 시각을 주입 가능하게 |

```rust
// 시각을 주입 가능하게 — 만료·TTL 테스트가 안정적으로 됩니다
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
pub struct SystemClock;
pub struct FixedClock(pub DateTime<Utc>);
```

### 9-4. 로컬 실행 편의

```makefile
# Makefile 또는 justfile
test:            ## 빠른 테스트 (DB 불필요)
	cargo nextest run --workspace --lib

test-integration: up  ## 통합 테스트
	DATABASE_URL=$(TEST_DB) cargo nextest run --workspace --test '*'

test-all: test test-integration chaos

up:              ## 테스트 인프라 기동
	docker compose -f docker-compose.test.yml up -d --wait

chaos: up        ## 장애 주입
	./scripts/chaos.sh

coverage:
	cargo llvm-cov nextest --workspace --html
	open target/llvm-cov/html/index.html
```

**`--wait`가 유용합니다.** healthcheck가 통과할 때까지 기다려주므로
"컨테이너는 떴는데 PG가 아직 준비 안 됨"으로 인한 불안정이 사라집니다.

---

## 10. 테스트 전략 요약

| 테스트 | 개수 | 속도 | 잡는 것 | 어디서 |
|---|---|---|---|---|
| 유닛 (순수 로직) | 多 | ms | 계산 오류 | 매 저장 |
| 통합 (실제 DB) | 中 | 초 | SQL, 트랜잭션, 제약 | 매 커밋 |
| HTTP (oneshot) | 中 | 초 | 라우팅, 직렬화, 인증, 에러 규약 | 매 커밋 |
| 모킹 (wiremock) | 中 | ms | 타임아웃, 서킷, 재시도 | 매 커밋 |
| 계약 ([16 §6](./16_api_contract.md)) | 少 | 초 | 서비스 간 호환 | 매 PR |
| 마이그레이션 린트 | 少 | ms | 위험한 스키마 변경 | 매 PR |
| 카오스 (로컬) | 少 | 분 | 실패 모드 | 매 PR 또는 야간 |
| 부하 | 극소 | 분~시간 | 용량, 한계 동작 | 릴리스 전 |
| E2E | 극소 | 분 | 전체 흐름 | 배포 후 스모크 |

**[09 §6](./09_deploy.md)의 스모크 스크립트가 E2E 자리입니다.** 그 정도면 충분합니다.
E2E를 늘리면 느리고 불안정해서 아무도 안 보게 됩니다.

---

## 체크포인트

```
[ ] sqlx::test로 실제 DB에 대해 repo를 테스트한다
[ ] unique 제약 이름에 의존하는 에러 매핑에 테스트가 있다
[ ] build_app을 lib.rs에 노출해 oneshot 테스트가 가능하다
[ ] 응답에 민감 필드(password_hash)가 없음을 테스트한다
[ ] 모든 에러 응답이 JSON 규약을 지키는지 테스트한다
[ ] 5xx가 내부 정보를 흘리지 않는지 테스트한다
[ ] 헤더 스푸핑이 무시되는지 테스트한다 (12 §1 회귀 방지)
[ ] 리소스 소유권 검사에 테스트가 있다
[ ] 타임아웃/서킷브레이커/재시도 동작을 테스트한다
[ ] 동시 요청 멱등성을 테스트한다 (10_errata §8)
[ ] outbox가 같은 트랜잭션에 쓰이고 실패 시 롤백됨을 테스트한다
[ ] 워커의 백오프/dead 처리와 head-of-line 미차단을 테스트한다
[ ] 소비자 멱등성을 동시 실행으로 테스트한다
[ ] 옛 이벤트 payload가 역직렬화되는지 테스트한다
[ ] 마이그레이션 린트가 위험 구문을 막는다
[ ] 부하 테스트로 ConcurrencyLimit 값을 정했다
[ ] 카오스 스크립트가 01 §7의 실패 모드를 검증한다
[ ] graceful shutdown 중 5xx가 없음을 부하로 검증한다
[ ] cargo-nextest + 테스트 타임아웃을 쓴다
[ ] 불안정한 테스트를 즉시 격리하는 규칙이 있다
[ ] 픽스처가 랜덤 값을 써서 병렬 실행이 안전하다
```

---

다음: [18_cicd — 회귀를 사람이 잡지 않게](./18_cicd.md)
