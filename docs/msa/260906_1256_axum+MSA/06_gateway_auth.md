# 06. API Gateway · JWT 인증

NestJS의 글로벌 Guard + Spring Cloud Gateway 역할을  
**`gateway` 서비스**가 담당합니다. 비즈니스 로직은 두지 않습니다.

---

## 1. Gateway 책임

| 한다 | 안 한다 |
|---|---|
| TLS 종료(또는 ingress에 위임) | 주문 생성 비즈니스 |
| CORS / 압축 | DB 직접 접근 |
| JWT 검증 | 사용자 비밀번호 해시 저장 |
| Rate limit | 장기 상태 보관 |
| 라우팅 프록시 | 서비스별 세밀 권한의 SSOT (클레임만 전달) |
| Request ID 부여 | |

인증 **발급**은 `user-service` (login),  
**검증**은 gateway(+ 필요 시 내부 서비스)가 합니다.

---

## 2. 라우팅 개념도

```
Client
  │  Authorization: Bearer <access>
  ▼
gateway :8080
  ├─ POST /v1/auth/login     → proxy → user-service
  ├─ POST /v1/auth/register  → proxy → user-service
  ├─ /v1/users/*             → user-service   (JWT 필요)
  ├─ /v1/orders/*            → order-service  (JWT 필요)
  └─ /v1/products/*          → catalog-service (GET는 public 가능)
```

---

## 3. JWT 공통 크레이트

```toml
# crates/auth-jwt/Cargo.toml
[dependencies]
jsonwebtoken = { workspace = true }
serde = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
thiserror = { workspace = true }
```

```rust
// crates/auth-jwt/src/lib.rs
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,          // user id
    pub email: String,
    pub roles: Vec<String>,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
}

pub struct JwtKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
    issuer: String,
    ttl_secs: i64,
}

impl JwtKeys {
    pub fn from_secret(secret: &str, issuer: &str, ttl_secs: i64) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            issuer: issuer.into(),
            ttl_secs,
        }
    }

    pub fn issue(&self, user_id: Uuid, email: &str, roles: Vec<String>) -> Result<String, JwtError> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            email: email.into(),
            roles,
            iat: now.timestamp(),
            exp: (now + Duration::seconds(self.ttl_secs)).timestamp(),
            iss: self.issuer.clone(),
        };
        encode(&Header::default(), &claims, &self.encoding).map_err(Into::into)
    }

    pub fn verify(&self, token: &str) -> Result<Claims, JwtError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        let data = decode::<Claims>(token, &self.decoding, &validation)?;
        Ok(data.claims)
    }
}
```

프로덕션에서는 HS256 공유 시크릿 대신 **RS256/EdDSA** (gateway는 public key만) 를 권장합니다.

---

## 4. Gateway 프록시 (reqwest)

단순 버전 — 경로 프리픽스 기반:

```rust
use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
    http::{HeaderMap, HeaderValue, StatusCode, Uri},
};
use common::ApiError;

#[derive(Clone)]
pub struct GatewayState {
    pub http: reqwest::Client,
    pub jwt: auth_jwt::JwtKeys,
    pub user_base: String,
    pub order_base: String,
    pub catalog_base: String,
    pub redis: redis::aio::ConnectionManager,
}

pub async fn proxy_orders(
    State(state): State<GatewayState>,
    req: Request,
) -> Result<Response, ApiError> {
    forward(&state, &state.order_base, req, true).await
}

async fn forward(
    state: &GatewayState,
    upstream_base: &str,
    mut req: Request,
    auth_required: bool,
) -> Result<Response, ApiError> {
    // 1) rate limit (생략 가능 — 05 참고)
    // 2) JWT
    let mut headers = HeaderMap::new();
    if auth_required {
        let token = bearer_token(req.headers()).ok_or(ApiError::Unauthorized)?;
        let claims = state.jwt.verify(token).map_err(|_| ApiError::Unauthorized)?;
        headers.insert(
            "x-user-id",
            HeaderValue::from_str(&claims.sub.to_string()).unwrap(),
        );
        headers.insert(
            "x-user-email",
            HeaderValue::from_str(&claims.email).unwrap(),
        );
        // roles도 필요 시
    }

    // 3) request id 전파
    let rid = request_id_from(&req);
    headers.insert("x-request-id", HeaderValue::from_str(&rid).unwrap());

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str())
        .unwrap_or("/");
    let url = format!("{upstream_base}{path_and_query}");

    let method = req.method().clone();
    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let mut ub = state.http.request(method, &url);
    for (k, v) in headers.iter() {
        ub = ub.header(k, v);
    }
    // content-type 등 원본 헤더 선별 복사

    let upstream = ub
        .body(body)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap();
    let bytes = upstream.bytes().await.map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Response::builder().status(status).body(Body::from(bytes)).unwrap())
}
```

실무에서는:
- 스트리밍 바디 (대용량 업로드)
- 헤더 화이트리스트
- 타임아웃·재시도 정책 (GET만 재시도)
- gRPC 백엔드면 Envoy/tonic 검토

초기에 이 정도면 Nest Gateway 미들웨어와 동급입니다.

---

## 5. 내부 서비스는 JWT를 다시 검증할까?

두 가지 모델:

### A. Gateway만 신뢰 (간단)

- 내부망에서만 서비스 포트 오픈
- `X-User-Id` 헤더 신뢰
- **외부에서 서비스 포트가 노출되면 끝** → mTLS / mesh 필요

### B. 서비스도 JWT 재검증 (안전)

- 같은 키/공개키로 검증
- 헤더 스푸핑 무시

로컬·소규모: A + Docker 내부 네트워크  
규제/멀티테넌트: B 또는 짧은 내부 토큰 재발급

---

## 6. 로그인 플로우

```
POST /v1/auth/login { email, password }
  → user-service: 비밀번호 검증
  → access_token (JWT) + refresh_token 발급
  → refresh는 Redis/DB에 해시 저장
```

```rust
// user-service handler 개념
pub async fn login(...) -> ApiResult<Json<TokenResponse>> {
    let user = state.user_svc.verify_credentials(&email, &password).await?;
    let access = state.jwt.issue(user.id, &user.email, user.roles.clone())?;
    let refresh = state.user_svc.issue_refresh(user.id).await?;
    Ok(Json(TokenResponse { access_token: access, refresh_token: refresh, token_type: "Bearer" }))
}
```

비밀번호: `argon2` 크레이트 권장 (bcrypt도 가능).

---

## 7. 권한 (RBAC) 스케치

```rust
pub struct AuthUser {
    pub user_id: Uuid,
    pub roles: Vec<String>,
}

impl AuthUser {
    pub fn require_role(&self, role: &str) -> Result<(), ApiError> {
        if self.roles.iter().any(|r| r == role) {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
}
```

세밀 권한(리소스 소유)은 **서비스 내부**에서 `order.user_id == claims.sub` 로 검사합니다.  
Gateway는 인증(Authentication), 서비스는 인가(Authorization)에 가깝게 나눕니다.

---

## 8. CORS

```rust
use tower_http::cors::{Any, CorsLayer};
use axum::http::{HeaderName, Method};

let cors = CorsLayer::new()
    .allow_origin(/* 구체 origin */)
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([
        HeaderName::from_static("authorization"),
        HeaderName::from_static("content-type"),
        HeaderName::from_static("idempotency-key"),
        HeaderName::from_static("x-request-id"),
    ]);
```

`CorsLayer::permissive()` 는 로컬만.

---

## 체크포인트

```
[ ] 로그인 발급(user) / 검증(gateway) 책임이 나뉘어 있다
[ ] Access 짧게 + Refresh 회전 정책을 정했다
[ ] 내부 헤더 신뢰 모델을 선택했다 (A or B)
[ ] Rate limit + request-id가 gateway에 있다
[ ] 서비스 포트가 퍼블릭에 직접 노출되지 않는다
```

다음: [07_messaging — 서비스 간 통신](./07_messaging.md)
