# 19. 플랫폼 결정 — 자작할 것과 사올 것

[01_architecture](./01_architecture.md)는 **서비스 경계**를 정합니다.
이 문서는 그보다 앞선 질문을 다룹니다: **무엇을 직접 만들고 무엇을 가져다 쓸 것인가.**

이 결정을 잘못하면 6개월 뒤 "우리 팀은 왜 제품이 아니라 인프라만 만들고 있지?"가 됩니다.

---

## 1. Gateway를 직접 만들 것인가

### 1-1. 06의 프록시 코드가 다루지 못하는 것들

[06 §4](./06_gateway_auth.md)는 정직하게 적어뒀습니다.

> 실무에서는:
> - 스트리밍 바디 (대용량 업로드)
> - 헤더 화이트리스트
> - 타임아웃·재시도 정책
> - gRPC 백엔드면 Envoy/tonic 검토

여기에 [10_errata §1~2](./10_errata.md)가 더 찾아낸 것들(헤더 스푸핑, 응답 헤더 유실)을 더하고,
실제 프로덕션 gateway가 해야 하는 일을 전부 나열하면:

```
[ ] HTTP/2, HTTP/3 지원
[ ] 요청/응답 스트리밍 (버퍼링 없이)
[ ] 커넥션 풀링 + keep-alive 관리
[ ] hop-by-hop 헤더 처리 (RFC 7230)
[ ] WebSocket 업그레이드 프록시
[ ] SSE(Server-Sent Events) 통과
[ ] 재시도 (멱등 메서드만) + 백오프
[ ] 서킷브레이커
[ ] 부하 분산 (라운드로빈, least-request, 일관 해싱)
[ ] 아웃라이어 감지 (느린 백엔드 자동 제외)
[ ] TLS 종료 + SNI + 인증서 자동 갱신
[ ] mTLS (업스트림)
[ ] 압축/해제 협상
[ ] 요청/응답 크기 제한
[ ] 슬로로리스(slowloris) 방어
[ ] 헤더 정규화
[ ] 접근 로그
[ ] 트레이스 전파
[ ] 카나리 트래픽 분배
[ ] 미러링 (섀도 트래픽)
```

**Envoy, Traefik, Nginx는 이걸 전부 합니다.** 수년간의 프로덕션 검증과 함께요.

06의 `forward()` 함수는 이 중 3개쯤 합니다.

### 1-2. 그럼 gateway를 만들지 말아야 하나

아닙니다. **역할을 나누세요.**

```
┌─ 인프라 프록시 (Envoy / Traefik / ALB / Ingress) ────┐
│ TLS · HTTP/2 · 스트리밍 · 부하분산 · 재시도 · 압축     │
│ → 검증된 소프트웨어에 맡긴다                          │
└──────────────────────┬───────────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────────┐
│ 앱 레벨 auth/edge 서비스 (Rust, 얇게)                  │
│ JWT 검증 · 토큰 폐기 확인 · 사용자별 rate limit         │
│ 도메인 특화 인가 · BFF 응답 조합                        │
│ → 우리 도메인 지식이 필요한 것만                       │
└──────────────────────────────────────────────────────┘
```

[09 §7](./09_deploy.md)의 이 문장이 정확합니다.

> Ingress가 Compose의 gateway 포트 역할을 일부 대체할 수 있으나,
> **앱 레벨 gateway(JWT/rate limit)** 는 여전히 유용합니다.

이 보강판의 입장은 조금 더 강합니다: **프록시 기능은 인프라에, 인증/인가만 앱에.**

### 1-3. 구체적 조합

**옵션 A — Traefik + 얇은 auth 서비스** (중소 규모 권장)

```yaml
# Traefik ForwardAuth — 인증만 Rust 서비스에 위임
http:
  middlewares:
    auth:
      forwardAuth:
        address: "http://auth-service:3000/verify"
        authResponseHeaders:
          - "X-Internal-User-Id"      # auth 서비스가 설정한 것만 전달
          - "X-Internal-Roles"
        trustForwardHeader: false      # ⚠️ 클라이언트 헤더 불신 (12 §1)
  routers:
    orders:
      rule: "PathPrefix(`/v1/orders`)"
      middlewares: [auth]
      service: order-service
```

```rust
// auth-service — 이것만 만들면 됩니다
async fn verify(headers: HeaderMap, State(s): State<AuthState>) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(claims) = s.jwt.verify(token).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if s.revocation.is_revoked(&claims).await.unwrap_or(false) {   // 12 §4
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !s.rate_limiter.allow(&claims.sub).await {                  // 12 §6
        return (StatusCode::TOO_MANY_REQUESTS, [("retry-after", "60")]).into_response();
    }

    (StatusCode::OK, [
        ("x-internal-user-id", claims.sub.to_string()),
        ("x-internal-roles", claims.roles.join(",")),
    ]).into_response()
}
```

**200줄 대신 30줄입니다.** 프록시 로직이 전부 사라졌습니다.

**옵션 B — Envoy + 앱 gateway** (규모가 커지면)

Envoy가 L7을 처리하고, gateway는 BFF(응답 조합, 클라이언트별 뷰)에 집중합니다.

**옵션 C — 06대로 직접** (학습 목적 또는 특수 요구)

이 경우 [10_errata §1~2](./10_errata.md)를 **반드시** 먼저 적용하세요.

### 1-4. 판단 기준

| 상황 | 권장 |
|---|---|
| Rust/MSA 학습 중 | C — 만들어보는 게 배움 |
| 서비스 3개 이하, 트래픽 낮음 | C 또는 A |
| 프로덕션, 팀 규모 있음 | **A** |
| K8s 사용 중 | **A** (Ingress가 이미 있음) |
| 서비스 메시 도입함 | B (Envoy가 이미 사이드카에) |
| WebSocket/업로드 있음 | A 또는 B (C는 §1-1 목록을 다 만들어야 함) |
| gRPC 백엔드 | B |

**"우리는 특별해서 직접 만들어야 한다"는 대부분 틀립니다.**
정말 특별한 요구가 있다면, 그건 인프라 프록시 뒤의 앱 레이어에서 구현하면 됩니다.

---

## 2. 서비스 메시를 도입할 것인가

### 2-1. 메시가 해결하는 것

[11](./11_resilience.md)과 [12](./12_security.md)에서 손으로 만든 것들의 상당수를
Istio/Linkerd는 **코드 변경 없이** 제공합니다.

| 기능 | 손으로 (이 가이드) | 메시 |
|---|---|---|
| 재시도/타임아웃 | [11 §6-3](./11_resilience.md) | 설정 |
| 서킷브레이커 | [11 §6-2](./11_resilience.md) | 설정 |
| mTLS | 직접 구현 | 자동 |
| 트래픽 분배(카나리) | [18 §8-3](./18_cicd.md) | 설정 |
| 서비스 간 인가 | [12 §1-4](./12_security.md) | 정책 |
| 골든 메트릭 | [15 §6-3](./15_observability_deep.md) | 자동 |

### 2-2. 그런데 비용이 큽니다

```
- 사이드카 프록시 = 파드마다 메모리 50~200MB, CPU 오버헤드
- 지연 추가 (홉마다 ~1ms)
- 디버깅 복잡도 급증 (트래픽이 프록시를 통과)
- 컨트롤 플레인 자체가 운영 대상
- 버전 업그레이드가 무섭다
- 팀 전체가 새 개념(VirtualService, DestinationRule…)을 배워야 함
```

**서비스 5개에 메시를 넣으면 배보다 배꼽이 큽니다.**

### 2-3. 판단

```
서비스 < 10  → 메시 불필요. 이 가이드의 라이브러리 방식으로 충분
서비스 10~30 → 검토. mTLS가 규제 요구면 우선순위 상승
서비스 > 30  → 라이브러리 방식이 한계 (언어별 구현, 버전 파편화)
```

**중간 단계로 Linkerd를 고려하세요.** Istio보다 훨씬 가볍고 학습 곡선이 완만합니다.
mTLS와 골든 메트릭만 필요하다면 Linkerd로 충분한 경우가 많습니다.

---

## 3. 스케줄러 · 배치 작업

### 3-1. 전 문서에 빠져 있습니다

01~09 어디에도 **정기 작업**이 없습니다. 그런데 실제로는 반드시 생깁니다.

```
- 만료된 refresh token 정리 (12 §5)
- outbox / processed_events 정리 (14 §6, §7-4)
- 정합성 감사 (14 §10-3)
- 일일 리포트 집계
- 미결제 주문 자동 취소
- 구독 갱신
```

### 3-2. 가장 흔한 사고

```rust
// ❌ 이렇게 하면
tokio::spawn(async {
    let mut ticker = tokio::time::interval(Duration::from_secs(3600));
    loop {
        ticker.tick().await;
        send_daily_report().await;
    }
});
```

```
replicas: 3
→ 리포트 메일이 3통 발송됨
→ 정합성 감사가 3번 돌아 서로 다른 결론
→ 정리 작업이 서로 충돌
```

**어떤 형태로든 리더 선출이 필요합니다.**

### 3-3. 해법 A — PostgreSQL advisory lock (가장 간단)

```rust
/// 락을 잡은 인스턴스에서만 f를 실행한다.
/// advisory lock은 세션이 끊기면 자동 해제되므로 데드락이 남지 않는다.
pub async fn with_leader_lock<F, Fut>(pool: &PgPool, lock_id: i64, f: F) -> anyhow::Result<()>
where F: FnOnce() -> Fut, Fut: Future<Output = anyhow::Result<()>> {
    // 전용 커넥션을 잡아야 함 — 풀에 반환되면 락이 풀린다
    let mut conn = pool.acquire().await?;

    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(lock_id)
        .fetch_one(&mut *conn)
        .await?;

    if !acquired {
        tracing::debug!(lock_id, "another instance holds the lock, skipping");
        return Ok(());
    }

    let result = f().await;

    let _: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(lock_id).fetch_one(&mut *conn).await?;

    result
}
```

```rust
// 사용
const LOCK_CLEANUP_TOKENS: i64 = 1001;

async fn scheduler(pool: PgPool, shutdown: CancellationToken) {
    let mut ticker = tokio::time::interval(Duration::from_secs(3600));
    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            _ = ticker.tick() => {
                let _ = with_leader_lock(&pool, LOCK_CLEANUP_TOKENS, || async {
                    cleanup_expired_tokens(&pool).await
                }).await;
            }
        }
    }
}
```

**PostgreSQL을 이미 쓰고 있으므로 새 의존성이 없습니다.** 대부분의 팀에 이걸 권합니다.

`lock_id`를 상수 테이블로 관리하세요. 충돌하면 서로 다른 작업이 배타적으로 실행됩니다.

### 3-4. 해법 B — K8s CronJob

```yaml
apiVersion: batch/v1
kind: CronJob
metadata: { name: cleanup-tokens }
spec:
  schedule: "0 3 * * *"
  concurrencyPolicy: Forbid          # 이전 실행이 안 끝났으면 건너뜀
  successfulJobsHistoryLimit: 3
  failedJobsHistoryLimit: 3
  startingDeadlineSeconds: 300
  jobTemplate:
    spec:
      backoffLimit: 2
      template:
        spec:
          restartPolicy: Never
          containers:
            - name: cleanup
              image: user-service:{{ .Values.image.tag }}
              command: ["user-service", "job", "cleanup-tokens"]
```

```rust
// main.rs에 서브커맨드 추가 (13 §7-2와 같은 패턴)
match (args.next().as_deref(), args.next().as_deref()) {
    (Some("job"), Some(name)) => return run_job(name, settings).await,
    (Some("migrate"), _) => return run_migrations(settings).await,
    _ => serve(settings).await,
}
```

| | advisory lock | CronJob |
|---|---|---|
| 리더 선출 | 필요 없음 (락이 대신) | 불필요 (K8s가 1회 실행) |
| 관측 | 앱 로그/메트릭에 통합 | Job 상태 별도 확인 |
| 실패 재시도 | 직접 구현 | `backoffLimit` |
| 짧은 주기 (분 단위) | 적합 | 파드 시작 오버헤드 |
| 긴 작업 (수십 분) | 앱 재배포에 취약 | 적합 |
| 로컬 개발 | 그냥 됨 | K8s 필요 |

**권장: 짧고 가벼운 것은 advisory lock, 무겁고 긴 것은 CronJob.**

### 3-5. 어느 쪽이든 필요한 것

```rust
async fn run_job(name: &str, settings: Settings) -> anyhow::Result<()> {
    let start = Instant::now();
    tracing::info!(job = name, "job started");

    // 작업에도 타임아웃 — 멈춘 배치가 영원히 도는 것 방지
    let result = tokio::time::timeout(
        Duration::from_secs(1800),
        execute_job(name, &settings),
    ).await;

    let elapsed = start.elapsed().as_secs_f64();
    match &result {
        Ok(Ok(stats)) => {
            metrics::counter!("job_success_total", "job" => name.to_string()).increment(1);
            metrics::histogram!("job_duration_seconds", "job" => name.to_string()).record(elapsed);
            tracing::info!(job = name, ?stats, elapsed, "job completed");
        }
        Ok(Err(e)) => {
            metrics::counter!("job_failure_total", "job" => name.to_string()).increment(1);
            tracing::error!(job = name, error = %e, elapsed, "job failed");
        }
        Err(_) => {
            metrics::counter!("job_timeout_total", "job" => name.to_string()).increment(1);
            tracing::error!(job = name, elapsed, "job timed out");
        }
    }
    ...
}
```

**"돌지 않은 배치"에도 알림을 거세요.** 실패보다 더 조용하고 더 위험합니다.

```yaml
- alert: JobNotRunning
  expr: time() - max(job_last_success_timestamp{job="cleanup-tokens"}) > 90000   # 25시간
  annotations: { summary: "일일 배치가 25시간 동안 성공하지 않음" }
```

---

## 4. 파일 업로드와 객체 스토리지

### 4-1. gateway를 통과시키지 마세요

[10_errata §2](./10_errata.md)에서 본 대로, [06 §4](./06_gateway_auth.md)의 프록시는
바디를 전량 버퍼링합니다. 100MB 파일 10개가 동시에 오면 gateway가 1GB를 씁니다.

**정석: presigned URL로 클라이언트가 스토리지에 직접 업로드**

```
1. 클라이언트 → 앱: "파일 업로드할게요" (파일명, 크기, 타입)
2. 앱: 권한 확인 → presigned PUT URL 발급 (15분 유효)
3. 클라이언트 → S3: 직접 PUT (앱을 거치지 않음)
4. 클라이언트 → 앱: "업로드 완료했어요" (key 전달)
5. 앱: S3에 실제 존재하는지 확인 → 메타데이터 저장
```

```rust
pub async fn create_upload_url(
    auth: AuthUser,
    State(s): State<AppState>,
    Json(req): Json<UploadRequest>,
) -> ApiResult<Json<UploadResponse>> {
    // 서버 측 검증 — 클라이언트 값을 믿지 않는다
    if req.size_bytes > 100 * 1024 * 1024 {
        return Err(ApiError::BadRequest("파일이 너무 큽니다 (최대 100MB)".into()));
    }
    const ALLOWED: &[&str] = &["image/jpeg", "image/png", "image/webp", "application/pdf"];
    if !ALLOWED.contains(&req.content_type.as_str()) {
        return Err(ApiError::BadRequest("허용되지 않는 파일 형식".into()));
    }

    // 키는 서버가 정한다 — 경로 조작(../) 방지
    let key = format!("uploads/{}/{}", auth.user_id, Uuid::now_v7());

    let url = s.storage.presigned_put(
        &key,
        Duration::from_secs(900),
        &req.content_type,
        req.size_bytes,          // 크기도 서명에 포함 → 초과 업로드 거부
    ).await?;

    // 아직 확정되지 않은 업로드로 기록
    s.uploads.create_pending(auth.user_id, &key, &req).await?;

    Ok(Json(UploadResponse { upload_url: url, key, expires_in: 900 }))
}

pub async fn confirm_upload(
    auth: AuthUser,
    State(s): State<AppState>,
    Json(req): Json<ConfirmRequest>,
) -> ApiResult<Json<FileResponse>> {
    // ⚠️ 클라이언트 말을 믿지 말고 실제로 확인
    let meta = s.storage.head(&req.key).await
        .map_err(|_| ApiError::BadRequest("업로드가 확인되지 않습니다".into()))?;

    let file = s.uploads.confirm(auth.user_id, &req.key, meta.size, meta.content_type).await?;
    Ok(Json(file.into()))
}
```

### 4-2. 다운로드도 마찬가지

```rust
// presigned GET — 앱은 URL만 발급하고 데이터는 지나가지 않음
pub async fn download_url(auth: AuthUser, State(s): State<AppState>, Path(id): Path<Uuid>)
    -> ApiResult<Json<DownloadResponse>>
{
    let file = s.uploads.find(id).await?.ok_or(ApiError::NotFound("file".into()))?;
    if file.owner_id != auth.user_id && !auth.has_role("admin") {
        return Err(ApiError::NotFound("file".into()));   // 존재 자체를 숨김
    }

    let url = s.storage.presigned_get(&file.key, Duration::from_secs(300)).await?;
    Ok(Json(DownloadResponse { url, expires_in: 300 }))
}
```

### 4-3. 놓치기 쉬운 것들

| 항목 | 대응 |
|---|---|
| 확정되지 않은 업로드 정리 | 24시간 후 pending 삭제 (§3의 배치) |
| 스토리지에 남은 고아 객체 | S3 lifecycle rule + 정기 대조 |
| 바이러스 스캔 | 업로드 완료 이벤트 → 스캔 워커 → 통과 후 공개 |
| 이미지 리사이즈 | 별도 워커 ([10_errata §6](./10_errata.md) — CPU 집약) |
| MIME 위조 | `content-type` 헤더를 믿지 말고 매직 바이트 검사 |
| CORS | S3 버킷에 직접 CORS 설정 필요 |
| 로컬 개발 | MinIO를 compose에 추가 |

```yaml
  minio:
    image: minio/minio
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: minio
      MINIO_ROOT_PASSWORD: minio123
    ports: ["9000:9000", "9001:9001"]
    volumes: ["minio_data:/data"]
```

S3 호환이므로 `aws-sdk-s3`가 그대로 동작합니다.

---

## 5. 실시간 통신 (WebSocket / SSE)

### 5-1. 상태가 있는 연결은 MSA와 상성이 나쁩니다

```
사용자 A가 파드 1에 WebSocket 연결
주문 상태 변경 이벤트가 파드 3에 도착
→ 파드 3은 A의 연결을 모름
```

### 5-2. 해법: Pub/Sub 팬아웃

```rust
// 각 파드가 Redis Pub/Sub을 구독하고, 자기가 가진 연결에만 전달
pub struct ConnectionRegistry {
    local: Arc<DashMap<Uuid, Vec<mpsc::Sender<ServerMessage>>>>,
}

async fn redis_fanout(registry: ConnectionRegistry, client: redis::Client,
                      shutdown: CancellationToken) -> anyhow::Result<()>
{
    // ⚠️ Pub/Sub은 전용 커넥션이 필요 (10_errata §12-2)
    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.psubscribe("ws:user:*").await?;

    let mut stream = pubsub.on_message();
    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            Some(msg) = stream.next() => {
                let channel: String = msg.get_channel()?;
                let user_id: Uuid = channel.strip_prefix("ws:user:")
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| anyhow::anyhow!("bad channel"))?;

                let payload: ServerMessage = serde_json::from_slice(msg.get_payload_bytes())?;

                // 이 파드가 가진 연결에만 전달
                if let Some(senders) = registry.local.get(&user_id) {
                    for tx in senders.iter() {
                        let _ = tx.try_send(payload.clone());   // 느린 소비자는 버림
                    }
                }
            }
        }
    }
    Ok(())
}

// 발행 측 — 어느 파드에서든
async fn notify_user(redis: &mut ConnectionManager, user_id: Uuid, msg: &ServerMessage) {
    let _: Result<(), _> = redis
        .publish(format!("ws:user:{user_id}"), serde_json::to_vec(msg).unwrap())
        .await;
}
```

### 5-3. 주의사항

| 항목 | 대응 |
|---|---|
| Graceful shutdown | 종료 전 클라이언트에 "재연결하세요" 프레임 전송 ([11 §1](./11_resilience.md)) |
| 인증 | WebSocket은 헤더를 못 보내는 클라이언트가 있음 → 연결 후 첫 프레임으로 토큰 전송 |
| 토큰 만료 | 장시간 연결 중 access token 만료 → 주기적 재인증 또는 연결 종료 |
| 느린 소비자 | 송신 채널을 bounded로 두고 넘치면 연결 끊기 ([11 §4](./11_resilience.md)) |
| 하트비트 | ping/pong으로 죽은 연결 탐지 (중간 프록시가 60초에 끊는 경우가 많음) |
| 연결 수 한계 | 파드당 상한 + 초과 시 503 |
| 프록시 통과 | Traefik/Envoy에서 WebSocket 업그레이드 허용 설정 |

### 5-4. SSE를 먼저 고려하세요

단방향(서버 → 클라이언트)이면 SSE가 훨씬 간단합니다.

```rust
use axum::response::sse::{Event, Sse};

async fn order_events(auth: AuthUser, State(s): State<AppState>)
    -> Sse<impl Stream<Item = Result<Event, Infallible>>>
{
    let rx = s.registry.subscribe(auth.user_id);
    let stream = ReceiverStream::new(rx)
        .map(|msg| Ok(Event::default().json_data(msg).unwrap()));

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
```

| | SSE | WebSocket |
|---|---|---|
| 방향 | 서버 → 클라이언트 | 양방향 |
| 프로토콜 | 일반 HTTP | 업그레이드 필요 |
| 자동 재연결 | 브라우저 기본 제공 | 직접 구현 |
| 프록시 호환 | 좋음 | 설정 필요 |
| 인증 | 일반 헤더 | 까다로움 |

**알림, 상태 업데이트, 진행률 표시는 SSE로 충분합니다.**
채팅이나 협업 편집이 아니면 WebSocket이 필요 없는 경우가 많습니다.

---

## 6. 서비스 디스커버리와 설정

### 6-1. 지금 방식의 한계

[01 §6](./01_architecture.md), [09 §2](./09_deploy.md)는 환경변수에 URL을 넣습니다.

```
USER_SERVICE_URL=http://user-service:3001
ORDER_SERVICE_URL=http://order-service:3002
CATALOG_SERVICE_URL=http://catalog-service:3003
```

**K8s에서는 이걸로 충분합니다.** DNS가 로드밸런싱까지 해줍니다.
Consul 같은 별도 디스커버리는 대개 불필요합니다.

문제는 **gateway의 라우팅 테이블이 코드에 하드코딩**되는 것입니다.

```rust
// 06 §4 — 서비스가 늘 때마다 gateway 코드 수정 + 재배포
pub async fn proxy_orders(...) { forward(&state, &state.order_base, req, true).await }
pub async fn proxy_users(...)  { forward(&state, &state.user_base, req, true).await }
```

### 6-2. 라우팅을 설정으로

```yaml
# gateway-routes.yaml
routes:
  - prefix: /v1/users
    upstream: http://user-service:3001
    auth: required
    timeout_ms: 5000
  - prefix: /v1/orders
    upstream: http://order-service:3002
    auth: required
    timeout_ms: 8000
  - prefix: /v1/products
    upstream: http://catalog-service:3003
    auth: optional          # GET은 public (06 §2)
    timeout_ms: 3000
    cache_ttl_s: 60
```

```rust
#[derive(Deserialize, Clone)]
pub struct RouteConfig {
    pub prefix: String,
    pub upstream: String,
    pub auth: AuthRequirement,
    pub timeout_ms: u64,
    #[serde(default)]
    pub cache_ttl_s: Option<u64>,
}

pub fn build_router(routes: &[RouteConfig], state: GatewayState) -> Router {
    let mut router = Router::new();
    for route in routes {
        let r = route.clone();
        router = router.route(
            &format!("{}/{{*rest}}", r.prefix),
            any(move |req| forward_with(r.clone(), req)),
        );
    }
    router.with_state(state)
}
```

**서비스 추가가 설정 변경이 됩니다.** ConfigMap을 바꾸고 재시작하면 끝입니다.

...그리고 여기까지 오면 **Traefik의 설정 파일을 다시 만들고 있다**는 것을 알게 됩니다 (§1-1).
그게 §1의 결론을 뒷받침합니다.

### 6-3. 설정 변경 반영

```rust
/// 설정 파일 변경을 감지해 무중단 반영
async fn watch_config(path: PathBuf, state: GatewayState, shutdown: CancellationToken) {
    let mut ticker = tokio::time::interval(Duration::from_secs(30));
    let mut last_hash = String::new();

    loop {
        tokio::select! {
            biased;
            _ = shutdown.cancelled() => break,
            _ = ticker.tick() => {
                let Ok(content) = tokio::fs::read_to_string(&path).await else { continue };
                let hash = sha256_hex(&content);
                if hash == last_hash { continue; }

                match serde_yaml::from_str::<Vec<RouteConfig>>(&content) {
                    Ok(routes) => {
                        state.routes.store(Arc::new(routes));
                        last_hash = hash;
                        tracing::info!("routing config reloaded");
                    }
                    // ⚠️ 잘못된 설정으로 교체하지 않는다
                    Err(e) => tracing::error!(error = %e, "invalid config, keeping current"),
                }
            }
        }
    }
}
```

**파싱 실패 시 기존 설정 유지**가 핵심입니다.
[12 §3-4](./12_security.md)의 JWKS 갱신 실패 처리와 같은 원리입니다.

### 6-4. 피처 플래그

```rust
#[derive(Clone, Deserialize, Default)]
pub struct Features {
    pub new_checkout_flow: bool,
    pub async_inventory_check: bool,
    pub maintenance_mode: bool,
}

// 사용
if state.features.load().new_checkout_flow {
    new_checkout(...).await
} else {
    legacy_checkout(...).await
}
```

**`maintenance_mode` 같은 킬 스위치는 특히 유용합니다.**
장애 시 배포 없이 문제 기능만 끌 수 있습니다.

```rust
// 미들웨어로
if state.features.load().maintenance_mode {
    return (StatusCode::SERVICE_UNAVAILABLE,
            [("retry-after", "300")],
            Json(json!({"error":"maintenance","message":"점검 중입니다"}))
    ).into_response();
}
```

**플래그를 정리하는 규칙도 정하세요.** 안 그러면 6개월 뒤 죽은 플래그 30개가 남습니다.
"플래그 생성 시 제거 예정일을 함께 기록" 정도면 충분합니다.

---

## 7. 언제 서비스를 나눌 것인가 (01 §1의 재확인)

[01 §1](./01_architecture.md)의 판단 기준은 정확합니다. 이 보강판을 다 읽고 나서 다시 보면 무게가 다릅니다.

**서비스 하나를 추가한다는 것은:**

```
+ 배포 파이프라인 하나 (18)
+ 데이터베이스 하나 (13)
+ 대시보드/알림 세트 하나 (15)
+ 런북 하나
+ 계약 여러 개 (16)
+ 장애 모드 여러 개 (11)
+ 온콜 부담
```

**서비스 하나당 연간 유지 비용이 실질적으로 존재합니다.**

### 7-1. 모듈러 모놀리스를 먼저

01이 정확히 권한 것입니다.

> Axum 단일 바이너리 + `mod user; mod order;` 로 시작해도 됩니다.
> 나중에 crate로 쪼개고, 그다음 바이너리로 올리면 됩니다.

**이 경로를 진지하게 받아들이세요.**

```
1단계: 단일 바이너리, mod로 분리
       → 경계를 코드로 강제 (pub(crate) 제한)
2단계: crate로 분리 (services/를 crates/로)
       → 의존 방향을 컴파일러가 검사
3단계: 바이너리로 분리
       → 필요한 것만
```

1~2단계에서 **경계 설계를 검증**할 수 있습니다. 잘못 그은 경계는 이때 고치는 게 거의 공짜입니다.
3단계 이후에는 [01 §1](./01_architecture.md)의 말대로 *"쪼개고 합치는 비용이 급증"* 합니다.

### 7-2. crate 경계로 강제하기

```toml
# crates/order-domain/Cargo.toml
[dependencies]
common = { workspace = true }
# user-domain을 여기 넣지 않는다 → 컴파일러가 직접 참조를 막는다
```

**단일 바이너리 안에서도 "다른 도메인의 repo를 직접 부르는" 실수를 컴파일 에러로 만들 수 있습니다.**
이게 모듈러 모놀리스의 핵심 가치입니다.

### 7-3. 분리 신호 재확인

01의 표에 실무 관찰을 더합니다.

| 신호 | 진짜 신호인가 |
|---|---|
| 배포 주기가 다름 | ✅ 진짜 |
| 스케일 특성이 다름 | ✅ 진짜 (읽기 폭주 vs 쓰기 폭주) |
| 장애 격리 필요 | ✅ 진짜 |
| 팀이 분리됨 | ✅ 진짜 (콘웨이의 법칙) |
| "코드가 너무 커서" | ❌ 모듈로 해결 |
| "MSA가 표준이라서" | ❌ |
| "채용에 좋아서" | ❌ |
| "나중에 필요할 것 같아서" | ❌ 그때 나누세요 |

---

## 8. 기술 선택 요약

| 영역 | 자작 | 가져다 씀 | 이 가이드의 권장 |
|---|---|---|---|
| HTTP 프레임워크 | — | axum | ✅ axum |
| L7 프록시/TLS | 06 §4 | Traefik/Envoy | **가져다 씀** (§1) |
| 인증/인가 로직 | 12 | Auth0/Keycloak | **자작** (도메인 지식) |
| 서비스 간 회복탄력성 | 11 | 서비스 메시 | **자작** (서비스 <10) |
| 메시지 브로커 | — | NATS/Kafka | ✅ NATS로 시작 |
| outbox 워커 | 14 | Debezium(CDC) | **자작** (단순함) |
| 스케줄러 | 19 §3 | K8s CronJob | **혼합** |
| 파일 스토리지 | — | S3/MinIO | ✅ 가져다 씀 |
| 관측성 백엔드 | — | Grafana 스택 | ✅ 가져다 씀 |
| 관측성 계측 | 15 | 자동 계측 | **자작** (Rust는 수동) |
| API 문서 | — | utoipa | ✅ 가져다 씀 |
| 시크릿 관리 | — | Vault/K8s Secret | ✅ 가져다 씀 |
| CI/CD | 18 | GitHub Actions | ✅ 가져다 씀 |

### 8-1. Debezium(CDC)을 안 쓰는 이유

outbox 대신 **DB의 WAL을 직접 읽어** 이벤트를 만드는 방식이 있습니다.

| | outbox (14) | CDC (Debezium) |
|---|---|---|
| 앱 코드 | outbox 삽입 필요 | 불필요 |
| 인프라 | 없음 | Kafka Connect + 커넥터 |
| 이벤트 형태 | 도메인 이벤트 (의미 있음) | 행 변경 (테이블 구조 노출) |
| 운영 | 앱 안에서 | 별도 시스템 |

**CDC의 가장 큰 문제는 이벤트가 "테이블 행 변경"이라는 것입니다.**
소비자가 생산자의 테이블 구조에 결합되어, [13](./13_data_evolution.md)의 스키마 변경이 곧 계약 변경이 됩니다.

**도메인 이벤트를 명시적으로 쓰는 outbox가 MSA에는 더 맞습니다.**
CDC는 데이터 웨어하우스 동기화 같은 용도에 적합합니다.

---

## 9. 6개월 뒤에 후회하지 않으려면

이 문서 세트 전체를 관통하는 판단들입니다.

```
✅ 처음부터 하세요 (나중에 하면 전면 수정)
   - graceful shutdown          (11 §1)
   - 에러 응답 규약 + trace_id   (10_errata §3)
   - 구조화 로그 + trace 전파    (15)
   - 마이그레이션 expand/contract (13 §2)
   - CI 게이트 (fmt/clippy/test) (18)
   - 서비스마다 README           (16 §10)

⏸ 필요해지면 하세요 (미리 하면 낭비)
   - 서비스 메시                 (19 §2)
   - gRPC                        (01 §5가 정확)
   - Kafka                       (07 §1이 정확)
   - CQRS / 이벤트 소싱
   - 읽기 레플리카               (13 §9)
   - 멀티리전

❌ 하지 마세요
   - 3개 이상의 동기 호출 체인    (01 §4)
   - 서비스 간 DB 공유            (01 §3)
   - 도메인 로직을 common에       (02 §4)
   - 프로덕션에 Swagger UI 공개    (16 §2-5)
   - synchronize/auto-migrate     (04 §3)
   - 이벤트 없이 강한 결합
   - 계약 없는 API 변경           (16 §6)
```

---

## 10. 마지막 — 09 §8의 문장을 다시

> 병목은 언어보다 **경계·데이터소유·관측성**입니다.

이 문장이 맞습니다. 그리고 이 보강판을 다 쓰고 나서 한 가지를 덧붙이고 싶습니다.

**병목은 그 다음에 "운영 가능성"입니다.**

```
경계를 잘 그었다        → 01, 02가 다룸
데이터 소유를 지켰다     → 04, 13
관측이 된다            → 08, 15
────────────────────────────────────
배포해도 안 죽는다      → 11, 13, 18   ← 여기가 실제 난관
장애가 번지지 않는다     → 11
사고가 재발하지 않는다   → 12, 16, 17
```

Rust와 axum은 위 목록의 어느 것도 자동으로 해주지 않습니다.
대신 **명시적으로 만들 수 있게** 해주고, 한 번 만들면 **타입 시스템이 지켜줍니다.**

NestJS/Spring에서 넘어온다면, 그 프레임워크들이 조용히 해주던 일의 목록
([00 §6](./00_addendum_index.md))을 먼저 확인하세요.
그 목록이 이 보강판의 존재 이유이고, Rust MSA의 실제 난이도가 어디에 있는지를 보여줍니다.

---

## 체크포인트

```
[ ] gateway 자작 여부를 의식적으로 결정했다 (§1-4의 표)
[ ] 프록시 기능과 인증 기능의 책임이 분리됐다
[ ] 서비스 메시 도입 기준을 정했다 (서비스 수 기반)
[ ] 정기 작업에 리더 선출(advisory lock 또는 CronJob)이 있다
[ ] 배치 작업에 타임아웃과 "돌지 않음" 알림이 있다
[ ] 파일 업로드가 presigned URL 방식이다
[ ] 업로드 확정 시 스토리지에 실제 존재를 확인한다
[ ] 실시간 통신이 필요하면 SSE를 먼저 검토했다
[ ] WebSocket 사용 시 graceful shutdown 처리가 있다
[ ] gateway 라우팅이 설정으로 분리됐다
[ ] 설정 리로드 실패 시 기존 설정을 유지한다
[ ] 킬 스위치(maintenance_mode)가 있다
[ ] 새 서비스 추가 전에 모듈/crate 분리를 먼저 검토했다
[ ] crate 의존 방향으로 도메인 경계가 강제된다
[ ] §9의 "처음부터" 목록이 전부 되어 있다
```

---

## 관련 문서

- [00_addendum_index](./00_addendum_index.md) — 보강판 인덱스
- [10_errata](./10_errata.md) — 기존 예제 정정
- [11_resilience](./11_resilience.md) · [12_security](./12_security.md)
- [13_data_evolution](./13_data_evolution.md) · [14_messaging_ops](./14_messaging_ops.md)
- [15_observability_deep](./15_observability_deep.md) · [16_api_contract](./16_api_contract.md)
- [17_testing](./17_testing.md) · [18_cicd](./18_cicd.md)

기존 문서: [readme](./readme.md) · [01](./01_architecture.md) ~ [09](./09_deploy.md)
