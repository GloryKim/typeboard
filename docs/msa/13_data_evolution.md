# 13. 데이터 진화 — 무중단 스키마 변경과 PostgreSQL 운영

[04_database](./04_database.md)는 **스키마를 만드는 법**을 다룹니다.
이 문서는 **이미 트래픽이 흐르는 스키마를 바꾸는 법**을 다룹니다.

이 둘은 완전히 다른 문제입니다.

> Spring의 `@Transactional`, Flyway의 baseline/repair, JPA의 낙관적 잠금(`@Version`)이
> 여기서는 전부 손으로 만들어야 하는 것들입니다.

---

## 1. 왜 마이그레이션이 위험한가

### 1-1. 롤링 업데이트 중에는 두 버전이 공존합니다

```
t=0   v1 파드 3개  ← 모두 옛 스키마 기준 코드
t=10  마이그레이션 실행 (컬럼 추가/삭제)
t=20  v2 파드 1개 + v1 파드 2개   ← 여기가 위험 구간
t=60  v2 파드 3개
```

**t=20에서 v1 코드가 v2 스키마를 봅니다.** 그리고 롤백하면 v2 코드가 사라진 상태에서 v2 스키마가 남습니다.

[09 §5](./09_deploy.md)는 *"배포 Job이 앱보다 먼저 migrate"* 라고 정확히 말합니다.
그런데 **먼저 migrate하면 옛 코드가 새 스키마와 마주칩니다.** 그래서 마이그레이션 자체가 호환되어야 합니다.

### 1-2. 실제로 터지는 것들

| 변경 | 무슨 일이 나는가 |
|---|---|
| `DROP COLUMN status` | v1 코드의 `SELECT status`가 전부 실패 |
| `ALTER COLUMN ... SET NOT NULL` | v1이 NULL로 INSERT → 실패 |
| `RENAME COLUMN a TO b` | v1은 a를, v2는 b를 찾음 → 양쪽 다 깨짐 |
| `ALTER TYPE varchar → int` | 테이블 전체 재작성 + ACCESS EXCLUSIVE 락 |
| `ADD COLUMN ... NOT NULL DEFAULT` | PG 11+ 는 빠르지만, volatile default면 전체 재작성 |
| `CREATE INDEX` (CONCURRENTLY 없이) | **쓰기가 전부 차단됨** (수 분~수십 분) |
| `ADD FOREIGN KEY` | 검증 위해 양 테이블 잠금 |

큰 테이블에서 `CREATE INDEX`를 그냥 돌리면 **그 시간 동안 서비스가 멈춥니다.**

---

## 2. Expand / Contract 패턴

모든 파괴적 변경을 **3단계 배포**로 쪼갭니다.

```
┌─ Expand ─────────────────────────────────────┐
│ 새 구조를 추가한다. 옛 구조는 그대로 둔다.      │
│ → 옛 코드도 새 코드도 모두 동작                │
├─ Migrate ────────────────────────────────────┤
│ 코드를 새 구조로 전환한다. 데이터를 이관한다.    │
├─ Contract ───────────────────────────────────┤
│ 옛 구조를 제거한다. 되돌릴 수 없다.             │
│ → 반드시 별도 배포, 며칠~몇 주 후               │
└──────────────────────────────────────────────┘
```

### 2-1. 예: 컬럼 이름 바꾸기 (`name` → `display_name`)

**❌ 절대 하면 안 되는 것**

```sql
ALTER TABLE users RENAME COLUMN name TO display_name;
```

배포 순간 v1 파드 전부가 500을 뱉습니다.

**✅ 3단계로**

```sql
-- 1) EXPAND — 배포 #1
ALTER TABLE users ADD COLUMN display_name TEXT;

-- 기존 데이터 백필 (배치로, §3 참고)
UPDATE users SET display_name = name WHERE display_name IS NULL;

-- 이중 쓰기 트리거 — 옛 코드가 name만 써도 display_name이 채워짐
CREATE OR REPLACE FUNCTION sync_display_name() RETURNS trigger AS $$
BEGIN
    IF NEW.display_name IS NULL THEN NEW.display_name := NEW.name; END IF;
    IF NEW.name IS NULL THEN NEW.name := NEW.display_name; END IF;
    RETURN NEW;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER users_sync_display_name
    BEFORE INSERT OR UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION sync_display_name();
```

```rust
// 코드 배포 #1: 읽기는 옛 컬럼, 쓰기는 양쪽
// (트리거가 있으면 코드 변경 없이도 됨)
```

```sql
-- 2) MIGRATE — 배포 #2
-- 코드가 display_name만 읽고 쓴다. 트리거가 name을 계속 채워줌.
ALTER TABLE users ALTER COLUMN display_name SET NOT NULL;
```

```sql
-- 3) CONTRACT — 배포 #3 (며칠 후, 롤백 가능성이 사라진 뒤)
DROP TRIGGER users_sync_display_name ON users;
DROP FUNCTION sync_display_name();
ALTER TABLE users DROP COLUMN name;
```

배포가 3번 필요합니다. **번거롭지만 이게 무중단의 값입니다.**

### 2-2. 변경 유형별 안전 절차

| 하고 싶은 것 | 안전한 절차 |
|---|---|
| 컬럼 추가 | nullable로 추가 → 백필 → (필요시) NOT NULL |
| 컬럼 삭제 | 코드에서 참조 제거 배포 → 다음 배포에서 DROP |
| 컬럼 이름 변경 | §2-1의 3단계 |
| 타입 변경 | 새 컬럼 추가 → 이중 쓰기 → 백필 → 전환 → 옛 컬럼 삭제 |
| NOT NULL 추가 | 백필 완료 확인 → `NOT VALID` CHECK → `VALIDATE` → SET NOT NULL |
| 인덱스 추가 | `CREATE INDEX CONCURRENTLY` |
| 인덱스 삭제 | `DROP INDEX CONCURRENTLY` |
| FK 추가 | `ADD CONSTRAINT ... NOT VALID` → 나중에 `VALIDATE CONSTRAINT` |
| 테이블 이름 변경 | 새 테이블 + 뷰 또는 이중 쓰기. 단순 RENAME 금지 |

### 2-3. `NOT NULL`을 안전하게 추가하기

`ALTER COLUMN SET NOT NULL`은 **전체 테이블 스캔 + ACCESS EXCLUSIVE 락**입니다.

```sql
-- 1) 검증하지 않는 제약을 먼저 (락이 짧다)
ALTER TABLE users ADD CONSTRAINT users_display_name_not_null
    CHECK (display_name IS NOT NULL) NOT VALID;

-- 2) 별도 트랜잭션에서 검증 (SHARE UPDATE EXCLUSIVE — 읽기/쓰기 허용)
ALTER TABLE users VALIDATE CONSTRAINT users_display_name_not_null;

-- 3) PG 12+ 는 검증된 CHECK가 있으면 SET NOT NULL이 스캔을 생략한다
ALTER TABLE users ALTER COLUMN display_name SET NOT NULL;
ALTER TABLE users DROP CONSTRAINT users_display_name_not_null;
```

### 2-4. 인덱스는 반드시 CONCURRENTLY

```sql
CREATE INDEX CONCURRENTLY orders_user_created
    ON orders (user_id, created_at DESC);
```

주의사항:

- **트랜잭션 안에서 실행할 수 없습니다.** sqlx 마이그레이션은 기본적으로 트랜잭션으로 감싸므로 별도 처리가 필요합니다.
- 실패하면 `INVALID` 인덱스가 남습니다. 반드시 확인하고 지우세요.

```sql
-- 무효 인덱스 탐지
SELECT indexrelid::regclass AS index_name
FROM pg_index WHERE NOT indisvalid;
```

sqlx에서는 파일명에 표시를 넣고 별도 실행하거나, `-- sqlx:no-transaction` 관례를 팀에서 정하세요.
(sqlx 버전별로 지원이 다르므로 확인 필요. 안 되면 이 마이그레이션만 수동/스크립트로 분리)

---

## 3. 대용량 백필

### 3-1. 한 번에 UPDATE 하지 마세요

```sql
-- ❌ 1,000만 행
UPDATE users SET display_name = name WHERE display_name IS NULL;
```

- 트랜잭션이 수십 분 → 그동안 VACUUM 정지, WAL 폭증
- 모든 행에 락 → 서비스 정지
- 실패하면 전부 롤백 → 처음부터

### 3-2. 배치로 나눠서

```rust
pub async fn backfill_display_name(pool: &PgPool, shutdown: CancellationToken)
    -> anyhow::Result<()>
{
    const BATCH: i64 = 1_000;
    let mut total = 0i64;

    loop {
        if shutdown.is_cancelled() {
            tracing::info!(total, "backfill interrupted, safe to resume");
            return Ok(());
        }

        // 매 배치가 독립 트랜잭션 — 언제 멈춰도 안전
        let updated = sqlx::query_scalar::<_, i64>(
            r#"
            WITH batch AS (
                SELECT id FROM users
                WHERE display_name IS NULL
                ORDER BY id
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE users u SET display_name = u.name
            FROM batch b WHERE u.id = b.id
            RETURNING 1
            "#,
        )
        .bind(BATCH)
        .fetch_all(pool)
        .await?
        .len() as i64;

        total += updated;
        if updated == 0 {
            tracing::info!(total, "backfill complete");
            return Ok(());
        }

        tracing::info!(total, "backfill progress");
        // 프로덕션 부하에 양보 — 이게 핵심
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

**포인트:**

| 요소 | 이유 |
|---|---|
| 작은 배치 (1,000) | 락 시간과 WAL 크기 제한 |
| `FOR UPDATE SKIP LOCKED` | 실제 트래픽과 충돌 회피 |
| 배치 간 sleep | 복제 지연·IO 포화 방지 |
| 중단 가능 | 언제든 멈추고 이어서 |
| 진행률 로그 | 몇 시간짜리 작업의 상태 파악 |

### 3-3. 복제 지연을 감시하며

읽기 레플리카가 있으면 백필이 복제를 밀어냅니다.

```rust
async fn wait_for_replication(pool: &PgPool) -> anyhow::Result<()> {
    loop {
        let lag: Option<f64> = sqlx::query_scalar(
            "SELECT EXTRACT(EPOCH FROM (now() - pg_last_xact_replay_timestamp()))"
        ).fetch_one(pool).await?;

        match lag {
            Some(l) if l > 5.0 => {
                tracing::warn!(lag_secs = l, "replication lag high, pausing backfill");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            _ => return Ok(()),
        }
    }
}
```

---

## 4. 커넥션 풀과 타임아웃

### 4-1. 04 §4의 계산을 구체적으로

> MSA에서 서비스 인스턴스 × max_connections 가 PG `max_connections`를 뚫지 않게 주의합니다.

맞는 말인데 실제 숫자가 필요합니다.

```
PG max_connections = 100 (기본값)
- superuser 예약           : 3
- 모니터링/백업 도구        : 5
- 마이그레이션 Job          : 5
= 앱이 쓸 수 있는 것        : 87

user-service  replicas 3 × pool 10 = 30
order-service replicas 5 × pool 10 = 50
합계 80  ← 87 이하 ✅

여기서 HPA가 order를 10개로 늘리면 → 130 ❌ 접속 거부
```

**HPA 최대 replica × pool ≤ 가용 커넥션** 으로 계산해야 합니다.
안 그러면 **트래픽이 늘어난 바로 그 순간 DB 접속이 실패**합니다. 최악의 타이밍입니다.

### 4-2. 풀이 클수록 좋은 게 아닙니다

```
PostgreSQL은 커넥션당 프로세스를 하나 만듭니다 (스레드 아님)
→ 커넥션 100개 = 프로세스 100개 = 메모리 수백 MB + 컨텍스트 스위칭
→ 실제 처리량은 코어 수 부근에서 정점을 찍고 그 뒤로는 떨어집니다
```

경험 공식:

```
pool_size = ((core_count × 2) + effective_spindle_count) / 앱 인스턴스 수
```

4코어 PG + SSD 기준 총 커넥션 ~10개면 충분한 경우가 많습니다.
**"느리니까 풀을 늘리자"는 대개 반대 방향입니다.**

### 4-3. 전체 타임아웃 설정 ([11 §5-4](./11_resilience.md)의 상세)

```rust
use sqlx::{postgres::PgPoolOptions, ConnectOptions, Executor};

let pool = PgPoolOptions::new()
    .max_connections(10)
    .min_connections(2)                              // 콜드 스타트 완화
    .acquire_timeout(Duration::from_secs(3))         // 풀 대기 상한
    .idle_timeout(Duration::from_secs(600))
    .max_lifetime(Duration::from_secs(1800))         // 커넥션 주기적 재생성
    .test_before_acquire(true)                       // 죽은 커넥션 배제
    .after_connect(|conn, _| Box::pin(async move {
        conn.execute(
            "SET statement_timeout = '3s';
             SET lock_timeout = '2s';
             SET idle_in_transaction_session_timeout = '10s';
             SET application_name = 'user-service';"
        ).await?;
        Ok(())
    }))
    .connect(&url)
    .await?;
```

`application_name`은 사소해 보이지만, `pg_stat_activity`에서 **어떤 서비스가 문제 쿼리를 돌리는지** 즉시 보입니다.
서비스가 5개 넘어가면 이거 없이는 장애 대응이 안 됩니다.

`max_lifetime`은 로드밸런서 뒤의 PG(RDS 페일오버 등)에서 **오래된 커넥션이 죽은 노드를 붙들고 있는 것**을 막습니다.

### 4-4. 배치 작업은 별도 풀로

```rust
pub struct AppState {
    pub pool: PgPool,        // API용: 작은 풀, 짧은 타임아웃
    pub batch_pool: PgPool,  // 배치용: 커넥션 2개, statement_timeout 5분
}
```

[11 §6-1](./11_resilience.md)의 bulkhead와 같은 논리입니다.
**백필 작업이 API 요청의 커넥션을 먹으면 안 됩니다.**

---

## 5. PgBouncer 함정

### 5-1. 04 §4가 권하지만 주의가 필요합니다

> 큰 서비스 | PgBouncer 앞단 + 앱은 보수적으로

PgBouncer의 **transaction pooling 모드**에서는 sqlx의 기본 동작이 깨집니다.

```
sqlx는 prepared statement를 캐싱합니다 (성능 최적화)
  → "이 커넥션에 statement S1을 준비했다"
PgBouncer transaction 모드는 매 트랜잭션마다 다른 백엔드 커넥션을 배정합니다
  → 다음 요청이 S1이 없는 커넥션으로 감
  → ERROR: prepared statement "sqlx_s_1" does not exist
```

**증상이 고약합니다.** 부하가 낮으면 우연히 같은 커넥션에 배정되어 잘 되다가,
부하가 오르면 랜덤하게 실패합니다. 재현이 안 됩니다.

### 5-2. 해결

```rust
use sqlx::postgres::PgConnectOptions;
use std::str::FromStr;

let opts = PgConnectOptions::from_str(&url)?
    .statement_cache_capacity(0);   // ← prepared statement 캐시 비활성화

let pool = PgPoolOptions::new().max_connections(10).connect_with(opts).await?;
```

또는 **session pooling 모드**를 쓰되, 그러면 PgBouncer의 이점(커넥션 다중화)이 크게 줄어듭니다.

### 5-3. 모드별 정리

| 모드 | 커넥션 절약 | prepared stmt | LISTEN/NOTIFY | 세션 변수 | 권장 |
|---|---|---|---|---|---|
| session | 낮음 | ✅ | ✅ | ✅ | 안전하지만 이점 적음 |
| transaction | 높음 | ❌ (캐시 끄면 OK) | ❌ | ❌ | **실무 기본** + 캐시 off |
| statement | 최고 | ❌ | ❌ | ❌ | 멀티 문장 트랜잭션 불가 |

transaction 모드에서 **`SET`이 유지되지 않는다**는 점도 중요합니다.
§4-3의 `after_connect` 설정이 PgBouncer 뒤에서는 적용되지 않을 수 있습니다.
그 경우 PgBouncer의 `server_reset_query` 또는 PG 사용자 기본값으로 설정하세요.

```sql
ALTER ROLE user_service SET statement_timeout = '3s';
ALTER ROLE user_service SET lock_timeout = '2s';
```

**PgBouncer는 정말 필요할 때만 넣으세요.** 커넥션이 부족하지 않다면 복잡도만 늘어납니다.

---

## 6. 트랜잭션 경계

### 6-1. sqlx에는 `@Transactional`이 없습니다

Spring에서는 애노테이션 하나로 됐지만, 여기서는 `&mut tx`를 손으로 넘깁니다.
그래서 **repo 함수 시그니처를 처음부터 executor 제네릭으로** 만드세요.

```rust
use sqlx::{Executor, Postgres};

impl UserRepo {
    /// 풀에서도, 트랜잭션에서도 호출 가능
    pub async fn find_by_id<'e, E>(exec: E, id: Uuid) -> Result<Option<UserRow>, sqlx::Error>
    where E: Executor<'e, Database = Postgres> {
        sqlx::query_as::<_, UserRow>("SELECT ... WHERE id = $1")
            .bind(id).fetch_optional(exec).await
    }
}

// 호출
UserRepo::find_by_id(&pool, id).await?;          // 단독
UserRepo::find_by_id(&mut *tx, id).await?;       // 트랜잭션 안
```

[03 §4](./03_service_anatomy.md)처럼 `&self.pool`을 내부에 고정하면 **나중에 트랜잭션이 필요해질 때 전부 고쳐야 합니다.**
처음부터 이 형태로 시작하세요.

### 6-2. 트랜잭션 안에서 하면 안 되는 것

```rust
let mut tx = pool.begin().await?;
OrderRepo::insert(&mut *tx, &cmd).await?;

// ❌ 외부 HTTP 호출 — 2초 걸리면 그동안 락과 커넥션을 붙들고 있음
let user = user_client.get_user(cmd.user_id).await?;

// ❌ Redis 호출 — 같은 이유
// ❌ 이벤트 publish — 커밋 전에 발행하면 롤백 시 유령 이벤트

tx.commit().await?;
```

**트랜잭션은 최대한 짧게.** 외부 호출은 트랜잭션 밖에서 하고, 이벤트는 outbox로 넘깁니다 ([14](./14_messaging_ops.md)).

```rust
// ✅ 올바른 순서
let user = user_client.get_user(cmd.user_id).await?;   // 밖에서 먼저

let mut tx = pool.begin().await?;
let order = OrderRepo::insert(&mut *tx, &cmd, &user).await?;
OutboxRepo::push(&mut *tx, "order.created", payload).await?;   // DB 쓰기만
tx.commit().await?;                                             // 짧게 끝
```

### 6-3. 격리 수준

PostgreSQL 기본은 `READ COMMITTED`입니다. 대부분 충분하지만, 알아야 할 것:

```rust
// 잔액 확인 후 차감 — READ COMMITTED에서는 경쟁 조건 발생
let balance = get_balance(&mut *tx, user_id).await?;   // 100
if balance >= amount {
    deduct(&mut *tx, user_id, amount).await?;          // 동시 요청 2개가 둘 다 통과
}
```

세 가지 해법:

```rust
// (A) 행 잠금 — 가장 간단
sqlx::query("SELECT balance FROM accounts WHERE user_id = $1 FOR UPDATE")

// (B) 원자적 UPDATE — 가장 빠름
sqlx::query(
    "UPDATE accounts SET balance = balance - $1
     WHERE user_id = $2 AND balance >= $1
     RETURNING balance"
)   // 0행 반환 = 잔액 부족

// (C) 직렬화 격리 — 가장 엄격, 재시도 필요
let mut tx = pool.begin().await?;
sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE").execute(&mut *tx).await?;
// serialization_failure(40001) 발생 시 재시도 루프 필수
```

**(B)를 우선 검토하세요.** 락도 재시도도 필요 없습니다.

### 6-4. 낙관적 잠금

동시 수정 충돌을 사용자에게 알려야 하는 경우 (JPA의 `@Version`):

```sql
ALTER TABLE orders ADD COLUMN version INT NOT NULL DEFAULT 0;
```

```rust
let updated = sqlx::query(
    "UPDATE orders SET status = $1, version = version + 1
     WHERE id = $2 AND version = $3"
)
.bind(new_status).bind(order_id).bind(expected_version)
.execute(&mut *tx).await?
.rows_affected();

if updated == 0 {
    return Err(OrderError::Conflict);   // 409 — 클라이언트가 다시 읽고 재시도
}
```

REST에서는 `ETag` + `If-Match` 헤더로 노출하는 게 정석입니다.

### 6-5. 데드락 재시도

두 트랜잭션이 서로의 락을 기다리면 PG가 하나를 죽입니다 (SQLSTATE `40P01`).

```rust
pub async fn with_retry<F, Fut, T>(mut f: F) -> Result<T, sqlx::Error>
where F: FnMut() -> Fut, Fut: Future<Output = Result<T, sqlx::Error>> {
    for attempt in 0..3 {
        match f().await {
            Err(sqlx::Error::Database(db))
                if matches!(db.code().as_deref(), Some("40001") | Some("40P01")) =>
            {
                tracing::warn!(attempt, code = ?db.code(), "serialization/deadlock, retrying");
                tokio::time::sleep(Duration::from_millis(50 << attempt)).await;
            }
            other => return other,
        }
    }
    Err(sqlx::Error::PoolTimedOut)
}
```

**예방이 더 중요합니다: 항상 같은 순서로 락을 잡으세요.**
`orders → order_items` 순서를 팀 규칙으로 정하고 지키면 데드락이 거의 사라집니다.

---

## 7. 마이그레이션 운영

### 7-1. 앱 기동 시 migrate의 함정

[04 §3](./04_database.md)은 `sqlx::migrate!`를 권하고, [09 §5](./09_deploy.md)는 프로덕션에서 별도 Job을 권합니다.
**09가 맞습니다.** 이유:

```
replicas 3개가 동시에 부팅
→ 3개가 동시에 sqlx::migrate! 실행
→ sqlx의 advisory lock으로 1개만 진행, 나머지 2개는 대기
→ 마이그레이션이 5분 걸리면 2개 파드가 5분간 부팅 대기
→ startupProbe 타임아웃 → K8s가 재시작 → 무한 루프
```

advisory lock이 정합성은 지켜주지만 **가용성은 안 지켜줍니다.**

### 7-2. 권장 구조

```yaml
# 배포 전 Job
apiVersion: batch/v1
kind: Job
metadata: { name: user-service-migrate-{{ .Release.Revision }} }
spec:
  backoffLimit: 2
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: migrate
          image: user-service:{{ .Values.image.tag }}
          command: ["user-service", "migrate"]   # 서브커맨드
          env:
            - name: APP_DATABASE__URL
              valueFrom: { secretKeyRef: { name: user-db, key: url } }
```

```rust
// main.rs — 서브커맨드로 분리
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load()?;

    match std::env::args().nth(1).as_deref() {
        Some("migrate") => {
            let pool = PgPoolOptions::new().max_connections(1)
                .connect(&settings.database.url).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            tracing::info!("migrations applied");
            return Ok(());
        }
        Some("verify-migrations") => {
            // 앱이 기대하는 버전과 DB 상태가 일치하는지만 확인
            return verify_schema_version(&settings).await;
        }
        _ => {}
    }

    serve(settings).await
}
```

앱은 **마이그레이션을 실행하지 않고 검증만** 합니다.

```rust
async fn verify_schema_version(settings: &Settings) -> anyhow::Result<()> {
    let pool = connect(settings).await?;
    let applied: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM _sqlx_migrations WHERE success = true"
    ).fetch_one(&pool).await?;

    let expected = sqlx::migrate!("./migrations").iter().count() as i64;
    anyhow::ensure!(
        applied >= expected,
        "스키마 버전 불일치: 적용 {applied} < 기대 {expected}. migrate Job을 먼저 실행하세요."
    );
    Ok(())
}
```

**로컬/테스트는 앱 기동 시 migrate, 프로덕션은 Job + 검증.** 환경변수로 분기하세요.

### 7-3. 롤백 가능한 마이그레이션

sqlx는 down 마이그레이션을 지원하지만, **실무에서는 거의 쓰지 않습니다.**

```
데이터가 있는 상태에서 DROP COLUMN을 되돌릴 수 없습니다.
컬럼은 복구되지만 데이터는 영영 사라졌습니다.
```

**진짜 롤백 전략은 §2의 expand/contract입니다.**
Contract 단계 전까지는 코드만 되돌리면 되고, Contract 이후에는 되돌리지 않습니다.

```
배포 #1 (expand)   → 롤백 가능 ✅
배포 #2 (migrate)  → 롤백 가능 ✅ (옛 컬럼이 아직 있음)
배포 #3 (contract) → 롤백 불가 ❌ (그래서 충분히 기다린 뒤에)
```

### 7-4. 마이그레이션 리뷰 체크리스트

PR 템플릿에 넣으세요.

```
[ ] 이 변경이 옛 버전 코드와 호환되는가? (롤링 중 공존)
[ ] 락을 오래 잡는 구문이 있는가? (ALTER TYPE, 인덱스, NOT NULL)
[ ] 인덱스 생성에 CONCURRENTLY가 있는가?
[ ] 큰 테이블에 UPDATE/DELETE가 있는가? → 배치로 분리
[ ] contract(삭제)라면 expand가 몇 배포 전에 나갔는가?
[ ] 롤백하면 어떻게 되는가?
[ ] 프로덕션 크기의 데이터로 테스트했는가? (소요 시간 측정)
```

특히 마지막이 중요합니다. **개발 DB의 1,000행에서 0.1초인 쿼리가 프로덕션 1,000만 행에서 20분입니다.**

---

## 8. 쿼리 성능

### 8-1. 느린 쿼리 찾기

```sql
-- pg_stat_statements 확장 (기본 켜두세요)
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

SELECT
    calls,
    round(mean_exec_time::numeric, 2) AS avg_ms,
    round(total_exec_time::numeric / 1000, 1) AS total_s,
    round((100 * total_exec_time / sum(total_exec_time) OVER ())::numeric, 1) AS pct,
    left(query, 100) AS query
FROM pg_stat_statements
ORDER BY total_exec_time DESC
LIMIT 20;
```

**`mean_exec_time`이 아니라 `total_exec_time`으로 정렬하세요.**
1ms짜리가 100만 번 도는 게 1초짜리 100번보다 훨씬 큰 문제입니다.

### 8-2. 앱에서 느린 쿼리 로깅

```rust
use sqlx::ConnectOptions;
use std::str::FromStr;

let opts = PgConnectOptions::from_str(&url)?
    .log_statements(log::LevelFilter::Debug)
    .log_slow_statements(log::LevelFilter::Warn, Duration::from_millis(200));
```

### 8-3. sqlx 매크로를 실제로 쓰기

[04 §5](./04_database.md)가 `query_as!`를 "강력 추천"하는데, [03 §4](./03_service_anatomy.md)의 예제는
런타임 `query_as`를 씁니다. 매크로를 쓰려면:

```toml
sqlx = { version = "0.8", features = ["macros", ...] }   # macros 필요
```

```bash
# 오프라인 캐시 생성 (개발 DB 연결 상태에서)
cargo sqlx prepare --workspace -- --all-targets

# .sqlx/ 디렉터리를 반드시 커밋
git add .sqlx && git commit -m "chore: update sqlx offline cache"
```

```bash
# CI
SQLX_OFFLINE=true cargo build
```

**CI에서 캐시 최신성을 강제하세요** ([18_cicd](./18_cicd.md)):

```bash
cargo sqlx prepare --workspace --check -- --all-targets
```

이게 없으면 "로컬은 되는데 CI가 깨진다"가 반복됩니다.

### 8-4. N+1

[07 §2](./07_messaging.md)가 서비스 간 N+1을 다루지만, **DB 안에서도 납니다.**

```rust
// ❌ 주문 20개 → 쿼리 21번
let orders = OrderRepo::list(&pool, user_id).await?;
for o in &orders {
    let items = OrderItemRepo::find_by_order(&pool, o.id).await?;
}

// ✅ 2번
let orders = OrderRepo::list(&pool, user_id).await?;
let ids: Vec<Uuid> = orders.iter().map(|o| o.id).collect();
let items = OrderItemRepo::find_by_orders(&pool, &ids).await?;  // WHERE order_id = ANY($1)
let grouped: HashMap<Uuid, Vec<Item>> = items.into_iter().into_group_map_by(|i| i.order_id);
```

PostgreSQL의 `= ANY($1)`에 배열을 넘기는 게 sqlx에서 가장 깔끔합니다.

```rust
sqlx::query_as::<_, ItemRow>("SELECT * FROM order_items WHERE order_id = ANY($1)")
    .bind(&ids[..])
    .fetch_all(pool).await?
```

---

## 9. 읽기 레플리카

### 9-1. 언제

```
읽기가 쓰기의 5배 이상 + PG CPU가 읽기로 포화
→ 레플리카 도입 검토
```

그 전에 **캐시([05](./05_redis.md))와 인덱스**를 먼저 보세요. 레플리카는 복잡도가 큽니다.

### 9-2. 복제 지연이 만드는 버그

```
1. 사용자가 프로필 수정 (마스터에 쓰기)
2. 즉시 프로필 조회 (레플리카에서 읽기)
3. 복제 지연 50ms → 옛 데이터가 보임
4. "저장이 안 됐다"는 문의
```

**read-your-writes 일관성**이 깨지는 전형적 사례입니다.

### 9-3. 라우팅 정책

```rust
pub struct Db {
    primary: PgPool,
    replica: PgPool,
}

impl Db {
    /// 쓰기 — 항상 primary
    pub fn write(&self) -> &PgPool { &self.primary }

    /// 읽기 — 지연을 허용할 수 있을 때만 replica
    pub fn read(&self, consistency: Consistency) -> &PgPool {
        match consistency {
            Consistency::Strong   => &self.primary,   // 방금 쓴 것을 읽음
            Consistency::Eventual => &self.replica,   // 목록, 검색, 통계
        }
    }
}
```

**기본을 `Strong`으로 두고, 명시적으로 `Eventual`을 고르게 하세요.**
반대로 하면 미묘한 버그가 조용히 쌓입니다.

간단한 대안: **쓰기 직후 N초 동안은 그 사용자의 읽기를 primary로** (세션 스티키니스).

```rust
// 쓰기 후 Redis에 마킹
conn.set_ex(format!("rw:{user_id}"), "1", 5).await?;

// 읽기 시 확인
let pool = if conn.exists(format!("rw:{user_id}")).await? {
    &self.primary
} else {
    &self.replica
};
```

---

## 10. 백업과 복구

[09 §7](./09_deploy.md)의 체크리스트에 "PG PITR"이 한 줄 있습니다. 실제로 필요한 것:

### 10-1. 복구 목표를 먼저 정하세요

| 지표 | 질문 | 예시 |
|---|---|---|
| RPO (Recovery Point Objective) | 데이터를 얼마나 잃어도 되나 | 5분 |
| RTO (Recovery Time Objective) | 얼마나 빨리 복구해야 하나 | 1시간 |

이게 정해지지 않으면 백업 전략을 정할 수 없습니다.

```
RPO 5분  → WAL 아카이빙 필수 (일일 덤프로는 불가)
RTO 1시간 → 복구 리허설을 해봐야 알 수 있음
```

### 10-2. 복구 훈련

**백업이 있다는 것과 복구할 수 있다는 것은 다릅니다.**

```
분기 1회:
1. 프로덕션 백업으로 격리 환경에 복구
2. 소요 시간 측정 → RTO와 비교
3. 데이터 검증 (행 수, 최신 레코드 시각)
4. 문서 갱신
```

한 번도 안 해봤다면, 지금 백업은 **없는 것과 같다고 가정**하세요.

### 10-3. 서비스마다 다른 정책

| 데이터 | RPO | 비고 |
|---|---|---|
| user_db (계정) | 0에 가깝게 | 유실 시 로그인 불가 |
| order_db (주문) | 0에 가깝게 | 금전 |
| catalog_db (상품) | 시간 단위 | 재생성 가능 |
| notification_log | 하루 | 유실 감수 가능 |
| audit_log | 0 | 법적 요구 ([12 §8](./12_security.md)) |

**전부 최고 등급으로 하면 비용이 감당 안 됩니다.** 서비스별로 다르게 가세요.

### 10-4. 논리 백업도 같이

물리 백업(PITR)은 빠르지만 **버전 간 이식이 안 되고, 특정 테이블만 복구하기 어렵습니다.**

```bash
# 주간 논리 덤프 — "실수로 테이블 하나 지웠다"에 대응
pg_dump --format=custom --compress=9 -d user_db > user_db_$(date +%F).dump

# 특정 테이블만 복구
pg_restore -d user_db -t users user_db_2026-07-23.dump
```

---

## 체크포인트

```
[ ] 모든 스키마 변경이 롤링 중 옛 코드와 호환된다
[ ] 파괴적 변경이 expand/migrate/contract 3배포로 나뉜다
[ ] CREATE INDEX에 CONCURRENTLY가 있다
[ ] NOT NULL 추가가 NOT VALID → VALIDATE 순서다
[ ] 백필이 배치 + sleep + 중단 가능하다
[ ] HPA 최대 replica × pool ≤ PG 가용 커넥션
[ ] statement_timeout / lock_timeout / idle_in_transaction이 설정됐다
[ ] application_name이 서비스명으로 설정됐다
[ ] PgBouncer transaction 모드면 statement_cache_capacity(0)이다
[ ] repo 함수가 Executor 제네릭이라 트랜잭션에서도 쓰인다
[ ] 트랜잭션 안에 외부 HTTP/Redis 호출이 없다
[ ] 잔액 차감류가 원자적 UPDATE이거나 FOR UPDATE다
[ ] 데드락(40P01) 재시도가 있고 락 순서 규칙이 있다
[ ] 프로덕션은 migrate Job + 앱은 검증만
[ ] .sqlx 캐시가 커밋되고 CI에서 --check 한다
[ ] pg_stat_statements가 켜져 있다
[ ] RPO/RTO가 정해졌고 복구 훈련을 했다
```

---

다음: [14_messaging_ops — outbox 워커를 실제로 굴리기](./14_messaging_ops.md)
