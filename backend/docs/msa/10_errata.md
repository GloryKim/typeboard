# 10. Errata — 01~09 예제의 정정 목록

기존 문서의 코드 예제 중 **그대로 복붙하면 보안 사고·컴파일 실패·운영 장애**로 이어지는 항목들입니다.
기존 파일은 수정하지 않으므로, 구현할 때 **이 문서를 옆에 두고** 대조하세요.

각 항목은 `증상 → 왜 → 고친 코드` 순서입니다.

| # | 위치 | 심각도 | 한 줄 |
|---|---|---|---|
| 1 | 06 §4 `forward()` | 🔴 치명 | 클라이언트가 보낸 `x-user-id`를 지우지 않음 → 사칭 |
| 2 | 06 §4 응답 생성 | 🔴 치명 | 업스트림 응답 헤더 전부 유실 (content-type 포함) |
| 3 | 02 §3 `IntoResponse` | 🔴 치명 | 내부 에러 체인이 클라이언트에 그대로 노출 |
| 4 | 02 §3 `Settings` | 🔴 치명 | `Debug` 파생 → 시크릿이 로그로 유출 |
| 5 | 05 §4 캐시 | 🔴 치명 | `password_hash`가 든 DB row를 통째로 Redis에 저장 |
| 6 | 06 §6 / 03 §5 argon2 | 🟠 높음 | 동기 해싱이 tokio 워커를 블로킹 |
| 7 | 05 §5 rate limit | 🟠 높음 | `INCR`/`EXPIRE` 비원자 → 영구 차단 가능 |
| 8 | 05 §7 idempotency | 🟠 높음 | `NX` 없음 → 동시 요청 이중 처리 |
| 9 | 04 §8 페이지네이션 | 🟠 높음 | UUIDv4 + 키셋 정렬은 성립하지 않음 |
| 10 | 03 §7 layer 순서 | 🟡 중간 | TraceLayer가 안쪽 → 타임아웃/CORS 응답이 로그에 안 남음 |
| 11 | 02 §3 config 로더 | 🟡 중간 | prefix 없음 → 모든 환경변수를 흡수 |
| 12 | 여러 곳 | 🟡 중간 | 타입 불일치·의존성 누락·버전 노후 |

---

## 1. 🔴 Gateway 헤더 스푸핑 — `x-user-id` 위조

**위치:** [06_gateway_auth §4](./06_gateway_auth.md) `forward()`

### 증상

```rust
let mut headers = HeaderMap::new();
if auth_required {
    let claims = state.jwt.verify(token)?;
    headers.insert("x-user-id", HeaderValue::from_str(&claims.sub.to_string())?);
}
```

새 `HeaderMap`을 만들어 주입하는 것 자체는 맞습니다. 문제는 **원본 요청의 헤더를 선별 복사할 때**입니다.
06 §4에는 `// content-type 등 원본 헤더 선별 복사` 라는 주석만 있고, 여기서 원본 `x-user-id`를 같이 복사하면 그대로 끝입니다.

그리고 06 §5의 **모델 A(Gateway만 신뢰)** 를 선택하면, 내부 서비스는 `x-user-id`를 무조건 믿습니다.
공격 경로는 gateway 우회만이 아닙니다.

```
1. 어떤 서비스에 SSRF 취약점 하나 → 내부에서 http://order-service:3002 호출
   + x-user-id: <피해자 UUID>  → 남의 주문 전체 조회
2. 사내망 접근 권한이 있는 누구나 curl 한 줄로 관리자 사칭
3. 개발자가 실수로 서비스 포트를 LoadBalancer로 노출 → 인증이 통째로 사라짐
```

### 고친 코드

**신뢰 헤더 prefix를 정의하고, 진입 시점에 무조건 제거합니다.**

```rust
/// gateway가 스스로 주입하는 헤더. 외부에서 들어온 것은 신뢰하지 않는다.
const TRUSTED_PREFIX: &str = "x-internal-";

const HOP_BY_HOP: &[&str] = &[
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "te", "trailer", "transfer-encoding", "upgrade",
];

/// 업스트림으로 넘길 헤더를 만든다.
/// 1) 신뢰 prefix 헤더는 전부 버린다 (위조 차단)
/// 2) hop-by-hop 헤더는 프록시가 전달하면 안 된다 (RFC 7230)
/// 3) 나머지는 화이트리스트만 통과
fn sanitize_headers(original: &HeaderMap) -> HeaderMap {
    const FORWARD_ALLOW: &[&str] = &[
        "content-type", "content-length", "accept", "accept-encoding",
        "accept-language", "user-agent", "idempotency-key", "traceparent", "tracestate",
    ];

    let mut out = HeaderMap::new();
    for (name, value) in original.iter() {
        let n = name.as_str();
        if n.starts_with(TRUSTED_PREFIX) {
            // 외부에서 온 신뢰 헤더 → 조용히 폐기 (또는 400으로 거절)
            tracing::warn!(header = %n, "dropped spoofed trusted header");
            continue;
        }
        if HOP_BY_HOP.contains(&n) {
            continue;
        }
        if FORWARD_ALLOW.contains(&n) {
            out.insert(name.clone(), value.clone());
        }
    }
    out
}
```

그리고 주입은 sanitize **이후에**:

```rust
let mut headers = sanitize_headers(req.headers());

if auth_required {
    let token = bearer_token(req.headers()).ok_or(ApiError::Unauthorized)?;
    let claims = state.jwt.verify(token).map_err(|_| ApiError::Unauthorized)?;

    headers.insert("x-internal-user-id", HeaderValue::from_str(&claims.sub.to_string())
        .map_err(|_| ApiError::Unauthorized)?);
    headers.insert("x-internal-roles", HeaderValue::from_str(&claims.roles.join(","))
        .map_err(|_| ApiError::Unauthorized)?);
}
```

> `HeaderValue::from_str(...).unwrap()` 을 쓰지 마세요.
> `claims.email`처럼 **사용자가 통제하는 값**이 들어가면, 개행이 포함된 이메일로 gateway를 패닉시킬 수 있습니다.
> (헤더 인젝션 시도가 `unwrap()` 패닉 = DoS)

### 더 근본적으로

문서 06 §5는 모델 A/B를 대등하게 제시하지만, **B(서비스도 JWT 재검증)를 기본**으로 하세요.
RS256/EdDSA를 쓰면 내부 서비스는 **공개키만** 들고 검증하므로 비용이 거의 없고, gateway 우회 경로가 통째로 사라집니다.

자세한 설계는 [12_security §1](./12_security.md).

---

## 2. 🔴 프록시 응답의 헤더가 전부 사라짐

**위치:** [06_gateway_auth §4](./06_gateway_auth.md) 마지막 줄

### 증상

```rust
let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap();
let bytes = upstream.bytes().await?;
Ok(Response::builder().status(status).body(Body::from(bytes)).unwrap())
```

응답에 **헤더가 하나도 없습니다.**

| 유실되는 헤더 | 결과 |
|---|---|
| `content-type` | 브라우저가 JSON을 텍스트로 취급, `fetch().json()` 실패 |
| `location` | 3xx 리다이렉트가 동작 안 함 |
| `set-cookie` | 세션/refresh 쿠키 방식이면 로그인 자체가 안 됨 |
| `cache-control`, `etag` | 캐싱 전략 전멸 |
| `content-encoding` | 업스트림이 gzip을 보냈으면 클라이언트가 깨진 바이트를 받음 |
| `retry-after` | 429/503 재시도 힌트 소실 |

### 고친 코드

```rust
const RESP_ALLOW: &[&str] = &[
    "content-type", "cache-control", "etag", "last-modified",
    "location", "retry-after", "x-request-id", "content-language",
];

let status = StatusCode::from_u16(upstream.status().as_u16())
    .map_err(|e| ApiError::Internal(e.into()))?;

let upstream_headers = upstream.headers().clone();
let bytes = upstream.bytes().await.map_err(|e| ApiError::Internal(e.into()))?;

let mut builder = Response::builder().status(status);
for name in RESP_ALLOW {
    if let Some(v) = upstream_headers.get(*name) {
        builder = builder.header(*name, v);
    }
}
// set-cookie는 여러 개일 수 있으므로 get_all로 따로 처리
for v in upstream_headers.get_all("set-cookie") {
    builder = builder.header("set-cookie", v);
}

builder.body(Body::from(bytes)).map_err(|e| ApiError::Internal(e.into()))
```

> `content-encoding` / `content-length`는 **복사하지 마세요.**
> `upstream.bytes()`는 reqwest가 이미 압축을 해제한 바이트를 주므로,
> `content-encoding: gzip`을 그대로 붙이면 클라이언트가 압축 해제를 두 번 시도해서 깨집니다.
> 압축은 gateway의 `CompressionLayer`가 다시 하게 두세요.

### 그리고 바디 버퍼링 문제

같은 함수의:

```rust
let body = axum::body::to_bytes(req.into_body(), 1024 * 1024).await?;
```

- **1MB 초과 요청은 무조건 실패** → 파일 업로드 불가
- 동시 요청 N개면 메모리 N × 1MB가 gateway에 고입니다
- 업스트림 응답도 `upstream.bytes()`로 전량 버퍼링 → 대용량 다운로드도 마찬가지

스트리밍으로 바꾸는 게 정석이지만, 실무 판단은 **"업로드/다운로드는 gateway를 통과시키지 않는다"** 입니다.
S3 presigned URL 방식은 [19_platform_decisions §4](./19_platform_decisions.md).

---

## 3. 🔴 내부 에러 메시지가 클라이언트로 나감

**위치:** [02_workspace §3](./02_workspace.md) `impl IntoResponse for ApiError`

### 증상

```rust
#[error(transparent)]
Internal(#[from] anyhow::Error),
// ...
let body = ErrorBody {
    error: code.into(),
    message: self.to_string(),   // ← 여기
};
```

`#[error(transparent)]`이므로 `Internal`의 `to_string()`은 **anyhow 체인 전체**입니다.

```json
{
  "error": "internal",
  "message": "error returned from database: relation \"users_v2\" does not exist"
}
```

실제로 새어나가는 것들:

- 테이블·컬럼명 (SQL 인젝션 지점을 찾는 데 직접 쓰임)
- 커넥션 문자열 일부 (`postgres://order:***@postgres-order:5432/...`)
- 내부 호스트명·포트 (`http://user-service:3001` → 내부 토폴로지 지도)
- 파일 경로, 크레이트 버전

### 고친 코드

```rust
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            ApiError::NotFound(_)     => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Unauthorized    => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Forbidden       => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::BadRequest(_)   => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Conflict(_)     => (StatusCode::CONFLICT, "conflict"),
            ApiError::Internal(_)     => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };

        // 현재 span에서 trace_id를 꺼내 사용자에게 준다 (지원 문의 → 로그 검색)
        let trace_id = current_trace_id();

        let message = match &self {
            // 5xx: 상세는 로그에만, 클라이언트에는 고정 문구
            ApiError::Internal(e) => {
                tracing::error!(error = ?e, trace_id = %trace_id, "internal error");
                "internal server error".to_string()
            }
            // 4xx: 사용자가 고칠 수 있는 정보이므로 그대로
            other => {
                tracing::warn!(error = %other, trace_id = %trace_id, "client error");
                other.to_string()
            }
        };

        let body = ErrorBody { error: code.into(), message, trace_id };
        (status, Json(body)).into_response()
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: String,
    trace_id: String,   // 추가
}
```

### 4xx도 무조건 안전하진 않습니다

`ApiError::NotFound(format!("user {email} not found"))` 같은 코드는 **계정 열거(enumeration)** 를 허용합니다.
로그인/비밀번호 재설정 경로에서는 "존재하지 않는 이메일"과 "비밀번호 틀림"을 **구분하지 마세요.**

```rust
// ❌ 계정 존재 여부가 새어나감
if user.is_none() { return Err(ApiError::NotFound("email not found".into())); }
if !verify(pw)?   { return Err(ApiError::Unauthorized); }

// ✅ 동일 응답 + 동일 소요시간(타이밍 공격 방어)
let user = repo.find_by_email(email).await?;
let ok = match &user {
    Some(u) => verify_password(pw, &u.password_hash)?,
    None    => { verify_password(pw, DUMMY_HASH)?; false }  // 더미 해싱으로 시간 맞춤
};
if !ok { return Err(ApiError::Unauthorized); }
```

---

## 4. 🔴 `Settings`의 `Debug` 파생 → 시크릿 로그 유출

**위치:** [02_workspace §3](./02_workspace.md)

### 증상

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub database: DbConfig,   // url에 비밀번호 포함
    pub jwt_secret: String,   // ← 그대로
}
```

`tracing::info!(?settings, "loaded config")` 한 줄이면 끝입니다.
그리고 이건 로컬 콘솔이 아니라 **Loki/ELK에 영구 저장**됩니다. 로그 접근 권한 = 시크릿 접근 권한이 됩니다.

더 나쁜 경로: `anyhow` 에러가 `Settings`를 컨텍스트로 물고 있으면, 위 §3의 에러 노출과 합쳐져
**시크릿이 HTTP 응답으로 나갈 수도** 있습니다.

### 고친 코드 (A) — `secrecy` 크레이트

```toml
secrecy = { version = "0.10", features = ["serde"] }
```

```rust
use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub http: HttpConfig,
    pub database: DbConfig,
    pub redis: RedisConfig,
    pub jwt_secret: SecretString,   // Debug 출력이 "SecretString([REDACTED])"
}

// 사용처에서만 명시적으로 꺼냄
let keys = JwtKeys::from_secret(settings.jwt_secret.expose_secret(), &issuer, ttl);
```

`expose_secret()`을 호출해야만 값을 볼 수 있으므로, **grep 한 번으로 시크릿이 어디서 쓰이는지 전수 감사**가 됩니다.

### 고친 코드 (B) — 수동 `Debug`

크레이트를 늘리기 싫으면:

```rust
#[derive(Clone, Deserialize)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
}

impl std::fmt::Debug for DbConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // postgres://user:pass@host:5432/db → postgres://user:***@host:5432/db
        f.debug_struct("DbConfig")
            .field("url", &redact_url(&self.url))
            .field("max_connections", &self.max_connections)
            .finish()
    }
}
```

### 부수적으로: 헤더도 마스킹

`TraceLayer`는 요청 헤더를 로그에 남길 수 있습니다. `authorization`, `cookie`가 평문으로 남습니다.

```rust
use tower_http::sensitive_headers::SetSensitiveHeadersLayer;
use axum::http::header::{AUTHORIZATION, COOKIE, SET_COOKIE};

// TraceLayer보다 바깥에 붙여야 효과가 있음
.layer(SetSensitiveHeadersLayer::new([AUTHORIZATION, COOKIE, SET_COOKIE]))
```

---

## 5. 🔴 `password_hash`가 든 DB row를 Redis에 통째로 저장

**위치:** [05_redis §4](./05_redis.md) 캐시-aside 예제

### 증상 1 — 애초에 컴파일이 안 됨

```rust
serde_json::from_str::<UserRow>(&json)
```

`UserRow`는 [03 §4](./03_service_anatomy.md)에서 `#[derive(Debug, sqlx::FromRow)]` 뿐입니다.
`Serialize`/`Deserialize`가 없어서 컴파일 실패합니다.

### 증상 2 — derive를 추가하면 그때부터 진짜 문제

`UserRow`에는 `password_hash: String`이 있습니다. derive를 붙이고 캐싱하면:

```
redis> GET user:018f...
"{\"id\":\"018f...\",\"email\":\"a@b.com\",\"password_hash\":\"$argon2id$v=19$m=19456,...\"}"
```

- Redis는 보통 **디스크에 AOF/RDB로 남습니다** ([05 §2](./05_redis.md)의 `--appendonly yes`)
- Redis는 보통 **여러 서비스가 공유**합니다 ([05 §1](./05_redis.md))
- Redis는 보통 **TLS 없이 내부망**에 있습니다
- 백업이 어디로 가는지 아무도 추적하지 않습니다

비밀번호 해시의 노출 범위가 "PG의 users 테이블"에서 "Redis + 그 백업 + 그걸 읽는 모든 서비스"로 폭발합니다.

### 고친 코드 — 캐시 전용 DTO + 키 버저닝

```rust
/// DB row와 분리된 캐시 표현.
/// 여기에 필드를 추가하려면 반드시 의식적인 결정이 필요하게 만든다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    // password_hash 없음. 절대 추가하지 않는다.
}

impl From<&UserRow> for CachedUser {
    fn from(r: &UserRow) -> Self {
        Self { id: r.id, email: r.email.clone(), name: r.name.clone(), created_at: r.created_at }
    }
}

/// 캐시 키에 스키마 버전을 넣는다.
/// CachedUser 필드가 바뀌면 v2로 올려서, 옛 페이로드 역직렬화 실패를 원천 차단.
const CACHE_VER: &str = "v1";
fn user_key(id: Uuid) -> String { format!("user:{CACHE_VER}:{id}") }
```

```rust
pub async fn find_by_id(&self, id: Uuid) -> Result<CachedUser, UserError> {
    let key = user_key(id);
    let mut conn = self.redis.clone();

    // 1) cache — 실패해도 조용히 DB로
    match conn.get::<_, Option<String>>(&key).await {
        Ok(Some(json)) => match serde_json::from_str::<CachedUser>(&json) {
            Ok(u) => {
                metrics::counter!("cache_hit", "key" => "user").increment(1);
                return Ok(u);
            }
            Err(e) => {
                // 스키마가 어긋난 경우 — 지우고 진행
                tracing::warn!(error = %e, "stale cache payload, evicting");
                let _: Result<(), _> = conn.del(&key).await;
            }
        },
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "redis unavailable, falling back to db"),
    }
    metrics::counter!("cache_miss", "key" => "user").increment(1);

    // 2) db
    let row = self.repo.find_by_id(id).await?.ok_or(UserError::NotFound)?;
    let cached = CachedUser::from(&row);

    // 3) set — TTL에 jitter를 주어 stampede 완화 (05 §4에서 언급된 것을 실제로 적용)
    if let Ok(payload) = serde_json::to_string(&cached) {
        let ttl = 300 + (rand::random::<u64>() % 60);
        let _: Result<(), _> = conn.set_ex(&key, payload, ttl).await;
    }

    Ok(cached)
}
```

### 캐시 무효화도 같이 고쳐야 합니다

05 §4의 `invalidate_user`는 키 하나만 지웁니다. 실제로는:

```rust
// 이메일로도 조회한다면 그 키도 같이 지워야 함
pub async fn invalidate_user(&self, u: &UserRow) {
    let mut conn = self.redis.clone();
    let _: Result<(), _> = conn.del(&[user_key(u.id), email_key(&u.email)]).await;
}
```

그리고 **다른 서비스가 이 유저를 스냅샷으로 들고 있다면** 캐시 삭제만으로는 부족합니다.
`user.updated` 이벤트를 발행해야 합니다 ([14_messaging_ops](./14_messaging_ops.md)).

---

## 6. 🟠 argon2 동기 해싱이 tokio 워커를 블로킹

**위치:** [06_gateway_auth §6](./06_gateway_auth.md) "argon2 권장", [03_service_anatomy §5](./03_service_anatomy.md) `hash_password(password)?`

### 증상

argon2는 **설계상 느리도록 만들어진** 알고리즘입니다. 권장 파라미터에서 한 번에 50~300ms의 CPU를 씁니다.

```rust
pub async fn register(&self, email: &str, name: &str, password: &str) -> Result<UserRow, UserError> {
    let hash = hash_password(password)?;   // ← async fn 안의 동기 CPU 작업
    ...
}
```

tokio 워커 스레드는 기본 **논리 코어 수만큼**만 존재합니다. 4코어 컨테이너에서:

```
동시 로그인 4건 → 워커 4개 전부 argon2에 묶임
→ 그 순간 이 서비스의 모든 요청(health check 포함)이 멈춤
→ /ready 타임아웃 → K8s가 파드를 트래픽에서 뺌
→ 남은 파드에 부하 집중 → 연쇄 붕괴
```

로그인 트래픽이 몰리는 순간(이벤트 오픈, 앱 업데이트 직후)에 정확히 이렇게 됩니다.
**Node.js에서 bcrypt를 동기로 부르면 안 되는 것과 똑같은 문제**인데, Rust에서는 "빠르니까 괜찮겠지"로 넘어가기 쉽습니다.

### 고친 코드

```rust
use argon2::{Argon2, PasswordHasher, PasswordVerifier, PasswordHash};
use argon2::password_hash::{rand_core::OsRng, SaltString};

pub async fn hash_password(password: String) -> Result<String, UserError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| UserError::Hash(e.to_string()))
    })
    .await
    .map_err(|e| UserError::Hash(e.to_string()))?   // JoinError
}

pub async fn verify_password(password: String, hash: String) -> Result<bool, UserError> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(|e| UserError::Hash(e.to_string()))?;
        Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
    })
    .await
    .map_err(|e| UserError::Hash(e.to_string()))?
}
```

### 여기서 끝이 아닙니다 — blocking 풀도 유한합니다

`spawn_blocking`의 기본 풀은 512 스레드입니다. 무제한 로그인 시도가 오면 512개 스레드가 전부 argon2를 돌려
**CPU가 포화**됩니다. 로그인 엔드포인트에는 반드시 동시성 제한이 필요합니다.

```rust
// AppState에 세마포어를 두고 해싱 동시 실행 수를 CPU 수준으로 제한
pub struct AppState {
    pub hash_permits: Arc<tokio::sync::Semaphore>,  // Semaphore::new(num_cpus)
}

let _permit = state.hash_permits.acquire().await.map_err(|_| UserError::Unavailable)?;
let hash = hash_password(pw).await?;
```

+ 로그인 경로에 IP/계정 단위 rate limit ([12_security §6](./12_security.md))

### 같은 함정이 있는 다른 작업

| 작업 | 왜 블로킹 | 대응 |
|---|---|---|
| argon2 / bcrypt | CPU 집약 | `spawn_blocking` + 세마포어 |
| 이미지 리사이즈 | CPU 집약 | `spawn_blocking` 또는 별도 워커 서비스 |
| 큰 JSON 직렬화 (수 MB) | CPU 집약 | `spawn_blocking` |
| `std::fs` 파일 IO | 동기 IO | `tokio::fs` |
| 동기 DB 드라이버 | 동기 IO | sqlx는 async라 OK |
| `std::thread::sleep` | 스레드 정지 | `tokio::time::sleep` |

**판단 기준: 한 번에 100µs 이상 CPU를 쓰면 `spawn_blocking`을 검토합니다.**

---

## 7. 🟠 Rate limit의 `INCR`/`EXPIRE` 레이스

**위치:** [05_redis §5](./05_redis.md)

### 증상

```rust
let count: i64 = redis::cmd("INCR").arg(key).query_async(conn).await?;
if count == 1 {
    let _: () = redis::cmd("EXPIRE").arg(key).arg(window_secs).query_async(conn).await?;
}
```

두 명령 사이에 무슨 일이든 일어날 수 있습니다.

| 사건 | 결과 |
|---|---|
| gateway 프로세스가 그 사이에 종료 | 키에 TTL이 영영 안 걸림 → **해당 IP 영구 차단** |
| Redis 페일오버 (마스터 교체) | 같은 결과 |
| `EXPIRE` 자체가 네트워크 오류 | 같은 결과 (`let _: () =`로 에러를 삼키고 있음) |

한번 발생하면 재현이 어렵고, "특정 사용자만 항상 429"라는 최악의 버그 리포트가 됩니다.

### 고친 코드 (A) — Lua로 원자화

```rust
/// INCR + 최초 1회 EXPIRE를 원자적으로.
/// 반환: 현재 카운트
const RL_SCRIPT: &str = r#"
    local c = redis.call('INCR', KEYS[1])
    if c == 1 then
        redis.call('EXPIRE', KEYS[1], ARGV[1])
    end
    return c
"#;

pub async fn allow(
    conn: &mut ConnectionManager,
    key: &str,
    limit: i64,
    window_secs: i64,
) -> redis::RedisResult<RateLimitVerdict> {
    let script = redis::Script::new(RL_SCRIPT);
    let count: i64 = script.key(key).arg(window_secs).invoke_async(conn).await?;

    Ok(RateLimitVerdict {
        allowed: count <= limit,
        remaining: (limit - count).max(0),
        limit,
    })
}

pub struct RateLimitVerdict { pub allowed: bool, pub remaining: i64, pub limit: i64 }
```

`redis::Script`는 `EVALSHA` → `NOSCRIPT`면 `EVAL`로 자동 폴백하므로 매번 스크립트를 보내지 않습니다.

### 고친 코드 (B) — 슬라이딩 윈도우

고정 윈도우는 경계에서 **한도의 2배**를 허용합니다 (윈도우 끝에 limit개, 다음 윈도우 시작에 limit개).
정확도가 필요하면 정렬 집합 기반:

```lua
-- KEYS[1]=key ARGV[1]=now_ms ARGV[2]=window_ms ARGV[3]=limit ARGV[4]=member
redis.call('ZREMRANGEBYSCORE', KEYS[1], 0, ARGV[1] - ARGV[2])
local c = redis.call('ZCARD', KEYS[1])
if c < tonumber(ARGV[3]) then
    redis.call('ZADD', KEYS[1], ARGV[1], ARGV[4])
    redis.call('PEXPIRE', KEYS[1], ARGV[2])
    return 1
end
return 0
```

### 응답 헤더를 빼먹지 마세요

클라이언트가 백오프할 수 있어야 합니다. 없으면 재시도 폭풍이 옵니다.

```rust
if !verdict.allowed {
    return Ok((
        StatusCode::TOO_MANY_REQUESTS,
        [
            ("retry-after", window_secs.to_string()),
            ("x-ratelimit-limit", verdict.limit.to_string()),
            ("x-ratelimit-remaining", "0".to_string()),
        ],
    ).into_response());
}
```

### 그리고 키의 `{ip}`가 위조 가능합니다

[05 §5](./05_redis.md)의 키는 `gw:rl:{ip}:{route}` 입니다. `ip`를 `X-Forwarded-For`에서 뽑는다면:

```
X-Forwarded-For: 1.2.3.4      ← 공격자가 매 요청마다 랜덤 값
→ 매번 새 키 → rate limit 완전 무력화
```

**신뢰 프록시 홉 수를 고정**하고 오른쪽에서 N번째를 취해야 합니다.

```rust
/// TRUSTED_HOPS: 우리가 통제하는 프록시 개수 (ALB 1개면 1)
fn client_ip(headers: &HeaderMap, peer: IpAddr, trusted_hops: usize) -> IpAddr {
    let xff: Vec<&str> = headers.get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').map(str::trim).collect())
        .unwrap_or_default();

    // 오른쪽 끝이 가장 가까운 프록시. trusted_hops개를 건너뛴 값이 실제 클라이언트.
    if xff.len() >= trusted_hops && trusted_hops > 0 {
        if let Ok(ip) = xff[xff.len() - trusted_hops].parse() { return ip; }
    }
    peer   // 헤더를 못 믿으면 TCP peer 주소
}
```

인증 이후에는 **IP가 아니라 `user_id`** 로 제한하세요. IP 기준이면 회사/학교 NAT 뒤의 사용자들이 서로를 굶깁니다.

---

## 8. 🟠 Idempotency-Key 구현의 레이스

**위치:** [05_redis §7](./05_redis.md)

### 증상

```
SET idem:order:{key}  → processing
... 작업 ...
SET idem:order:{key}  → {"order_id":"..."}  EX 86400
```

`NX`가 없습니다. 동시 요청 2개가 **둘 다 `processing`을 쓰고 둘 다 주문을 만듭니다.**
멱등 키를 붙인 이유가 정확히 이걸 막는 건데 못 막습니다.

모바일 클라이언트의 재시도는 대개 **거의 동시에** 옵니다 (네트워크 끊김 → 즉시 재시도). 실전에서 바로 터집니다.

### 고친 코드

```rust
pub enum IdemOutcome {
    /// 내가 소유권을 얻음 — 실제 작업을 진행
    Acquired,
    /// 이미 완료된 요청 — 저장된 응답을 그대로 반환
    Replay(String),
    /// 다른 요청이 처리 중 — 409로 거절 (클라이언트가 잠시 후 재시도)
    InFlight,
}

pub async fn begin(
    conn: &mut ConnectionManager,
    key: &str,
    body_hash: &str,
) -> redis::RedisResult<IdemOutcome> {
    let k = format!("idem:{key}");

    // SET k <sentinel> NX EX 60  — 처리 중 락을 원자적으로 획득
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&k).arg(format!("PROCESSING:{body_hash}"))
        .arg("NX").arg("EX").arg(60)
        .query_async(conn).await?;

    if acquired.is_some() {
        return Ok(IdemOutcome::Acquired);
    }

    // 이미 존재 — 완료된 응답인지 처리 중인지 확인
    let existing: Option<String> = conn.get(&k).await?;
    match existing {
        Some(v) if v.starts_with("PROCESSING:") => {
            // 같은 키인데 바디가 다르면 오용 — 422로 거절해야 함
            if v != format!("PROCESSING:{body_hash}") {
                return Ok(IdemOutcome::InFlight); // 호출부에서 422 처리
            }
            Ok(IdemOutcome::InFlight)
        }
        Some(v) => Ok(IdemOutcome::Replay(v)),
        None => Ok(IdemOutcome::Acquired), // TTL 만료 직후 — 재시도
    }
}

pub async fn complete(
    conn: &mut ConnectionManager,
    key: &str,
    response_json: &str,
) -> redis::RedisResult<()> {
    // 완료 응답으로 덮어쓰고 TTL 연장
    let _: () = conn.set_ex(format!("idem:{key}"), response_json, 86_400).await?;
    Ok(())
}

pub async fn abort(conn: &mut ConnectionManager, key: &str) -> redis::RedisResult<()> {
    // 실패 시 락을 즉시 풀어 재시도를 허용
    let _: () = conn.del(format!("idem:{key}")).await?;
    Ok(())
}
```

### 왜 `body_hash`가 필요한가

같은 `Idempotency-Key`로 **다른 내용**을 보내는 것은 클라이언트 버그이거나 공격입니다.
Stripe는 이 경우 422를 반환합니다. 바디 해시를 저장해두면 탐지할 수 있습니다.

### 그리고 Redis만으로는 부족합니다

Redis가 키를 잃으면(evict, 페일오버) 멱등성이 깨집니다.
[05 §6](./05_redis.md)이 정확히 말한 대로 **"락만으로 돈을 지키지 마세요"** — 최종 방어선은 DB입니다.

```sql
-- order_db
ALTER TABLE orders ADD COLUMN idempotency_key TEXT;
CREATE UNIQUE INDEX orders_idem_key ON orders (user_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
```

INSERT가 unique violation을 내면 → 기존 주문을 조회해서 반환.
Redis는 **빠른 경로**, DB unique는 **정확한 경로**입니다. 둘 다 필요합니다.

---

## 9. 🟠 UUIDv4 + 키셋 페이지네이션은 성립하지 않음

**위치:** [04_database §8](./04_database.md) vs [04_database §3](./04_database.md)

### 증상

§8은 이렇게 권합니다.

```sql
WHERE id < $1 ORDER BY id DESC LIMIT 20
```

그런데 §3의 스키마는:

```sql
id UUID PRIMARY KEY DEFAULT gen_random_uuid()
```

`gen_random_uuid()`는 **UUIDv4 = 완전 랜덤**입니다.

| 기대 | 실제 |
|---|---|
| 최신순 정렬 | 무작위 순서 (사용자에게 의미 없는 순서) |
| 커서가 시간 위치를 의미 | 아무 의미 없음 |
| 인덱스 지역성 | B-tree 전역에 흩뿌려져 삽입 → 페이지 분할, WAL 증가, 캐시 미스 |

세 번째가 특히 큽니다. 랜덤 UUID PK는 쓰기 처리량을 눈에 띄게 떨어뜨립니다.

### 고친 코드 (A) — UUIDv7 (권장)

[02 §2](./02_workspace.md)의 workspace deps에 `uuid`의 `v7` feature는 **이미 켜져 있습니다.** 쓰기만 하면 됩니다.

```rust
// 앱에서 생성 (DB DEFAULT를 쓰지 않음)
let id = uuid::Uuid::now_v7();   // 상위 48비트가 밀리초 타임스탬프
```

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,          -- DEFAULT 제거, 앱이 생성
    ...
);
```

UUIDv7은 **시간 정렬 가능**하므로:
- `ORDER BY id DESC` = 최신순 (의미 있음)
- 인덱스 삽입이 오른쪽 끝에 몰림 = B-tree 친화적
- 커서 페이지네이션이 그대로 동작
- 애플리케이션에서 ID를 미리 알 수 있음 (outbox 이벤트에 넣기 편함)

### 고친 코드 (B) — 복합 커서

기존 v4 데이터가 이미 있다면 스키마를 못 바꿉니다. 복합 커서를 쓰세요.

```sql
-- (created_at, id) 복합 인덱스 필수
CREATE INDEX orders_created_at_id ON orders (created_at DESC, id DESC);

-- 커서: 마지막 행의 (created_at, id)
SELECT * FROM orders
WHERE user_id = $1
  AND (created_at, id) < ($2, $3)   -- 행 비교(row comparison)
ORDER BY created_at DESC, id DESC
LIMIT 20;
```

`(created_at, id) < ($2, $3)`는 PostgreSQL의 행 비교 구문으로, **같은 타임스탬프의 동점을 id로 안정적으로 깹니다.**
`created_at < $2 OR (created_at = $2 AND id < $3)`와 동치이지만 인덱스를 더 잘 씁니다.

### 커서 인코딩

원시 값을 노출하면 클라이언트가 조작합니다. base64로 감싸고 서버에서 검증하세요.

```rust
#[derive(Serialize, Deserialize)]
struct Cursor { created_at: DateTime<Utc>, id: Uuid }

fn encode(c: &Cursor) -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(c).unwrap())
}
```

### OFFSET을 쓰지 않는 이유 (04 §8이 옳은 이유)

```sql
OFFSET 100000 LIMIT 20   -- PostgreSQL이 100,020행을 읽고 100,000행을 버림
```

깊은 페이지에서 선형으로 느려지고, 그 사이 행이 삽입되면 **같은 항목이 두 페이지에 나타납니다.**
04 §8의 판단은 맞고, 여기서는 그걸 **실제로 동작하게** 만드는 것뿐입니다.

---

## 10. 🟡 미들웨어 layer 순서가 뒤집혀 있음

**위치:** [03_service_anatomy §7](./03_service_anatomy.md)

### 증상

```rust
let app = router()
    .layer(TraceLayer::new_for_http())      // ← 가장 안쪽
    .layer(CompressionLayer::new())
    .layer(TimeoutLayer::with_status_code(...))
    .layer(CorsLayer::permissive())         // ← 가장 바깥
    .with_state(state);
```

axum에서 **나중에 붙인 layer가 바깥**입니다. (axum 0.8 `src/docs/middleware.md`의 onion 그림 기준)

```
요청 →  Cors  →  Timeout  →  Compression  →  Trace  →  handler
```

결과:

| 상황 | 지금 | 기대 |
|---|---|---|
| 요청이 15s 타임아웃 | TimeoutLayer가 408을 만들고 Trace는 못 봄 → **로그에 안 남음** | 408로 로그에 남아야 함 |
| CORS preflight 거절 | 로그 없음 | 남아야 함 |
| 응답 크기 메트릭 | 압축 전 크기만 보임 | 실제 전송 크기 |

**"타임아웃이 나는데 로그에 아무것도 없다"** 는 디버깅 지옥이 여기서 시작됩니다.
[08_observability](./08_observability.md)를 통째로 쓰고도 정작 장애 시각의 로그가 비는 겁니다.

### 고친 코드

`ServiceBuilder`를 쓰면 **위에서 아래로 = 바깥에서 안쪽**이라 읽기가 직관적입니다.

```rust
use tower::ServiceBuilder;
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    cors::CorsLayer,
    request_id::{PropagateRequestIdLayer, SetRequestIdLayer, MakeRequestUuid},
    sensitive_headers::SetSensitiveHeadersLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

let app = router()
    .layer(
        ServiceBuilder::new()
            // ── 바깥 ──────────────────────────────
            .layer(SetSensitiveHeadersLayer::new([AUTHORIZATION, COOKIE]))
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(TraceLayer::new_for_http())      // 모든 응답을 관측 (타임아웃 포함)
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(CatchPanicLayer::new())          // 패닉 → 500 (§12 참고)
            .layer(CorsLayer::permissive())         // prod에선 제한
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(15),
            ))
            .layer(CompressionLayer::new())
            // ── 안쪽 ──────────────────────────────
    )
    .with_state(state);
```

### 순서 규칙

```
1. SetSensitiveHeaders   ← Trace보다 반드시 바깥 (안 그러면 마스킹 전에 로깅됨)
2. SetRequestId          ← Trace가 request_id를 볼 수 있게 앞
3. TraceLayer            ← 최대한 바깥. 아래 모든 것을 관측
4. CatchPanic            ← Trace 안쪽 (패닉도 로그에 남아야 함)
5. Cors                  ← preflight를 빨리 끝내되 로그는 남게
6. Timeout / ConcurrencyLimit / LoadShed
7. Compression           ← handler에 가까이 (실제 바디만 압축)
8. 인증 미들웨어          ← 가장 안쪽 (여기까지 온 요청만 인증 비용 지출)
```

> `TimeoutLayer`의 `with_status_code`는 tower-http 0.6.11에 **실제로 존재합니다.**
> (기존 문서가 맞습니다. 순서만 문제입니다.)

---

## 11. 🟡 config 로더가 모든 환경변수를 흡수함

**위치:** [02_workspace §3](./02_workspace.md)

### 증상

```rust
config::Environment::default().separator("__")
```

`default()`는 **prefix가 없습니다.** 프로세스의 모든 환경변수가 설정 후보가 됩니다.

```
PATH, HOME, LANG, PWD, HOSTNAME,
KUBERNETES_SERVICE_HOST, KUBERNETES_PORT_443_TCP_PROTO,   ← K8s가 자동 주입
USER_SERVICE_PORT_3001_TCP_ADDR,                          ← Docker link 잔재
GITHUB_ACTIONS, RUNNER_TEMP, ...                          ← CI가 주입
```

문제가 두 가지입니다.

1. `__`가 든 시스템 변수가 있으면 **중첩 구조로 잘못 해석**됩니다.
   K8s의 `SERVICE_PORT_8080_TCP` 류가 실제로 이런 패턴을 만듭니다.
2. **로컬에서는 되는데 K8s/CI에서만 깨집니다.** 원인 파악에 몇 시간이 갑니다.

또 하나: [09 §2](./09_deploy.md)의 compose는 `HTTP__ADDR`(중첩)과 `JWT_SECRET`(평면)을 섞어 씁니다.
지금은 `jwt_secret`이 최상위 필드라 우연히 동작하지만, 나중에 `jwt.secret`으로 중첩시키는 순간 조용히 깨집니다.

### 고친 코드

```rust
impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let s = config::Config::builder()
            // 기본값을 코드에 둔다 — 환경변수 누락 시 명확한 동작
            .set_default("http.addr", "0.0.0.0:3001")?
            .set_default("database.max_connections", 10)?
            .set_default("log.level", "info")?
            // APP_ 접두사만 소비 → 시스템 변수와 격리
            .add_source(
                config::Environment::with_prefix("APP")
                    .separator("__")
                    .try_parsing(true),   // "10" → u32 로 파싱
            )
            .build()?;

        let settings: Settings = s.try_deserialize()
            .map_err(|e| anyhow::anyhow!("config error (APP_* 환경변수 확인): {e}"))?;

        settings.validate()?;   // ← 아래
        Ok(settings)
    }
}
```

환경변수는 이렇게 됩니다.

```bash
APP_HTTP__ADDR=0.0.0.0:3001
APP_DATABASE__URL=postgres://...
APP_DATABASE__MAX_CONNECTIONS=20
APP_REDIS__URL=redis://...
APP_JWT_SECRET=...
```

### 시작 시 검증 (fail fast)

MSA에서 **잘못된 설정으로 뜬 서비스**는 안 뜬 서비스보다 나쁩니다. 헬스체크는 통과하는데 요청마다 실패하니까요.

```rust
impl Settings {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.http.addr.parse::<std::net::SocketAddr>().is_ok(),
            "http.addr가 유효한 SocketAddr이 아님: {}", self.http.addr
        );
        anyhow::ensure!(
            self.database.url.starts_with("postgres://")
                || self.database.url.starts_with("postgresql://"),
            "database.url이 postgres 스킴이 아님"
        );
        anyhow::ensure!(
            (1..=200).contains(&self.database.max_connections),
            "database.max_connections 범위 초과: {}", self.database.max_connections
        );

        // prod에서 개발용 시크릿이 남아있는지 — 가장 흔한 사고
        if self.app_env == "prod" {
            anyhow::ensure!(
                self.jwt_secret.expose_secret().len() >= 32,
                "prod의 jwt_secret이 32자 미만"
            );
            anyhow::ensure!(
                !self.jwt_secret.expose_secret().contains("change-me"),
                "prod에 개발용 기본 시크릿이 남아있음"
            );
        }
        Ok(())
    }
}
```

> [09 §4](./09_deploy.md)의 `.env.example`에 `JWT_SECRET=change-me`가, §2 compose에 `dev-secret-change-me`가 있습니다.
> 이 검증이 없으면 그 값이 프로덕션까지 갑니다. 실제로 자주 일어납니다.

---

## 12. 🟡 타입 불일치 · 의존성 누락 · 버전 노후

### 12-1. `redis::Client` vs `ConnectionManager` 타입 불일치

[03 §1](./03_service_anatomy.md):

```rust
let redis = redis::Client::open(settings.redis.url.as_str())?;
let user_svc = UserService::new(users, redis.clone());   // Client를 넘김
```

[03 §5](./03_service_anatomy.md):

```rust
pub fn new(repo: UserRepo, redis: ConnectionManager) -> Self   // ConnectionManager를 기대
```

컴파일이 안 됩니다. [05 §3](./05_redis.md)이 맞는 형태입니다.

```rust
let client = redis::Client::open(settings.redis.url.as_str())?;
let redis = redis::aio::ConnectionManager::new(client).await?;   // ← 이 한 줄이 빠짐
let user_svc = UserService::new(users, redis.clone());
```

### 12-2. `ConnectionManager`의 성질을 알고 쓰기

`ConnectionManager`는 **멀티플렉스된 단일 커넥션**입니다. 풀이 아닙니다.

| 특성 | 의미 |
|---|---|
| 자동 재연결 | ✅ 05 §3의 설명이 맞음 |
| `Clone`이 쌈 | ✅ AppState에 넣어도 됨 |
| 파이프라이닝 | ✅ 동시 요청이 한 소켓에 다중화됨 |
| **블로킹 명령 불가** | ❌ `BLPOP`, `BRPOP`, `XREAD BLOCK`을 쓰면 **소켓 전체가 멈춤** |
| **트랜잭션(MULTI) 불가** | ❌ 다중화되므로 명령이 섞임 |
| **`SUBSCRIBE` 불가** | ❌ 전용 커넥션 필요 |

[05 §1](./05_redis.md)이 "단기 큐 — Redis Streams"를 제안하는데, **Streams 소비는 블로킹 읽기**를 씁니다.
그건 `ConnectionManager`가 아니라 **전용 커넥션**으로 해야 합니다.

```rust
// 캐시/락/rate limit → ConnectionManager (공유)
// Streams 소비, Pub/Sub, MULTI → client.get_multiplexed_async_connection() 대신 전용 커넥션
let dedicated = client.get_async_connection().await?;   // 소비 루프 전용
```

고부하에서는 `deadpool-redis`로 실제 풀을 쓰는 것을 검토하세요 ([05 §3](./05_redis.md)의 표에 이미 있음).

### 12-3. sqlx features에 TLS가 없음

[02 §2](./02_workspace.md):

```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "migrate"] }
```

[09 §7](./09_deploy.md) 체크리스트는 **"DB TLS"** 를 요구하는데 의존성이 그걸 못 합니다.
관리형 PG(RDS, Cloud SQL, Neon)는 대부분 TLS가 필수라 연결 자체가 실패합니다.

```toml
sqlx = { version = "0.8", features = [
    "runtime-tokio", "tls-rustls",     # ← 추가
    "postgres", "uuid", "chrono", "migrate",
    "macros",                          # query! 매크로용 (04 §5에서 쓰는데 누락)
] }
```

```bash
DATABASE_URL=postgres://u:p@host:5432/db?sslmode=require
```

### 12-4. tower-http features 누락

[02 §2](./02_workspace.md)에는 `cors, trace, compression-gzip, timeout`만 있습니다.
이 보강판에서 쓰는 것들을 추가하세요.

```toml
tower-http = { version = "0.6", features = [
    "cors", "trace", "compression-gzip", "timeout",
    "request-id",      # SetRequestIdLayer / PropagateRequestIdLayer
    "catch-panic",     # CatchPanicLayer
    "sensitive-headers",
    "limit",           # RequestBodyLimitLayer
    "set-header",      # 보안 헤더
    "util",            # ServiceExt
] }

tower = { version = "0.5", features = ["limit", "load-shed", "timeout", "util", "retry"] }
```

### 12-5. 버전 노후

| 문서의 버전 | 비고 |
|---|---|
| `redis = "0.27"` | 여러 세대 뒤. API가 꽤 바뀌었으므로 올릴 때 마이그레이션 노트 확인 |
| `config = "0.14"` | 한 세대 뒤 |
| `opentelemetry = "0.27"` / `tracing-opentelemetry = "0.28"` | OTel 계열은 3개 크레이트 버전이 **정확히 맞물려야** 함 → [15 §7](./15_observability_deep.md) 조합표 참고 |
| `validator = "0.19"` | 확인 필요 |
| `jaegertracing/all-in-one:1.57` | Jaeger v2 계열 확인 |

**버전을 올릴 때 한 번에 다 올리지 마세요.** OTel 계열은 특히 한 크레이트만 어긋나도 trait 충돌로 컴파일이 안 됩니다.

### 12-6. `Cargo.lock`이 언급되지 않음

[09 §3](./09_deploy.md)의 Dockerfile은 `Cargo.lock`을 COPY하지 않고 `--locked`도 안 씁니다.
**바이너리 워크스페이스는 `Cargo.lock`을 반드시 커밋하고 빌드에서 강제해야** 재현 가능한 이미지가 나옵니다.
자세히는 [18_cicd §4](./18_cicd.md).

### 12-7. axum `Json` 거절 응답이 에러 포맷과 다름

axum의 기본 `Json` extractor는 파싱 실패 시 **`text/plain` 본문**을 반환합니다 (axum-core 0.5.6 확인).

```
400 Bad Request
content-type: text/plain; charset=utf-8

Failed to parse the request body as JSON: expected value at line 1 column 1
```

[02 §3](./02_workspace.md)에서 공들여 만든 `ErrorBody { error, message }` JSON 규약이
**클라이언트가 실수한 순간에만 깨집니다.** 프론트엔드의 에러 처리 코드가 여기서 터집니다.

```rust
// 커스텀 extractor로 거절을 ApiError로 변환
use axum::extract::{FromRequest, Request, rejection::JsonRejection};

pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(AppJson(v)),
            Err(rej) => Err(ApiError::BadRequest(rej.body_text())),
        }
    }
}
```

`Path`, `Query`, `Form`도 같은 문제가 있습니다. 같은 방식으로 감싸거나 `axum-extra`의 `WithRejection`을 쓰세요.
Nest의 전역 `ValidationPipe`가 해주던 일을 여기서는 직접 해야 합니다.

### 12-8. `unwrap()`이 요청 경로에 있음

[06 §4](./06_gateway_auth.md)에만 `unwrap()`이 4개 있습니다.

```rust
HeaderValue::from_str(&claims.email).unwrap()          // 이메일에 개행 → 패닉
StatusCode::from_u16(upstream.status().as_u16()).unwrap()
Response::builder()...body(...).unwrap()
```

**요청 경로의 패닉은 그 요청의 커넥션을 끊습니다.** 사용자 입력으로 도달 가능한 `unwrap()`은 DoS입니다.
전부 `?`로 바꾸고, 마지막 안전망으로 `CatchPanicLayer`를 켜세요 ([11_resilience §3](./11_resilience.md)).

CI에서 강제하는 방법:

```toml
# clippy.toml 또는 lib.rs
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
```

---

## 정정 반영 체크포인트

```
[ ] gateway에서 x-internal-* 인바운드 헤더를 strip한다
[ ] 프록시 응답에 content-type 등 헤더를 복사한다
[ ] 5xx 응답에 내부 에러 문자열이 없고 trace_id가 있다
[ ] Settings/DbConfig의 Debug가 시크릿을 마스킹한다
[ ] 캐시에 password_hash가 들어가지 않는다 (전용 DTO)
[ ] argon2가 spawn_blocking + 세마포어 뒤에 있다
[ ] rate limit이 Lua로 원자적이고 retry-after를 준다
[ ] Idempotency가 SET NX + DB unique 이중 방어다
[ ] ID가 UUIDv7이거나 커서가 (created_at, id) 복합이다
[ ] TraceLayer가 Timeout/CORS보다 바깥이다
[ ] config가 APP_ prefix를 쓰고 시작 시 검증한다
[ ] 요청 경로에 unwrap()이 없다 (clippy로 강제)
```

---

다음: [11_resilience — 죽지 않게, 그리고 깔끔하게 죽게](./11_resilience.md)
