# 07. 서비스 간 통신 — 동기 · 비동기 · Outbox

Nest microservices / Spring Cloud의 핵심은 **동기 RPC + 이벤트 버스**입니다.  
Axum MSA도 동일하게 가져가되, 처음부터 Kafka 풀세팅은 과합니다.

---

## 1. 선택 가이드

| 패턴 | 언제 | Rust 쪽 |
|---|---|---|
| HTTP/JSON | 기본, 디버깅 쉬움 | `reqwest` |
| gRPC | 내부 핫패스, 스키마 강제 | `tonic` + `prost` |
| NATS JetStream | 중소규모 이벤트, 운영 단순 | `async-nats` |
| Kafka | 대규모 로그/이벤트, 이미 인프라 있음 | `rdkafka` |
| Redis Streams | 이미 Redis만 있을 때 가벼운 큐 | `redis` |

**추천 시작:** HTTP + NATS JetStream (또는 Redis Streams)  
**나중에:** 병목만 gRPC, 이벤트 볼륨 커지면 Kafka.

---

## 2. 동기 호출 (order → user)

```rust
#[derive(Clone)]
pub struct UserClient {
    http: reqwest::Client,
    base: String,
}

impl UserClient {
    pub async fn get_user(&self, id: uuid::Uuid) -> Result<UserDto, ClientError> {
        let url = format!("{}/v1/users/{}", self.base, id);
        let res = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(2))
            .header("x-request-id", /* 전파 */)
            .send()
            .await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ClientError::NotFound);
        }
        if !res.status().is_success() {
            return Err(ClientError::Upstream(res.status()));
        }
        Ok(res.json().await?)
    }
}
```

### 규칙

1. **타임아웃 필수** (기본 1~3s)
2. GET만 제한적 재시도 (멱등), POST는 재시도 금지 또는 Idempotency-Key
3. Circuit breaker — `tower::retry` / 직접 상태머신 / `failsafe` 류
4. request-id · trace context 헤더 전파

주문 목록에 매번 user를 치면 N+1이 됩니다 → **스냅샷·배치 API·캐시**.

---

## 3. 이벤트 계약

```json
{
  "id": "018f...",
  "type": "order.created",
  "source": "order-service",
  "time": "2026-07-23T12:00:00Z",
  "specversion": "1.0",
  "data": {
    "order_id": "...",
    "user_id": "...",
    "total_cents": 12000,
    "currency": "KRW"
  }
}
```

`crates/events` 에 serde 타입을 공유:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderCreated {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub total_cents: i64,
    pub currency: String,
}
```

버전 업: `order.created.v2` 또는 data 스키마 진화 규칙을 문서화.

---

## 4. Outbox (신뢰할 수 있는 발행)

문제: DB 커밋 후 `publish`가 실패하면 이벤트 유실.

해결: **같은 트랜잭션에 outbox row**

```
begin
  insert order
  insert outbox(topic, payload)
commit

worker:
  select ... for update skip locked
  publish
  mark published_at
```

```rust
pub async fn create_order(&self, cmd: CreateOrder) -> Result<Order, OrderError> {
    let mut tx = self.pool.begin().await?;

    let order = OrderRepo::insert(&mut tx, &cmd).await?;
    let payload = serde_json::to_value(OrderCreated { /* ... */ })?;
    OutboxRepo::push(&mut tx, "order.created", payload).await?;

    tx.commit().await?;
    Ok(order)
}
```

워커는 같은 서비스 프로세스의 `tokio::spawn` 루프여도 되고, 별도 바이너리여도 됩니다.

---

## 5. NATS JetStream 스케치

Compose:

```yaml
  nats:
    image: nats:2.10
    command: ["-js", "-m", "8222"]
    ports: ["4222:4222", "8222:8222"]
```

발행:

```rust
let client = async_nats::connect("nats://127.0.0.1:4222").await?;
let js = async_nats::jetstream::new(client);
js.publish("order.created", bytes.into()).await?.await?;
```

소비 (notification-service):

```rust
let mut consumer = /* durable consumer */;
while let Some(msg) = consumer.next().await {
    let event: OrderCreated = serde_json::from_slice(&msg.payload)?;
    notify(&event).await?;
    msg.ack().await?;
}
```

**적어도 한 번(at-least-once)** 이므로 소비자는 **멱등**이어야 합니다  
(`notification_log`에 `event_id` UNIQUE).

---

## 6. Saga (필요해질 때만)

주문 → 재고 차감 → 결제 예:

```
OrderPending
  → InventoryReserved
  → PaymentCaptured
  → OrderConfirmed

실패 시:
  PaymentFailed → InventoryRelease → OrderCancelled
```

오케스트레이션(중앙 코디네이터) vs 코레오그래피(이벤트만으로 진행).  
초보 팀은 **코레오그래피 + 명확한 상태머신**이 운영이 쉽습니다.

**아직 재고/결제가 한 팀·한 DB면 굳이 쪼개지 마세요.**

---

## 7. API 버전과 호환

- URL: `/v1/...`
- 이벤트: 필드 추가만 (제거·의미변경은 새 type)
- 클라이언트 SDK가 있다면 workspace에 `crates/clients` 로 생성

---

## 8. 안티패턴

| 안티패턴 | 결과 |
|---|---|
| 동기 체인 A→B→C→D | 지연·장애 전파 |
| 이벤트 없이 DB 공유 | MSA 붕괴 |
| 소비자 non-idempotent | 중복 알림/중복 차감 |
| fire-and-forget publish after commit without outbox | 유실 |
| 모든 걸 Kafka | 운영 비용 > 이득 |

---

## 체크포인트

```
[ ] 동기 호출에 timeout이 있다
[ ] 쓰기 연계는 outbox를 검토했다
[ ] 이벤트 소비가 멱등이다
[ ] 호출 체인 깊이가 정책 이하다
[ ] 로컬에서 NATS 또는 Redis Streams로 한 사이클이 돈다
```

다음: [08_observability — 로그·트레이스·메트릭](./08_observability.md)
