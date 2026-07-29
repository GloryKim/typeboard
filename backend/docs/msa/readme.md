# Axum으로 NestJS / Spring급 MSA 구축하기

> NestJS·Spring Boot로 익숙한 **모듈화 · DI · 레이어드 아키텍처 · DB · Redis · 서비스 간 통신**을  
> Rust + Axum 스택으로 같은 규모까지 끌어올리는 실전 가이드입니다.

NestJS의 `@Module` / Spring의 `@Service` 같은 매크로 DI는 없지만,  
**Cargo Workspace + 레이어 분리 + `Extension`/`State` + 미들웨어(Tower)** 로 동일한 규모를 안정적으로 운용할 수 있습니다.

### 상세 문서 바로가기

| 단계 | 문서 | 핵심 |
|---|---|---|
| 0 | [readme](./readme.md) | 로드맵 · NestJS/Spring 대응표 |
| 1 | [01_architecture](./01_architecture.md) | MSA 경계 나누기 · 서비스 맵 |
| 2 | [02_workspace](./02_workspace.md) | Cargo workspace · 공통 크레이트 |
| 3 | [03_service_anatomy](./03_service_anatomy.md) | 단일 서비스 내부 구조 (Axum = Nest Controller) |
| 4 | [04_database](./04_database.md) | PostgreSQL · sqlx · 마이그레이션 · 트랜잭션 |
| 5 | [05_redis](./05_redis.md) | 캐시 · 세션 · 분산 락 · Rate limit |
| 6 | [06_gateway_auth](./06_gateway_auth.md) | API Gateway · JWT · 권한 |
| 7 | [07_messaging](./07_messaging.md) | 동기(HTTP/gRPC) · 비동기(이벤트/큐) |
| 8 | [08_observability](./08_observability.md) | tracing · metrics · health · 상관 ID |
| 9 | [09_deploy](./09_deploy.md) | Docker Compose · 환경설정 · 배포 체크리스트 |

---

## NestJS / Spring ↔ Axum 대응표

| NestJS | Spring | Axum / Rust |
|---|---|---|
| `@Controller` | `@RestController` | `Router` + handler `async fn` |
| `@Injectable` / DI | `@Service` + DI | `AppState` / `Extension` / 생성자 주입 패턴 |
| `@Module` | `@Configuration` | crate / module + workspace |
| Guards / Interceptors | Filter / Interceptor | Tower `Layer` / middleware |
| `ConfigModule` | `@Value` / `Environment` | `config` 크레이트 + env |
| TypeORM / Prisma | JPA / MyBatis | **sqlx** (권장) 또는 SeaORM |
| `ioredis` / Cache | RedisTemplate | **redis** / `fred` + `deadpool-redis` |
| Kafka / RabbitMQ | Spring Cloud Stream | NATS / Kafka / Redis Streams |
| Nest microservices | Spring Cloud | 독립 바이너리 + Gateway |
| Swagger | SpringDoc | `utoipa` + OpenAPI |

---

## 권장 스택 (실무 기준)

```
Gateway / Edge
  └── axum + tower-http (CORS, compression, tracing)

Services (독립 바이너리)
  └── axum + tokio + serde
  └── sqlx + PostgreSQL (서비스별 DB 권장)
  └── redis (캐시 / 세션 / 락 / 짧은 큐)

Async backbone (선택)
  └── NATS JetStream 또는 Kafka (도메인 이벤트)

Ops
  └── tracing + opentelemetry
  └── Prometheus metrics
  └── Docker Compose → (이후) K8s
```

### 왜 sqlx인가?

- compile-time 쿼리 검증 (`query!`)으로 NestJS의 런타임 ORM 실수 감소
- 마이그레이션 내장 (`sqlx migrate`)
- Axum async와 자연스럽게 맞음
- SeaORM은 Active Record 스타일이 필요할 때 대안

---

## 권장 서비스 분할 (시작 세트)

처음부터 10개로 쪼개지 말고, **도메인 경계가 뚜렷한 4~5개**로 시작합니다.

```
┌─────────────┐
│   gateway   │  :8080  인증·라우팅·rate limit
└──────┬──────┘
       │
       ├──── user-service      :3001   회원 / 프로필
       ├──── order-service     :3002   주문
       ├──── catalog-service   :3003   상품
       └──── notification      :3004   알림 (이벤트 소비)
```

규칙:
1. **서비스 = 하나의 배포 단위 = 하나의 DB 스키마(또는 DB)**
2. 다른 서비스 DB를 JOIN하지 않는다 → API / 이벤트로 조회
3. 동기 호출은 최소화, 쓰기 경로는 이벤트 우선

---

## 학습 · 구축 순서

```
[ ] 1. 아키텍처 경계 확정 (01)
[ ] 2. Cargo workspace + common 크레이트 (02)
[ ] 3. user-service 골격 (03) — health, config, error
[ ] 4. PostgreSQL + sqlx 마이그레이션 (04)
[ ] 5. Redis 캐시/세션 (05)
[ ] 6. gateway + JWT (06)
[ ] 7. order → user 동기 호출 + 주문 생성 이벤트 (07)
[ ] 8. tracing / metrics / request-id (08)
[ ] 9. docker compose 로 전체 기동 (09)
```

1~5까지면 “Spring Boot 단일 앱 + Redis” 수준이고,  
6~9까지면 **진짜 MSA 운영 골격**입니다.

---

## 디렉터리 목표 형태 (미리보기)

```
platform/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── common/                # Error, Id, time, config helpers
│   ├── auth-jwt/              # JWT 발급/검증 공유
│   └── proto/                 # (선택) tonic gRPC
├── services/
│   ├── gateway/
│   ├── user-service/
│   ├── order-service/
│   ├── catalog-service/
│   └── notification-service/
├── migrations/                # 또는 서비스별 migrations/
├── docker-compose.yml
└── .env.example
```

상세는 [02_workspace](./02_workspace.md)에서 다룹니다.

---

## 참고 자료

- [Axum](https://docs.rs/axum) — HTTP 프레임워크
- [Tower](https://docs.rs/tower) — 미들웨어 레이어
- [sqlx](https://docs.rs/sqlx) — async SQL
- [redis-rs](https://docs.rs/redis) — Redis 클라이언트
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- 이 리포의 [tokio/readme](../tokio/readme.md) · [reqwest/01.md](../reqwest/01.md) (Axum 송수신 예제)
