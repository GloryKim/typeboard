# 04. Database — PostgreSQL + sqlx

Spring Data JPA / TypeORM 자리를 **sqlx + PostgreSQL**이 가져갑니다.  
MSA에서는 **서비스마다 DB(또는 스키마)를 분리**하는 것이 기본입니다.

---

## 1. Docker로 Postgres 올리기

`docker-compose.yml` 발췌 ([09](./09_deploy.md) 전체):

```yaml
services:
  postgres-user:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: user
      POSTGRES_PASSWORD: user
      POSTGRES_DB: user_db
    ports: ["5432:5432"]
    volumes: ["pg_user:/var/lib/postgresql/data"]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U user -d user_db"]
      interval: 5s
      timeout: 5s
      retries: 10

  postgres-order:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: order
      POSTGRES_PASSWORD: order
      POSTGRES_DB: order_db
    ports: ["5433:5432"]
    volumes: ["pg_order:/var/lib/postgresql/data"]
```

로컬 개발 비용을 줄이려면 **초기엔 인스턴스 하나 + DB만 분리**해도 됩니다.

```
postgres://user:user@localhost:5432/user_db
postgres://order:order@localhost:5432/order_db
```

프로덕션에서는 인스턴스/클러스터 분리를 재검토합니다.

---

## 2. 의존성

```toml
sqlx = { workspace = true }
# features: runtime-tokio, postgres, uuid, chrono, migrate
```

환경변수:

```bash
DATABASE_URL=postgres://user:user@localhost:5432/user_db
```

---

## 3. 마이그레이션

서비스 루트에:

```
services/user-service/
└── migrations/
    ├── 202607230001_create_users.sql
    └── 202607230002_create_indexes.sql
```

```sql
-- 202607230001_create_users.sql
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### CLI

```bash
# 설치
cargo install sqlx-cli --no-default-features --features rustls,postgres

# 생성
cd services/user-service
sqlx migrate add create_users

# 적용
DATABASE_URL=postgres://... sqlx migrate run

# 앱 기동 시 자동 적용 (권장)
```

```rust
sqlx::migrate!("./migrations")
    .run(&pool)
    .await?;
```

Nest의 `TypeOrmModule.forRoot({ synchronize: true })` 는 **쓰지 마세요**.  
항상 마이그레이션으로만 스키마를 바꿉니다.

---

## 4. 풀 생성

```rust
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

let pool = PgPoolOptions::new()
    .max_connections(settings.database.max_connections) // 예: 20
    .acquire_timeout(Duration::from_secs(5))
    .idle_timeout(Duration::from_secs(60))
    .connect(&settings.database.url)
    .await?;
```

### 커넥션 수 감각

| 환경 | 가이드 |
|---|---|
| 로컬 | 5~10 |
| 작은 서비스 | 10~20 |
| 큰 서비스 | PgBouncer 앞단 + 앱은 보수적으로 |

MSA에서 서비스 인스턴스 × max_connections 가 PG `max_connections`를 뚫지 않게 주의합니다.

---

## 5. 쿼리 스타일

### (A) 런타임 쿼리 — 빠른 개발

```rust
sqlx::query_as::<_, UserRow>("SELECT ... WHERE id = $1")
    .bind(id)
    .fetch_optional(&pool)
    .await?;
```

### (B) 매크로 쿼리 — 컴파일 타임 검증 (강력 추천)

```rust
let user = sqlx::query_as!(
    UserRow,
    r#"SELECT id, email, name, password_hash, created_at
       FROM users WHERE id = $1"#,
    id
)
.fetch_optional(&pool)
.await?;
```

요구사항:
- 빌드 머신에서 `DATABASE_URL` 접근 **또는**
- `cargo sqlx prepare` 로 `.sqlx` 오프라인 캐시 커밋

```bash
cargo sqlx prepare --workspace
# CI에서는 SQLX_OFFLINE=true
```

Spring의 쿼리 메서드 오타가 런타임에야 터지는 문제를 상당수 제거합니다.

---

## 6. 트랜잭션

```rust
let mut tx = pool.begin().await?;

sqlx::query("INSERT INTO orders ...")
    .execute(&mut *tx)
    .await?;

sqlx::query("INSERT INTO outbox ...")
    .execute(&mut *tx)
    .await?;

tx.commit().await?;
```

**Outbox 패턴** (주문 + 이벤트를 같은 TX에):

```sql
CREATE TABLE outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    topic TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ
);
```

발행 워커가 `published_at IS NULL` 을 폴링해 NATS/Kafka로 보냅니다.  
상세: [07_messaging](./07_messaging.md).

---

## 7. 에러 매핑

```rust
match repo.insert(...).await {
    Err(sqlx::Error::Database(db)) if db.constraint() == Some("users_email_key") => {
        Err(UserError::EmailTaken)
    }
    Err(e) => Err(UserError::Db(e)),
    Ok(v) => Ok(v),
}
```

unique violation → 409 Conflict 로 올리는 패턴은 Nest/Spring과 동일합니다.

---

## 8. 읽기 최적화 메모

- 목록은 항상 **키셋 페이지네이션** (`WHERE id < $1 ORDER BY id DESC LIMIT 20`)
- `SELECT *` 금지, 응답 DTO에 맞는 컬럼만
- 핫 키 조회는 Redis ([05](./05_redis.md))
- 검색/통계는 나중에 읽기 모델(별도 DB)로 분리

---

## 9. SeaORM을 쓰는 경우

팀이 Active Record / Relation 로딩에 익숙하면 SeaORM도 가능합니다.  
다만 MSA + 성능·명시성 관점에서는 **sqlx가 기본 추천**입니다.

| | sqlx | SeaORM |
|---|---|---|
| 제어 | SQL 명시 | 엔티티 중심 |
| 안전성 | `query!` 강함 | 런타임 실수 여지 |
| 학습곡선 | SQL 필요 | ORM 익숙하면 쉬움 |
| Axum 궁합 | 최상 | 좋음 |

---

## 10. 서비스별 스키마 예시

### user_db

```sql
users(id, email, name, password_hash, created_at, updated_at)
refresh_tokens(id, user_id, token_hash, expires_at)
```

### order_db

```sql
orders(id, user_id, status, total_cents, currency, created_at)
order_items(id, order_id, product_id, qty, unit_price_cents, product_name_snapshot)
outbox(id, topic, payload, created_at, published_at)
```

`product_name_snapshot` — 다른 서비스 JOIN 대신 **생성 시점 스냅샷**을 저장하는 실무 패턴입니다.

---

## 체크포인트

```
[ ] 서비스별 DATABASE_URL / DB(스키마)가 분리됐다
[ ] synchronize 없이 migrations만 사용한다
[ ] 기동 시 sqlx::migrate! 또는 CI migrate job이 있다
[ ] 풀 크기와 PG max_connections를 계산했다
[ ] 분산 트랜잭션 대신 로컬 TX + outbox를 검토했다
```

다음: [05_redis — 캐시·세션·락·레이트리밋](./05_redis.md)
