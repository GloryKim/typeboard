# 14. 메시징 운영 — outbox 워커 · inbox · DLQ · 순서

[07_messaging](./07_messaging.md)은 **outbox 패턴이 무엇인지**를 설명합니다.
이 문서는 그 워커를 **실제로 몇 달 굴렸을 때 필요한 것들**을 다룹니다.

07의 스케치와 운영 가능한 구현 사이의 거리가 이 문서의 분량입니다.

---

## 1. 07의 outbox를 다시 봅니다

```
worker:
  select ... for update skip locked
  publish
  mark published_at
```

`FOR UPDATE SKIP LOCKED`는 정확합니다. 그런데 이 3줄로는 답할 수 없는 질문들:

| 질문 | 07의 답 |
|---|---|
| publish가 실패하면? | 없음 (다음 폴링에 재시도되겠지만 무한 반복) |
| 10번 실패한 이벤트는? | 없음 (영원히 재시도하며 다른 이벤트를 막음) |
| 발행된 행은 언제 지우나? | 없음 (무한 증식) |
| 같은 주문의 이벤트 순서는? | 없음 (뒤집힐 수 있음) |
| 워커가 2개면? | SKIP LOCKED로 안전하지만, 순서는 더 깨짐 |
| 지금 밀려 있는지 어떻게 아나? | 08에 "outbox lag" 언급만 |
| 소비자가 두 번 받으면? | "멱등이어야 한다"고만 |

하나씩 채웁니다.

---

## 2. outbox 스키마 확장

[04 §6](./04_database.md)의 테이블에 운영 컬럼을 더합니다.

```sql
CREATE TABLE outbox (
    id             UUID PRIMARY KEY,              -- UUIDv7 (시간 정렬)
    aggregate_type TEXT NOT NULL,                 -- 'order'
    aggregate_id   UUID NOT NULL,                 -- 순서 보장 단위 (§5)
    topic          TEXT NOT NULL,                 -- 'order.created'
    payload        JSONB NOT NULL,
    headers        JSONB NOT NULL DEFAULT '{}',   -- trace context 등 (§7)

    -- 발행 상태
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at   TIMESTAMPTZ,                   -- NULL = 미발행
    attempts       INT NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error     TEXT,
    dead           BOOLEAN NOT NULL DEFAULT false -- 포기 (§4)
);

-- 워커의 핵심 쿼리를 위한 부분 인덱스
-- 미발행 행만 인덱싱 → 발행된 행이 쌓여도 인덱스가 작게 유지됨
CREATE INDEX outbox_pending
    ON outbox (next_attempt_at, id)
    WHERE published_at IS NULL AND dead = false;

-- 정리(§6)용
CREATE INDEX outbox_published_at
    ON outbox (published_at)
    WHERE published_at IS NOT NULL;

-- 순서 보장용 (§5)
CREATE INDEX outbox_aggregate
    ON outbox (aggregate_type, aggregate_id, id)
    WHERE published_at IS NULL;
```

**부분 인덱스(`WHERE published_at IS NULL`)가 핵심입니다.**
이게 없으면 발행된 1억 행이 인덱스에 남아 워커 쿼리가 점점 느려집니다.

---

## 3. 워커 구현

### 3-1. 배치 조회 + 잠금

```rust
pub struct OutboxWorker {
    pool: PgPool,
    publisher: Arc<dyn Publisher>,
    cfg: OutboxConfig,
}

pub struct OutboxConfig {
    pub batch_size: i64,        // 100
    pub poll_interval: Duration, // 500ms
    pub max_attempts: i32,       // 10
    pub claim_ttl: Duration,     // 30s
}

impl OutboxWorker {
    async fn process_batch(&self) -> anyhow::Result<usize> {
        let mut tx = self.pool.begin().await?;

        // SKIP LOCKED로 다른 워커와 충돌 없이 배치를 확보
        let rows = sqlx::query_as::<_, OutboxRow>(
            r#"
            SELECT id, aggregate_type, aggregate_id, topic, payload, headers, attempts
            FROM outbox
            WHERE published_at IS NULL
              AND dead = false
              AND next_attempt_at <= now()
            ORDER BY id                      -- UUIDv7 = 생성 순서
            LIMIT $1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(self.cfg.batch_size)
        .fetch_all(&mut *tx)
        .await?;

        if rows.is_empty() {
            tx.rollback().await?;
            return Ok(0);
        }

        let mut ok_ids = Vec::new();
        let mut failures = Vec::new();

        for row in &rows {
            match self.publish_one(row).await {
                Ok(()) => ok_ids.push(row.id),
                Err(e) => {
                    tracing::warn!(
                        outbox_id = %row.id, topic = %row.topic,
                        attempts = row.attempts, error = %e,
                        "outbox publish failed"
                    );
                    failures.push((row.id, row.attempts + 1, e.to_string()));
                }
            }
        }

        // 성공 표시
        if !ok_ids.is_empty() {
            sqlx::query("UPDATE outbox SET published_at = now() WHERE id = ANY($1)")
                .bind(&ok_ids[..])
                .execute(&mut *tx).await?;
        }

        // 실패 → 백오프 또는 사망 처리 (§4)
        for (id, attempts, err) in failures {
            let dead = attempts >= self.cfg.max_attempts;
            let backoff = backoff_seconds(attempts);
            sqlx::query(
                r#"
                UPDATE outbox
                SET attempts = $2,
                    last_error = $3,
                    dead = $4,
                    next_attempt_at = now() + ($5 || ' seconds')::interval
                WHERE id = $1
                "#,
            )
            .bind(id).bind(attempts).bind(&err).bind(dead).bind(backoff)
            .execute(&mut *tx).await?;

            if dead {
                metrics::counter!("outbox_dead_total").increment(1);
                tracing::error!(outbox_id = %id, error = %err, "outbox event marked dead");
            }
        }

        tx.commit().await?;
        metrics::counter!("outbox_published_total").increment(ok_ids.len() as u64);
        Ok(rows.len())
    }
}

/// 지수 백오프 + 상한. 1s, 2s, 4s ... 최대 5분
fn backoff_seconds(attempts: i32) -> i64 {
    let base = 1i64 << attempts.min(9);   // 최대 512
    base.min(300)
}
```

### 3-2. 트랜잭션이 오래 열리는 문제

위 코드는 **HTTP/NATS publish를 트랜잭션 안에서** 합니다.
[13 §6-2](./13_data_evolution.md)에서 하지 말라고 한 바로 그것입니다.

배치가 100개이고 각 publish가 10ms면 트랜잭션이 1초 열려 있습니다. 브로커가 느려지면 더 길어집니다.

**대안: 클레임(claim) 방식으로 트랜잭션을 짧게 유지**

```rust
async fn process_batch_claimed(&self) -> anyhow::Result<usize> {
    let worker_id = self.worker_id;   // 프로세스 고유 ID

    // 1) 짧은 트랜잭션으로 소유권만 표시
    let rows = sqlx::query_as::<_, OutboxRow>(
        r#"
        UPDATE outbox SET
            claimed_by = $2,
            claimed_until = now() + ($3 || ' seconds')::interval
        WHERE id IN (
            SELECT id FROM outbox
            WHERE published_at IS NULL AND dead = false
              AND next_attempt_at <= now()
              AND (claimed_until IS NULL OR claimed_until < now())  -- 만료된 클레임 회수
            ORDER BY id LIMIT $1
            FOR UPDATE SKIP LOCKED
        )
        RETURNING id, aggregate_type, aggregate_id, topic, payload, headers, attempts
        "#,
    )
    .bind(self.cfg.batch_size)
    .bind(worker_id)
    .bind(self.cfg.claim_ttl.as_secs() as i64)
    .fetch_all(&self.pool)   // 트랜잭션 없이 단일 문장 = 즉시 커밋
    .await?;

    // 2) 트랜잭션 밖에서 발행 (오래 걸려도 DB에 부담 없음)
    for row in &rows { /* publish → 결과 수집 */ }

    // 3) 짧은 트랜잭션으로 결과 반영
    // ...
}
```

`claimed_until`이 만료되면 다른 워커가 회수하므로, **워커가 죽어도 이벤트가 영구 정체되지 않습니다.**

```sql
ALTER TABLE outbox ADD COLUMN claimed_by TEXT;
ALTER TABLE outbox ADD COLUMN claimed_until TIMESTAMPTZ;
```

### 3-3. 폴링 대신 LISTEN/NOTIFY

500ms 폴링은 **지연 500ms + 유휴 시에도 초당 2회 쿼리**입니다.

```sql
-- 트리거로 알림
CREATE OR REPLACE FUNCTION notify_outbox() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('outbox_new', '');
    RETURN NULL;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER outbox_notify
    AFTER INSERT ON outbox
    FOR EACH STATEMENT EXECUTE FUNCTION notify_outbox();
```

```rust
use sqlx::postgres::PgListener;

async fn run(&self, shutdown: CancellationToken) -> anyhow::Result<()> {
    let mut listener = PgListener::connect(&self.url).await?;
    listener.listen("outbox_new").await?;

    // 폴링도 병행 — NOTIFY는 유실될 수 있고, 재시도 대기 건은 알림이 안 옴
    let mut ticker = tokio::time::interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            _ = listener.recv() => { self.drain().await; }   // 즉시 반응
            _ = ticker.tick()    => { self.drain().await; }  // 안전망
        }
    }
    Ok(())
}

/// 처리할 게 없을 때까지 반복
async fn drain(&self) {
    loop {
        match self.process_batch_claimed().await {
            Ok(0) => break,
            Ok(_) => continue,
            Err(e) => { tracing::error!(error = %e, "outbox batch error"); break; }
        }
    }
}
```

**폴링을 없애지 말고 간격만 늘리세요.** NOTIFY는 보장이 없습니다(커넥션 끊김 시 유실).

> ⚠️ **PgBouncer transaction 모드에서는 LISTEN/NOTIFY가 동작하지 않습니다** ([13 §5-3](./13_data_evolution.md)).
> 워커는 PgBouncer를 우회해 PG에 직접 연결하세요.

### 3-4. 워커를 어디서 돌릴까

| 방식 | 장점 | 단점 |
|---|---|---|
| 서비스 프로세스 내 `tokio::spawn` | 배포 단순, 코드 공유 | API 파드 수만큼 워커가 생김 |
| 별도 Deployment | 독립 스케일, 리소스 격리 | 이미지/배포 하나 더 |
| 별도 바이너리 + `replicas: 1` | 순서 보장 쉬움 | 단일 장애점 (K8s가 재시작하지만 공백 발생) |

**추천: 초기엔 프로세스 내, 이벤트 볼륨이 커지면 분리.**

프로세스 내로 돌릴 때 주의 — API 파드 10개면 워커도 10개입니다.
`SKIP LOCKED` 덕에 중복 발행은 없지만, **폴링 쿼리가 10배**가 됩니다.
폴링 간격을 파드 수에 맞춰 늘리거나 LISTEN/NOTIFY로 가세요.

[11 §1-5](./11_resilience.md)의 `CancellationToken` 정리를 반드시 적용하세요.

---

## 4. DLQ — 포기할 줄 알아야 합니다

### 4-1. 무한 재시도의 함정

```
outbox에 잘못된 payload가 하나 들어감 (소비자가 파싱 불가)
→ 영원히 재시도
→ ORDER BY id 이므로 이 행이 항상 먼저 조회됨
→ 뒤의 정상 이벤트가 영원히 발행되지 않음   ← head-of-line blocking
```

`max_attempts` 초과 시 `dead = true`로 표시하면, 워커 쿼리(`WHERE dead = false`)에서 빠져
**뒤의 이벤트가 흐르기 시작합니다.**

### 4-2. 죽은 이벤트를 관리하기

```rust
/// 운영용 조회 — 내부 관리 API 또는 CLI
pub async fn list_dead(pool: &PgPool, limit: i64) -> Result<Vec<OutboxRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT * FROM outbox WHERE dead = true ORDER BY created_at DESC LIMIT $1"
    ).bind(limit).fetch_all(pool).await
}

/// 원인을 고친 뒤 재투입
pub async fn revive(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE outbox
         SET dead = false, attempts = 0, next_attempt_at = now(), last_error = NULL
         WHERE id = $1"
    ).bind(id).execute(pool).await?;
    Ok(())
}
```

**알림은 필수입니다.**

```yaml
- alert: OutboxDeadLetters
  expr: increase(outbox_dead_total[10m]) > 0
  labels: { severity: critical }
  annotations:
    summary: "이벤트가 영구 실패했습니다 — 데이터 정합성 위험"
```

죽은 이벤트 = **다른 서비스가 영원히 모르는 사실**입니다. 조용히 넘어가면 안 됩니다.

### 4-3. 실패 유형을 구분하세요

```rust
enum PublishError {
    /// 브로커 다운, 네트워크 — 재시도하면 됨
    Transient(anyhow::Error),
    /// payload 스키마 오류, 존재하지 않는 토픽 — 재시도해도 무의미
    Permanent(anyhow::Error),
}

// Permanent는 attempts와 무관하게 즉시 dead
let dead = matches!(err, PublishError::Permanent(_))
    || attempts >= self.cfg.max_attempts;
```

영구 오류를 10번 재시도하는 것은 시간과 로그 낭비입니다.

---

## 5. 순서 보장

### 5-1. 기본은 순서가 없습니다

```
워커 2개 + 배치 100개 + 병렬 publish
→ order.created(주문A) 와 order.cancelled(주문A) 의 순서가 뒤집힐 수 있음
→ 소비자가 "취소 → 생성" 순으로 받음 → 취소된 주문이 살아남
```

### 5-2. 언제 순서가 필요한가

| 이벤트 | 순서 필요? |
|---|---|
| `order.created` → `order.cancelled` (같은 주문) | ✅ 필수 |
| `user.updated` × 2회 (같은 사용자) | ✅ 필수 (안 그러면 옛 값이 최종) |
| 서로 다른 주문의 이벤트 | ❌ 무관 |
| `notification.sent` 로그 | ❌ 무관 |

**"같은 aggregate_id 안에서만" 순서가 필요합니다.** 전역 순서는 필요 없고, 강제하면 처리량이 죽습니다.

### 5-3. 해법 A — 브로커의 파티션 키

Kafka/NATS는 파티션(또는 subject) 단위로 순서를 보장합니다.

```rust
// Kafka: 같은 키 → 같은 파티션 → 순서 보장
producer.send(
    FutureRecord::to(&row.topic)
        .key(&row.aggregate_id.to_string())   // ← 이게 핵심
        .payload(&row.payload),
    Duration::from_secs(5),
).await?;

// NATS JetStream: subject에 aggregate를 포함
// "order.created" 대신 "events.order.{aggregate_id}.created"
```

발행 순서만 맞으면 소비 순서가 보장됩니다. 발행 순서는 §5-4로.

### 5-4. 해법 B — aggregate당 직렬 발행

```rust
async fn process_batch_ordered(&self) -> anyhow::Result<usize> {
    let rows = self.claim_batch().await?;

    // aggregate별로 그룹핑
    let mut groups: HashMap<(String, Uuid), Vec<OutboxRow>> = HashMap::new();
    for row in rows {
        groups.entry((row.aggregate_type.clone(), row.aggregate_id))
              .or_default().push(row);
    }

    // 그룹 간에는 병렬, 그룹 안에서는 순차
    let results = futures::stream::iter(groups.into_values())
        .map(|mut group| async move {
            group.sort_by_key(|r| r.id);   // UUIDv7 = 생성 순
            for row in &group {
                if self.publish_one(row).await.is_err() {
                    // ⚠️ 실패하면 같은 aggregate의 뒤 이벤트는 중단
                    //    순서를 지키려면 여기서 멈춰야 함
                    break;
                }
            }
        })
        .buffer_unordered(16)   // 동시 16 aggregate
        .collect::<Vec<_>>()
        .await;
    ...
}
```

**"실패하면 그 aggregate는 멈춘다"** 가 핵심입니다.
멈추지 않으면 순서가 깨지고, 순서를 지키려면 멈춰야 합니다. 트레이드오프를 의식적으로 고르세요.

### 5-5. 해법 C — 소비자가 순서에 의존하지 않게

가장 견고한 방법입니다.

```rust
// 이벤트에 버전을 싣는다
#[derive(Serialize, Deserialize)]
pub struct UserUpdated {
    pub user_id: Uuid,
    pub version: i64,      // ← aggregate 버전
    pub name: String,
}

// 소비자: 자기가 아는 버전보다 낮으면 무시
if event.version <= local.version {
    tracing::debug!("stale event, ignoring");
    return Ok(());
}
```

순서가 뒤집혀도 **최종 상태가 올바릅니다.** 가능하면 이 방향을 택하세요.

---

## 6. outbox 테이블 정리

### 6-1. 안 지우면 어떻게 되나

```
일 100만 이벤트 × 30일 = 3,000만 행
payload JSONB 평균 500B → 15GB
→ VACUUM 부담, 백업 시간 증가, 부분 인덱스는 작지만 테이블 스캔이 필요한 순간 폭발
```

### 6-2. 방법 A — 주기적 삭제

```rust
async fn cleanup(pool: &PgPool) -> anyhow::Result<u64> {
    // 배치로 삭제 (13 §3과 같은 이유)
    let mut total = 0;
    loop {
        let deleted = sqlx::query(
            r#"
            DELETE FROM outbox
            WHERE id IN (
                SELECT id FROM outbox
                WHERE published_at < now() - interval '7 days'
                LIMIT 10000
            )
            "#,
        ).execute(pool).await?.rows_affected();

        total += deleted;
        if deleted == 0 { break; }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Ok(total)
}
```

**보존 기간의 근거:** 장애 조사에 필요한 기간 + 재발행 가능성.
7일이면 대개 충분합니다. 감사 목적이면 별도 테이블로 옮기세요.

### 6-3. 방법 B — 파티셔닝 (권장)

`DELETE`는 죽은 튜플을 만들고 VACUUM 부담을 줍니다. `DROP PARTITION`은 즉시입니다.

```sql
CREATE TABLE outbox (
    id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ...
    PRIMARY KEY (id, created_at)          -- 파티션 키가 PK에 포함되어야 함
) PARTITION BY RANGE (created_at);

CREATE TABLE outbox_2026_07 PARTITION OF outbox
    FOR VALUES FROM ('2026-07-01') TO ('2026-08-01');
CREATE TABLE outbox_2026_08 PARTITION OF outbox
    FOR VALUES FROM ('2026-08-01') TO ('2026-09-01');

-- 정리는 즉시
DROP TABLE outbox_2026_06;
```

`pg_partman` 확장으로 파티션 생성/삭제를 자동화하세요.
**미래 파티션을 미리 만들어두지 않으면 INSERT가 실패합니다.** 모니터링 필수.

---

## 7. Inbox 패턴 — 소비자 측 멱등성

### 7-1. 07의 "멱등이어야 한다"를 실제로

> **적어도 한 번(at-least-once)** 이므로 소비자는 **멱등**이어야 합니다
> (`notification_log`에 `event_id` UNIQUE).

방향은 맞습니다. 이걸 **모든 소비자가 쓰는 공통 패턴**으로 만듭니다.

```sql
-- 각 소비 서비스의 DB에
CREATE TABLE processed_events (
    event_id     UUID PRIMARY KEY,
    consumer     TEXT NOT NULL,          -- 한 서비스에 소비자가 여럿일 수 있음
    event_type   TEXT NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ON processed_events (processed_at);   -- 정리용
```

```rust
/// 이벤트 처리를 멱등하게 감싼다.
/// 비즈니스 로직과 처리 기록이 같은 트랜잭션에 들어가는 것이 핵심.
pub async fn process_once<F, Fut>(
    pool: &PgPool,
    event_id: Uuid,
    consumer: &str,
    event_type: &str,
    handler: F,
) -> anyhow::Result<ProcessOutcome>
where
    F: FnOnce(&mut Transaction<'_, Postgres>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut tx = pool.begin().await?;

    // 1) 먼저 기록을 시도 — 중복이면 여기서 걸린다
    let inserted = sqlx::query(
        "INSERT INTO processed_events (event_id, consumer, event_type)
         VALUES ($1, $2, $3) ON CONFLICT (event_id) DO NOTHING"
    )
    .bind(event_id).bind(consumer).bind(event_type)
    .execute(&mut *tx).await?
    .rows_affected();

    if inserted == 0 {
        tx.rollback().await?;
        metrics::counter!("events_duplicate_total", "consumer" => consumer.to_string())
            .increment(1);
        return Ok(ProcessOutcome::AlreadyProcessed);
    }

    // 2) 같은 트랜잭션에서 실제 처리
    handler(&mut tx).await?;

    // 3) 둘 다 커밋되거나 둘 다 롤백
    tx.commit().await?;
    Ok(ProcessOutcome::Processed)
}
```

**같은 트랜잭션이라는 점이 결정적입니다.**
따로 하면 "처리는 됐는데 기록이 안 된" 또는 그 반대 상태가 생깁니다.

### 7-2. 사용

```rust
async fn on_order_created(pool: &PgPool, msg: &Message) -> anyhow::Result<()> {
    let env: EventEnvelope<OrderCreated> = serde_json::from_slice(&msg.payload)?;

    let outcome = process_once(pool, env.id, "notification-service", &env.event_type,
        |tx| async move {
            NotificationRepo::insert(tx, &env.data).await?;
            Ok(())
        }
    ).await?;

    match outcome {
        ProcessOutcome::Processed        => tracing::info!(event_id = %env.id, "processed"),
        ProcessOutcome::AlreadyProcessed => tracing::debug!(event_id = %env.id, "duplicate, skipped"),
    }

    msg.ack().await?;   // 둘 다 ack (중복도 정상 처리된 것)
    Ok(())
}
```

### 7-3. 부수효과가 DB 밖이면?

이메일 발송처럼 **롤백할 수 없는 부수효과**가 있으면 트랜잭션에 못 넣습니다.

```rust
// 2단계로 분리
// 1) 이벤트 수신 → "보낼 것"을 DB에 기록 (멱등, 트랜잭션 안)
process_once(pool, event_id, "notification", ty, |tx| async move {
    NotificationRepo::enqueue(tx, &to_send).await
}).await?;

// 2) 별도 워커가 DB를 읽어 실제 발송 (자체 재시도 + 발송 기록)
//    → 이건 또 하나의 outbox입니다
```

이메일 API 자체의 멱등 키(대부분 지원)를 같이 쓰면 이중 발송을 막습니다.

### 7-4. processed_events 정리

```sql
-- 이벤트 최대 재전달 기간보다 넉넉하게 (예: 30일)
DELETE FROM processed_events WHERE processed_at < now() - interval '30 days';
```

§6과 같이 배치로, 또는 파티셔닝하세요.

---

## 8. 이벤트 봉투(envelope) 표준화

### 8-1. 07 §3의 JSON을 타입으로

```rust
// crates/events/src/lib.rs — 서비스 간 공유 (02 §4가 허용한 것)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub id: Uuid,                        // 이벤트 고유 ID (inbox 키)
    #[serde(rename = "type")]
    pub event_type: String,              // "order.created"
    pub source: String,                  // "order-service"
    pub time: DateTime<Utc>,
    pub specversion: String,             // "1.0" (CloudEvents)

    // 확장 — CloudEvents의 extension attributes
    pub aggregate_id: Uuid,              // 순서 보장 키 (§5)
    pub aggregate_version: i64,          // 이벤트 소싱 / 스테일 감지 (§5-5)
    pub correlation_id: String,          // 요청 추적 (15 참고)
    pub causation_id: Option<Uuid>,      // 이 이벤트를 유발한 이벤트

    pub data: T,
}
```

`causation_id`가 유용합니다. **"이 알림이 왜 갔지?"** 를 역추적할 수 있습니다.

```
order.created (id=A)
  → inventory.reserved (id=B, causation=A)
     → notification.sent (id=C, causation=B)
```

### 8-2. 스키마 진화 규칙

07 §7의 *"필드 추가만"* 을 구체화합니다.

| 변경 | 호환 | 규칙 |
|---|---|---|
| 옵셔널 필드 추가 | ✅ | `Option<T>` + `#[serde(default)]` |
| 필수 필드 추가 | ❌ | 옛 소비자가 파싱 실패 → 옵셔널로 추가 후 나중에 필수화 |
| 필드 제거 | ❌ | 먼저 소비자에서 참조 제거 → 몇 배포 후 삭제 |
| 필드 이름 변경 | ❌ | 새 필드 추가 + 둘 다 채움 → 전환 → 옛 필드 삭제 |
| 타입 변경 | ❌ | 새 필드로 |
| 의미 변경 | ❌❌ | **가장 위험.** 새 event_type을 만드세요 |

[13 §2](./13_data_evolution.md)의 expand/contract와 같은 원리입니다. 스키마 진화의 법칙은 하나입니다.

```rust
// 소비자는 모르는 필드를 무시하도록
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]   // ❌ 절대 쓰지 마세요 — 생산자가 필드를 추가하면 깨짐
pub struct OrderCreated { ... }

#[derive(Deserialize)]          // ✅ serde 기본값이 무시입니다
pub struct OrderCreated {
    pub order_id: Uuid,
    #[serde(default)]
    pub coupon_code: Option<String>,   // 나중에 추가된 필드
}
```

### 8-3. 버전은 event_type에

```
order.created        → order.created.v2
```

**두 버전을 동시에 발행하는 기간**을 두고, 소비자가 전부 이전한 뒤 v1을 끕니다.

```rust
// 전환기: 둘 다 발행
OutboxRepo::push(&mut tx, "order.created",    v1_payload).await?;
OutboxRepo::push(&mut tx, "order.created.v2", v2_payload).await?;
```

### 8-4. 소비자 등록부

이벤트가 늘어나면 **누가 무엇을 소비하는지** 아무도 모르게 됩니다.

```markdown
| 이벤트 | 생산자 | 소비자 | 비고 |
|---|---|---|---|
| order.created | order-service | notification, catalog(재고), analytics | v2 전환 중 |
| user.updated | user-service | order(스냅샷 갱신) | |
| payment.captured | payment-service | order | 순서 필요 |
```

이 표가 없으면 **이벤트 스키마를 바꿀 때 누구에게 알려야 할지 모릅니다.**
[16_api_contract](./16_api_contract.md)에서 이걸 자동화하는 방법을 다룹니다.

---

## 9. NATS JetStream 실전 설정

### 9-1. 07 §5의 스케치에 없는 것들

```rust
use async_nats::jetstream::{self, stream, consumer};

async fn setup(js: &jetstream::Context) -> anyhow::Result<()> {
    // 스트림 — 이벤트 보관 정책
    js.get_or_create_stream(stream::Config {
        name: "EVENTS".to_string(),
        subjects: vec!["events.>".to_string()],
        retention: stream::RetentionPolicy::Limits,
        max_age: Duration::from_secs(7 * 24 * 3600),   // 7일 보관
        max_bytes: 10 * 1024 * 1024 * 1024,            // 10GB 상한
        storage: stream::StorageType::File,            // 메모리 아님 — 재시작 후 유지
        num_replicas: 3,                               // 클러스터면 3
        duplicate_window: Duration::from_secs(120),    // 중복 제거 창 (§9-2)
        ..Default::default()
    }).await?;

    // 컨슈머 — 소비 정책
    js.get_or_create_consumer_on_stream(consumer::pull::Config {
        durable_name: Some("notification-service".to_string()),  // 재시작해도 위치 유지
        filter_subject: "events.order.>".to_string(),
        ack_policy: consumer::AckPolicy::Explicit,
        ack_wait: Duration::from_secs(30),      // 이 안에 ack 없으면 재전달
        max_deliver: 5,                          // 5번 실패하면 포기
        max_ack_pending: 100,                    // 동시 처리 상한 (백프레셔)
        backoff: vec![                           // 재전달 백오프
            Duration::from_secs(1),
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(300),
        ],
        ..Default::default()
    }, "EVENTS").await?;

    Ok(())
}
```

| 설정 | 없으면 |
|---|---|
| `durable_name` | 재시작 시 처음부터 다시 소비 (중복 폭탄) |
| `ack_wait` | 처리 중 죽으면 영원히 재전달 안 됨 |
| `max_deliver` | 독성 메시지가 무한 재전달 (§4의 outbox와 같은 문제) |
| `max_ack_pending` | 소비자가 감당 못 할 속도로 밀려듦 |
| `backoff` | 실패 시 즉시 재시도 → 부하 증폭 |
| `storage: File` | 재시작하면 이벤트 전부 소실 |

### 9-2. 중복 제거

`Nats-Msg-Id` 헤더를 주면 JetStream이 `duplicate_window` 안에서 중복을 걸러줍니다.

```rust
let mut headers = async_nats::HeaderMap::new();
headers.insert("Nats-Msg-Id", envelope.id.to_string().as_str());

js.publish_with_headers(subject, headers, payload.into()).await?.await?;
```

outbox 워커가 "발행 성공 → published_at 기록" 사이에 죽으면 재발행이 일어납니다.
이 헤더가 그 창을 막아줍니다. **단, inbox 패턴(§7)을 대체하지는 않습니다** — 창이 지나면 무효입니다.

### 9-3. 소비 루프

```rust
async fn consume(consumer: consumer::Consumer<consumer::pull::Config>,
                 pool: PgPool, shutdown: CancellationToken) -> anyhow::Result<()>
{
    let mut messages = consumer.messages().await?;

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => {
                tracing::info!("consumer stopping");
                break;
            }
            Some(msg) = messages.next() => {
                let msg = msg?;
                let info = msg.info()?;

                match handle(&pool, &msg).await {
                    Ok(_) => msg.ack().await.map_err(|e| anyhow::anyhow!("{e}"))?,
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            delivered = info.delivered,
                            "handler failed"
                        );
                        // 마지막 시도였으면 DLQ 기록
                        if info.delivered >= 5 {
                            dead_letter(&pool, &msg, &e).await.ok();
                            msg.ack().await.ok();   // 더는 재전달 받지 않음
                        } else {
                            // 명시적 nak — backoff에 따라 재전달
                            msg.ack_with(async_nats::jetstream::AckKind::Nak(None))
                                .await.ok();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
```

**`max_deliver` 도달 시 DLQ 테이블에 남기세요.** 안 그러면 이벤트가 조용히 사라집니다.

```sql
CREATE TABLE event_dlq (
    id UUID PRIMARY KEY,
    subject TEXT NOT NULL,
    payload JSONB NOT NULL,
    error TEXT NOT NULL,
    delivered INT NOT NULL,
    failed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### 9-4. Redis Streams를 쓸 때

[05 §1](./05_redis.md)이 "소규모면 NATS 대체 가능"이라고 한 경우:

```rust
// 소비자 그룹으로 읽기
// XREADGROUP GROUP notification c1 COUNT 10 BLOCK 5000 STREAMS events >
```

주의:
- **블로킹 읽기이므로 `ConnectionManager`를 쓰면 안 됩니다** ([10_errata §12-2](./10_errata.md)) — 전용 커넥션 필요
- `XAUTOCLAIM`으로 죽은 소비자의 pending 메시지를 회수해야 합니다
- `MAXLEN ~ 100000`으로 스트림 길이를 제한하지 않으면 메모리가 무한 증가합니다
- Redis는 기본적으로 **디스크 영속성이 약합니다** — 이벤트 유실 허용 범위를 확인하세요

**금전이 걸린 이벤트에는 Redis Streams를 쓰지 마세요.**

---

## 10. 모니터링

### 10-1. 핵심 지표

```rust
// outbox 밀림 — 가장 중요
metrics::gauge!("outbox_pending").set(pending_count as f64);
metrics::gauge!("outbox_oldest_age_seconds").set(oldest_age);
metrics::counter!("outbox_published_total").increment(n);
metrics::counter!("outbox_dead_total").increment(1);
metrics::histogram!("outbox_publish_duration_seconds").record(elapsed);

// 소비자
metrics::counter!("events_consumed_total", "type" => ty).increment(1);
metrics::counter!("events_duplicate_total", "consumer" => c).increment(1);
metrics::counter!("events_dlq_total").increment(1);
metrics::histogram!("event_e2e_latency_seconds").record(now - event.time);
```

`outbox_oldest_age_seconds`가 **가장 유용한 단일 지표**입니다.
"가장 오래된 미발행 이벤트가 몇 초 됐나" — 이게 곧 시스템 지연입니다.

```rust
async fn collect_metrics(pool: &PgPool) -> anyhow::Result<()> {
    let (pending, oldest): (i64, Option<f64>) = sqlx::query_as(
        r#"
        SELECT COUNT(*),
               EXTRACT(EPOCH FROM (now() - MIN(created_at)))
        FROM outbox WHERE published_at IS NULL AND dead = false
        "#
    ).fetch_one(pool).await?;

    metrics::gauge!("outbox_pending").set(pending as f64);
    metrics::gauge!("outbox_oldest_age_seconds").set(oldest.unwrap_or(0.0));
    Ok(())
}
```

### 10-2. 알림

```yaml
- alert: OutboxBacklog
  expr: outbox_oldest_age_seconds > 60
  for: 5m
  annotations: { summary: "이벤트 발행이 1분 이상 밀림" }

- alert: OutboxDead
  expr: increase(outbox_dead_total[10m]) > 0
  labels: { severity: critical }

- alert: EventDlq
  expr: increase(events_dlq_total[10m]) > 0
  labels: { severity: critical }

- alert: EventE2eLatency
  expr: histogram_quantile(0.99, rate(event_e2e_latency_seconds_bucket[5m])) > 30
  annotations: { summary: "이벤트 종단 지연 p99 > 30s" }
```

### 10-3. 정합성 감사

이벤트 기반 시스템에서는 **결국 어긋납니다.** 주기적으로 대조하세요.

```rust
/// 예: order-service의 주문 수 vs notification-service가 받은 order.created 수
async fn audit_daily() -> anyhow::Result<()> {
    let orders = count_orders_yesterday().await?;
    let notified = count_notifications_yesterday().await?;

    let drift = (orders - notified).abs();
    metrics::gauge!("consistency_drift", "pair" => "order_notification")
        .set(drift as f64);

    if drift > orders / 100 {   // 1% 이상 차이
        tracing::error!(orders, notified, drift, "consistency drift detected");
    }
    Ok(())
}
```

이런 감사가 없으면 **몇 달 뒤에 고객이 먼저 발견합니다.**

---

## 11. 07 §6의 Saga를 다시

기존 문서는 *"초보 팀은 코레오그래피 + 명확한 상태머신"* 을 권합니다. 동의하되, 조건을 붙입니다.

### 11-1. 코레오그래피의 진짜 비용

```
OrderCreated → InventoryReserved → PaymentCaptured → OrderConfirmed
```

각 서비스가 다음을 알아서 하므로 **중앙 코드가 없습니다.**
이건 장점이자 단점입니다.

| 상황 | 코레오그래피 |
|---|---|
| 흐름 파악 | 4개 서비스 코드를 다 봐야 함 |
| 지금 어디까지 갔나 | 알 수 없음 (각자 자기 상태만) |
| 3분째 멈춰 있으면 | 아무도 모름 |
| 타임아웃 | 구현 지점이 없음 |

**3단계를 넘으면 오케스트레이션(중앙 코디네이터)이 낫습니다.**

### 11-2. 상태를 명시적으로

어느 쪽이든 **saga 상태를 DB에 저장**하세요.

```sql
CREATE TABLE order_saga (
    order_id     UUID PRIMARY KEY,
    state        TEXT NOT NULL,     -- 'pending'|'reserved'|'paid'|'confirmed'|'compensating'|'failed'
    step         INT NOT NULL DEFAULT 0,
    started_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    deadline_at  TIMESTAMPTZ NOT NULL,   -- 이 시각까지 안 끝나면 보상
    last_error   TEXT
);
CREATE INDEX ON order_saga (state, deadline_at);
```

```rust
/// 타임아웃 감시자 — 이게 없으면 saga가 영원히 멈춰 있습니다
async fn saga_timeout_sweeper(pool: &PgPool) -> anyhow::Result<()> {
    let stuck: Vec<SagaRow> = sqlx::query_as(
        "SELECT * FROM order_saga
         WHERE state NOT IN ('confirmed', 'failed')
           AND deadline_at < now()
         LIMIT 100"
    ).fetch_all(pool).await?;

    for saga in stuck {
        tracing::error!(order_id = %saga.order_id, state = %saga.state, "saga timed out");
        start_compensation(pool, &saga).await?;
    }
    Ok(())
}
```

**보상 트랜잭션도 실패할 수 있습니다.** 보상 실패는 사람이 개입해야 하므로 반드시 알림을 거세요.

### 11-3. 07의 조언은 여전히 맞습니다

> **아직 재고/결제가 한 팀·한 DB면 굳이 쪼개지 마세요.**

이 문장이 이 장에서 가장 중요합니다.
로컬 트랜잭션 하나로 되는 것을 saga로 만들면, **위의 모든 복잡도를 이유 없이 짊어집니다.**

---

## 체크포인트

```
[ ] outbox에 attempts / next_attempt_at / dead 컬럼이 있다
[ ] 미발행 행에 대한 부분 인덱스가 있다
[ ] 지수 백오프 + 상한이 있다
[ ] max_attempts 초과 시 dead 처리되어 뒤가 막히지 않는다
[ ] dead 이벤트에 알림이 걸려 있다
[ ] 영구 오류와 일시 오류를 구분한다
[ ] publish가 긴 트랜잭션 밖에서 일어난다 (클레임 방식)
[ ] claimed_until 만료로 죽은 워커의 건이 회수된다
[ ] 순서가 필요한 이벤트에 aggregate_id 파티션 키가 있다
[ ] 가능하면 소비자가 버전으로 스테일을 거른다
[ ] 발행된 outbox 행이 정리되거나 파티셔닝된다
[ ] 소비자에 processed_events(inbox) 테이블이 있다
[ ] inbox 기록과 비즈니스 로직이 같은 트랜잭션이다
[ ] 이벤트 봉투에 correlation_id / causation_id가 있다
[ ] 스키마 진화 규칙(추가만)이 문서화됐다
[ ] deny_unknown_fields를 쓰지 않는다
[ ] NATS consumer에 durable/ack_wait/max_deliver/backoff가 설정됐다
[ ] max_deliver 도달 건이 DLQ에 남는다
[ ] outbox_oldest_age_seconds에 알림이 있다
[ ] 정기 정합성 감사가 있다
[ ] saga 상태와 타임아웃 감시자가 있다
```

---

다음: [15_observability_deep — 트레이스가 실제로 이어지게](./15_observability_deep.md)
