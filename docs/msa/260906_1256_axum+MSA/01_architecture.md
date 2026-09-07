# 01. MSA 아키텍처 — 경계를 어떻게 나눌까

NestJS/Spring에서 모놀리스를 모듈로 나누던 감각을,  
Rust에서는 **프로세스(바이너리) 단위**로 올립니다. 모듈 ≠ 서비스입니다.

---

## 1. 모놀리스 → MSA 전환 기준

### 아직 모놀리스(또는 모듈러 모놀리스)로 충분한 경우

- 팀 1~3명, 트래픽 예측 가능
- 도메인 간 트랜잭션이 자주 필요 (`주문 + 재고 + 결제`를 한 트랜잭션으로)
- 배포·운영 경험이 아직 적음

→ Axum 단일 바이너리 + `mod user; mod order;` 로 시작해도 됩니다.  
나중에 crate로 쪼개고, 그다음 바이너리로 올리면 됩니다.

### MSA로 가는 신호

| 신호 | 의미 |
|---|---|
| 팀/배포 주기가 도메인별로 다름 | 독립 배포 가치 |
| 스케일 특성이 다름 (읽기 폭주 vs 쓰기 폭주) | 독립 스케일 |
| 장애 격리가 필요 | blast radius 축소 |
| 기술 스택이 달라질 수 있음 | 폴리글롯 (당장 Rust만 써도 OK) |

**원칙:** 처음엔 **4개 이하**. 쪼개고 합치는 비용은 DB 분리 이후 급증합니다.

---

## 2. 서비스 맵 (이 가이드의 기준안)

```
                    ┌──────────────────┐
   Client ─────────▶│     gateway      │
                    │  JWT / rate /    │
                    │  route / cors    │
                    └────────┬─────────┘
           ┌─────────────────┼─────────────────┐
           ▼                 ▼                 ▼
    ┌────────────┐    ┌────────────┐    ┌────────────┐
    │   user     │    │   order    │    │  catalog   │
    │  PG + Redis│    │  PG + Redis│    │  PG + Redis│
    └─────┬──────┘    └──────┬─────┘    └────────────┘
          │                  │
          │   domain events  │
          └────────┬─────────┘
                   ▼
            ┌────────────┐
            │notification│  (이메일/푸시, 이벤트 소비)
            └────────────┘
```

| 서비스 | 책임 | 소유 데이터 | 포트(로컬) |
|---|---|---|---|
| `gateway` | 인증 검증, 라우팅, rate limit, CORS | 없음(또는 라우팅 설정만) | 8080 |
| `user-service` | 회원가입, 로그인 발급 원천, 프로필 | users, credentials | 3001 |
| `order-service` | 주문 생성/조회/상태 | orders, order_items | 3002 |
| `catalog-service` | 상품/카테고리 | products, categories | 3003 |
| `notification-service` | 알림 발송 | notification_log | 3004 |

---

## 3. 데이터 소유권 (가장 중요)

### Database-per-service

```
user DB  ──── user-service만 R/W
order DB ──── order-service만 R/W
```

**금지:**
- order-service가 `users` 테이블을 직접 SELECT
- 크로스 DB JOIN

**허용:**
- order가 필요하면 `user_id`만 저장하고, 이름은 **캐시 스냅샷** 또는 **조회 API**
- `UserUpdated` 이벤트로 로컬 read model 갱신 (CQRS light)

### 분산 트랜잭션은 피한다

NestJS/Spring에서도 Saga를 바로 쓰지 않듯, 여기서도:

1. **로컬 트랜잭션**만 강하게 (한 DB 안)
2. 실패 가능 단계는 **이벤트 + 보상(compensation)** 또는 **Outbox**
3. 2PC / XA 는 거의 쓰지 않음

예시 — 주문 생성:

```
1. order-service: orders INSERT (status=PENDING)     ← 로컬 TX
2. outbox에 OrderCreated 기록                         ← 같은 TX
3. 백그라운드가 outbox → NATS/Kafka publish
4. catalog가 재고 차감 / notification이 메일
5. 실패 시 → OrderCancelled 보상 이벤트
```

상세는 [07_messaging](./07_messaging.md).

---

## 4. 동기 vs 비동기 호출 정책

| 상황 | 선택 | 이유 |
|---|---|---|
| 로그인 직후 프로필 필요 | 동기 HTTP/gRPC | 사용자 대기 |
| 주문 생성 후 알림 | 비동기 이벤트 | 실패해도 주문은 유지 가능 |
| 주문 목록에 상품명 표시 | 스냅샷 저장 or 캐시 | N+1 연쇄 호출 방지 |
| 결제 확정 | 동기 + 멱등키 | 돈은 확실해야 함 |

**규칙 of thumb**
- 요청 경로에 서비스 체인은 **최대 2 hop** (gateway → A → B)
- B 뒤에 C를 또 부르면 타임아웃·장애 전파가 폭발합니다

---

## 5. API 스타일

| 계층 | 권장 |
|---|---|
| 외부(클라이언트 ↔ gateway) | REST + JSON (OpenAPI/`utoipa`) |
| 내부(서비스 ↔ 서비스) | REST 시작 → 핫패스만 gRPC(`tonic`) |
| 이벤트 | CloudEvents 스타일 JSON 또는 protobuf |

처음부터 전부 gRPC로 갈 필요 없습니다.  
Axum REST로 경계를 고정한 뒤, 병목이 보이는 구간만 tonic으로 올립니다.

---

## 6. 설정 · 비밀 · 환경

서비스마다:

```
APP_ENV=local|staging|prod
HTTP_ADDR=0.0.0.0:3001
DATABASE_URL=postgres://...
REDIS_URL=redis://...
JWT_SECRET=...                 # gateway + auth 관련만
USER_SERVICE_URL=http://user:3001
NATS_URL=nats://nats:4222
```

- 코드에 시크릿 금지
- `common` 크레이트에 `Settings` 로더 공유 ([02](./02_workspace.md))
- prod는 시크릿 매니저 / K8s Secret

---

## 7. 실패 모드를 먼저 설계

MSA는 “연결이 된다”보다 **“끊겼을 때 어떻게 보이는가”**가 설계입니다.

| 장애 | 기대 동작 |
|---|---|
| catalog 다운 | 주문 생성 거절 or 재고 미검증 플래그(정책 선택) |
| notification 다운 | 주문 성공, 알림은 재시도 큐에 적재 |
| Redis 다운 | DB로 fallback (캐시 miss로 간주) or 명확한 503 |
| DB 다운 | 해당 서비스만 503, gateway는 부분 응답 가능 시 부분 성공 |

Circuit breaker / timeout / retry는 [08_observability](./08_observability.md)와 Tower 레이어로 붙입니다.

---

## 8. NestJS 모듈 사고방식 → Axum 서비스 사고방식

NestJS:

```
AppModule
  UserModule
  OrderModule
  → 같은 프로세스, 같은 DI 컨테이너
```

Axum MSA:

```
user-service binary
order-service binary
→ 네트워크로만 결합
→ 공유는 crates/common (타입·에러·JWT), 비즈니스 로직은 공유 금지
```

**공유 금지 예시:** `OrderService` 로직을 common에 넣기  
**공유 허용:** `ApiError`, `RequestId`, `JwtClaims`, `Money` newtype

---

## 체크포인트

```
[ ] 서비스 목록과 소유 테이블을 표로 적었다
[ ] 크로스 DB JOIN이 필요한 유스케이스가 없는지 검토했다
[ ] 동기 호출 체인 최대 깊이를 정했다 (권장 ≤ 2)
[ ] “알림/검색/감사로그”처럼 비동기 후보를 표시했다
[ ] 장애 시 UX/응답 정책을 한 줄씩 적었다
```

다음: [02_workspace — Cargo workspace로 모노레포 짜기](./02_workspace.md)
