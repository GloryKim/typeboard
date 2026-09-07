# 15. 관측성 심화 — 트레이스가 실제로 이어지게

[08_observability](./08_observability.md)는 이렇게 약속합니다.

> Gateway → order → user 호출이 하나의 Trace로 보이게 하면
> Nest 모놀리스의 콜스택을 대체합니다.

**그 약속을 지키는 코드가 없습니다.** `X-Request-Id`만으로는 트레이스가 이어지지 않습니다.
이 문서가 그 간극을 채웁니다.

---

## 1. request-id와 trace context는 다릅니다

08 §3은 `X-Request-Id` 전파를 다룹니다. 유용하지만 **로그 상관용**입니다.

| | X-Request-Id | W3C traceparent |
|---|---|---|
| 목적 | 로그를 묶기 | **스팬 트리를 구성** |
| 구조 | 문자열 하나 | trace-id + **span-id** + flags |
| Grafana Loki | ✅ 검색 가능 | ✅ |
| Jaeger/Tempo 폭포수 뷰 | ❌ 불가 | ✅ |
| "어느 호출이 느렸나" | ❌ | ✅ |
| 부모-자식 관계 | ❌ | ✅ |

**둘 다 필요합니다.** request-id는 사람이 읽고 공유하기 좋고, traceparent는 도구가 씁니다.

### traceparent 형식

```
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01
             ││ └─ trace-id (32 hex) ────────┘ └─ span-id ──┘ └flags
             └─ version
```

`span-id`가 홉마다 바뀌는 게 핵심입니다. **이게 부모-자식 관계를 만듭니다.**
`X-Request-Id`를 그대로 넘기면 모든 홉이 같은 값이라 트리를 만들 수 없습니다.

---

## 2. OpenTelemetry 셋업

### 2-1. 버전 조합이 까다롭습니다

08 §6은 세 크레이트 버전을 나열하지만, **이 계열은 버전이 정확히 맞물려야 합니다.**
하나만 어긋나면 trait 구현 충돌로 컴파일이 안 됩니다.

```toml
# ⚠️ 아래 버전은 예시입니다. 반드시 실제 호환 조합을 확인하세요.
opentelemetry          = "0.27"
opentelemetry_sdk      = { version = "0.27", features = ["rt-tokio"] }
opentelemetry-otlp     = { version = "0.27", features = ["grpc-tonic"] }
opentelemetry-semantic-conventions = "0.27"
tracing-opentelemetry  = "0.28"   # ← opentelemetry보다 한 단계 높은 경우가 많음
```

**호환 조합을 찾는 방법:**

```bash
# 1) tracing-opentelemetry의 실제 의존 버전을 확인
cargo add tracing-opentelemetry
cargo tree -p tracing-opentelemetry | grep -E "^\s*[├└]── opentelemetry"

# 2) 그 버전에 나머지를 맞춘다
# 3) 중복 버전이 들어왔는지 확인 — 이게 있으면 반드시 깨진다
cargo tree -d | grep opentelemetry
```

`cargo tree -d`(duplicates)에 opentelemetry가 나오면 **버전을 통일할 때까지 진행하지 마세요.**
[02 §2](./02_workspace.md)의 `[workspace.dependencies]`에 고정해서 서비스마다 어긋나는 것을 막습니다.

### 2-2. 초기화

```rust
// crates/common/src/telemetry.rs — 모든 서비스가 공유
use opentelemetry::{global, KeyValue, trace::TracerProvider as _};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{self, Sampler},
    Resource,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct TelemetryConfig {
    pub service_name: String,
    pub service_version: String,
    pub environment: String,
    pub otlp_endpoint: Option<String>,
    pub sample_ratio: f64,
    pub json_logs: bool,
}

pub fn init(cfg: TelemetryConfig) -> anyhow::Result<TelemetryGuard> {
    // ① W3C 전파자를 전역 등록 — 이게 없으면 traceparent가 읽히지도 쓰이지도 않는다
    global::set_text_map_propagator(TraceContextPropagator::new());

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn,hyper=warn"));

    let registry = tracing_subscriber::registry().with(filter);

    // ② 로그 레이어
    let fmt_layer = if cfg.json_logs {
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(false)      // 너무 장황해짐
            .flatten_event(true)        // Loki에서 라벨 추출이 쉬움
            .boxed()
    } else {
        tracing_subscriber::fmt::layer().pretty().boxed()
    };

    // ③ OTLP 익스포터 (설정됐을 때만)
    let otel_layer = match &cfg.otlp_endpoint {
        Some(endpoint) => {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint)
                .with_timeout(Duration::from_secs(5))
                .build()?;

            let provider = trace::TracerProvider::builder()
                .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
                .with_sampler(Sampler::ParentBased(Box::new(
                    Sampler::TraceIdRatioBased(cfg.sample_ratio),
                )))
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", cfg.service_name.clone()),
                    KeyValue::new("service.version", cfg.service_version.clone()),
                    KeyValue::new("deployment.environment", cfg.environment.clone()),
                ]))
                .build();

            let tracer = provider.tracer("app");
            global::set_tracer_provider(provider.clone());
            Some(tracing_opentelemetry::layer().with_tracer(tracer))
        }
        None => None,
    };

    registry.with(fmt_layer).with(otel_layer).init();
    Ok(TelemetryGuard)
}

/// Drop 시 남은 스팬을 flush — 없으면 종료 직전 트레이스가 유실된다
pub struct TelemetryGuard;
impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}
```

**`Sampler::ParentBased`가 중요합니다.**
부모가 샘플링됐으면 자식도 샘플링합니다. 이게 없으면 **트레이스가 중간에 끊깁니다.**

**`TelemetryGuard`도 중요합니다.** 배치 익스포터는 버퍼링하므로,
[11 §1](./11_resilience.md)의 graceful shutdown에서 flush하지 않으면 **장애 직전 트레이스가 사라집니다.**
가장 필요한 순간의 데이터가 없어지는 겁니다.

---

## 3. 서버 측 — traceparent 추출

들어오는 요청의 `traceparent`를 읽어 **현재 스팬의 부모로 연결**합니다.

```toml
opentelemetry-http = "0.27"   # HeaderMap ↔ OTel 어댑터
```

```rust
use axum::{extract::Request, middleware::Next, response::Response};
use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub async fn trace_context_middleware(req: Request, next: Next) -> Response {
    // ① 요청 헤더에서 상위 컨텍스트를 추출
    let parent_cx = global::get_text_map_propagator(|prop| {
        prop.extract(&opentelemetry_http::HeaderExtractor(req.headers()))
    });

    // ② 서버 스팬 생성
    let span = tracing::info_span!(
        "http_request",
        otel.name = %format!("{} {}", req.method(), route_template(&req)),
        otel.kind = "server",
        http.request.method = %req.method(),
        http.route = %route_template(&req),          // ⚠️ 원본 path 아님 (§5)
        url.path = %req.uri().path(),
        http.response.status_code = tracing::field::Empty,
        request_id = tracing::field::Empty,
        user_id = tracing::field::Empty,
        error = tracing::field::Empty,
    );

    // ③ 추출한 컨텍스트를 부모로 지정 — 이 한 줄이 트레이스를 잇는다
    span.set_parent(parent_cx);

    let response = {
        let _guard = span.enter();
        // request_id도 같은 스팬에 기록해 로그↔트레이스를 연결
        if let Some(rid) = req.headers().get("x-request-id").and_then(|v| v.to_str().ok()) {
            span.record("request_id", rid);
        }
        drop(_guard);
        next.run(req).instrument(span.clone()).await
    };

    span.record("http.response.status_code", response.status().as_u16());
    if response.status().is_server_error() {
        span.record("error", true);
    }
    response
}
```

### 라우트 템플릿을 얻기

```rust
use axum::extract::MatchedPath;

fn route_template(req: &Request) -> String {
    req.extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
```

`MatchedPath`는 `/v1/users/{id}` 형태를 줍니다. **원본 path를 쓰면 카디널리티가 폭발합니다** (§5).

---

## 4. 클라이언트 측 — traceparent 주입

**여기가 08에 완전히 빠진 부분입니다.** 주입하지 않으면 다음 홉이 새 트레이스를 시작합니다.

```rust
use opentelemetry::global;
use tracing_opentelemetry::OpenTelemetrySpanExt;

impl UserClient {
    #[tracing::instrument(
        skip(self),
        fields(
            otel.kind = "client",
            otel.name = "GET /v1/users/{id}",
            peer.service = "user-service",
            http.response.status_code = tracing::field::Empty,
        )
    )]
    pub async fn get_user(&self, id: Uuid, dl: Deadline) -> Result<UserDto, ClientError> {
        let url = format!("{}/v1/users/{id}", self.base);
        let mut req = self.http.get(&url).timeout(dl.child_budget(Duration::from_secs(2)));

        // ① 현재 스팬의 컨텍스트를 헤더로 직렬화
        let cx = tracing::Span::current().context();
        let mut headers = reqwest::header::HeaderMap::new();
        global::get_text_map_propagator(|prop| {
            prop.inject_context(&cx, &mut opentelemetry_http::HeaderInjector(&mut headers));
        });
        // → traceparent, tracestate가 headers에 채워짐

        for (k, v) in headers.iter() {
            req = req.header(k, v);
        }

        // ② request-id도 같이 (로그 상관용)
        req = req.header("x-request-id", current_request_id());
        req = req.header("x-deadline-ms", dl.remaining().as_millis().to_string());

        let res = req.send().await.map_err(|e| {
            if e.is_timeout() { ClientError::Timeout } else { ClientError::Connect(e) }
        })?;

        tracing::Span::current().record("http.response.status_code", res.status().as_u16());

        match res.status() {
            s if s.is_success() => Ok(res.json().await?),
            reqwest::StatusCode::NOT_FOUND => Err(ClientError::NotFound),
            s => Err(ClientError::Upstream(s)),
        }
    }
}
```

### 미들웨어로 자동화

모든 호출에 이 코드를 쓰면 실수합니다. reqwest 미들웨어로 감싸세요.

```toml
reqwest-middleware = "0.4"
reqwest-tracing = "0.5"     # traceparent 자동 주입
```

```rust
let http = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
    .with(reqwest_tracing::TracingMiddleware::default())
    .build();
```

**한 곳에서만 클라이언트를 만들고 공유하세요.** 서비스마다 `reqwest::Client::new()`를 부르면
커넥션 풀이 분리되어 연결이 재사용되지 않습니다.

### gateway는 반드시 시작점

[06 §4](./06_gateway_auth.md)의 `forward()`는 헤더를 화이트리스트로 복사합니다.
`traceparent`와 `tracestate`를 **반드시 포함**시키세요 ([10_errata §1](./10_errata.md)의 `FORWARD_ALLOW`).

빠뜨리면 gateway에서 트레이스가 끊기고, 정작 가장 보고 싶은 **"gateway → 어느 서비스가 느린가"** 를 못 봅니다.

---

## 5. 카디널리티 — 모니터링을 죽이는 법

### 5-1. 08 §5의 예제에 잠재된 폭탄

```rust
counter!("http_requests_total", "route" => "/v1/orders", "status" => "201").increment(1);
```

`route`를 **원본 path**로 넣으면:

```
/v1/orders/018f4a2b-...  ← 주문마다 새 시계열
/v1/orders/018f4a2c-...
...
주문 100만 개 = 시계열 100만 개
→ Prometheus 메모리 폭발 → OOM
```

Prometheus는 **라벨 조합마다 별도 시계열**을 만듭니다. 시계열 하나에 약 1~3KB입니다.

### 5-2. 규칙

| 라벨 | 허용? | 이유 |
|---|---|---|
| `route` = `/v1/orders/{id}` | ✅ | 엔드포인트 수만큼 (수십 개) |
| `method` = `GET` | ✅ | 7개 |
| `status` = `200` | ✅ | 수십 개 |
| `service` = `order-service` | ✅ | 서비스 수 |
| `user_id` | ❌ | 사용자 수만큼 |
| `order_id` | ❌ | 주문 수만큼 |
| `error_message` | ❌ | 무한 |
| `url` (원본) | ❌ | 무한 |
| `ip` | ❌ | 무한 |

**추정 시계열 수 = 각 라벨 카디널리티의 곱**

```
route(50) × method(4) × status(10) = 2,000  ✅ 괜찮음
+ user_id(100,000) 추가 → 2억          ❌ 즉사
```

### 5-3. 고유 값은 트레이스와 로그로

```rust
// ❌ 메트릭 라벨
counter!("orders_created", "user_id" => user_id.to_string()).increment(1);

// ✅ 메트릭은 집계, 개별 값은 스팬 속성/로그
counter!("orders_created_total", "channel" => channel).increment(1);
tracing::Span::current().record("user_id", user_id.to_string());
tracing::info!(user_id = %user_id, order_id = %order.id, "order created");
```

**메트릭 = 집계, 트레이스 = 개별 요청, 로그 = 상세.** 역할을 섞지 마세요.

### 5-4. 오류 라벨은 분류해서

```rust
// ❌ 무한
counter!("errors_total", "message" => e.to_string()).increment(1);

// ✅ 유한한 분류
fn error_kind(e: &ApiError) -> &'static str {
    match e {
        ApiError::NotFound(_)   => "not_found",
        ApiError::Unauthorized  => "unauthorized",
        ApiError::BadRequest(_) => "bad_request",
        ApiError::Conflict(_)   => "conflict",
        ApiError::Internal(_)   => "internal",
    }
}
counter!("errors_total", "kind" => error_kind(&e)).increment(1);
```

---

## 6. Prometheus 익스포터 바로 쓰기

### 6-1. 08 §5의 `install()`은 별도 리스너를 띄웁니다

```rust
PrometheusBuilder::new().install().expect("metrics");
```

이건 **자체 HTTP 서버**를 시작합니다. axum의 `/metrics` 라우트로 노출하려면 핸들을 받아야 합니다.

```rust
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle, Matcher};

pub fn init_metrics() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new()
        // 히스토그램 버킷을 지연 분포에 맞게 (기본값은 대개 부적절)
        .set_buckets_for_metric(
            Matcher::Suffix("_duration_seconds".to_string()),
            &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        )?
        // 유휴 시계열 정리 — 사라진 라벨이 영원히 남지 않게
        .idle_timeout(
            metrics_util::MetricKindMask::HISTOGRAM,
            Some(Duration::from_secs(600)),
        )
        .install_recorder()?;   // ← 리스너 없이 핸들만
    Ok(handle)
}
```

```rust
// 라우터에 붙이기
let metrics_handle = init_metrics()?;

let app = Router::new()
    .route("/metrics", get(move || {
        let h = metrics_handle.clone();
        async move { h.render() }
    }))
    .merge(api_routes);
```

### 6-2. `/metrics`를 외부에 노출하지 마세요

08 §5의 *"별도 포트 또는 내부망만"* 이 맞습니다. 메트릭은 정보 노출입니다.

```
- 엔드포인트 목록 = API 지도
- 요청량 = 사업 규모
- 에러율 = 취약점 탐색 힌트
- 버전 라벨 = 알려진 CVE 매칭
```

```rust
// 관리 포트를 별도로 — 가장 깔끔
let admin = Router::new()
    .route("/metrics", get(metrics))
    .route("/health", get(health))
    .route("/ready", get(ready));

tokio::spawn(async move {
    let l = TcpListener::bind("127.0.0.1:9090").await.unwrap();
    axum::serve(l, admin).await.unwrap();
});
```

K8s에서는 두 번째 컨테이너 포트로 열고 Service에서는 제외하면 됩니다.

### 6-3. RED 메트릭 미들웨어

08 §5가 RED를 언급하지만 구현이 없습니다.

```rust
pub async fn metrics_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().to_string();
    let route = route_template(&req);     // §3 — 반드시 템플릿

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    // Rate + Errors
    metrics::counter!("http_requests_total",
        "method" => method.clone(), "route" => route.clone(), "status" => status
    ).increment(1);

    // Duration
    metrics::histogram!("http_request_duration_seconds",
        "method" => method, "route" => route
    ).record(elapsed);

    response
}
```

**진행 중 요청 수도 유용합니다** (포화도 — USE의 U).

```rust
metrics::gauge!("http_requests_in_flight").increment(1.0);
let response = next.run(req).await;
metrics::gauge!("http_requests_in_flight").decrement(1.0);
```

이 값이 [11 §4](./11_resilience.md)의 `ConcurrencyLimit`에 근접하면 곧 load shedding이 시작된다는 신호입니다.

### 6-4. 인프라 메트릭

08 §5의 *"Postgres connections, Redis hit ratio, outbox lag"* 을 실제로:

```rust
async fn collect_infra_metrics(state: AppState, shutdown: CancellationToken) {
    let mut ticker = tokio::time::interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            _ = ticker.tick() => {
                // sqlx 풀 상태 — 포화되면 acquire_timeout이 터진다
                metrics::gauge!("db_pool_connections", "state" => "idle")
                    .set(state.pool.num_idle() as f64);
                metrics::gauge!("db_pool_connections", "state" => "total")
                    .set(state.pool.size() as f64);

                // outbox (14 §10)
                if let Ok((pending, oldest)) = outbox_stats(&state.pool).await {
                    metrics::gauge!("outbox_pending").set(pending as f64);
                    metrics::gauge!("outbox_oldest_age_seconds").set(oldest);
                }

                // tokio 런타임 (tokio_unstable 필요)
                // metrics::gauge!("tokio_workers_busy").set(...);
            }
        }
    }
}
```

`db_pool_connections{state="idle"}`이 0에 붙어 있으면 **풀이 부족하거나 쿼리가 느린 것**입니다.
장애 원인 파악에서 가장 빨리 답을 주는 지표 중 하나입니다.

---

## 7. 로그를 트레이스와 잇기

### 7-1. 모든 로그에 trace_id

```rust
/// 현재 스팬의 trace_id를 꺼낸다 (10_errata §3의 에러 응답에도 사용)
pub fn current_trace_id() -> String {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let cx = tracing::Span::current().context();
    let span = cx.span();
    let sc = span.span_context();
    if sc.is_valid() { sc.trace_id().to_string() } else { String::new() }
}
```

JSON 로그 레이어에 자동으로 넣으려면 커스텀 레이어가 필요합니다. 간단한 대안:

```rust
// 최상위 스팬에 필드로 기록하면 하위 로그에 상속된다
let span = tracing::info_span!("http_request", trace_id = tracing::field::Empty, ...);
span.set_parent(parent_cx);
span.record("trace_id", current_trace_id());   // set_parent 이후에 호출
```

### 7-2. 결과: 하나의 흐름

```json
{"timestamp":"2026-07-23T12:00:00.123Z","level":"INFO","target":"gateway",
 "trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","request_id":"018f-...",
 "http.route":"/v1/orders","message":"request started"}

{"timestamp":"2026-07-23T12:00:00.145Z","level":"ERROR","target":"order_service",
 "trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","request_id":"018f-...",
 "order_id":"018f-...","message":"create_order failed"}
```

```
Grafana에서:
  1. 사용자가 trace_id를 문의 (에러 응답에 있었으므로 — 10_errata §3)
  2. Loki에서 {trace_id="4bf92..."} → 모든 서비스의 로그
  3. 같은 화면에서 Tempo로 점프 → 어느 홉이 느렸는지 폭포수
```

Nest 모놀리스의 스택트레이스가 하던 일을 이 조합이 대신합니다.
**`trace_id`가 없으면 이 흐름 전체가 성립하지 않습니다.**

### 7-3. 로그 레벨 규약 (08 §7의 확장)

| 레벨 | 언제 | 예 |
|---|---|---|
| `error` | 사람이 봐야 함 | 5xx, outbox dead, 패닉, 보상 실패 |
| `warn` | 비정상이나 자동 회복 | 재시도, 캐시 실패 폴백, rate limit |
| `info` | 상태 변화 | 서비스 기동, 주문 생성, 배포 |
| `debug` | 개발 중 | 쿼리 파라미터, 분기 |
| `trace` | 극단적 상세 | 프로덕션에서 절대 켜지 않음 |

**`info`는 요청당 1~2줄 이하로.** 요청마다 10줄이면 초당 1,000요청에서 하루 8억 줄입니다.

```rust
// ❌ 요청 경로에 이런 게 쌓이면
tracing::info!("entering handler");
tracing::info!("validating input");
tracing::info!("querying db");
tracing::info!("input valid");

// ✅ 스팬 하나 + 결과 한 줄
#[tracing::instrument(skip(state, body), fields(order_id = tracing::field::Empty))]
async fn create_order(...) -> ApiResult<Json<OrderResponse>> {
    let order = state.order_svc.create(body).await?;
    tracing::Span::current().record("order_id", order.id.to_string());
    Ok(Json(order.into()))
}
```

### 7-4. 로그 비용

```
100 bytes/줄 × 1,000 rps × 2줄 × 86,400초 = 17GB/일
× 30일 보존 = 520GB
```

관리형 로깅 서비스면 이게 곧 비용입니다. 제어 수단:

```rust
// 정상 요청은 샘플링, 에러는 전부
if response.status().is_success() && rand::random::<f64>() > 0.1 {
    // 90%는 로그 생략 (트레이스와 메트릭에는 남아 있음)
} else {
    tracing::info!(...);
}
```

**단, 감사 로그는 절대 샘플링하지 마세요** ([12 §8](./12_security.md)).

---

## 8. 샘플링 전략

### 8-1. 전부 수집할 수는 없습니다

```
1,000 rps × 스팬 5개 × 1KB = 5MB/s = 432GB/일
```

### 8-2. 계층적 샘플링

```rust
Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(0.05)))   // 5%
```

- **부모 결정 존중**: gateway가 샘플링을 결정하면 하위가 따릅니다. 끊긴 트레이스가 안 생깁니다.
- **trace-id 기반**: 같은 trace-id면 모든 서비스가 같은 결정을 합니다.

### 8-3. Tail 샘플링 (권장)

문제: 5% 무작위 샘플링에서 **느린 요청과 에러 요청이 대부분 버려집니다.**
정작 보고 싶은 건 그것들인데요.

**해결: OTel Collector에서 트레이스 완료 후 결정**

```yaml
# otel-collector-config.yaml
processors:
  tail_sampling:
    decision_wait: 10s
    policies:
      # 에러는 100%
      - name: errors
        type: status_code
        status_code: { status_codes: [ERROR] }
      # 느린 것은 100%
      - name: slow
        type: latency
        latency: { threshold_ms: 1000 }
      # 나머지는 5%
      - name: baseline
        type: probabilistic
        probabilistic: { sampling_percentage: 5 }
```

앱은 100% 보내고(`Sampler::AlwaysOn`) Collector가 거릅니다.
앱↔Collector 트래픽은 늘지만 **의미 있는 트레이스를 놓치지 않습니다.**

### 8-4. Collector를 두는 이유

```
앱 → OTel Collector (DaemonSet) → Tempo/Jaeger
```

| 이점 | 설명 |
|---|---|
| 앱에서 백엔드 주소 분리 | Jaeger→Tempo 교체 시 앱 재배포 불필요 |
| 버퍼링·재시도 | 백엔드 장애가 앱에 전파되지 않음 |
| tail 샘플링 | §8-3 |
| 속성 가공 | PII 제거, 라벨 정규화 |
| 프로토콜 변환 | OTLP → Jaeger/Zipkin/Datadog |

**앱이 관측 백엔드를 직접 알지 않게 하세요.** 관측 시스템 장애가 서비스 장애가 되면 안 됩니다.

---

## 9. SLO와 알림

### 9-1. 08 §8의 임계치에 근거를 주기

> Prometheus Alertmanager: p99 > 1s, 5xx > 1%, outbox lag > 1000

숫자는 그럴듯하지만 **어디서 나온 값인지 없습니다.** SLO에서 유도해야 합니다.

```
SLI (측정 지표) : 성공한 요청의 비율, p99 지연
SLO (목표)      : 30일간 가용성 99.9%, p99 < 300ms
에러 버짓        : 0.1% = 30일 43분
```

### 9-2. 버짓 소진 속도로 알림

절대 임계치("5xx > 1%")는 **너무 자주 울리거나 너무 늦게 울립니다.**
"이 속도면 언제 버짓이 바닥나는가"로 알림을 거세요.

```yaml
# 빠른 소진 — 1시간에 2% 소진 = 즉시 대응
- alert: ErrorBudgetBurnFast
  expr: |
    (
      sum(rate(http_requests_total{status=~"5.."}[5m]))
      / sum(rate(http_requests_total[5m]))
    ) > (14.4 * 0.001)
  for: 2m
  labels: { severity: page }

# 느린 소진 — 6시간에 5% = 근무 시간에 대응
- alert: ErrorBudgetBurnSlow
  expr: |
    (
      sum(rate(http_requests_total{status=~"5.."}[6h]))
      / sum(rate(http_requests_total[6h]))
    ) > (6 * 0.001)
  for: 15m
  labels: { severity: ticket }
```

`14.4`와 `6`은 Google SRE 워크북의 다중 창(multi-window) 계수입니다.

### 9-3. 알림 원칙

| 원칙 | 의미 |
|---|---|
| **증상에 걸어라** | "CPU 80%"가 아니라 "사용자 요청 실패" |
| **행동 가능해야** | 받고 할 일이 없으면 알림이 아니라 대시보드 |
| **호출은 드물게** | 하루 2건 이상이면 알림 피로 → 무시하게 됨 |
| **런북을 붙여라** | `annotations.runbook_url` |

```yaml
- alert: HighErrorRate
  annotations:
    summary: "{{ $labels.service }} 5xx 비율 {{ $value | humanizePercentage }}"
    runbook_url: "https://wiki/runbooks/high-error-rate"
    dashboard: "https://grafana/d/service-overview?var-service={{ $labels.service }}"
```

### 9-4. 서비스별 최소 알림 세트

```yaml
# 가용성
- ErrorBudgetBurnFast / Slow           (§9-2)
- ServiceDown: up == 0 for 2m

# 지연
- LatencyP99: histogram_quantile(0.99, ...) > SLO for 10m

# 포화
- DbPoolExhausted: db_pool_connections{state="idle"} == 0 for 5m
- LoadShedding: rate(load_shed_total[5m]) > 0

# 정합성 (14 §10)
- OutboxBacklog: outbox_oldest_age_seconds > 60 for 5m
- OutboxDead / EventDlq: increase(...) > 0

# 안정성 (11 §3)
- HandlerPanic: increase(panics_total[5m]) > 0

# 자원
- MemoryNearLimit: container_memory_working_set_bytes / limit > 0.9
- CpuThrottling: rate(container_cpu_cfs_throttled_seconds_total[5m]) > 0.1   # 11 §7
```

마지막 항목이 [11 §7](./11_resilience.md)의 tokio 워커 스레드 문제를 직접 탐지합니다.

---

## 10. 대시보드 구성

### 10-1. 세 층으로

```
1. 서비스 개요 (온콜이 처음 보는 화면)
   - RED: 요청량 / 에러율 / p50·p95·p99
   - 진행 중 요청 수, load shed 비율
   - 의존성별 성공률 (서킷 상태)

2. 서비스 상세 (원인 좁히기)
   - 엔드포인트별 지연·에러
   - DB: 풀 사용률, 쿼리 지연, 느린 쿼리
   - Redis: hit ratio, 지연
   - outbox: pending, oldest age
   - 런타임: 메모리, CPU throttle, 태스크 수

3. 비즈니스 (전체가 정상인지)
   - 분당 주문 수, 결제 성공률
   - 가입/로그인 성공률
```

**3번이 의외로 강력합니다.** 모든 기술 지표가 정상인데 주문이 0이면 확실히 문제가 있습니다.
반대로 에러율이 조금 올라도 주문이 정상이면 급하지 않습니다.

### 10-2. 배포 마커

```
배포 시각을 대시보드에 수직선으로 표시
→ "이 문제가 배포 직후 시작됐나?" 를 즉시 판단
```

Grafana annotation API를 CD 파이프라인에서 호출하세요 ([18_cicd](./18_cicd.md)).
장애 대응 시간을 가장 크게 줄이는 단순한 조치 중 하나입니다.

---

## 11. 프로파일링 (필요해지면)

메트릭이 "느리다"를 알려주면, 프로파일링이 "어디가"를 알려줍니다.

```toml
[dependencies]
pprof = { version = "0.14", features = ["flamegraph"], optional = true }
```

```rust
// 관리 포트에만 노출 (§6-2)
#[cfg(feature = "profiling")]
async fn pprof_handler(Query(q): Query<ProfileQuery>) -> impl IntoResponse {
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(100)
        .blocklist(&["libc", "libgcc", "pthread"])
        .build().unwrap();

    tokio::time::sleep(Duration::from_secs(q.seconds.unwrap_or(30))).await;

    let report = guard.report().build().unwrap();
    let mut svg = Vec::new();
    report.flamegraph(&mut svg).unwrap();
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg)
}
```

`[profile.release-debug]` ([11 §7-4](./11_resilience.md))로 빌드하면 프로파일에 함수명이 보입니다.

**async 코드는 프로파일링이 까다롭습니다.** 스택이 런타임에서 끊깁니다.
`tokio-console`(`tokio_unstable` 필요)이 태스크 단위 분석에 더 유용한 경우가 많습니다.

---

## 12. 08 §4의 health를 마무리

[11 §1-6](./11_resilience.md)에서 캐싱을 다뤘고, 여기서는 세 번째 엔드포인트를 추가합니다.

```rust
Router::new()
    // liveness — 프로세스만. 절대 의존성 체크 금지
    .route("/health", get(|| async { "ok" }))

    // readiness — 트래픽 받을 준비. 캐싱 + 드레인 반영
    .route("/ready", get(ready))

    // startup — 초기화 완료 (마이그레이션 검증, 캐시 워밍, JWKS 로드)
    .route("/startup", get(startup))
```

```yaml
startupProbe:
  httpGet: { path: /startup, port: 3001 }
  failureThreshold: 30      # 30 × 2s = 최대 60초 대기
  periodSeconds: 2
livenessProbe:
  httpGet: { path: /health, port: 3001 }
  periodSeconds: 10
readinessProbe:
  httpGet: { path: /ready, port: 3001 }
  periodSeconds: 2
```

`startupProbe`가 있으면 **`livenessProbe`의 `initialDelaySeconds`를 추측하지 않아도 됩니다.**
느린 시작(마이그레이션 검증, JWKS 로드)이 재시작 루프를 만드는 것을 막아줍니다.

---

## 체크포인트

```
[ ] OTel 크레이트 버전이 통일됐다 (cargo tree -d 로 확인)
[ ] TraceContextPropagator가 전역 등록됐다
[ ] 서버가 요청의 traceparent를 추출해 부모로 설정한다
[ ] 클라이언트가 traceparent를 주입한다
[ ] gateway 헤더 화이트리스트에 traceparent/tracestate가 있다
[ ] Sampler가 ParentBased다
[ ] 종료 시 tracer provider를 flush한다
[ ] 메트릭 라벨에 route 템플릿을 쓴다 (원본 path 아님)
[ ] 메트릭 라벨에 user_id/order_id 같은 고유값이 없다
[ ] install_recorder()로 핸들을 받아 /metrics를 서빙한다
[ ] /metrics가 관리 포트에만 노출된다
[ ] RED 메트릭 + in_flight gauge가 있다
[ ] db_pool / outbox 인프라 메트릭이 있다
[ ] 모든 로그에 trace_id와 request_id가 있다
[ ] 요청당 info 로그가 1~2줄 이하다
[ ] tail 샘플링으로 에러·느린 트레이스를 100% 보관한다
[ ] OTel Collector를 경유한다
[ ] 알림이 SLO 버짓 소진 속도 기반이다
[ ] 알림에 runbook_url이 있다
[ ] 대시보드에 배포 마커가 있다
[ ] /health /ready /startup 세 개가 분리됐다
```

---

다음: [16_api_contract — OpenAPI와 계약 테스트](./16_api_contract.md)
