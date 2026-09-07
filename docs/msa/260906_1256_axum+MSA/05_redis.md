# 05. Redis — 캐시 · 세션 · 락 · Rate Limit

NestJS `cache-manager` / Spring `RedisTemplate` 자리를 Redis가 채웁니다.  
MSA에서 Redis는 **DB 대체가 아니라 가속·조율 계층**입니다.

---

## 1. 역할 정리

| 용도 | 키 예 | TTL | 비고 |
|---|---|---|---|
| 캐시 | `user:{id}` | 60s~10m | stampede 주의 |
| 세션 / refresh | `sess:{id}` | 정책 따름 | JWT만 쓰면 축소 가능 |
| 분산 락 | `lock:order:{id}` | 짧음 + 연장 | 중복 결제 방지 |
| Rate limit | `rl:{ip}:{route}` | 윈도우 | gateway에 |
| 단기 큐 | Redis Streams | - | 소규모면 NATS 대체 가능 |
| Idempotency | `idem:{key}` | 24h | POST 멱등 |

**서비스별 Redis DB index 또는 key prefix**로 충돌을 막습니다.

```
user-service:   user:...
order-service:  order:...
gateway:        gw:...
```

한 Redis를 공유하되, **flushall 금지**, ACL/논리 분리 권장.

---

## 2. Compose

```yaml
  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]
    command: ["redis-server", "--appendonly", "yes"]
    volumes: ["redis_data:/data"]
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 10
```

```bash
REDIS_URL=redis://127.0.0.1:6379
```

프로덕션: Redis Cluster / ElastiCache / Memorystore + 비밀번호 + TLS.

---

## 3. Rust 클라이언트 선택

| 크레이트 | 특징 |
|---|---|
| `redis` + `ConnectionManager` | 공식에 가깝고 흔함, 시작용 추천 |
| `fred` | 고성능, cluster 친화 |
| `deadpool-redis` | 풀 명시적 관리 |

이 가이드는 `redis` 기준입니다.

```toml
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }
```

```rust
use redis::aio::ConnectionManager;

let client = redis::Client::open(settings.redis.url.as_str())?;
let conn = ConnectionManager::new(client).await?;
```

`ConnectionManager`는 자동 재연결을 제공합니다. `AppState`에 `Clone`으로 넣습니다.

---

## 4. 캐시-aside 패턴 (가장 흔함)

```rust
use redis::AsyncCommands;
use uuid::Uuid;

pub async fn find_by_id(&self, id: Uuid) -> Result<UserRow, UserError> {
    let key = format!("user:{id}");

    // 1) cache
    let mut conn = self.redis.clone();
    if let Ok(Some(json)) = conn.get::<_, Option<String>>(&key).await {
        if let Ok(user) = serde_json::from_str::<UserRow>(&json) {
            return Ok(user);
        }
    }

    // 2) db
    let user = self
        .repo
        .find_by_id(id)
        .await?
        .ok_or(UserError::NotFound)?;

    // 3) set
    let payload = serde_json::to_string(&user)?;
    let _: Result<(), _> = conn.set_ex(&key, payload, 300).await; // 실패해도 본 요청은 성공

    Ok(user)
}

pub async fn invalidate_user(&self, id: Uuid) {
    let mut conn = self.redis.clone();
    let _: Result<(), _> = conn.del(format!("user:{id}")).await;
}
```

업데이트/삭제 후 **반드시 invalidate** (또는 short TTL만 믿기).

### Cache stampede 완화

- TTL에 jitter (`300 + rand(0..30)`)
- 단일 플라이트: `SET lock:... NX EX 5` 후 로더 하나만 DB 조회

---

## 5. Rate limiting (Gateway)

고정 윈도우 초간단 버전:

```rust
/// 분당 max 요청
pub async fn allow(
    conn: &mut ConnectionManager,
    key: &str,
    limit: i64,
    window_secs: i64,
) -> redis::RedisResult<bool> {
    let count: i64 = redis::cmd("INCR").arg(key).query_async(conn).await?;
    if count == 1 {
        let _: () = redis::cmd("EXPIRE").arg(key).arg(window_secs).query_async(conn).await?;
    }
    Ok(count <= limit)
}
```

키: `gw:rl:{ip}:{route}`

실무에서는 **sliding window / token bucket** 또는 `tower` rate limit + Redis 를 검토합니다.  
정확도가 중요하면 `redis-cell` / Lua 스크립트.

---

## 6. 분산 락

중복 주문/결제 방지:

```rust
pub async fn try_lock(
    conn: &mut ConnectionManager,
    key: &str,
    token: &str,
    ttl_secs: i64,
) -> redis::RedisResult<bool> {
    // SET key token NX EX ttl
    let r: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(token)
        .arg("NX")
        .arg("EX")
        .arg(ttl_secs)
        .query_async(conn)
        .await?;
    Ok(r.is_some())
}
```

해제는 **token 비교 후 DEL** (Lua) — 남의 락을 지우지 않기.

긴 작업은 TTL 연장(watchdog) 또는 DB 유니크 제약과 이중으로 방어합니다.  
**락만으로 돈을 지키지 마세요.** DB 멱등/유니크가 본진입니다.

---

## 7. Idempotency-Key

클라이언트가 `Idempotency-Key` 헤더를 주면:

```
SET idem:order:{key}  → processing
... 작업 ...
SET idem:order:{key}  → {"order_id":"..."}  EX 86400
```

같은 키 재요청 시 저장된 응답 반환.  
결제·주문 POST에 특히 중요 (Stripe 패턴).

---

## 8. 세션 vs JWT

| | JWT (stateless) | Redis 세션 |
|---|---|---|
| 확장 | 쉬움 | Redis 의존 |
| 강제 로그아웃 | 블랙리스트 필요 | 키 삭제면 끝 |
| 크기 | 클레임 커질 수 있음 | 서버에 상태 |

추천 하이브리드:
- Access Token: 짧은 JWT (5~15m)
- Refresh Token: DB 또는 Redis에 해시 저장, 회전(rotation)

---

## 9. 장애 정책

```rust
match cache_get(...).await {
    Ok(Some(v)) => return v,
    Ok(None) => { /* miss */ }
    Err(e) => {
        tracing::warn!(error = %e, "redis unavailable, fallback db");
        // DB로 진행 — 캐시를 hard dependency로 두지 않기
    }
}
```

Rate limit용 Redis가 죽으면:
- **fail-open** (통과, 가용성 우선) 또는
- **fail-closed** (429/503, 보호 우선)

Gateway는 보통 정책에 따라 명시합니다.

---

## 10. 모니터링할 메트릭

- `redis_command_duration`
- hit / miss ratio
- connection errors
- evicted keys
- 메모리 사용량

캐시 hit가 너무 낮으면 TTL·키 설계를 의심합니다.

---

## 체크포인트

```
[ ] REDIS_URL과 key prefix 규칙이 있다
[ ] 캐시는 aside + invalidate 정책을 정했다
[ ] Redis 장애 시 fallback이 문서화됐다
[ ] 결제/주문은 DB 유니크 + idempotency를 함께 쓴다
[ ] gateway rate limit 키 설계가 있다
```

다음: [06_gateway_auth — API Gateway와 JWT](./06_gateway_auth.md)
