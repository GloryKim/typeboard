# 12. 보안 — 신뢰 경계 · 키 회전 · 토큰 폐기 · 감사

[06_gateway_auth](./06_gateway_auth.md)는 **인증이 동작하게** 만듭니다.
이 문서는 그 인증이 **깨지지 않게** 만듭니다.

MSA는 모놀리스보다 공격면이 큽니다. 서비스 간 네트워크가 통째로 새로운 신뢰 경계이기 때문입니다.

---

## 1. 신뢰 경계를 명시적으로

### 1-1. 06 §5의 선택지를 다시 봅니다

기존 문서는 두 모델을 대등하게 제시합니다.

> **A. Gateway만 신뢰** — 내부망에서만 포트 오픈, `X-User-Id` 헤더 신뢰
> **B. 서비스도 JWT 재검증** — 헤더 스푸핑 무시

**B를 기본으로 하세요.** A는 "로컬 개발" 한정입니다.

이유는 A의 전제가 현실에서 거의 성립하지 않기 때문입니다.

| A의 전제 | 실제 |
|---|---|
| "내부망은 안전하다" | 어떤 서비스든 SSRF 하나면 내부에서 임의 요청 가능 |
| "서비스 포트는 외부에 안 열린다" | 디버깅하다 `LoadBalancer`로 바꾸고 되돌리는 걸 잊음 |
| "gateway를 우회할 수 없다" | 사이드카, 잡(Job), CronJob, 개발자 노트북의 포트포워딩 |
| "내부 트래픽은 신뢰할 수 있다" | 침해된 파드 하나 = 전 사용자 사칭 |

A는 **경계가 뚫리면 방어가 0**입니다. B는 **경계가 뚫려도 서명 검증이 남습니다.**

### 1-2. B의 비용은 생각보다 작습니다

"모든 서비스가 JWT를 검증하면 느려지지 않나?"

```
HS256 검증: ~2µs
EdDSA 검증: ~30µs
RS256 검증: ~50µs

요청당 5ms 예산에서 50µs = 1%
```

무시할 수 있습니다. **비대칭 키를 쓰면 시크릿 배포 문제도 사라집니다.**

```
user-service   : 개인키 보유 (발급)
gateway        : 공개키만  (검증)
order-service  : 공개키만  (검증)
catalog-service: 공개키만  (검증)
```

공개키는 유출돼도 아무 일도 안 일어납니다. 시크릿 매니저에 넣을 것이 **하나로 줄어듭니다.**

### 1-3. 계층별 책임 (06 §7의 확장)

```
┌─ Gateway ───────────────────────────────────────┐
│ 인증(누구인가) · rate limit · 요청 위생          │
│ → x-internal-* 헤더 strip 후 재주입 (10_errata §1)│
└─────────────────┬───────────────────────────────┘
                  │ Authorization: Bearer <원본 JWT도 함께 전달>
┌─────────────────▼───────────────────────────────┐
│ Service                                          │
│ 1) JWT 서명 재검증 (헤더가 아니라 토큰을 믿음)     │
│ 2) 역할 인가 (require_role)                      │
│ 3) 리소스 소유권 (order.user_id == claims.sub)   │
└──────────────────────────────────────────────────┘
```

**gateway는 원본 JWT를 그대로 넘깁니다.** `x-internal-user-id`는 "로깅·디버깅용 편의값"이지
**인가 판단의 근거가 되면 안 됩니다.**

```rust
// ❌ 헤더를 신뢰
let user_id: Uuid = headers.get("x-internal-user-id")?.to_str()?.parse()?;

// ✅ 토큰을 검증
let claims = state.jwt.verify(bearer_token(&headers).ok_or(ApiError::Unauthorized)?)?;
let user_id = claims.sub;
```

### 1-4. 서비스 간 호출의 신원 (사용자 ≠ 서비스)

order-service가 user-service를 부를 때, 두 가지 신원이 필요합니다.

| 신원 | 질문 | 표현 |
|---|---|---|
| 사용자 | "누구를 위해 부르는가" | 원본 사용자 JWT 전달 |
| 서비스 | "누가 부르는가" | 서비스 계정 토큰 / mTLS |

배치 잡처럼 **사용자가 없는 호출**도 있으므로 둘은 별개입니다.

```rust
// 서비스 전용 클레임
#[derive(Serialize, Deserialize)]
pub struct ServiceClaims {
    pub sub: String,        // "order-service"
    pub aud: Vec<String>,   // ["user-service"]  ← 대상 제한
    pub exp: i64,
    pub scope: Vec<String>, // ["users:read"]
}
```

`aud`(audience)가 핵심입니다. user-service용 토큰을 훔쳐도 catalog-service에서는 못 씁니다.

실무 순서:

```
1단계: 사용자 JWT 전달 + 서비스도 검증   ← 여기부터 시작
2단계: + 서비스 토큰 (aud/scope)         ← 서비스 5개 넘어가면
3단계: + mTLS (서비스 메시)              ← 규제/멀티테넌트
```

---

## 2. JWT를 제대로 설정하기

### 2-1. 06 §3의 `Validation::default()`로는 부족합니다

```rust
let mut validation = Validation::default();
validation.set_issuer(&[&self.issuer]);
```

기본값은 `exp`와 알고리즘만 봅니다. 추가해야 할 것들:

```rust
pub fn verify(&self, token: &str, expected_audience: &str) -> Result<Claims, JwtError> {
    let mut v = Validation::new(Algorithm::EdDSA);   // ← 알고리즘 명시 (§2-2)

    v.set_issuer(&[&self.issuer]);
    v.set_audience(&[expected_audience]);            // aud 검증 — 토큰 오용 방지
    v.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);  // 누락 = 거절
    v.leeway = 5;                                    // 클럭 스큐 5초만 (기본 60은 너무 큼)
    v.validate_exp = true;
    v.validate_nbf = true;

    let data = decode::<Claims>(token, &self.decoding, &v)?;
    Ok(data.claims)
}
```

`set_required_spec_claims`가 중요합니다. 이게 없으면 **`exp`가 아예 없는 토큰이 통과**합니다.

### 2-2. 알고리즘 혼동 공격 (algorithm confusion)

`jsonwebtoken` 크레이트는 이 부분이 비교적 안전하지만, 원리는 알아야 합니다.

```
공격자: 헤더의 alg를 RS256 → HS256으로 바꿔서 제출
서버 : "alg를 보고 알아서 검증"하면
      → 공개키를 HMAC 시크릿으로 취급
      → 공개키는 누구나 아는 값 → 임의 토큰 위조 가능
```

**방어: 검증 시 알고리즘을 코드에 하드코딩합니다.** 토큰의 `alg` 헤더를 믿지 마세요.

```rust
Validation::new(Algorithm::EdDSA)   // ✅ 우리가 정한다
// Validation::default()는 HS256 고정이라 우연히 안전하지만, 명시하는 습관이 낫습니다
```

그리고 `alg: none`은 절대 허용 금지 (`jsonwebtoken`은 기본적으로 거부합니다).

### 2-3. HS256 → EdDSA 전환

06 §3은 HS256 공유 시크릿을 쓰고 "프로덕션에서는 RS256/EdDSA 권장"이라고만 합니다. 실제 코드:

```bash
# Ed25519 키쌍 생성
openssl genpkey -algorithm ed25519 -out jwt_private.pem
openssl pkey -in jwt_private.pem -pubout -out jwt_public.pem
```

```rust
// crates/auth-jwt/src/lib.rs
pub struct JwtIssuer {           // user-service만 보유
    encoding: EncodingKey,
    kid: String,
    issuer: String,
    ttl: Duration,
}

pub struct JwtVerifier {         // 모든 서비스가 보유
    keys: HashMap<String, DecodingKey>,   // kid → 공개키 (복수! §3 참고)
    issuer: String,
    audience: String,
}

impl JwtIssuer {
    pub fn from_ed25519_pem(pem: &[u8], kid: &str, issuer: &str, ttl: Duration) -> Result<Self, JwtError> {
        Ok(Self {
            encoding: EncodingKey::from_ed_pem(pem)?,
            kid: kid.to_string(),
            issuer: issuer.to_string(),
            ttl,
        })
    }

    pub fn issue(&self, user_id: Uuid, email: &str, roles: Vec<String>, aud: &str)
        -> Result<String, JwtError>
    {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            email: email.to_string(),
            roles,
            iat: now.timestamp(),
            exp: (now + self.ttl).timestamp(),
            nbf: now.timestamp(),
            iss: self.issuer.clone(),
            aud: aud.to_string(),
            jti: Uuid::now_v7(),        // ← 폐기용 식별자 (§4)
        };

        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.kid.clone());   // ← 키 회전용 (§3)
        encode(&header, &claims, &self.encoding).map_err(Into::into)
    }
}
```

### 2-4. 클레임에 뭘 넣을까

```rust
pub struct Claims {
    pub sub: Uuid,          // 사용자 ID
    pub jti: Uuid,          // 토큰 고유 ID — 폐기에 필요
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub nbf: i64,
    pub roles: Vec<String>, // 소수의 굵은 역할만
}
```

**넣지 말아야 할 것:**

| 넣지 말 것 | 이유 |
|---|---|
| 이메일, 이름, 전화번호 | JWT는 **서명만 되고 암호화되지 않습니다.** base64 디코드하면 누구나 읽습니다 |
| 세밀한 권한 목록 | 토큰이 비대해지고, 권한 변경이 만료까지 반영 안 됨 |
| 자주 바뀌는 상태 | 같은 이유 |

> **06 §3의 `Claims`에 `email`이 있습니다.** 브라우저 localStorage에 든 JWT를
> 디코드하면 이메일이 그대로 나옵니다. XSS 한 번에 전체 사용자 이메일이 수집됩니다.
> `sub`만 넣고 이메일은 API로 조회하세요.

### 2-5. 토큰 저장 위치

06은 언급하지 않지만 실무에서 가장 자주 잘못하는 부분입니다.

| 저장소 | XSS | CSRF | 평가 |
|---|---|---|---|
| localStorage | ❌ 완전 노출 | ✅ 안전 | 흔하지만 위험 |
| 메모리 (JS 변수) | 🔸 실행 중만 | ✅ 안전 | access token에 적합 |
| httpOnly Cookie | ✅ JS 접근 불가 | ❌ 대책 필요 | refresh token에 적합 |

**권장 조합:**

```
Access Token  : 메모리에만 (새로고침하면 사라짐 → refresh로 재발급)
Refresh Token : httpOnly + Secure + SameSite=Strict 쿠키
                + CSRF 토큰 (SameSite를 못 쓰는 크로스 도메인이면 필수)
```

```rust
// refresh 쿠키 설정
let cookie = format!(
    "refresh_token={token}; HttpOnly; Secure; SameSite=Strict; \
     Path=/v1/auth/refresh; Max-Age={max_age}"
);
```

`Path=/v1/auth/refresh`가 포인트입니다. **refresh 토큰이 다른 모든 요청에 실려 나가지 않습니다.**

---

## 3. 키 회전 — JWKS

### 3-1. 지금 구조의 문제

06 §3은 단일 시크릿입니다. 키를 바꾸면 **기존 토큰이 전부 무효**가 됩니다.

```
시크릿 유출 발견 → 즉시 교체해야 함
→ 교체하는 순간 전 사용자 강제 로그아웃
→ 로그인 폭주로 argon2가 CPU를 태움 (10_errata §6)
→ 서비스 다운
```

보안 사고 대응이 가용성 사고를 만듭니다. **키는 무중단으로 회전 가능해야 합니다.**

### 3-2. `kid`로 복수 키 지원

검증 측이 **여러 키를 동시에** 들고 있으면 됩니다.

```rust
pub struct JwtVerifier {
    keys: Arc<RwLock<HashMap<String, DecodingKey>>>,
    issuer: String,
    audience: String,
}

impl JwtVerifier {
    pub async fn verify(&self, token: &str) -> Result<Claims, JwtError> {
        // 1) 서명 검증 전에 헤더에서 kid만 읽는다
        let header = jsonwebtoken::decode_header(token)?;
        let kid = header.kid.ok_or(JwtError::MissingKid)?;

        // 2) 해당 kid의 공개키를 찾는다
        let keys = self.keys.read().await;
        let key = keys.get(&kid).ok_or(JwtError::UnknownKid(kid.clone()))?;

        // 3) 알고리즘은 우리가 정한다 (§2-2)
        let mut v = Validation::new(Algorithm::EdDSA);
        v.set_issuer(&[&self.issuer]);
        v.set_audience(&[&self.audience]);
        v.set_required_spec_claims(&["exp", "iss", "aud", "sub", "jti"]);
        v.leeway = 5;

        Ok(decode::<Claims>(token, key, &v)?.claims)
    }
}
```

### 3-3. 회전 절차 (무중단)

```
현재: kid=2026-01 로 발급·검증 중

1. 새 키쌍 생성 (kid=2026-07)
2. 모든 검증 서비스에 새 공개키 배포 → 이제 둘 다 검증 가능
   (아직 발급은 옛 키로. 이 상태를 하루 이상 유지)
3. user-service의 발급 키를 kid=2026-07로 전환
   → 새 토큰은 새 키, 기존 토큰은 옛 키로 계속 검증됨
4. access token TTL(15분) + refresh 최대 수명이 지날 때까지 대기
5. 옛 공개키 제거
```

**핵심: 3번과 5번 사이에 옛 토큰이 자연 만료됩니다. 아무도 로그아웃되지 않습니다.**

### 3-4. JWKS 엔드포인트로 자동화

수동 배포 대신 user-service가 공개키를 게시합니다.

```rust
// user-service: GET /.well-known/jwks.json
#[derive(Serialize)]
struct Jwks { keys: Vec<Jwk> }

#[derive(Serialize)]
struct Jwk {
    kty: String,   // "OKP" (Ed25519)
    crv: String,   // "Ed25519"
    kid: String,
    x:   String,   // base64url 공개키
    #[serde(rename = "use")]
    use_: String,  // "sig"
    alg: String,   // "EdDSA"
}

async fn jwks(State(s): State<AppState>) -> Json<Jwks> {
    Json(Jwks { keys: s.jwt.public_jwks() })   // 현재 + 이전 키 모두
}
```

검증 측은 주기적으로 가져옵니다.

```rust
async fn jwks_refresher(verifier: JwtVerifier, url: String, token: CancellationToken) {
    let mut ticker = tokio::time::interval(Duration::from_secs(300));
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => break,
            _ = ticker.tick() => {
                match fetch_jwks(&url).await {
                    Ok(keys) => {
                        verifier.replace_keys(keys).await;
                        tracing::debug!("jwks refreshed");
                    }
                    // ⚠️ 실패해도 기존 키를 지우지 않는다.
                    //    user-service 장애가 전 서비스 인증 마비로 번지면 안 됨
                    Err(e) => tracing::warn!(error = %e, "jwks refresh failed, keeping cached keys"),
                }
            }
        }
    }
}
```

**"실패해도 기존 키 유지"** 가 결정적입니다. 이게 없으면 JWKS 제공자가 **단일 장애점**이 됩니다.

부팅 시에도 JWKS를 못 가져오면? **부팅을 실패시키세요.** 검증 없이 뜨는 것보다 안 뜨는 게 낫습니다.

### 3-5. 알 수 없는 `kid` 처리

```rust
// 캐시에 없는 kid → 즉시 거절하지 말고 한 번 갱신 시도 (회전 직후 레이스 대응)
// 단, 갱신은 rate limit을 걸어야 함 — 랜덤 kid 폭탄으로 JWKS를 DDoS할 수 있음
```

---

## 4. 토큰 폐기 — 강제 로그아웃

### 4-1. JWT의 근본 문제

05 §8은 트레이드오프를 정확히 짚습니다.

> 강제 로그아웃 | 블랙리스트 필요 | 키 삭제면 끝

그런데 그 블랙리스트를 **어떻게 만드는지는 없습니다.** 필요한 순간:

- 사용자가 "모든 기기에서 로그아웃"
- 계정 침해 탐지
- 관리자가 계정 정지
- 권한 강등 (관리자 → 일반)

### 4-2. 두 층으로 구현

```rust
pub struct Revocation {
    redis: ConnectionManager,
}

impl Revocation {
    /// (A) 개별 토큰 폐기 — jti 블랙리스트
    /// TTL을 토큰의 잔여 수명으로 두면 자동 청소된다
    pub async fn revoke_token(&self, jti: Uuid, exp: i64) -> RedisResult<()> {
        let ttl = (exp - Utc::now().timestamp()).max(0) as u64;
        if ttl > 0 {
            let mut c = self.redis.clone();
            let _: () = c.set_ex(format!("revoked:jti:{jti}"), "1", ttl).await?;
        }
        Ok(())
    }

    /// (B) 사용자 전체 폐기 — 특정 시각 이전 발급 토큰을 전부 무효화
    /// 토큰 하나하나를 몰라도 된다
    pub async fn revoke_all_for_user(&self, user_id: Uuid) -> RedisResult<()> {
        let mut c = self.redis.clone();
        let now = Utc::now().timestamp();
        // 최대 access token 수명(15분)만 유지하면 충분
        let _: () = c.set_ex(format!("revoked:user:{user_id}"), now, 900).await?;
        Ok(())
    }

    pub async fn is_revoked(&self, claims: &Claims) -> bool {
        let mut c = self.redis.clone();

        // (A) 이 토큰이 개별 폐기됐나
        if matches!(c.exists(format!("revoked:jti:{}", claims.jti)).await, Ok(true)) {
            return true;
        }
        // (B) 이 사용자의 토큰이 일괄 폐기됐고, 내가 그 이전에 발급됐나
        if let Ok(Some(cutoff)) = c.get::<_, Option<i64>>(format!("revoked:user:{}", claims.sub)).await {
            if claims.iat < cutoff { return true; }
        }
        false
    }
}
```

(B)가 실용적입니다. **`revoked:user:{id}` 키 하나로 그 사용자의 모든 토큰이 무효화**됩니다.

### 4-3. Redis가 죽으면?

여기가 정책 결정 지점입니다.

```rust
match revocation.is_revoked(&claims).await {
    Ok(true)  => return Err(ApiError::Unauthorized),
    Ok(false) => { /* 통과 */ }
    Err(e) => {
        tracing::error!(error = %e, "revocation check failed");
        // fail-open : 가용성 우선 — 폐기된 토큰이 최대 15분 살아남음
        // fail-closed: 보안 우선 — Redis 장애 = 전면 인증 실패
    }
}
```

**추천: 일반 API는 fail-open, 민감 작업(결제·관리자)은 fail-closed.**
access token TTL을 5~15분으로 짧게 유지하면 fail-open의 노출 창이 그만큼 좁습니다.

이걸 문서에 명시하세요. [01 §7](./01_architecture.md)의 실패 모드 표에 한 줄로 추가하면 됩니다.

### 4-4. 체크 비용

**모든 요청에 Redis 조회 2회는 부담입니다.** 최적화:

```
- 폐기는 드문 사건 → Redis에 "폐기 세대 카운터"를 두고 로컬 캐시
- 또는 민감 엔드포인트에서만 체크
- 또는 Bloom filter로 1차 필터링
```

가장 간단한 실전 해법: **access token TTL을 5분으로 줄이고 폐기 체크를 생략**.
5분이면 대부분의 위협 모델에서 수용 가능합니다. 팀 상황에 맞게 고르세요.

---

## 5. Refresh 토큰 — 회전과 재사용 탐지

### 5-1. 05 §8의 "회전(rotation)"을 실제로

```
1. 클라이언트가 refresh_token_A로 갱신 요청
2. 서버: A를 무효화, 새 refresh_token_B 발급
3. 다음 갱신은 B로
```

여기까지가 기존 문서입니다. **핵심이 빠졌습니다: 그런데 누가 A를 또 쓰면?**

### 5-2. 재사용 탐지 (reuse detection)

```
정상: A → B → C → D  (한 방향)

탈취: 공격자가 B를 훔침
      정상 사용자: B → C → D
      공격자:      B → ??   ← 이미 쓴 B가 다시 등장!

  이 순간 "탈취됐다"는 것을 확실히 알 수 있습니다.
```

**대응: 그 토큰 패밀리 전체를 무효화합니다.** 정상 사용자도 재로그인하지만, 공격자는 차단됩니다.

```sql
-- user_db  (04 §10의 refresh_tokens를 확장)
CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    family_id   UUID NOT NULL,           -- 로그인 1회 = 패밀리 1개
    token_hash  TEXT NOT NULL UNIQUE,    -- ⚠️ 평문 저장 금지
    parent_id   UUID REFERENCES refresh_tokens(id),
    used_at     TIMESTAMPTZ,             -- NULL이면 미사용
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    user_agent  TEXT,
    ip          INET
);
CREATE INDEX ON refresh_tokens (family_id);
CREATE INDEX ON refresh_tokens (user_id, expires_at);
```

```rust
pub async fn rotate(&self, presented: &str) -> Result<TokenPair, AuthError> {
    let hash = sha256_hex(presented);       // 해시로 조회
    let mut tx = self.pool.begin().await?;

    let row = RefreshRepo::find_by_hash(&mut tx, &hash).await?
        .ok_or(AuthError::InvalidRefresh)?;

    // ── 재사용 탐지 ──────────────────────────────
    if row.used_at.is_some() {
        tracing::error!(
            user_id = %row.user_id, family_id = %row.family_id,
            "refresh token reuse detected — revoking family"
        );
        RefreshRepo::revoke_family(&mut tx, row.family_id).await?;
        self.revocation.revoke_all_for_user(row.user_id).await.ok();  // access도 무효화
        self.audit.log(AuditEvent::TokenReuseDetected { user_id: row.user_id }).await;
        tx.commit().await?;
        return Err(AuthError::TokenReuse);
    }

    if row.expires_at < Utc::now() {
        return Err(AuthError::ExpiredRefresh);
    }

    // ── 정상 회전 ───────────────────────────────
    RefreshRepo::mark_used(&mut tx, row.id).await?;
    let (new_token, new_hash) = generate_refresh_token();
    let new_row = RefreshRepo::insert(&mut tx, RefreshInsert {
        user_id:   row.user_id,
        family_id: row.family_id,      // 패밀리 유지
        parent_id: Some(row.id),
        token_hash: new_hash,
        expires_at: Utc::now() + Duration::days(30),
    }).await?;

    let access = self.jwt.issue(row.user_id, /* ... */)?;
    tx.commit().await?;

    Ok(TokenPair { access, refresh: new_token })
}
```

### 5-3. 세부 규칙

| 항목 | 규칙 |
|---|---|
| 저장 | **반드시 해시** (SHA-256으로 충분 — 고엔트로피 랜덤값이라 argon2 불필요) |
| 생성 | `rand::rngs::OsRng`로 32바이트 이상 |
| 절대 수명 | 회전해도 최초 로그인 후 최대 30~90일이면 재로그인 강제 |
| 동시 회전 | 모바일에서 요청 2개가 동시에 오면 오탐 가능 → **짧은 유예(5초) 내 같은 토큰 재사용은 허용** |
| 정리 | 만료된 행을 주기적으로 삭제 (안 하면 무한 증식) |

동시 회전 유예가 실전에서 중요합니다. 없으면 앱이 백그라운드 복귀할 때 두 요청이 겹쳐 **정상 사용자가 로그아웃**됩니다.

```rust
// 5초 이내에 같은 토큰이 다시 오면, 방금 발급한 것을 그대로 반환
if let Some(used_at) = row.used_at {
    if (Utc::now() - used_at).num_seconds() < 5 {
        if let Some(child) = RefreshRepo::find_child(&mut tx, row.id).await? {
            return Ok(cached_pair_for(child));
        }
    }
    // 5초 넘었으면 진짜 재사용 → 패밀리 폐기
}
```

---

## 6. 인증 경로 보호

### 6-1. 로그인 rate limit은 다층으로

```rust
// 1) IP 기준 — 분산 공격에는 약하지만 단순 스크립트는 막음
"auth:ip:{ip}"           → 분당 10회

// 2) 계정 기준 — 크리덴셜 스터핑 방어의 핵심
"auth:user:{email_hash}" → 분당 5회, 시간당 20회

// 3) 전역 — 대규모 공격 탐지
"auth:global"            → 초당 N회 초과 시 알림 + CAPTCHA
```

**계정 기준이 가장 중요합니다.** 공격자는 봇넷으로 IP를 바꿀 수 있지만, 노리는 계정은 고정입니다.

이메일을 그대로 키에 쓰지 마세요 — Redis 덤프가 곧 계정 목록이 됩니다.

```rust
fn account_key(email: &str) -> String {
    format!("auth:user:{}", sha256_hex(&email.to_lowercase())[..16].to_string())
}
```

### 6-2. 점진적 지연 (계정 잠금 대신)

계정을 잠그면 **공격자가 남의 계정을 잠글 수 있습니다** (DoS).

```rust
fn delay_for(failures: u32) -> Duration {
    match failures {
        0..=2   => Duration::ZERO,
        3..=5   => Duration::from_secs(1),
        6..=10  => Duration::from_secs(5),
        _       => Duration::from_secs(30),
    }
}
```

10회 이상이면 CAPTCHA나 이메일 확인을 요구합니다. **잠그지는 않습니다.**

### 6-3. 계정 열거 방지 (10_errata §3의 확장)

로그인 외에도 새는 곳이 많습니다.

| 엔드포인트 | 새는 방식 | 대응 |
|---|---|---|
| `POST /auth/login` | 404 vs 401 | 동일 응답 + 더미 해싱으로 시간 맞춤 |
| `POST /auth/register` | "이미 존재하는 이메일" 409 | 항상 202 + "메일을 확인하세요" |
| `POST /auth/reset` | "해당 이메일 없음" | 항상 202 |
| `GET /users?email=` | 검색 API | 인증 필수 + rate limit |

register에서 409를 안 주면 UX가 나빠지는 건 사실입니다.
**공개 서비스면 열거 방지, 사내 서비스면 UX 우선** — 의식적으로 고르세요.

### 6-4. 비밀번호 정책

```rust
pub fn validate_password(pw: &str, email: &str) -> Result<(), ApiError> {
    // 길이가 복잡도보다 중요 (NIST SP 800-63B)
    if pw.chars().count() < 12 {
        return Err(ApiError::BadRequest("비밀번호는 12자 이상".into()));
    }
    if pw.len() > 1024 {
        // ⚠️ 상한이 없으면 1MB 비밀번호로 argon2 DoS 가능
        return Err(ApiError::BadRequest("비밀번호가 너무 김".into()));
    }
    if pw.to_lowercase().contains(&email.split('@').next().unwrap_or("").to_lowercase()) {
        return Err(ApiError::BadRequest("이메일과 유사한 비밀번호 불가".into()));
    }
    // 유출 비밀번호 목록 대조 (HIBP k-anonymity API 등)
    Ok(())
}
```

**복잡도 규칙(대문자+특수문자)은 강제하지 마세요.** NIST 권고가 바뀌었습니다.
길이 + 유출 목록 대조가 훨씬 효과적입니다.

### 6-5. argon2 파라미터

06 §6은 "argon2 권장"에서 끝납니다. 파라미터가 실제 강도를 정합니다.

```rust
use argon2::{Algorithm, Argon2, Params, Version};

fn argon2_config() -> Argon2<'static> {
    // OWASP 권고 기준선
    let params = Params::new(
        19 * 1024,   // m: 19 MiB
        2,           // t: 반복 2회
        1,           // p: 병렬 1
        None,
    ).expect("valid argon2 params");

    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}
```

**메모리 파라미터의 함정:** 19MiB × 동시 해싱 64개 = **1.2GB**입니다.
[10_errata §6](./10_errata.md)의 세마포어와 반드시 같이 쓰세요.

```
컨테이너 메모리 512Mi + m=19MiB → 동시 해싱 최대 ~20개
→ Semaphore::new(8) 정도가 안전
```

파라미터는 해시 문자열에 인코딩되므로 **나중에 올릴 수 있습니다.**

```rust
// 로그인 성공 시 옛 파라미터면 조용히 재해싱
if needs_rehash(&stored_hash) {
    let new_hash = hash_password(pw.clone()).await?;
    repo.update_password_hash(user.id, &new_hash).await.ok();
}
```

---

## 7. 보안 헤더

```rust
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::{HeaderName, HeaderValue};

fn security_headers() -> impl Layer<...> + Clone {
    ServiceBuilder::new()
        // 중간자 공격 방지 — HTTPS 강제 (TLS 종료 지점에서만)
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        // MIME 스니핑 방지 — 업로드된 파일이 HTML로 해석되는 것 차단
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        // 클릭재킹 방지
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        // 리퍼러 유출 방지 — URL에 토큰이 있는 경우 특히
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        // JSON API는 아무것도 실행하지 않음을 명시
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
        ))
}
```

`SetResponseHeaderLayer::overriding` / `appending` / `if_not_present`는 tower-http 0.6.11에 실존합니다.

### CORS를 다시 봅니다

[06 §8](./06_gateway_auth.md)은 `CorsLayer::permissive()`를 "로컬만"이라고 하는데, 왜 위험한지가 없습니다.

```rust
// ❌ 절대 금지 조합 — 브라우저가 거부하지만, 직접 구현하면 뚫림
Access-Control-Allow-Origin: *
Access-Control-Allow-Credentials: true
// → 아무 사이트나 쿠키를 실어 우리 API를 호출

// ✅ 명시적 화이트리스트
let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::list([
        "https://app.example.com".parse().unwrap(),
        "https://admin.example.com".parse().unwrap(),
    ]))
    .allow_credentials(true)
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE,
        HeaderName::from_static("idempotency-key")])
    .max_age(Duration::from_secs(3600));   // preflight 캐시 — 지연 감소
```

**동적 origin 반영(요청의 Origin을 그대로 되돌려주기)은 하지 마세요.** 화이트리스트가 무의미해집니다.

---

## 8. 감사 로그

### 8-1. 일반 로그와 다릅니다

| | 애플리케이션 로그 | 감사 로그 |
|---|---|---|
| 목적 | 디버깅 | "누가 무엇을 했는가"의 법적 기록 |
| 보존 | 7~30일 | 1~7년 |
| 저장 | Loki/ELK | **DB 또는 추가 전용 스토리지** |
| 삭제 | 자유 | **금지** |
| 샘플링 | 가능 | **불가 — 전건 기록** |

`tracing::info!`로 남기면 감사 로그가 아닙니다. 로그 파이프라인은 유실될 수 있고 보존 기간도 짧습니다.

### 8-2. 스키마

```sql
CREATE TABLE audit_log (
    id           UUID PRIMARY KEY,          -- UUIDv7
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_id     UUID,                      -- 행위자 (시스템이면 NULL)
    actor_type   TEXT NOT NULL,             -- 'user' | 'service' | 'system'
    action       TEXT NOT NULL,             -- 'user.login' | 'order.cancel'
    resource_type TEXT,
    resource_id  TEXT,
    outcome      TEXT NOT NULL,             -- 'success' | 'failure'
    ip           INET,
    user_agent   TEXT,
    trace_id     TEXT,                      -- 앱 로그와 연결
    metadata     JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX ON audit_log (actor_id, occurred_at DESC);
CREATE INDEX ON audit_log (resource_type, resource_id, occurred_at DESC);
CREATE INDEX ON audit_log (action, occurred_at DESC);
```

월별 파티셔닝을 권합니다. 오래된 파티션은 콜드 스토리지로 옮기면 됩니다.

### 8-3. 무엇을 기록하나

```rust
pub enum AuditEvent {
    // 인증
    LoginSucceeded { user_id: Uuid },
    LoginFailed { email_hash: String, reason: &'static str },
    LogoutAll { user_id: Uuid },
    TokenReuseDetected { user_id: Uuid },     // §5-2
    PasswordChanged { user_id: Uuid },
    // 권한
    RoleGranted { target: Uuid, role: String, by: Uuid },
    // 데이터
    PiiAccessed { target: Uuid, by: Uuid, fields: Vec<String> },
    DataExported { by: Uuid, count: usize },
    RecordDeleted { resource: String, id: String, by: Uuid },
    // 설정
    ConfigChanged { key: String, by: Uuid },
}
```

**규칙: 인증·권한·PII 접근·삭제·설정 변경은 전부 기록.** 일반 조회는 기록하지 않습니다(양이 폭발).

### 8-4. 트랜잭션 안에서 기록

```rust
let mut tx = pool.begin().await?;
OrderRepo::cancel(&mut tx, order_id).await?;
AuditRepo::insert(&mut tx, AuditEvent::RecordDeleted { ... }).await?;
tx.commit().await?;   // 둘 다 되거나 둘 다 안 되거나
```

같은 트랜잭션이어야 **"작업은 됐는데 기록은 없는"** 상태가 안 생깁니다.
[07 §4](./07_messaging.md)의 outbox와 정확히 같은 논리입니다.

---

## 9. PII와 로그 위생

### 9-1. 08 §7은 금지만 하고 강제 수단이 없습니다

> 비밀번호·토큰·카드번호 로그 금지

사람이 지키게 하면 결국 샙니다. **타입으로 막으세요.**

```rust
/// Display/Debug가 절대 원본을 노출하지 않는 래퍼
#[derive(Clone, Serialize, Deserialize)]
pub struct Pii<T>(T);

impl<T> std::fmt::Debug for Pii<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T> Pii<T> {
    pub fn new(v: T) -> Self { Self(v) }
    /// 꺼내려면 명시적 호출 — grep으로 전수 감사 가능
    pub fn expose(&self) -> &T { &self.0 }
}

// 부분 마스킹이 필요하면
impl Pii<String> {
    pub fn masked_email(&self) -> String {
        match self.0.split_once('@') {
            Some((l, d)) if l.len() > 2 => format!("{}***@{}", &l[..2], d),
            _ => "***".into(),
        }
    }
}
```

```rust
pub struct UserRow {
    pub id: Uuid,
    pub email: Pii<String>,
    pub name: Pii<String>,
    pub password_hash: Pii<String>,
}

tracing::info!(?user, "loaded");
// → user: UserRow { id: 018f.., email: [REDACTED], name: [REDACTED], ... }
```

### 9-2. 로그 파이프라인에도 2차 방어

앱에서 놓쳐도 수집 단계에서 거르게 합니다.

```yaml
# Vector / Fluent Bit 등
transforms:
  redact:
    type: remap
    source: |
      .message = replace(.message, r'\b[\w.+-]+@[\w-]+\.[\w.]+\b', "[EMAIL]")
      .message = replace(.message, r'\b\d{13,19}\b', "[CARD]")
      .message = replace(.message, r'eyJ[A-Za-z0-9_-]{10,}', "[JWT]")
```

### 9-3. GDPR/개인정보보호법 대응

MSA에서 **"이 사용자의 데이터를 전부 지워라"** 는 어렵습니다. 데이터가 서비스마다 흩어져 있으니까요.

```
1. user.deletion_requested 이벤트 발행
2. 각 서비스가 자기 데이터를 익명화 또는 삭제
3. 각 서비스가 user.deletion_completed 응답
4. 전부 완료되면 user-service가 최종 삭제
```

- **감사 로그는 삭제하지 않습니다** (법적 보존 의무가 우선). `actor_id`만 익명화.
- **백업에서도 지워야 하는가**는 법무와 확인 (대부분 보존 기간 경과로 갈음).
- 어떤 서비스가 어떤 PII를 갖는지 **데이터 인벤토리 표**를 [01 §2](./01_architecture.md)의 서비스 표에 열로 추가하세요.

---

## 10. 의존성 · 시크릿 · 컨테이너

### 10-1. 공급망

[18_cicd §3](./18_cicd.md)에서 자동화하지만, 원칙만 여기에:

```bash
cargo deny check advisories   # RUSTSEC 취약점
cargo deny check licenses     # 라이선스 정책
cargo deny check bans         # 중복 버전 / 금지 크레이트
```

새 크레이트를 추가할 때 최소 확인:

| 항목 | 기준 |
|---|---|
| 최근 커밋 | 1년 이내 |
| 관리자 수 | 1명이면 위험 (bus factor) |
| `unsafe` 사용 | `cargo geiger`로 확인 |
| 의존성 수 | 작은 기능에 50개 의존은 재고 |

### 10-2. 시크릿 관리

```
❌ .env 파일을 이미지에 COPY
❌ Dockerfile의 ENV에 시크릿
❌ K8s ConfigMap에 시크릿
❌ git에 커밋 (히스토리에 영원히 남음)

✅ K8s Secret + etcd 암호화
✅ 외부 시크릿 매니저 (Vault, AWS Secrets Manager) + External Secrets Operator
✅ 파일 마운트 (환경변수보다 나음 — /proc/PID/environ 노출 방지)
```

```rust
/// 파일에서 우선 읽고 없으면 환경변수 (Docker Swarm/K8s 관례)
fn load_secret(name: &str) -> anyhow::Result<String> {
    if let Ok(path) = std::env::var(format!("{name}_FILE")) {
        return Ok(std::fs::read_to_string(path)?.trim().to_string());
    }
    std::env::var(name).map_err(|_| anyhow::anyhow!("secret {name} not found"))
}
```

**시크릿 누출 방지는 CI에서:** `gitleaks` / `trufflehog`를 pre-commit과 CI에 둘 다 겁니다.

### 10-3. 컨테이너

[09 §7](./09_deploy.md)의 체크리스트에 "non-root USER"가 있지만 §3의 Dockerfile에는 없습니다. 실제로:

```dockerfile
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r app && useradd -r -g app -u 10001 app

COPY --from=builder --chown=app:app /app/target/release/user-service /usr/local/bin/user-service

USER 10001
EXPOSE 3001
ENTRYPOINT ["user-service"]
```

```yaml
# K8s
securityContext:
  runAsNonRoot: true
  runAsUser: 10001
  readOnlyRootFilesystem: true       # 쓰기가 필요하면 emptyDir 볼륨
  allowPrivilegeEscalation: false
  capabilities: { drop: ["ALL"] }
  seccompProfile: { type: RuntimeDefault }
```

Rust 정적 바이너리는 `distroless` 또는 `scratch`에도 올라갑니다.

```dockerfile
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /app/target/release/user-service /user-service
USER nonroot
ENTRYPOINT ["/user-service"]
```

이미지가 ~20MB로 줄고 **셸이 없어서 침해 시 할 수 있는 게 거의 없습니다.**
단, 디버깅이 어려우므로 `kubectl debug`의 ephemeral container 사용법을 팀에 공유하세요.

### 10-4. 네트워크 정책

```yaml
# order-service는 user-service와 자기 DB에만 접근 가능
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: { name: order-service }
spec:
  podSelector: { matchLabels: { app: order-service } }
  policyTypes: [Ingress, Egress]
  ingress:
    - from: [{ podSelector: { matchLabels: { app: gateway } } }]
      ports: [{ port: 3002 }]
  egress:
    - to: [{ podSelector: { matchLabels: { app: user-service } } }]
    - to: [{ podSelector: { matchLabels: { app: postgres-order } } }]
    - to: [{ podSelector: { matchLabels: { app: nats } } }]
    - ports: [{ port: 53, protocol: UDP }]   # DNS
```

**기본 거부(default deny)에서 시작해서 필요한 것만 여세요.**
[01 §2](./01_architecture.md)의 서비스 맵이 그대로 NetworkPolicy가 됩니다.
이게 있으면 [10_errata §1](./10_errata.md)의 SSRF 시나리오도 상당 부분 막힙니다.

---

## 11. 위협 모델 요약

| 위협 | 이 문서의 대응 | 잔여 위험 |
|---|---|---|
| 헤더 스푸핑으로 사칭 | §1 서비스 JWT 재검증 + [10_errata §1](./10_errata.md) | — |
| 토큰 탈취 | §5 회전 + 재사용 탐지, §2-5 저장 위치 | 짧은 창 |
| 키 유출 | §3 무중단 회전 | 탐지 시점까지 |
| 크리덴셜 스터핑 | §6 다층 rate limit + 유출 목록 대조 | 저속 공격 |
| 브루트포스 | §6 점진적 지연, argon2 | — |
| 계정 열거 | §6-3 응답 통일 | 타이밍 |
| SSRF → 내부 호출 | §1 재검증 + §10-4 NetworkPolicy | — |
| 내부 에러 노출 | [10_errata §3](./10_errata.md) | — |
| 시크릿 로그 유출 | [10_errata §4](./10_errata.md) + §9 Pii 타입 | — |
| 의존성 취약점 | §10-1 cargo deny | 0-day |
| 컨테이너 탈출 | §10-3 non-root + seccomp | 커널 취약점 |
| PII 유출 | §9 타입 강제 + 파이프라인 마스킹 | — |

---

## 체크포인트

```
[ ] 모든 서비스가 JWT를 재검증한다 (헤더 신뢰 아님)
[ ] JWT가 EdDSA/RS256이고 공개키만 배포된다
[ ] Validation에 aud/iss/required_claims/leeway가 설정됐다
[ ] 검증 알고리즘이 코드에 하드코딩됐다 (토큰의 alg 불신)
[ ] Claims에 PII(email 등)가 없다
[ ] kid + JWKS로 무중단 키 회전이 가능하다
[ ] JWKS 갱신 실패 시 기존 키를 유지한다
[ ] jti/사용자 단위 토큰 폐기가 가능하다
[ ] Redis 장애 시 폐기 체크 정책(open/closed)이 문서화됐다
[ ] refresh 토큰이 해시로 저장되고 회전 + 재사용 탐지가 있다
[ ] 동시 회전 유예(5초)가 있다
[ ] 로그인 rate limit이 IP + 계정 이중이다
[ ] 계정 열거가 불가능하다 (login/register/reset 모두)
[ ] argon2 파라미터가 명시되고 메모리 × 동시성이 계산됐다
[ ] 보안 헤더 5종이 붙어 있다
[ ] CORS가 명시적 화이트리스트다
[ ] 감사 로그가 DB에 트랜잭션과 함께 기록된다
[ ] PII가 타입으로 마스킹된다
[ ] cargo deny가 CI에 있다
[ ] 컨테이너가 non-root + readOnlyRootFilesystem이다
[ ] NetworkPolicy가 기본 거부다
```

---

다음: [13_data_evolution — 무중단 스키마 변경과 DB 운영](./13_data_evolution.md)
