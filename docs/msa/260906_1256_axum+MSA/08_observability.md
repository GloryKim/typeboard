# 08. Observability — tracing · metrics · health

MSA에서 디버깅은 IDE 브레이크포인트보다 **상관 ID + 분산 트레이스**로 합니다.  
Nest의 interceptor 로깅 / Spring Actuator 자리를 여기에 둡니다.

---

## 1. 세 기둥

| 기둥 | 질문 | 도구 |
|---|---|---|
| Logs | 무엇이 일어났나 | `tracing` + JSON 로그 |
| Traces | 어디서 느려졌나 | OpenTelemetry → Jaeger/Tempo |
| Metrics | 지금 건강한가 | Prometheus + `/metrics` |

---

## 2. tracing 기본 세팅

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

tracing_subscriber::registry()
    .with(EnvFilter::from_default_env()) // RUST_LOG=info,tower_http=info,sqlx=warn
    .with(tracing_subscriber::fmt::layer().json()) // prod
    .init();
```

핸들러/서비스:

```rust
#[tracing::instrument(skip(state, body), fields(user_id = %user_id))]
pub async fn create_order(State(state): State<AppState>, body: Json<...>) { ... }
```

`tower_http::trace::TraceLayer` 로 HTTP 접근 로그:

```rust
.layer(TraceLayer::new_for_http())
```

---

## 3. Request ID 전파

Gateway가 생성하거나 클라이언트의 `X-Request-Id`를 존중:

```rust
// 미들웨어 개념
let rid = header_or_new_uuid(req.headers());
tracing::Span::current().record("request_id", &rid);
// 응답 헤더에도 넣고
// upstream 호출 시 동일 헤더 전달
```

로그 필드에 `request_id`가 있으면 Grafana Loki에서 한 줄로 전체 Hop을 묶습니다.

---

## 4. Health / Ready

Kubernetes·로드밸런서용:

```rust
// GET /health  — 프로세스 생존
async fn health() -> &'static str { "ok" }

// GET /ready   — 의존성 준비
async fn ready(State(state): State<AppState>) -> Result<&'static str, StatusCode> {
    sqlx::query("SELECT 1").execute(&state.pool).await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    // redis PING 선택
    Ok("ready")
}
```

| 엔드포인트 | 실패 시 |
|---|---|
| `/health` | 재시작 대상 |
| `/ready` | 트래픽 제외 |

DB 순간 흔들림에 `/health`까지 묶지 마세요.

---

## 5. Metrics (Prometheus)

```toml
metrics = "0.24"
metrics-exporter-prometheus = "0.16"
```

```rust
use metrics::{counter, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;

PrometheusBuilder::new().install().expect("metrics");

// 요청 후
counter!("http_requests_total", "route" => "/v1/orders", "status" => "201").increment(1);
histogram!("http_request_duration_seconds").record(elapsed);
```

`/metrics` 노출은 별도 포트 또는 내부망만.

**RED 메트릭 (서비스당):**
- Rate (QPS)
- Errors (5xx 비율)
- Duration (p95/p99)

인프라: Postgres connections, Redis hit ratio, outbox lag.

---

## 6. OpenTelemetry (다음 단계)

```toml
opentelemetry = "0.27"
tracing-opentelemetry = "0.28"
opentelemetry-otlp = "0.27"
```

로컬 Jaeger all-in-one:

```yaml
  jaeger:
    image: jaegertracing/all-in-one:1.57
    ports: ["16686:16686", "4317:4317"]
```

Gateway → order → user 호출이 하나의 Trace로 보이게 하면  
Nest 모놀리스의 콜스택을 대체합니다.

---

## 7. 에러 로그 규약

```rust
Err(e) => {
    tracing::error!(error = %e, order_id = %id, "create_order failed");
    ApiError::Internal(e.into())
}
```

- 4xx: `warn` 수준, 스택 최소화
- 5xx: `error`, 원인 포함 (시크릿·PII 제외)
- 비밀번호·토큰·카드번호 로그 금지

---

## 8. 슬랙/알림 (선택)

- Prometheus Alertmanager: p99 > 1s, 5xx > 1%, outbox lag > 1000
- 앱에서 직접 슬랙 웹훅은 최후 수단

---

## 체크포인트

```
[ ] RUST_LOG와 JSON 로그가 환경별로 다르다
[ ] X-Request-Id가 gateway→서비스→로그에 남는다
[ ] /health 와 /ready 가 분리됐다
[ ] RED 메트릭이 서비스당 존재한다
[ ] (선택) OTLP로 Jaeger에 트레이스가 보인다
```

다음: [09_deploy — Docker Compose로 한 번에 올리기](./09_deploy.md)
