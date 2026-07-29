# 16. API 계약 — OpenAPI · 클라이언트 생성 · 계약 테스트

[readme](./readme.md)의 대응표에 `utoipa`가 있습니다.

| Swagger | SpringDoc | `utoipa` + OpenAPI |

그런데 **01~09 어느 문서에도 `utoipa`가 나오지 않습니다.** 이 문서가 그 자리를 채웁니다.

MSA에서 계약은 선택이 아닙니다. 서비스가 3개일 때는 슬랙으로 물어보면 되지만,
**5개를 넘어가면 "이 API가 뭘 반환하는지" 아무도 정확히 모릅니다.**

---

## 1. 왜 계약이 필요한가

### 1-1. 계약이 없을 때 벌어지는 일

```
1. user-service 팀이 응답에서 `phone` 필드를 뺌 (아무도 안 쓴다고 판단)
2. 배포
3. order-service의 주문 확인 화면이 깨짐 (사실은 쓰고 있었음)
4. 장애 대응 30분
5. "다음부터 미리 알려주세요" — 지켜지지 않음
```

**"누가 내 API를 쓰는지 모른다"** 가 근본 원인입니다. MSA의 대표적 실패 모드입니다.

### 1-2. 세 층의 계약

| 층 | 대상 | 도구 |
|---|---|---|
| HTTP API | 클라이언트 ↔ 서비스 | OpenAPI (`utoipa`) |
| 이벤트 | 생산자 ↔ 소비자 | JSON Schema / protobuf ([14 §8](./14_messaging_ops.md)) |
| 소비자 기대 | 실제로 뭘 쓰는가 | 계약 테스트 (§6) |

세 번째가 가장 중요하고 가장 자주 빠집니다.
**OpenAPI는 "무엇을 제공하는가"만 말하고, "누가 무엇에 의존하는가"는 말하지 않습니다.**

---

## 2. utoipa 셋업

```toml
utoipa = { version = "5", features = ["axum_extras", "chrono", "uuid"] }
utoipa-axum = "0.2"           # Router 통합
utoipa-swagger-ui = { version = "9", features = ["axum"] }
```

> 버전은 예시입니다. `utoipa`와 `utoipa-axum`, `utoipa-swagger-ui`의 호환 조합을 확인하세요.
> [15 §2-1](./15_observability_deep.md)의 `cargo tree -d` 방법이 여기에도 적용됩니다.

### 2-1. 스키마 어노테이션

```rust
use utoipa::{ToSchema, IntoParams};

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    /// 사용자 고유 ID (UUIDv7)
    #[schema(example = "018f4a2b-1234-7890-abcd-ef0123456789")]
    pub id: Uuid,

    /// 이메일 주소
    #[schema(example = "ada@example.com", format = "email")]
    pub email: String,

    /// 표시 이름
    #[schema(example = "Ada Lovelace", min_length = 1, max_length = 100)]
    pub name: String,

    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    #[schema(example = "ada@example.com", format = "email")]
    pub email: String,

    #[validate(length(min = 1, max = 100))]
    #[schema(example = "Ada Lovelace")]
    pub name: String,

    /// 12자 이상 (12_security §6-4)
    #[validate(length(min = 12))]
    #[schema(min_length = 12, write_only = true)]   // 응답에는 절대 안 나감
    pub password: String,
}
```

`write_only = true`가 유용합니다. **스펙 문서에 "이건 요청에만 쓰인다"가 명시**되어
프론트엔드가 응답에서 찾다 헤매지 않습니다.

### 2-2. 에러 스키마도 반드시

[10_errata §3](./10_errata.md)의 `ErrorBody`를 스펙에 넣으세요.

```rust
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    /// 안정적인 에러 코드. 클라이언트는 message가 아니라 이 값으로 분기한다.
    #[schema(example = "not_found")]
    pub error: String,

    /// 사람이 읽는 설명. 언제든 바뀔 수 있으므로 분기 조건으로 쓰지 말 것.
    #[schema(example = "user not found")]
    pub message: String,

    /// 지원 문의 시 이 값을 전달 (15 §7-2)
    #[schema(example = "4bf92f3577b34da6a3ce929d0e0e4736")]
    pub trace_id: String,
}
```

**`error` 코드가 안정적이라는 것을 문서에 명시하세요.**
안 그러면 프론트엔드가 `message`를 문자열 비교합니다. 그리고 문구를 다듬는 순간 깨집니다.

### 2-3. 핸들러 어노테이션

```rust
#[utoipa::path(
    post,
    path = "/v1/users",
    tag = "users",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "생성됨", body = UserResponse),
        (status = 400, description = "입력 오류", body = ErrorBody),
        (status = 409, description = "이메일 중복", body = ErrorBody),
        (status = 429, description = "요청 한도 초과", body = ErrorBody,
            headers(("retry-after" = String, description = "재시도까지 초"))),
    ),
)]
pub async fn register(
    State(state): State<AppState>,
    AppJson(body): AppJson<RegisterRequest>,     // 10_errata §12-7
) -> ApiResult<(StatusCode, Json<UserResponse>)> {
    ...
}

#[utoipa::path(
    get,
    path = "/v1/users/{id}",
    tag = "users",
    params(("id" = Uuid, Path, description = "사용자 ID")),
    responses(
        (status = 200, body = UserResponse),
        (status = 404, body = ErrorBody),
    ),
    security(("bearer_auth" = [])),
)]
pub async fn get_by_id(...) -> ApiResult<Json<UserResponse>> { ... }
```

### 2-4. 라우터 조립

`utoipa-axum`을 쓰면 라우트와 스펙이 **한 곳에서** 등록되어 어긋나지 않습니다.

```rust
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "User Service API",
        version = env!("CARGO_PKG_VERSION"),
        description = "회원 가입·조회·인증",
        contact(name = "Platform Team", email = "platform@example.com"),
    ),
    servers(
        (url = "https://api.example.com", description = "Production"),
        (url = "http://localhost:8080", description = "Local"),
    ),
    modifiers(&SecurityAddon),
    tags((name = "users", description = "회원 관리")),
)]
pub struct ApiDoc;

struct SecurityAddon;
impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
        if let Some(c) = openapi.components.as_mut() {
            c.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(handlers::users::register))
        .routes(routes!(handlers::users::get_by_id))
        .routes(routes!(handlers::users::update))
}
```

**`routes!` 매크로가 핵심입니다.** 라우트를 추가하면 스펙에도 자동 반영되므로,
"코드는 바꿨는데 문서는 안 바꾼" 상태가 구조적으로 불가능해집니다.

### 2-5. 스펙 노출

```rust
let (router, api) = router().split_for_parts();

let app = router
    // 스펙 JSON — 클라이언트 생성·계약 테스트가 소비
    .route("/openapi.json", get({
        let api = api.clone();
        move || async move { Json(api) }
    }))
    // Swagger UI — 개발/스테이징만
    .merge(if cfg.expose_docs {
        SwaggerUi::new("/docs").url("/openapi.json", api)
    } else {
        SwaggerUi::new("/__disabled")
    })
    .with_state(state);
```

> **프로덕션에서 Swagger UI를 열지 마세요.** API 전체 지도를 공개하는 것입니다.
> 내부망 또는 인증 뒤에만 두세요. `/openapi.json` 자체도 마찬가지입니다.

---

## 3. 스펙을 CI 산출물로

### 3-1. 파일로 뽑기

```rust
// src/bin/dump-openapi.rs
fn main() -> anyhow::Result<()> {
    let (_, api) = user_service::router().split_for_parts();
    let json = api.to_pretty_json()?;
    std::fs::write("openapi.json", json)?;
    Ok(())
}
```

```bash
cargo run -p user-service --bin dump-openapi
```

### 3-2. 스펙 변경을 리뷰 대상으로

`openapi.json`을 **리포에 커밋**하고, CI에서 최신인지 확인합니다.

```yaml
- name: OpenAPI spec is up to date
  run: |
    cargo run -p user-service --bin dump-openapi
    git diff --exit-code services/user-service/openapi.json \
      || (echo "::error::스펙이 변경되었습니다. dump-openapi를 실행하고 커밋하세요." && exit 1)
```

**효과가 큽니다.** API를 바꾸면 PR diff에 스펙 변경이 나타나므로,
리뷰어가 **"이거 breaking change 아니야?"** 를 즉시 볼 수 있습니다.

### 3-3. 파괴적 변경 자동 탐지

```yaml
- name: Detect breaking API changes
  run: |
    # oasdiff 등의 도구로 base와 비교
    oasdiff breaking \
      origin/main:services/user-service/openapi.json \
      services/user-service/openapi.json
```

파괴적 변경 목록:

| 변경 | 파괴적? |
|---|---|
| 엔드포인트 삭제 | ✅ |
| 필수 요청 필드 추가 | ✅ |
| 응답 필드 삭제 | ✅ |
| 응답 필드 타입 변경 | ✅ |
| enum 값 제거 | ✅ |
| 옵셔널 요청 필드 추가 | ❌ |
| 응답 필드 추가 | ❌ (클라이언트가 무시하면) |
| 새 엔드포인트 | ❌ |

[13 §2](./13_data_evolution.md)의 expand/contract, [14 §8-2](./14_messaging_ops.md)의 이벤트 진화와
**정확히 같은 원리**입니다. 스키마 진화의 법칙은 하나입니다: **더하는 건 안전, 빼는 건 위험.**

---

## 4. 클라이언트 생성

### 4-1. 07 §7의 `crates/clients`를 실제로

> 클라이언트 SDK가 있다면 workspace에 `crates/clients` 로 생성

**손으로 쓴 클라이언트는 반드시 스펙과 어긋납니다.** 생성하세요.

```bash
# 방법 A: OpenAPI Generator
openapi-generator-cli generate \
  -i services/user-service/openapi.json \
  -g rust \
  -o crates/clients/user-client

# 방법 B: progenitor (Rust 네이티브, build.rs로 통합 가능)
```

### 4-2. 생성 코드를 그대로 쓰지 마세요

생성된 클라이언트에는 이 가이드가 요구하는 것들이 없습니다.

```rust
// crates/clients/src/user.rs — 생성 코드를 감싼다
pub struct UserClient {
    inner: generated::Client,
    bulkhead: Bulkhead,          // 11 §6-1
    breaker: CircuitBreaker,     // 11 §6-2
}

impl UserClient {
    pub async fn get_user(&self, id: Uuid, dl: Deadline) -> Result<UserDto, ClientError> {
        if dl.expired() { return Err(ClientError::DeadlineExceeded); }   // 11 §5-3

        self.bulkhead.run(
            self.breaker.call(
                self.inner.get_user()
                    .id(id)
                    .timeout(dl.child_budget(Duration::from_secs(2)))
                    .send()
            )
        ).await
        .map_err(Into::into)
    }
}
```

**생성 = 직렬화/역직렬화/URL 조립, 수동 = 회복탄력성.** 역할을 나누세요.

### 4-3. 프론트엔드용

```bash
# TypeScript 타입 + fetch 클라이언트
npx openapi-typescript services/gateway/openapi.json -o src/api/schema.d.ts
```

프론트엔드가 백엔드 타입을 **직접 가져다 쓰는** 상태가 목표입니다.
"백엔드가 필드를 바꿨는데 프론트가 몰랐다"가 컴파일 에러로 잡힙니다.

---

## 5. Gateway에서 스펙 통합

클라이언트 입장에서는 gateway가 유일한 API입니다. 스펙도 하나여야 합니다.

```rust
// gateway가 각 서비스의 스펙을 모아 합친다
async fn aggregate_specs(services: &[(&str, &str)]) -> anyhow::Result<OpenApi> {
    let mut merged = OpenApi::new(
        Info::new("Platform API", env!("CARGO_PKG_VERSION")),
        Paths::new(),
    );

    for (name, base_url) in services {
        let spec: OpenApi = reqwest::get(format!("{base_url}/openapi.json"))
            .await?.json().await?;

        // 경로 병합 (충돌 검사 필요)
        for (path, item) in spec.paths.paths {
            if merged.paths.paths.contains_key(&path) {
                anyhow::bail!("경로 충돌: {path} (서비스 {name})");
            }
            merged.paths.paths.insert(path, item);
        }

        // 컴포넌트 병합 — 같은 이름의 다른 스키마가 있으면 위험
        if let Some(c) = spec.components {
            let m = merged.components.get_or_insert_with(Default::default);
            for (k, v) in c.schemas {
                if let Some(existing) = m.schemas.get(&k) {
                    if existing != &v {
                        anyhow::bail!("스키마 충돌: {k} (서비스 {name})");
                    }
                }
                m.schemas.insert(k, v);
            }
        }
    }
    Ok(merged)
}
```

**충돌 검사가 핵심입니다.** 두 서비스가 서로 다른 `UserDto`를 정의했는데 조용히 병합되면,
생성된 클라이언트가 잘못된 타입을 갖게 됩니다.

빌드 타임에 하는 게 낫습니다. 런타임 aggregation은 서비스가 다 떠 있어야 하고, 부팅 의존성이 생깁니다.

---

## 6. 계약 테스트 — 가장 중요한 부분

### 6-1. OpenAPI만으로는 부족합니다

```
OpenAPI: "user-service는 name, email, phone을 반환합니다"
현실   : order-service는 name만 씁니다. catalog는 email만 씁니다.

phone을 지워도 되나? → OpenAPI로는 알 수 없습니다.
```

**소비자 주도 계약(consumer-driven contract)** 이 이 질문에 답합니다.

### 6-2. 개념

```
1. 소비자(order-service)가 "나는 이런 응답을 기대한다"를 계약으로 명시
2. 그 계약으로 모의 서버를 만들어 소비자 테스트 실행
3. 계약을 생산자(user-service)에게 전달
4. 생산자 CI가 "실제 응답이 모든 소비자 계약을 만족하는가" 검증
5. 만족하지 않으면 배포 차단
```

**생산자가 자기 소비자를 자동으로 알게 됩니다.** §1-1의 사고가 CI에서 잡힙니다.

### 6-3. 가벼운 자체 구현

Pact 같은 본격 도구는 러닝커브가 있습니다. 리포 안에서 시작해도 충분합니다.

```
contracts/
├── user-service/
│   ├── order-service.json      # order가 user에게 기대하는 것
│   └── gateway.json
└── catalog-service/
    └── order-service.json
```

```json
{
  "consumer": "order-service",
  "provider": "user-service",
  "interactions": [
    {
      "description": "주문 생성 시 사용자 조회",
      "request": {
        "method": "GET",
        "path": "/v1/users/{id}",
        "headers": { "authorization": "Bearer <token>" }
      },
      "response": {
        "status": 200,
        "requiredFields": ["id", "name"],
        "fieldTypes": { "id": "uuid", "name": "string" }
      }
    },
    {
      "description": "없는 사용자",
      "request": { "method": "GET", "path": "/v1/users/{id}" },
      "response": {
        "status": 404,
        "requiredFields": ["error"],
        "fieldValues": { "error": "not_found" }
      }
    }
  ]
}
```

두 번째 상호작용이 중요합니다. **에러 응답도 계약입니다.**
`error` 코드를 `not_found`에서 `user_not_found`로 바꾸면 소비자 로직이 깨집니다.

### 6-4. 생산자 측 검증

```rust
// services/user-service/tests/contracts.rs
#[tokio::test]
async fn satisfies_all_consumer_contracts() {
    let app = test_app().await;   // 17_testing §2

    for entry in std::fs::read_dir("../../contracts/user-service").unwrap() {
        let contract: Contract = serde_json::from_reader(
            std::fs::File::open(entry.unwrap().path()).unwrap()
        ).unwrap();

        for interaction in &contract.interactions {
            let res = app.request(&interaction.request).await;

            assert_eq!(
                res.status(), interaction.response.status,
                "계약 위반: {} → {} / {}",
                contract.consumer, contract.provider, interaction.description
            );

            let body: serde_json::Value = res.json().await;
            for field in &interaction.response.required_fields {
                assert!(
                    body.get(field).is_some(),
                    "필드 '{}' 가 사라졌습니다. 소비자 '{}' 가 이 필드를 사용합니다.",
                    field, contract.consumer
                );
            }
            for (field, expected) in &interaction.response.field_values {
                assert_eq!(body.get(field).and_then(|v| v.as_str()), Some(expected.as_str()));
            }
        }
    }
}
```

**실패 메시지가 핵심입니다.**

```
필드 'name' 가 사라졌습니다. 소비자 'order-service' 가 이 필드를 사용합니다.
```

이 한 줄이 §1-1의 30분 장애를 CI 3초로 바꿉니다.

### 6-5. 소비자 측 검증

계약이 실제 사용과 일치하는지도 확인해야 합니다.

```rust
// services/order-service/tests/user_client_contract.rs
#[tokio::test]
async fn user_client_matches_contract() {
    let contract = load_contract("../../contracts/user-service/order-service.json");
    let mock = MockServer::start().await;

    for interaction in &contract.interactions {
        Mock::given(method(&interaction.request.method))
            .and(path_regex(&interaction.request.path_pattern()))
            .respond_with(ResponseTemplate::new(interaction.response.status)
                .set_body_json(interaction.response.example_body()))
            .mount(&mock).await;
    }

    // 모의 서버만으로 실제 코드가 동작해야 함
    let client = UserClient::new(mock.uri());
    let user = client.get_user(uuid, Deadline::in_secs(5)).await.unwrap();
    assert_eq!(user.name, "Ada");
}
```

이러면 **계약에 없는 필드를 코드가 쓰면 테스트가 실패**합니다.
계약이 실제 의존성을 정확히 반영하게 유지됩니다.

### 6-6. 배포 게이트

```yaml
- name: Verify consumer contracts
  run: cargo test -p user-service --test contracts

# 통과해야만 배포
```

**계약을 깨야 한다면?** 정상적인 절차가 있습니다.

```
1. 소비자에게 알림 (계약 파일의 owner 필드)
2. 소비자가 의존을 제거하고 계약 갱신 → 배포
3. 그 다음에 생산자가 필드 제거
```

또 expand/contract입니다. 이 문서 세트 전체를 관통하는 하나의 패턴입니다.

---

## 7. 이벤트 계약

HTTP만 계약이 아닙니다. [14 §8](./14_messaging_ops.md)의 이벤트도 마찬가지입니다.

### 7-1. 스키마 생성

```rust
// crates/events — schemars로 JSON Schema 생성
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct OrderCreated {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub total_cents: i64,
    pub currency: String,
    #[serde(default)]
    pub coupon_code: Option<String>,
}
```

```rust
// src/bin/dump-event-schemas.rs
fn main() -> anyhow::Result<()> {
    for (name, schema) in [
        ("order.created", schemars::schema_for!(OrderCreated)),
        ("order.cancelled", schemars::schema_for!(OrderCancelled)),
    ] {
        std::fs::write(
            format!("schemas/{name}.json"),
            serde_json::to_string_pretty(&schema)?,
        )?;
    }
    Ok(())
}
```

### 7-2. 호환성 검사

```rust
#[test]
fn event_schemas_are_backward_compatible() {
    for (name, current) in current_schemas() {
        let previous = load_committed_schema(&name);

        // 필수 필드가 늘어나면 옛 생산자의 이벤트를 새 소비자가 못 읽는다
        let new_required: HashSet<_> = current.required.iter().collect();
        let old_required: HashSet<_> = previous.required.iter().collect();
        let added: Vec<_> = new_required.difference(&old_required).collect();
        assert!(added.is_empty(), "{name}: 필수 필드 추가는 파괴적입니다: {added:?}");

        // 필드 제거도 마찬가지
        for field in previous.properties.keys() {
            assert!(
                current.properties.contains_key(field),
                "{name}: 필드 '{field}' 제거는 파괴적입니다. 새 event_type을 만드세요."
            );
        }
    }
}
```

### 7-3. 소비자 등록부를 코드로

[14 §8-4](./14_messaging_ops.md)의 표를 사람이 관리하면 낡습니다. 선언하게 만드세요.

```rust
// 각 소비 서비스에서
inventory::submit! {
    EventSubscription {
        event_type: "order.created",
        consumer: "notification-service",
        fields_used: &["order_id", "user_id", "total_cents"],
        owner: "platform-team",
    }
}
```

CI가 이걸 수집해 등록부를 생성하면, **"이 필드를 지워도 되나"에 자동으로 답할 수 있습니다.**

---

## 8. API 버저닝

### 8-1. 07 §7의 URL 버저닝

```
/v1/orders
```

간단하고 명확합니다. 다만 **언제 v2를 만드는가**의 기준이 필요합니다.

| 상황 | 대응 |
|---|---|
| 필드 추가 | v1 유지 (호환) |
| 옵셔널 파라미터 추가 | v1 유지 |
| 필드 제거 | 소비자 이전 후 v1에서 제거 (계약 테스트가 지켜줌) |
| 응답 구조 전면 개편 | **v2 신설** |
| 리소스 의미 변경 | **v2 신설** |

**v2를 남발하지 마세요.** v1과 v2를 동시에 유지하는 비용이 큽니다.
대부분의 변경은 호환 가능하게 만들 수 있습니다.

### 8-2. v2를 만들 때

```rust
Router::new()
    .nest("/v1", v1::router())
    .nest("/v2", v2::router())
```

```
services/order-service/src/
├── api/
│   ├── v1/
│   │   ├── dto.rs        # v1 DTO — 절대 수정하지 않음
│   │   └── handlers.rs   # 도메인 → v1 DTO 변환
│   └── v2/
│       ├── dto.rs
│       └── handlers.rs
└── domain/               # 버전 없음 — 여기만 발전
```

**핵심: 도메인 모델은 하나, DTO만 버전별로.**
도메인을 v1/v2로 나누면 로직이 두 벌이 되어 유지가 불가능해집니다.

### 8-3. 폐기 절차

```rust
// Sunset 헤더로 예고 (RFC 8594)
.layer(SetResponseHeaderLayer::if_not_present(
    HeaderName::from_static("sunset"),
    HeaderValue::from_static("Sat, 31 Dec 2026 23:59:59 GMT"),
))
.layer(SetResponseHeaderLayer::if_not_present(
    HeaderName::from_static("deprecation"),
    HeaderValue::from_static("true"),
))
```

```rust
// 사용량을 측정해야 언제 끌 수 있는지 안다
metrics::counter!("deprecated_api_calls_total",
    "version" => "v1",
    "route" => route,
    "client" => client_id_from_token(&claims),   // 누가 아직 쓰는지
).increment(1);
```

**`client` 라벨의 카디널리티에 주의하세요** ([15 §5](./15_observability_deep.md)).
클라이언트가 수천 개면 이건 로그로 남기고 메트릭에는 넣지 마세요.

---

## 9. API 설계 규약

계약 도구가 있어도 **팀마다 다른 스타일**이면 클라이언트가 고통받습니다.
`crates/common`에 공통 타입을 두고 규약을 문서화하세요.

### 9-1. 페이지네이션 응답 형식 통일

```rust
#[derive(Serialize, ToSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// 다음 페이지 커서. null이면 마지막 (10_errata §9)
    pub next_cursor: Option<String>,
    /// 전체 개수는 비싸므로 요청 시에만 (?include_total=true)
    pub total: Option<i64>,
}
```

**`total`을 기본으로 주지 마세요.** `COUNT(*)`가 큰 테이블에서 매우 느립니다.

### 9-2. 시간과 금액

| 대상 | 규약 |
|---|---|
| 시각 | RFC 3339 UTC (`2026-07-23T12:00:00Z`). 로컬 시간대 금지 |
| 금액 | 최소 단위 정수 (`total_cents: i64`) + `currency`. **부동소수 금지** |
| 기간 | 초 단위 정수 또는 ISO 8601 duration |
| ID | UUIDv7 문자열 |
| enum | `snake_case` 문자열. 숫자 코드 금지 (의미 불명) |

[04 §10](./04_database.md)의 `total_cents` 선택은 옳습니다. 이걸 전 서비스 규약으로 올리세요.

### 9-3. 일관성 있는 필드명

```
✅ created_at, updated_at, deleted_at    (전 서비스 동일)
❌ created_at / createdAt / create_time  (서비스마다 다름)
```

**JSON은 `snake_case`로 통일**하고, 프론트엔드 변환은 클라이언트 생성 코드에 맡기세요.

### 9-4. `PATCH`의 null 문제

```json
{ "name": null }
```

"name을 null로 설정" 인가 "name을 변경하지 않음" 인가?

```rust
/// 3상태를 명시적으로 구분
#[derive(Deserialize)]
pub enum Patch<T> {
    /// 필드 자체가 없음 — 변경 안 함
    Undefined,
    /// null — 값을 지움
    Null,
    /// 값 설정
    Value(T),
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateUser {
    #[serde(default, skip_serializing_if = "Patch::is_undefined")]
    pub name: Patch<String>,
    #[serde(default)]
    pub phone: Patch<String>,
}
```

간단한 대안: **`PATCH` 대신 `PUT`(전체 교체)만 쓰기.** 모호함이 사라집니다.

---

## 10. 문서화 — 스펙이 못 담는 것

OpenAPI는 **구조**를 담지만 **맥락**은 못 담습니다. 서비스마다 `README.md`를 두세요.

```markdown
# order-service

## 책임
주문 생성·조회·상태 관리. 결제는 하지 않음(payment-service).

## 소유 데이터
orders, order_items, outbox, order_saga

## 의존
| 대상 | 방식 | 타임아웃 | 실패 시 |
|---|---|---|---|
| user-service | HTTP GET | 2s | 주문 생성 거절 |
| catalog-service | HTTP GET | 2s | 재고 미검증 플래그 |
| NATS | publish (outbox) | - | outbox에 적재, 재시도 |

## 발행 이벤트
- `order.created` — 주문 생성 시
- `order.cancelled` — 취소 시 (보상 포함)

## 구독 이벤트
- `payment.captured` → 주문 확정
- `user.updated` → 스냅샷 갱신

## 운영
- 대시보드: https://grafana/d/order-service
- 런북: https://wiki/runbooks/order-service
- 온콜: #team-orders

## 알려진 제약
- 주문 1건당 아이템 최대 100개
- 취소는 배송 시작 전까지만
```

**의존 표가 [01 §7](./01_architecture.md)의 실패 모드 표와 이어집니다.**
새로 합류한 사람이 이 파일 하나로 서비스를 파악할 수 있어야 합니다.

---

## 11. ADR — 결정을 기록하기

readme의 *"왜 sqlx인가?"* 는 좋은 ADR입니다. 이걸 습관으로 만드세요.

```
docs/adr/
├── 0001-use-sqlx-over-seaorm.md
├── 0002-database-per-service.md
├── 0003-nats-over-kafka.md
├── 0004-eddsa-jwt-with-jwks.md
└── 0005-custom-gateway-vs-envoy.md
```

```markdown
# ADR 0003: 이벤트 버스로 Kafka 대신 NATS JetStream

- 상태: 채택
- 날짜: 2026-07-23
- 결정자: 플랫폼팀

## 맥락
서비스 간 비동기 이벤트가 필요. 예상 볼륨 일 100만 건.
운영 인력 2명, Kafka 운영 경험 없음.

## 결정
NATS JetStream을 채택한다.

## 근거
- 단일 바이너리, 운영 부담이 Kafka보다 현저히 낮음
- 예상 볼륨에서 성능 충분
- at-least-once + durable consumer로 요구사항 충족
- Rust 클라이언트(async-nats)가 성숙

## 결과
- ✅ 운영 부담 감소, 도입 2일
- ⚠️ 장기 보관(수개월)이 필요하면 Kafka 재검토
- ⚠️ 생태계(Connect, Streams 등)가 Kafka보다 얕음

## 재검토 조건
- 일 이벤트 1,000만 건 초과
- 이벤트 소싱 스토어가 필요해질 때
```

**"재검토 조건"이 가장 유용합니다.** 결정이 영원하지 않다는 것을 명시하고,
언제 다시 볼지를 정해두면 나중에 "왜 이렇게 했지?"가 사라집니다.

---

## 체크포인트

```
[ ] 모든 공개 엔드포인트에 utoipa 어노테이션이 있다
[ ] ErrorBody가 스펙에 있고 error 코드의 안정성이 명시됐다
[ ] 비밀번호 등이 write_only로 표시됐다
[ ] utoipa-axum의 routes!로 라우트와 스펙이 한 곳에 있다
[ ] openapi.json이 커밋되고 CI가 최신성을 검사한다
[ ] 파괴적 변경 탐지가 CI에 있다
[ ] 프로덕션에 Swagger UI가 노출되지 않는다
[ ] 클라이언트가 스펙에서 생성되고 회복탄력성으로 감싸져 있다
[ ] gateway가 스펙을 통합하고 충돌을 검사한다
[ ] 소비자 계약 파일이 있고 생산자 CI가 검증한다
[ ] 계약 위반 메시지에 소비자 이름이 나온다
[ ] 이벤트 JSON Schema가 생성되고 호환성이 검사된다
[ ] 페이지네이션/시간/금액 규약이 통일됐다
[ ] total이 기본 응답에 없다
[ ] v1/v2가 DTO만 나뉘고 도메인은 하나다
[ ] 폐기 API에 Sunset 헤더와 사용량 메트릭이 있다
[ ] 서비스마다 README(책임·의존·이벤트·런북)가 있다
[ ] 주요 결정이 ADR로 남고 재검토 조건이 있다
```

---

다음: [17_testing — 실제로 굴러가는 테스트](./17_testing.md)
