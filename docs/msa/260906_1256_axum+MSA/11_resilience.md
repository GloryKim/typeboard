# 11. 회복탄력성 — graceful shutdown · 과부하 방어 · 런타임 튜닝

[03_service_anatomy](./03_service_anatomy.md)는 서비스가 **어떻게 요청을 처리하는지**를 다뤘습니다.
이 문서는 서비스가 **어떻게 죽고, 어떻게 안 죽는지**를 다룹니다.

MSA에서 이 두 가지가 장애의 대부분을 차지합니다.

> NestJS의 `app.enableShutdownHooks()`, Spring Boot의 `server.shutdown=graceful`,
> Spring Cloud의 서킷브레이커·벌크헤드가 여기서는 **전부 직접 만들어야 하는 것들**입니다.

---

## 1. Graceful shutdown — 없으면 배포마다 5xx

### 1-1. 지금 무슨 일이 일어나는가

[03 §6](./03_service_anatomy.md)의 main은 이렇습니다.

```rust
axum::serve(listener, app).await?;
```

K8s가 파드를 교체할 때:

```
1. K8s가 SIGTERM 전송
2. 프로세스 즉시 종료
3. 진행 중이던 요청 N개 → 커넥션 강제 종료 → 클라이언트는 502/ECONNRESET
4. outbox 워커가 publish 직전이었다면 → 그 이벤트는 다음 폴링까지 지연
5. 트랜잭션이 열려 있었다면 → PG가 타임아웃까지 락을 잡고 있음
```

`replicas: 3` 짜리 서비스를 롤링 업데이트하면 이게 **3번** 일어납니다.
배포 빈도가 하루 5회면 하루 15번의 에러 스파이크입니다.

"배포할 때만 잠깐 에러 나는 거야"로 넘어가는 팀이 많은데, **HPA가 스케일 다운할 때도 똑같이 일어납니다.**
트래픽이 빠지는 시점 = 축소가 일어나는 시점이라 눈에 잘 안 띌 뿐입니다.

### 1-2. 최소 구현

```rust
// main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = Settings::load()?;
    let app = build_app(settings.clone()).await?;

    let listener = tokio::net::TcpListener::bind(&settings.http.addr).await?;
    tracing::info!(addr = %settings.http.addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c    => tracing::info!("received SIGINT"),
        _ = terminate => tracing::info!("received SIGTERM"),
    }
}
```

`with_graceful_shutdown`은 axum 0.8.9에 실존합니다.
신호를 받으면 **새 커넥션 수락을 멈추고, 진행 중인 요청이 끝날 때까지 기다립니다.**

**컨테이너에서는 SIGTERM이 핵심입니다.** `Ctrl+C`(SIGINT)만 처리하면 K8s/Docker에서는 아무 효과가 없습니다.

### 1-3. 그런데 이것만으로는 부족합니다 — 드레인 순서

여기가 대부분이 놓치는 지점입니다.

```
문제: SIGTERM을 받자마자 리스너를 닫으면, 그 순간 LB가 아직 이 파드로 요청을 보내고 있음
      → LB의 엔드포인트 갱신은 비동기 (kube-proxy/iptables 전파에 수 초 걸림)
      → 리스너가 닫힌 파드로 간 요청 = ECONNREFUSED
```

**SIGTERM 직후 바로 닫으면 안 됩니다.** 순서는 이렇습니다.

```
1. SIGTERM 수신
2. /ready 를 즉시 실패로 전환        ← LB에게 "나 빼줘"
3. LB가 나를 뺄 때까지 대기 (5~15s)   ← 이 시간 동안 요청을 계속 정상 처리
4. 리스너 닫기 + 진행 중 요청 대기
5. 백그라운드 태스크 정리
6. DB/Redis 커넥션 닫기
7. 종료
```

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct Health {
    /// false가 되면 /ready가 503을 반환한다
    ready: Arc<AtomicBool>,
}

impl Health {
    pub fn new() -> Self { Self { ready: Arc::new(AtomicBool::new(true)) } }
    pub fn is_ready(&self) -> bool { self.ready.load(Ordering::Relaxed) }
    pub fn set_draining(&self) { self.ready.store(false, Ordering::Relaxed); }
}
```

```rust
async fn shutdown_signal(health: Health, drain: Duration) {
    wait_for_signal().await;

    // 2) 즉시 unready — 새 트래픽 유입 차단 요청
    tracing::info!("draining: marking /ready as unhealthy");
    health.set_draining();

    // 3) LB가 나를 제외할 시간을 준다. 이 동안 요청은 계속 정상 처리된다.
    tracing::info!(drain_secs = drain.as_secs(), "waiting for load balancer to drain");
    tokio::time::sleep(drain).await;

    tracing::info!("closing listener");
    // 반환 → axum이 리스너를 닫고 in-flight 요청 완료를 기다린다
}
```

```rust
// /ready 핸들러
async fn ready(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    if !state.health.is_ready() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);   // 드레인 중
    }
    // 의존성 체크는 §1-6 참고 (캐싱 필요)
    Ok("ready")
}
```

### 1-4. K8s 쪽 설정도 같이

앱만 고쳐서는 안 됩니다.

```yaml
spec:
  terminationGracePeriodSeconds: 60   # 드레인 + in-flight 처리에 충분해야 함
  containers:
    - name: user-service
      lifecycle:
        preStop:
          exec:
            # 앱이 드레인을 직접 하면 생략 가능하지만, 이중 안전망으로 흔히 둠
            command: ["sh", "-c", "sleep 5"]
      readinessProbe:
        httpGet: { path: /ready, port: 3001 }
        periodSeconds: 2          # 짧게 — 드레인 감지를 빨리
        failureThreshold: 2
      livenessProbe:
        httpGet: { path: /health, port: 3001 }
        periodSeconds: 10
        failureThreshold: 3
        initialDelaySeconds: 10
```

**`terminationGracePeriodSeconds`가 앱의 드레인+처리 시간보다 짧으면 K8s가 SIGKILL을 보냅니다.**
드레인 10초 + 최대 요청 15초 = 최소 30초 이상은 줘야 합니다.

| 값 | 관계 |
|---|---|
| `terminationGracePeriodSeconds` | > 드레인 시간 + 최대 요청 처리 시간 + 여유 |
| 드레인 시간 | > readinessProbe `periodSeconds × failureThreshold` |
| 최대 요청 처리 시간 | = `TimeoutLayer`의 값 |

### 1-5. 백그라운드 태스크도 같이 멈춰야 합니다

[07 §4](./07_messaging.md)의 outbox 워커, NATS consumer는 `tokio::spawn`으로 돌고 있습니다.
main이 끝나면 **그냥 사라집니다.** publish 도중이었다면 그 순간의 상태가 애매해집니다.

`tokio_util::sync::CancellationToken`으로 묶으세요.

```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

```rust
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let shutdown = CancellationToken::new();
    let tracker  = TaskTracker::new();

    // outbox 워커
    {
        let token = shutdown.clone();
        let pool  = pool.clone();
        tracker.spawn(async move { outbox_worker(pool, token).await });
    }

    // NATS consumer
    {
        let token = shutdown.clone();
        tracker.spawn(async move { event_consumer(js, token).await });
    }
    tracker.close();

    // HTTP 서버
    let server = axum::serve(listener, app)
        .with_graceful_shutdown({
            let token = shutdown.clone();
            let health = health.clone();
            async move {
                wait_for_signal().await;
                health.set_draining();
                tokio::time::sleep(DRAIN).await;
                token.cancel();          // 백그라운드 태스크에도 알림
            }
        });

    server.await?;

    // 백그라운드 태스크 정리 — 단, 무한정 기다리지 않는다
    tracing::info!("waiting for background tasks");
    match tokio::time::timeout(Duration::from_secs(20), tracker.wait()).await {
        Ok(_)  => tracing::info!("background tasks finished"),
        Err(_) => tracing::warn!("background tasks did not finish in time, forcing exit"),
    }

    pool.close().await;   // sqlx 풀을 명시적으로 닫아 PG 세션 정리
    tracing::info!("bye");
    Ok(())
}
```

워커 루프는 취소를 관측해야 합니다.

```rust
async fn outbox_worker(pool: PgPool, shutdown: CancellationToken) {
    let mut ticker = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            biased;                                   // 취소를 먼저 확인
            _ = shutdown.cancelled() => {
                tracing::info!("outbox worker: shutdown requested");
                break;
            }
            _ = ticker.tick() => {
                // 한 배치는 중간에 끊지 않고 끝까지 — 원자성 유지
                if let Err(e) = publish_batch(&pool).await {
                    tracing::error!(error = %e, "outbox batch failed");
                }
            }
        }
    }
    tracing::info!("outbox worker stopped");
}
```

> `biased;`를 쓰면 `select!`가 위에서부터 순서대로 폴링합니다.
> 취소 신호를 항상 먼저 보게 되므로 종료가 빨라집니다.

### 1-6. `/ready`의 의존성 체크는 캐싱하세요

[08 §4](./08_observability.md)의 `/ready`는 매번 `SELECT 1`을 칩니다.

```
replicas 20개 × periodSeconds 2초 = 초당 10 쿼리
+ 어떤 이유로 DB가 느려지면 → 모든 파드의 /ready가 동시에 느려짐
→ 전부 트래픽에서 빠짐 → 서비스 전체 다운
```

**헬스체크가 장애를 만드는** 전형적인 패턴입니다.

```rust
pub struct ReadinessCache {
    last_check: Arc<Mutex<(Instant, bool)>>,
}

impl ReadinessCache {
    pub async fn check(&self, pool: &PgPool) -> bool {
        const TTL: Duration = Duration::from_secs(5);
        let mut guard = self.last_check.lock().await;
        let (at, ok) = *guard;
        if at.elapsed() < TTL {
            return ok;   // 캐시된 결과
        }

        let fresh = tokio::time::timeout(
            Duration::from_secs(2),                 // 체크 자체에도 타임아웃
            sqlx::query("SELECT 1").execute(pool),
        ).await.is_ok_and(|r| r.is_ok());

        *guard = (Instant::now(), fresh);
        fresh
    }
}
```

**원칙:**

| 엔드포인트 | 체크 대상 | 실패 시 |
|---|---|---|
| `/health` (liveness) | **프로세스만.** 의존성 절대 금지 | 재시작 |
| `/ready` (readiness) | 필수 의존성만, 캐싱, 타임아웃 | 트래픽 제외 |
| `/startup` (startup) | 마이그레이션·워밍업 완료 | 아직 시작 중 |

[08 §4](./08_observability.md)의 *"DB 순간 흔들림에 `/health`까지 묶지 마세요"* 는 정확합니다.
여기에 **"`/ready`도 매번 치지 마세요"** 를 추가하는 겁니다.

---

## 2. 요청 바디 크기 제한

axum의 기본 바디 제한은 **2MB**입니다 (axum-core 0.5.6의 `DEFAULT_LIMIT = 2_097_152` 확인).
기본값이 있다는 건 좋지만, 엔드포인트별로 조정해야 합니다.

```rust
use axum::extract::DefaultBodyLimit;

Router::new()
    // 일반 API는 더 빡빡하게
    .route("/v1/users", post(register))
    .layer(DefaultBodyLimit::max(16 * 1024))          // 16KB
    // 업로드 경로만 크게
    .route("/v1/uploads", post(upload)
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024)))  // 20MB
```

`Content-Length`가 없는 chunked 요청도 있으므로, 스트리밍 경로에는 `RequestBodyLimitLayer`도 같이 씁니다.

```rust
use tower_http::limit::RequestBodyLimitLayer;
.layer(RequestBodyLimitLayer::new(20 * 1024 * 1024))
```

> [06 §4](./06_gateway_auth.md)의 gateway는 1MB로 하드코딩되어 있습니다.
> 값 자체보다 **"엔드포인트마다 다르게, 명시적으로"** 가 중요합니다.

---

## 3. 패닉 격리 — `CatchPanicLayer`

### 왜 필요한가

Rust에서도 패닉은 납니다.

```rust
slice[i]                    // 인덱스 초과
map["key"]                  // 없는 키
a / b                       // 정수 0 나눗셈
opt.unwrap()                // 06 §4에 4개 있음 (10_errata §12-8)
i32::from(big) .unwrap()    // 변환 실패
chrono 연산 오버플로
```

기본 동작은 **해당 태스크만 죽고 커넥션이 끊깁니다.**

- 클라이언트는 응답 없이 연결 종료 → 원인 불명
- 로그에 스택트레이스는 남지만 `request_id`가 없어 어떤 요청인지 모름
- 메트릭의 `http_requests_total`에도 안 잡힘 → **에러율 대시보드가 정상으로 보임**

가장 나쁜 조합입니다. 사용자는 깨지는데 대시보드는 초록색입니다.

### 켜기

```toml
tower-http = { version = "0.6", features = ["catch-panic", ...] }
```

```rust
use tower_http::catch_panic::CatchPanicLayer;

// 기본: 500 + "Service Internal Error"
.layer(CatchPanicLayer::new())
```

에러 포맷을 맞추려면 커스텀 핸들러:

```rust
use std::any::Any;

fn handle_panic(err: Box<dyn Any + Send + 'static>) -> Response {
    let details = if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else {
        "unknown panic".to_string()
    };

    // 패닉은 반드시 별도 메트릭으로 — 알림 대상
    metrics::counter!("panics_total").increment(1);
    tracing::error!(panic = %details, "handler panicked");

    // 클라이언트에는 상세를 주지 않는다 (10_errata §3)
    let body = serde_json::json!({
        "error": "internal",
        "message": "internal server error",
        "trace_id": current_trace_id(),
    });
    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(body)).into_response()
}

.layer(CatchPanicLayer::custom(handle_panic))
```

### 주의: `panic = "abort"`를 쓰면 무력화됩니다

이미지 크기를 줄이려고 `Cargo.toml`에 이렇게 쓰는 경우가 있습니다.

```toml
[profile.release]
panic = "abort"     # ❌ CatchPanicLayer가 동작하지 않음 — 프로세스가 즉사
```

서버에서는 **`unwind`(기본값)를 유지**하세요. 요청 하나의 버그로 프로세스 전체가 죽으면 안 됩니다.

### 그리고 패닉은 절대 정상이 아닙니다

`CatchPanicLayer`는 **안전망이지 해결책이 아닙니다.** `panics_total > 0`이면 즉시 알림을 걸고 원인을 제거하세요.

```yaml
# Alertmanager
- alert: HandlerPanic
  expr: increase(panics_total[5m]) > 0
  labels: { severity: critical }
```

---

## 4. 과부하 방어 — 동시성 제한과 load shedding

### 4-1. Rust가 빠른 게 오히려 문제입니다

```
초당 5,000 요청 도착 / 서비스 처리 능력 3,000 rps
→ 초당 2,000개씩 큐에 쌓임
→ tokio는 태스크를 거의 공짜로 만드니 계속 받아들임
→ 메모리 증가 → 지연 폭증 → 전부 타임아웃 → OOM Kill
```

여기서 최악은 **"이미 클라이언트가 포기한 요청을 계속 처리하는 것"** 입니다.
15초 타임아웃 후 클라이언트는 재시도했는데, 서버는 그 15초짜리 작업을 여전히 붙들고 있습니다.
재시도가 겹치면 **부하가 스스로 증폭**됩니다 (retry storm).

### 4-2. 처리할 수 없으면 즉시 거절

```toml
tower = { version = "0.5", features = ["limit", "load-shed", "util"] }
```

```rust
use tower::{ServiceBuilder, limit::ConcurrencyLimitLayer, load_shed::LoadShedLayer};

let app = router()
    .layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            // LoadShed가 ConcurrencyLimit보다 바깥이어야
            // "한도 초과 시 대기"가 아니라 "즉시 거절"이 된다
            .layer(HandleErrorLayer::new(handle_overload))
            .layer(LoadShedLayer::new())
            .layer(ConcurrencyLimitLayer::new(512))
            .layer(TimeoutLayer::new(Duration::from_secs(15)))
    )
    .with_state(state);

async fn handle_overload(err: tower::BoxError) -> Response {
    if err.is::<tower::load_shed::error::Overloaded>() {
        metrics::counter!("load_shed_total").increment(1);
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [("retry-after", "1")],
            "overloaded",
        ).into_response();
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "internal").into_response()
}
```

**순서가 핵심입니다.** `LoadShed`가 안쪽에 있으면 큐가 그대로 쌓입니다.

```
LoadShed(바깥) → ConcurrencyLimit(안쪽)
  = 동시 512개를 넘으면 즉시 503        ✅

ConcurrencyLimit(바깥) → LoadShed(안쪽)
  = 512개를 넘으면 대기 (큐 증가)       ❌ 의미 없음
```

### 4-3. 한도는 어떻게 정하나

추측하지 말고 **부하 테스트로 측정**하세요 ([17_testing §5](./17_testing.md)).

```
1. 부하를 서서히 올리며 p99 지연을 관측
2. p99가 SLO(예: 300ms)를 넘기 시작하는 지점의 동시 요청 수 = C
3. ConcurrencyLimit ≈ C  (약간 여유)
```

**리틀의 법칙**으로 검산합니다.

```
동시성 = 처리율 × 평균 지연
예) 3,000 rps × 0.05s = 150
→ 정상 상태 동시성이 150이면, 한도 512는 "3배 스파이크까지 흡수"를 의미
```

거절이 훨씬 낫습니다.

| | 전부 받고 다 느려짐 | 일부 거절하고 나머지는 빠름 |
|---|---|---|
| 성공한 요청 | 0% (전부 타임아웃) | 70% (즉시 성공) |
| 실패한 요청 | 100% (15초 후) | 30% (즉시, 재시도 가능) |
| 서버 상태 | OOM 위험 | 안정 |

### 4-4. 엔드포인트별 차등

전역 한도만으로는 부족합니다. 무거운 엔드포인트가 가벼운 엔드포인트를 굶깁니다.

```rust
Router::new()
    .route("/v1/users/{id}", get(get_user))            // 캐시 히트, 1ms
    .route("/v1/reports/heavy", get(heavy_report)
        .layer(ConcurrencyLimitLayer::new(4)))         // 5초짜리 → 4개만
    .route("/v1/auth/login", post(login)
        .layer(ConcurrencyLimitLayer::new(16)))        // argon2 (10_errata §6)
```

---

## 5. 타임아웃 예산과 데드라인 전파

### 5-1. 지금 뭐가 문제인가

기존 문서의 값들을 모으면:

```
gateway TimeoutLayer      : 15s   (03 §7)
gateway → order 호출      : ?     (06에 없음)
order 서비스 TimeoutLayer : 15s
order → user 호출         : 2s    (07 §2)
DB acquire_timeout        : 5s    (04 §4)
```

문제:

1. **gateway와 order가 둘 다 15초** → gateway가 15초를 기다리는 동안 order도 15초를 씁니다. 아무도 먼저 포기하지 않습니다.
2. **클라이언트가 3초에 포기해도** 서버는 15초 동안 계속 일합니다. 낭비 + 부하 증폭.
3. **`acquire_timeout` 5초 + 쿼리 무제한**이면 실제 요청은 15초를 다 채웁니다.

### 5-2. 예산은 안쪽으로 갈수록 줄어야 합니다

```
클라이언트          10s  (앱이 포기하는 시점)
  └ gateway          8s  (클라이언트보다 짧게)
      └ order        6s
          └ user     2s   ← 07 §2가 맞음
          └ DB 쿼리  1.5s
```

**규칙: 각 홉은 자기를 부른 쪽보다 반드시 짧은 타임아웃을 가진다.**

바깥이 더 짧으면 안쪽 작업은 아무도 안 보는 곳에서 계속 돌아갑니다.

### 5-3. 데드라인 전파

정적 값보다 **남은 시간**을 넘기는 게 정확합니다.

```rust
#[derive(Clone, Copy, Debug)]
pub struct Deadline(pub Instant);

impl Deadline {
    pub fn remaining(&self) -> Duration {
        self.0.saturating_duration_since(Instant::now())
    }
    pub fn expired(&self) -> bool {
        self.remaining().is_zero()
    }
    /// 하위 홉에 줄 예산 — 여유를 조금 남긴다
    pub fn child_budget(&self, max: Duration) -> Duration {
        self.remaining().saturating_sub(Duration::from_millis(100)).min(max)
    }
}
```

미들웨어에서 헤더를 읽어 확장에 심습니다.

```rust
pub async fn deadline_middleware(mut req: Request, next: Next) -> Response {
    // 상위가 남은 밀리초를 알려줌 (gRPC의 grpc-timeout과 같은 개념)
    let budget = req.headers()
        .get("x-deadline-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(8));   // 기본 예산

    req.extensions_mut().insert(Deadline(Instant::now() + budget));
    next.run(req).await
}
```

클라이언트에서 씁니다.

```rust
impl UserClient {
    pub async fn get_user(&self, id: Uuid, dl: Deadline) -> Result<UserDto, ClientError> {
        if dl.expired() {
            return Err(ClientError::DeadlineExceeded);   // 아예 호출하지 않는다
        }
        let budget = dl.child_budget(Duration::from_secs(2));

        let res = self.http.get(format!("{}/v1/users/{id}", self.base))
            .timeout(budget)
            .header("x-deadline-ms", budget.as_millis().to_string())   // 전파
            .header("traceparent", current_traceparent())              // 15 참고
            .send().await?;
        ...
    }
}
```

**"이미 늦었으면 아예 시작하지 않는다"** — 과부하 상황에서 이 한 줄이 큰 차이를 만듭니다.

### 5-4. DB에도 타임아웃을

sqlx의 `acquire_timeout`은 **커넥션을 얻는 시간**만 제한합니다. 쿼리 자체는 무제한입니다.
커넥션 수준에서 걸어야 합니다.

```rust
use sqlx::{postgres::PgPoolOptions, Executor};

let pool = PgPoolOptions::new()
    .max_connections(20)
    .acquire_timeout(Duration::from_secs(3))
    .after_connect(|conn, _meta| Box::pin(async move {
        conn.execute(
            "SET statement_timeout = '3s'; \
             SET lock_timeout = '2s'; \
             SET idle_in_transaction_session_timeout = '10s';"
        ).await?;
        Ok(())
    }))
    .connect(&url)
    .await?;
```

| 설정 | 막는 사고 |
|---|---|
| `statement_timeout` | 폭주 쿼리가 커넥션을 영원히 점유 |
| `lock_timeout` | 마이그레이션 `ALTER TABLE`이 테이블 전체를 잠금 |
| `idle_in_transaction_session_timeout` | 버그로 `commit`을 안 한 트랜잭션이 락과 VACUUM을 막음 |

세 번째는 특히 무섭습니다. 트랜잭션 하나가 열린 채 방치되면 **PostgreSQL의 VACUUM이 멈추고 테이블이 부풀어 오릅니다.**

자세히는 [13_data_evolution §4](./13_data_evolution.md).

---

## 6. Bulkhead와 서킷브레이커

### 6-1. Bulkhead — 한 의존성의 장애가 전체를 먹지 않게

```
order-service의 워커 512개
catalog-service가 느려짐 (응답 5초)
→ catalog를 부르는 요청이 워커를 하나씩 점유
→ 512개 전부 catalog 대기에 묶임
→ catalog와 무관한 "주문 조회"까지 응답 불가
```

배 격벽(bulkhead)처럼 **의존성별로 칸을 나눕니다.**

```rust
#[derive(Clone)]
pub struct Bulkhead {
    permits: Arc<Semaphore>,
    name: &'static str,
}

impl Bulkhead {
    pub fn new(name: &'static str, limit: usize) -> Self {
        Self { permits: Arc::new(Semaphore::new(limit)), name }
    }

    pub async fn run<F, T, E>(&self, f: F) -> Result<T, BulkheadError<E>>
    where F: Future<Output = Result<T, E>> {
        // 대기하지 않는다 — 칸이 차 있으면 즉시 거절
        let _permit = self.permits.try_acquire()
            .map_err(|_| {
                metrics::counter!("bulkhead_rejected", "dep" => self.name).increment(1);
                BulkheadError::Full
            })?;
        f.await.map_err(BulkheadError::Inner)
    }
}
```

```rust
pub struct AppState {
    pub catalog_bh: Bulkhead,   // Bulkhead::new("catalog", 64)
    pub user_bh:    Bulkhead,   // Bulkhead::new("user", 64)
}

// catalog가 죽어도 user 호출은 영향 없음
let product = state.catalog_bh.run(state.catalog.get(id)).await?;
```

`try_acquire`(대기 없음)를 쓰는 게 포인트입니다. 대기하면 격벽의 의미가 없습니다.

### 6-2. 서킷브레이커 — tower에는 없습니다

> [07 §2](./07_messaging.md)는 *"Circuit breaker — `tower::retry`"* 라고 적었지만,
> **tower에는 서킷브레이커가 없습니다.** `tower::retry`는 재시도이고, 재시도는 서킷브레이커의 **반대**입니다.
> 장애 중인 서비스에 재시도를 하면 부하가 늘어 회복을 방해합니다.

간단한 구현:

```rust
#[derive(Clone, Copy, PartialEq)]
enum CircuitState { Closed, Open, HalfOpen }

pub struct CircuitBreaker {
    inner: Arc<Mutex<Inner>>,
    name: &'static str,
    failure_threshold: u32,
    open_duration: Duration,
}

struct Inner {
    state: CircuitState,
    consecutive_failures: u32,
    opened_at: Option<Instant>,
}

impl CircuitBreaker {
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CbError<E>>
    where F: Future<Output = Result<T, E>> {
        // 1) 열려 있으면 즉시 실패 — 네트워크를 아예 안 씀
        {
            let mut g = self.inner.lock().await;
            if g.state == CircuitState::Open {
                if g.opened_at.is_some_and(|t| t.elapsed() >= self.open_duration) {
                    g.state = CircuitState::HalfOpen;   // 정찰 요청 1개 허용
                    tracing::info!(dep = self.name, "circuit half-open");
                } else {
                    metrics::counter!("circuit_rejected", "dep" => self.name).increment(1);
                    return Err(CbError::Open);
                }
            }
        }

        // 2) 실제 호출
        match f.await {
            Ok(v) => {
                let mut g = self.inner.lock().await;
                g.consecutive_failures = 0;
                if g.state == CircuitState::HalfOpen {
                    g.state = CircuitState::Closed;
                    tracing::info!(dep = self.name, "circuit closed");
                }
                Ok(v)
            }
            Err(e) => {
                let mut g = self.inner.lock().await;
                g.consecutive_failures += 1;
                if g.consecutive_failures >= self.failure_threshold
                    || g.state == CircuitState::HalfOpen
                {
                    g.state = CircuitState::Open;
                    g.opened_at = Some(Instant::now());
                    metrics::counter!("circuit_opened", "dep" => self.name).increment(1);
                    tracing::warn!(dep = self.name, "circuit opened");
                }
                Err(CbError::Inner(e))
            }
        }
    }
}
```

**중요:** 타임아웃·5xx만 실패로 세세요. **4xx는 실패가 아닙니다.**
사용자가 없는 ID를 조회해서 나온 404로 서킷을 열면, 정상 서비스가 차단됩니다.

```rust
fn is_circuit_failure(e: &ClientError) -> bool {
    matches!(e, ClientError::Timeout | ClientError::Connect(_))
        || matches!(e, ClientError::Upstream(s) if s.is_server_error())
}
```

### 6-3. 재시도는 조심해서

[07 §2](./07_messaging.md)의 *"GET만 제한적 재시도"* 는 맞습니다. 여기에 두 가지를 더합니다.

**(1) 지수 백오프 + 지터**

```rust
async fn with_retry<F, Fut, T>(mut f: F, max: u32) -> Result<T, ClientError>
where F: FnMut() -> Fut, Fut: Future<Output = Result<T, ClientError>> {
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if attempt < max && e.is_retryable() => {
                // 100ms, 200ms, 400ms ... + 랜덤 지터
                let base = 100u64 << attempt;
                let jitter = rand::random::<u64>() % base;
                tokio::time::sleep(Duration::from_millis(base + jitter)).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

지터가 없으면 **모든 클라이언트가 동시에 재시도**해서 회복 중인 서비스를 다시 무너뜨립니다 (thundering herd).

**(2) 재시도 예산 (retry budget)**

전체 요청의 일정 비율까지만 재시도를 허용합니다.

```
총 요청의 10%까지만 재시도 허용
→ 장애가 광범위하면 재시도가 자동으로 멈춤
→ 부하 증폭 방지
```

**조합:** `Bulkhead(격리) → CircuitBreaker(빠른 실패) → Retry(일시 오류 흡수) → Timeout`
셋 다 필요하고, 순서가 이대로여야 합니다.

---

## 7. tokio 런타임과 컨테이너 CPU

### 7-1. 문제

`#[tokio::main]`은 **호스트의 논리 코어 수**만큼 워커 스레드를 만듭니다.
tokio는 **cgroup의 CPU limit을 읽지 않습니다.**

```
노드: 64 vCPU
파드: resources.limits.cpu = 500m (0.5 코어)

→ tokio가 워커 스레드 64개 생성
→ 실제로는 0.5 코어를 64개가 나눠 씀
→ cgroup CFS throttling 발생 (100ms 주기마다 강제 정지)
→ p99 지연이 수백 ms씩 튐 — 원인 파악이 매우 어려움
+ 스레드 64개 × 스택 2MB = 128MB가 그냥 사라짐
```

JVM은 컨테이너 인식이 오래전에 들어갔지만, **tokio는 아직 직접 지정해야 합니다.**

### 7-2. 해결

```rust
fn main() -> anyhow::Result<()> {
    let workers = std::env::var("TOKIO_WORKER_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| n.get()));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .max_blocking_threads(workers * 4)   // spawn_blocking 풀도 제한 (기본 512는 과함)
        .thread_name("svc-worker")
        .enable_all()
        .build()?;

    rt.block_on(async_main())
}
```

```yaml
# K8s
env:
  - name: TOKIO_WORKER_THREADS
    value: "2"          # limits.cpu = 2000m 에 맞춤
resources:
  requests: { cpu: "1",  memory: "256Mi" }
  limits:   { cpu: "2",  memory: "512Mi" }
```

**가이드: `worker_threads` ≈ `limits.cpu` (올림). 최소 2.**

1로 두면 어떤 태스크가 잠깐 CPU를 물었을 때 다른 태스크가 전부 멈춥니다.

### 7-3. CPU limit을 아예 안 거는 선택지

CFS throttling을 피하려고 **`requests`만 걸고 `limits.cpu`는 생략**하는 것도 흔한 운영 전략입니다.
(메모리 limit은 반드시 겁니다 — OOM은 노드를 위협하지만 CPU는 그냥 느려질 뿐입니다.)

이 경우 `worker_threads`는 `requests.cpu` 기준으로 잡되, 버스트를 허용합니다.
어느 쪽이든 **의식적으로 결정하고 문서에 남기세요.**

### 7-4. 빌드 프로필

[02 §2](./02_workspace.md)에 `[profile.release]` 설정이 없습니다.

```toml
# 루트 Cargo.toml
[profile.release]
opt-level = 3
lto = "thin"          # "fat"은 빌드가 매우 느려짐. thin이 실용적
codegen-units = 1     # 최적화 향상, 빌드 시간 증가
strip = "debuginfo"   # 심볼은 남기고 디버그 정보만 제거 → 패닉 스택은 읽을 수 있음
panic = "unwind"      # ⚠️ abort로 바꾸지 말 것 (§3 참고)

# 프로덕션 프로파일링용 — 릴리즈 + 디버그 심볼
[profile.release-debug]
inherits = "release"
debug = 1
strip = "none"
```

`strip = true`(전부 제거) 대신 `"debuginfo"`를 쓰는 이유: **패닉 스택트레이스에 함수명이 남습니다.**
바이너리 몇 MB를 아끼려고 장애 분석 능력을 버릴 이유가 없습니다.

### 7-5. 메모리 할당자

Rust 기본 할당자는 멀티스레드 서버에서 파편화가 생길 수 있습니다.
RSS가 꾸준히 증가한다면(누수가 아닌데도) 할당자를 바꿔보세요.

```toml
[target.'cfg(not(target_env = "msvc"))'.dependencies]
tikv-jemallocator = "0.6"
```

```rust
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
```

**먼저 측정하고 나서 바꾸세요.** 대부분의 서비스는 기본 할당자로 충분합니다.

---

## 8. 이 문서의 설정을 한 곳에

```rust
// crates/common/src/resilience.rs — 모든 서비스가 같은 기본값을 쓰도록
pub struct ResilienceConfig {
    pub request_timeout:    Duration,   // 8s
    pub drain_duration:     Duration,   // 10s
    pub shutdown_timeout:   Duration,   // 20s
    pub max_concurrency:    usize,      // 512
    pub body_limit:         usize,      // 16KB
    pub db_statement_timeout: Duration, // 3s
    pub dep_bulkhead:       usize,      // 64
    pub cb_failure_threshold: u32,      // 5
    pub cb_open_duration:   Duration,   // 30s
}

impl Default for ResilienceConfig { /* 위 값들 */ }

/// 모든 서비스가 동일하게 적용
pub fn apply_defaults<S>(router: Router<S>, cfg: &ResilienceConfig) -> Router<S> { ... }
```

[02 §4](./02_workspace.md)의 *"공유해도 되는 것"* 표에 이 항목을 추가하세요.
**인프라 정책은 공유 대상입니다.** 도메인 로직만 공유가 금지입니다.

---

## 9. 장애 시나리오 대조표

[01 §7](./01_architecture.md)이 세운 실패 모드 표를, 이 문서의 도구로 실제로 구현하면 이렇게 됩니다.

| 장애 | 01의 기대 동작 | 실제 구현 |
|---|---|---|
| catalog 다운 | 주문 거절 or 미검증 플래그 | Bulkhead + CircuitBreaker → 정책에 따라 즉시 실패/기본값 |
| notification 다운 | 주문 성공, 재시도 큐 적재 | outbox가 이미 보장 ([14](./14_messaging_ops.md)) |
| Redis 다운 | DB fallback | 05 §9 + rate limit fail-open/closed 명시 |
| DB 다운 | 해당 서비스만 503 | `/ready` 실패 → 트래픽 제외, `/health`는 유지 |
| **트래픽 급증** | (01에 없음) | LoadShed → 503 + retry-after |
| **배포 중** | (01에 없음) | graceful drain → 무중단 |
| **핸들러 패닉** | (01에 없음) | CatchPanic → 500 + 알림 |
| **느린 의존성** | (01에 없음) | Deadline 전파 → 조기 포기 |

아래 4개가 이 문서가 채운 부분입니다.

---

## 체크포인트

```
[ ] SIGTERM을 처리하고 with_graceful_shutdown을 쓴다
[ ] SIGTERM → /ready 실패 → 드레인 대기 → 리스너 종료 순서다
[ ] terminationGracePeriodSeconds > 드레인 + 최대 요청 시간
[ ] 백그라운드 워커가 CancellationToken으로 정리된다
[ ] /ready의 의존성 체크가 캐싱 + 타임아웃을 쓴다
[ ] CatchPanicLayer가 켜져 있고 panic = "abort"가 아니다
[ ] panics_total 메트릭에 알림이 걸려 있다
[ ] LoadShed가 ConcurrencyLimit보다 바깥이다
[ ] 타임아웃이 홉마다 안쪽으로 갈수록 짧다
[ ] DB에 statement_timeout / lock_timeout이 걸려 있다
[ ] 의존성마다 Bulkhead 세마포어가 있다
[ ] 서킷브레이커가 4xx를 실패로 세지 않는다
[ ] 재시도에 지수 백오프 + 지터가 있다
[ ] TOKIO_WORKER_THREADS가 CPU limit에 맞춰져 있다
[ ] [profile.release]에 lto/strip이 설정돼 있다
```

---

다음: [12_security — 신뢰 경계·키 회전·토큰 폐기](./12_security.md)
