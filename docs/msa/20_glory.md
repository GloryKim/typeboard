앞선 분석의 A절 12건, B절 13개 주제를 모두 넣었고, 지적만 하지 않고 고친 코드와 장애 시나리오를 함께 넣었습니다. 몇 가지 설계 선택:

10_errata를 맨 앞에 — 기존 파일을 못 고치니, 구현할 때 대조할 수 있게 위치 → 증상 → 왜 → 고친 코드 형식에 심각도 표를 붙였습니다.
기존 문서로 역참조 — 모든 항목이 06 §4처럼 원본 절을 가리키고, 기존 문서가 옳게 판단한 곳은 그렇게 명시했습니다(04 §8의 키셋 페이지네이션, 07 §6의 "굳이 쪼개지 마세요", 01 §7의 실패 모드 표).
API는 로컬 소스로 검증 — axum-0.8.9 / axum-core-0.5.6 / tower-http-0.6.11을 직접 읽어 확인했습니다. TimeoutLayer::with_status_code는 실존하고(기존 문서가 맞음), layer 순서는 뒤집혀 있으며(나중 = 바깥), 바디 제한 2MB와 거절 응답이 text/plain인 것도 소스로 확인했습니다.
OTel 버전은 단정하지 않음 — 캐시에 없어 검증 불가라, 예시 버전 + cargo tree -d로 호환 조합을 찾는 방법을 적었습니다.
앞선 분석에 없던 것 중 추가한 것
작성하면서 추가로 발견한 것들입니다.

Claims에 email이 들어 있음 (06 §3) — JWT는 서명만 되고 암호화되지 않으므로 localStorage에서 디코드하면 전 사용자 이메일이 수집됩니다.
계정 열거 — 로그인뿐 아니라 register의 409, 비밀번호 재설정에서도 샙니다. 더미 해싱으로 타이밍까지 맞추는 코드를 넣었습니다.
/ready가 장애를 만드는 경로 — replicas × 짧은 probe 주기로 DB에 쿼리가 몰려, DB가 느려지면 전 파드가 동시에 트래픽에서 빠집니다.
argon2 메모리 × 동시성 — 19MiB × 64 = 1.2GB. 512Mi 컨테이너에서 세마포어 없이 쓰면 OOM입니다.
dead 이벤트의 head-of-line blocking — ORDER BY id라서 독성 이벤트 하나가 뒤의 정상 이벤트를 영구히 막습니다.
deny_unknown_fields 금지 — 이벤트 소비자에 붙이면 생산자가 필드를 추가하는 순간 전부 깨집니다.
한 가지 판단
19장에서 gateway 자작을 권장에서 내렸습니다. 06 §4가 "실무에서는~"으로 남긴 항목들(스트리밍, HTTP/2, 재시도, WebSocket 업그레이드)이 Traefik/Envoy가 이미 하는 일이라, ForwardAuth + 30줄짜리 Rust auth 서비스 조합을 대안으로 제시했습니다. 학습 목적이면 06대로 만드는 게 맞다는 것도 같이 적었습니다.










docs/msa/ 10개 문서 전부 읽었습니다. 코드 예제는 로컬에 받아둔 axum-0.8.9 / axum-core-0.5.6 / tower-http-0.6.11 소스로 API를 대조해서 확인했습니다.

총평
골격 자체는 좋습니다. 특히 잘 잡힌 부분:

데이터 소유권(DB-per-service) → 분산 트랜잭션 회피 → Outbox 순서로 논리가 이어지는 것
"호출 체인 최대 2 hop", "처음엔 4개 이하", "common이 비대해지면 다시 모놀리스" 같은 정량적 가드레일
실패 모드를 먼저 표로 설계하게 한 것 (01_architecture.md:152)
product_name_snapshot 같은 실무 패턴을 이유와 함께 제시한 것
axum 0.8 라우트 문법(/v1/users/{id})이 정확함 — 0.7의 :id로 쓴 자료가 많은데 여기는 맞습니다
문제는 "NestJS/Spring급"이라는 목표 대비 빠진 축이 3개 있고, 코드 예제에 그대로 복붙하면 사고 나는 것들이 섞여 있다는 점입니다.

A. 먼저 고쳐야 할 것 (실제 버그 / 보안)
A-1. Gateway 헤더 스푸핑 — 가장 위험
06_gateway_auth.md:150 forward()는 x-user-id를 새 HeaderMap에 넣지만, 클라이언트가 보낸 원본 x-user-id를 제거하지 않습니다. 문서 §5에서 "모델 A: X-User-Id 헤더 신뢰"를 허용해두었기 때문에, 내부망 침투나 SSRF 한 번이면 임의 사용자 사칭이 됩니다.

규칙으로 못 박아야 합니다:

gateway 진입 시점에 x-user-*, x-request-id 등 "신뢰 헤더 prefix" 전부 strip → 그 다음 주입
그리고 모델 A/B 선택지를 주는 대신 B(서비스도 검증)를 기본으로 하고 A는 "PoC 한정"으로 강등하길 권합니다. RS256을 쓰면 서비스는 public key만 들고 검증하므로 비용이 거의 없습니다.

A-2. 프록시 응답에서 헤더가 전부 사라짐
06_gateway_auth.md:195:

Ok(Response::builder().status(status).body(Body::from(bytes)).unwrap())
content-type이 없습니다. 클라이언트가 JSON을 JSON으로 못 받습니다. set-cookie, cache-control, location(리다이렉트), content-encoding도 다 유실. 응답 헤더 화이트리스트 복사가 필요합니다.

같은 함수에서 요청 바디를 to_bytes(.., 1MB)로 전량 버퍼링하므로 파일 업로드는 구조적으로 불가능하고, 대용량 요청이 오면 gateway 메모리가 요청 수 × 1MB로 증가합니다.

A-3. 내부 에러 메시지가 클라이언트로 나감
02_workspace.md:186:

message: self.to_string(),
ApiError::Internal(#[from] anyhow::Error)는 #[error(transparent)]라 anyhow 체인 전체가 그대로 응답 바디에 들어갑니다. DB 에러면 테이블/컬럼명, 커넥션 문자열 일부까지 노출됩니다.

// Internal만 분기
let message = match &self {
    ApiError::Internal(e) => { tracing::error!(error = ?e, "internal"); "internal server error".to_string() }
    other => other.to_string(),
};
덤으로 ErrorBody에 trace_id를 넣어야 사용자가 준 에러 화면으로 로그를 찾을 수 있습니다.

A-4. Settings가 Debug를 파생 — 시크릿 로그 유출
02_workspace.md:222의 Settings는 #[derive(Debug)]에 jwt_secret: String, database.url(비밀번호 포함)을 들고 있습니다. tracing::info!(?settings) 한 줄이면 시크릿이 로그 수집기(Loki/ELK)에 영구 저장됩니다. secrecy::SecretString을 쓰거나 Debug를 수동 구현해서 마스킹하세요.

A-5. 키셋 페이지네이션 + UUIDv4 조합이 깨져 있음
04_database.md:230은 WHERE id < $1 ORDER BY id DESC를 권하는데, 같은 문서 :80의 스키마는 DEFAULT gen_random_uuid() = UUIDv4(완전 랜덤) 입니다. 랜덤 UUID로 정렬하면 시간순도 아니고 인덱스 지역성도 없습니다.

둘 중 하나로 정리:

앱에서 UUIDv7 생성 (workspace deps에 uuid v7 feature는 이미 켜져 있음) → 시간 정렬 가능
또는 커서를 (created_at, id) 복합키로
A-6. 비밀번호 해싱이 async 런타임을 블로킹
06_gateway_auth.md:248에서 argon2를 권하는데, argon2는 설계상 수십~수백 ms CPU를 태웁니다. 03_service_anatomy.md:180의 hash_password(password)?처럼 async 함수 안에서 동기 호출하면 tokio 워커 스레드가 그 시간 동안 멈춥니다. 로그인 트래픽이 몰리면 서비스 전체 p99가 무너집니다.

let hash = tokio::task::spawn_blocking(move || hash_password(&pw)).await??;
이건 Node/JVM에서 넘어온 팀이 100% 밟는 지뢰라, 문서에 명시적으로 넣을 가치가 있습니다.

A-7. Rate limit INCR/EXPIRE 레이스
05_redis.md:137: INCR 직후 프로세스가 죽거나 Redis 페일오버가 나면 EXPIRE가 안 걸려 키가 영구히 남고, 해당 IP는 영구 차단됩니다. Lua 스크립트 한 방(또는 SET key 0 EX w NX → INCR)으로 원자화해야 합니다.

같은 맥락으로 rate limit 키 gw:rl:{ip} 의 ip를 X-Forwarded-For에서 뽑는다면 신뢰 프록시 홉 수를 고정하지 않으면 헤더 위조로 우회됩니다. 그리고 인증 후에는 IP가 아니라 user_id/tenant 기준이어야 NAT 뒤 사용자들이 서로를 굶기지 않습니다.

A-8. Idempotency 스케치에 레이스
05_redis.md:188의 SET idem:... → processing은 NX가 없어서 동시 요청 2개가 둘 다 통과합니다. SET NX로 획득에 성공한 쪽만 처리하고, 실패한 쪽은 409 또는 짧은 폴링으로 저장된 응답을 기다려야 합니다. 키에 요청 바디 해시도 포함해야 같은 키로 다른 내용을 보내는 공격을 막습니다.

A-9. 캐시 예제가 컴파일 안 됨 + 설계 문제
05_redis.md:90에서 serde_json::from_str::<UserRow> 하는데 UserRow는 03:105에서 sqlx::FromRow만 파생합니다(Serialize/Deserialize 없음).

더 중요한 건 DB row를 그대로 캐싱하면 안 된다는 점입니다. UserRow에는 password_hash가 들어 있습니다 — Redis에 비밀번호 해시를 평문 JSON으로 뿌리게 됩니다. 캐시 대상은 전용 DTO(CachedUser)여야 하고, 스키마 변경 시 역직렬화 실패를 대비해 키에 버전 프리픽스(user:v2:{id})를 넣는 게 정석입니다.

A-10. 미들웨어 순서가 뒤집혀 있음
03_service_anatomy.md:289:

.layer(TraceLayer::new_for_http())   // ← 가장 안쪽
.layer(CompressionLayer::new())
.layer(TimeoutLayer::with_status_code(...))
.layer(CorsLayer::permissive())      // ← 가장 바깥
axum은 나중에 붙인 layer가 바깥입니다(src/docs/middleware.md의 onion 그림 기준). 지금 구조면 TimeoutLayer가 만들어낸 408, CORS 거절 응답이 TraceLayer를 안 거쳐서 로그에 안 남습니다. 관측성 문서를 따로 쓰면서 이러면 아깝습니다. TraceLayer를 맨 마지막에 붙이거나 ServiceBuilder로 감싸세요.

(참고로 TimeoutLayer::with_status_code는 tower-http 0.6.11에 실존합니다 — 확인했습니다.)

A-11. config::Environment::default() 프리픽스 없음
02_workspace.md:238은 프리픽스 없이 모든 환경변수를 설정으로 흡수하려 시도합니다. PATH, HOME, CI가 주입하는 수백 개 변수까지 파싱 대상이 되어 배포 환경마다 다르게 깨집니다. .prefix("APP").separator("__")로 네임스페이스를 잡으세요. 그러면 .env.example의 JWT_SECRET/HTTP__ADDR 혼용 컨벤션도 자연스럽게 통일됩니다.

A-12. 사소하지만 정확도
07:59 "Circuit breaker — tower::retry" → tower에 circuit breaker는 없습니다. retry ≠ circuit breaker(재시도는 오히려 장애를 증폭시킵니다). 별도 구현이나 크레이트가 필요하다고 써야 합니다.
08:96 PrometheusBuilder::new().install()은 자체 HTTP 리스너를 띄웁니다. axum에 /metrics 라우트로 붙이려면 install_recorder()로 핸들을 받아 핸들러에서 render()해야 합니다.
02:82 sqlx features에 TLS가 없습니다. 09:235 체크리스트는 "DB TLS"를 요구하는데 의존성이 그걸 못 합니다 → tls-rustls 추가.
03:23 UserService::new(users, redis.clone()) — redis는 redis::Client인데 03:167 시그니처는 ConnectionManager를 받습니다.
09:130 Dockerfile에 Cargo.lock COPY도, --locked도, USER 비루트도, HEALTHCHECK도 없습니다(비루트는 체크리스트엔 있는데 예제엔 없음).
버전들이 한 세대 뒤쳐져 있습니다: redis 0.27, config 0.14, opentelemetry 0.27. OTel은 특히 tracing-opentelemetry와의 버전 매칭이 까다로우니 "이 조합으로 검증됨" 표를 붙이는 게 좋습니다.
B. 통째로 빠진 개념 (중요도 순)
B-1. Graceful shutdown — 문서 전체에 한 글자도 없음 ⚠️
MSA에서 가장 자주 겪는 운영 사고가 배포 때마다 나는 5xx인데, 지금 axum::serve(listener, app).await?는 SIGTERM을 받으면 진행 중인 요청을 그냥 끊습니다.

필요한 것:

axum::serve(...).with_graceful_shutdown(shutdown_signal()) + SIGTERM/SIGINT 핸들링
드레인 순서: /ready를 먼저 실패로 전환 → LB가 뺄 때까지 대기(preStop sleep 5~15s) → 그 다음 shutdown
백그라운드 태스크(outbox 워커, NATS consumer)를 CancellationToken으로 같이 정리
종료 타임아웃(강제 종료 데드라인)
이거 없으면 롤링 업데이트마다 주문이 유실됩니다. 10장으로 독립시킬 가치가 있습니다.

B-2. 과부하 방어 (backpressure / load shedding)
Rate limit은 있는데 자기 보호가 없습니다. Rust는 빠른 대신 큐가 무한정 쌓이면 메모리로 죽습니다.

tower::limit::ConcurrencyLimitLayer + LoadShedLayer (한계 초과 시 즉시 503)
DefaultBodyLimit (axum 기본 2MB — 확인했습니다. 엔드포인트별 조정 필요)
CatchPanicLayer — 이게 없으면 핸들러 패닉이 커넥션을 그냥 끊어버려서 클라이언트가 원인을 모릅니다
타임아웃 예산(budget): gateway 15s인데 내부 클라이언트가 2s면, 남은 시간을 하위 홉에 전파(deadline propagation)해야 무의미한 작업을 안 합니다
의존성별 세마포어(bulkhead) — catalog가 느려질 때 order 전체 워커가 그쪽에 묶이지 않도록
B-3. tokio 런타임과 컨테이너의 불일치
#[tokio::main]은 호스트의 논리 코어 수만큼 워커 스레드를 만듭니다. K8s에서 cpu: 500m 제한을 걸어도 64코어 노드면 64스레드를 띄우고, cgroup throttling으로 지연이 튑니다. worker_threads를 CPU limit에 맞춰 명시하는 항목이 배포 체크리스트에 필요합니다.

[profile.release](lto, codegen-units, strip) 설정도 없습니다 — 바이너리 크기와 이미지 크기에 직결됩니다.

B-4. CI/CD와 코드 품질 게이트 — 챕터 자체가 없음
"Spring Boot급"을 자칭하려면 이게 빠지면 안 됩니다:

cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo sqlx prepare --check — .sqlx 캐시가 최신인지 CI에서 강제 (안 하면 로컬만 되고 CI가 깨집니다)
cargo deny check (라이선스 + RUSTSEC 취약점), cargo audit
cargo nextest (병렬 테스트, 워크스페이스에서 체감 차이 큼)
빌드 캐시 전략(cargo-chef는 09에서 언급만 하고 예제는 없음), sccache
이미지 취약점 스캔(trivy), SBOM, 불변 태그(:latest 금지)
롤백 절차 — 배포 체크리스트에 롤백이 없습니다
B-5. 무중단 스키마 변경 (expand/contract)
04는 "마이그레이션 쓰세요"에서 끝납니다. 그런데 롤링 업데이트 중에는 구버전과 신버전 코드가 동시에 같은 DB를 봅니다. 필요한 규칙:

expand → migrate → contract 3단계 (컬럼 추가는 nullable로, 삭제는 다음 배포에서)
ALTER TABLE은 SET lock_timeout 걸고 실행 (안 그러면 테이블 전체가 잠깁니다)
CREATE INDEX CONCURRENTLY
파괴적 변경 금지 목록
Prisma/JPA에서 넘어온 팀이 특히 못 하는 부분입니다.

B-6. PgBouncer × sqlx 함정
04:136에서 PgBouncer를 권하는데, transaction pooling 모드에서는 prepared statement가 깨집니다. sqlx는 기본으로 statement 캐시를 쓰므로 .statement_cache_capacity(0) 또는 session 모드가 필요합니다. 이거 모르고 붙였다가 프로덕션에서 랜덤 에러 나는 사례가 흔합니다.

같이 빠진 것: 커넥션별 statement_timeout / idle_in_transaction_session_timeout 설정(after_connect 훅), 읽기 레플리카 분리, SELECT FOR UPDATE vs 낙관적 잠금(version 컬럼).

B-7. Outbox 운영 디테일
패턴은 맞는데 워커를 실제로 굴리면 나오는 것들이 없습니다:

attempts, next_retry_at, last_error 컬럼 → 지수 백오프
DLQ — 영구 실패 이벤트를 어디로 보낼 것인가
published_at IS NOT NULL 행 정리/파티셔닝 (안 하면 테이블이 무한 증식하고 폴링 쿼리가 느려집니다)
순서 보장 — 같은 aggregate의 이벤트 순서가 필요하면 파티션 키 개념 필요
폴링 간격 vs LISTEN/NOTIFY 트레이드오프
outbox lag 메트릭 (08에 한 줄 언급은 있음 ✓)
인박스(inbox) 패턴 — 소비자 측 processed_events(event_id) 테이블. 07에서 "notification_log에 event_id UNIQUE"로 사실상 언급했는데, 이걸 일반 패턴으로 승격시키면 좋습니다
NATS 쪽도 max_deliver, ack_wait, backoff, DLQ 스트림 설정이 필요합니다.

B-8. 보안 챕터 부재
A절 항목들 외에:

보안 헤더 (HSTS, X-Content-Type-Options, CSP) — tower-http::set_header
JWT 키 회전: kid 헤더 + JWKS 엔드포인트. 지금 구조는 시크릿 바꾸면 전 사용자가 로그아웃됩니다
강제 로그아웃 / 토큰 폐기 — jti 블랙리스트. 05에서 언급만 하고 설계가 없음
Refresh token 재사용 탐지 (rotation은 있는데 탈취 감지가 없음) — 회전된 토큰이 다시 오면 해당 패밀리 전체 무효화
argon2 파라미터 권고치(m/t/p)
감사 로그(audit log) — 누가 언제 무엇을. 규제 있는 도메인이면 필수
PII 처리 / 로그 마스킹 규칙 (08:146에 "금지"는 있으나 강제 수단 없음)
B-9. API 계약 / OpenAPI
readme 대응표에는 utoipa가 있는데 어느 챕터에도 안 나옵니다. MSA에서 계약은 핵심입니다:

utoipa + utoipa-swagger-ui로 서비스별 스펙 자동 생성
gateway에서 스펙 통합(aggregation)
스펙에서 클라이언트 생성 → crates/clients (07:194에 언급만)
컨슈머 주도 계약 테스트 — 서비스 간 결합에서 회귀를 잡는 유일한 실질적 수단
이벤트 스키마도 마찬가지 (JSON Schema / protobuf 레지스트리)
B-10. 분산 트레이스 컨텍스트 전파
08은 X-Request-Id만 다룹니다. 그런데 Jaeger에서 gateway → order → user가 한 트레이스로 보이려면 W3C traceparent 헤더 전파가 필요합니다(opentelemetry-http의 injector/extractor를 reqwest 요청과 axum 미들웨어에 각각 연결). 08:130이 "하나의 Trace로 보이게 하면"이라고 약속하지만 그걸 만드는 코드가 없습니다.

메트릭 쪽은 카디널리티 경고를 추가하세요 — 라벨에 user_id나 원본 path(/v1/orders/{uuid})를 넣으면 Prometheus가 죽습니다. 반드시 라우트 템플릿으로.

B-11. 테스트 전략이 얇음
03:336의 표 3줄이 전부입니다. 보강할 것:

sqlx::test + testcontainers 실제 예제 (DB 격리 방식: 트랜잭션 롤백 vs 스키마별 생성)
axum::Router::oneshot 통합 테스트 예제
외부 서비스 모킹 (wiremock)
계약 테스트 (B-9)
부하 테스트 도구(oha, k6)와 목표치 — 09:241에 "부하 테스트"만 있고 방법이 없음
장애 주입 (의존성 죽여보기) — 01에서 실패 모드를 설계했으니 검증도 있어야 짝이 맞습니다
B-12. 아키텍처 조언 하나 추가 권장
"gateway를 직접 만들 것인가" 를 짚어주세요. 06의 프록시 코드가 다루지 못하는 것들(스트리밍, HTTP/2, 커넥션 풀링, 재시도, 헤더 처리, WebSocket 업그레이드)은 문서도 "실무에서는"으로 넘겼는데, 사실 그건 Envoy/Traefik이 이미 다 하는 일입니다.

현실적 권장 구성:

Traefik/Envoy (L7 라우팅·TLS·재시도)  +  얇은 Rust auth 서비스 (JWT 검증·rate limit)
직접 만든 gateway는 학습용이거나, 정말 도메인 특화 로직이 필요할 때만. 이걸 안 적어두면 팀이 프록시 재구현에 몇 주를 씁니다.

B-13. 그 외 실무에서 곧 필요해질 것들
주제	왜
스케줄러/배치 잡	MSA에서 크론을 여러 replica가 동시에 돌리는 사고. 리더 선출 또는 advisory lock
파일 업로드 / S3	presigned URL로 gateway 우회 (B-2의 바디 버퍼링 문제 해결책)
WebSocket / SSE	프록시 통과, sticky session, 스케일아웃
서비스 디스커버리	URL이 env 하드코딩. gateway 라우팅 테이블을 설정으로 외부화
피처 플래그 / 킬 스위치	배포 없이 기능 차단
SLO / 에러 버짓	08의 알림 임계치가 근거 없이 나옴
ADR	"왜 sqlx인가" 같은 결정을 기록하는 습관
just/Makefile, 시드 데이터	09:284의 cargo run & 나열은 실제로는 관리 불가
C. 우선순위 제안
당장 (문서 신뢰도 문제 — 복붙하면 사고):
A-1 헤더 스푸핑 · A-2 응답 헤더 · A-3 에러 노출 · A-4 시크릿 로그 · A-6 argon2 블로킹 · A-9 password_hash 캐싱

1주차 (신규 챕터 2개):
10_resilience.md — graceful shutdown, load shedding, CatchPanic, 타임아웃 예산, bulkhead
11_security.md — A절 보안 항목 + JWT 회전 + 감사 로그

2주차:
12_cicd.md — clippy/deny/nextest/sqlx prepare --check/이미지 스캔/롤백
13_api_contract.md — utoipa + 클라이언트 생성 + 계약 테스트

기존 문서 보강:
04에 expand/contract + PgBouncer 함정, 07에 outbox 운영(재시도/DLQ/정리) + inbox, 08에 traceparent 전파 + 카디널리티, 03에 테스트 예제 실물

한 가지만 덧붙이면, 09:263의 "병목은 언어보다 경계·데이터소유·관측성"은 정확한 통찰입니다. 다만 지금 문서는 그 셋 중 관측성이 가장 얇고, 거기에 더해 위 B-1(graceful shutdown)과 B-2(과부하 방어)가 빠져 있어서 "Rust로 만들었는데 왜 배포할 때마다 에러 나지?"를 겪게 됩니다. 그 두 개가 실질적으로 가장 급합니다.

원하시면 10_resilience.md나 11_security.md를 기존 문서 톤(NestJS/Spring 대응표 + 체크포인트 형식)에 맞춰 바로 써드리겠습니다. 아니면 A절 지적사항만 기존 파일에 반영하는 것부터 할 수도 있습니다.