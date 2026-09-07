# 09. Deploy — Docker Compose에서 운영 골격까지

로컬에서 **PG · Redis · NATS · 전 서비스**를 한 번에 올리고,  
이후 레지스트리/K8s로 확장하는 실전 체크리스트입니다.

---

## 1. 로컬 개발 두 가지 모드

### 모드 A — 인프라만 Docker (추천 · 디버깅 쉬움)

```bash
docker compose up -d postgres-user postgres-order redis nats
cargo run -p gateway
cargo run -p user-service
# ...
```

### 모드 B — 전부 Compose

이미지 빌드 후 통합 스모크 테스트·데모용.

---

## 2. `docker-compose.yml` 예시

```yaml
services:
  postgres-user:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: user
      POSTGRES_PASSWORD: user
      POSTGRES_DB: user_db
    ports: ["5432:5432"]
    volumes: [pg_user:/var/lib/postgresql/data]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U user -d user_db"]
      interval: 5s
      retries: 10

  postgres-order:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: order
      POSTGRES_PASSWORD: order
      POSTGRES_DB: order_db
    ports: ["5433:5432"]
    volumes: [pg_order:/var/lib/postgresql/data]
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U order -d order_db"]
      interval: 5s
      retries: 10

  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]
    command: ["redis-server", "--appendonly", "yes"]
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      retries: 10

  nats:
    image: nats:2.10
    command: ["-js", "-m", "8222"]
    ports: ["4222:4222", "8222:8222"]

  user-service:
    build:
      context: .
      dockerfile: services/user-service/Dockerfile
    environment:
      HTTP__ADDR: 0.0.0.0:3001
      DATABASE__URL: postgres://user:user@postgres-user:5432/user_db
      REDIS__URL: redis://redis:6379
      JWT_SECRET: dev-secret-change-me
      RUST_LOG: info
    depends_on:
      postgres-user:
        condition: service_healthy
      redis:
        condition: service_healthy
    expose: ["3001"]

  order-service:
    build:
      context: .
      dockerfile: services/order-service/Dockerfile
    environment:
      HTTP__ADDR: 0.0.0.0:3002
      DATABASE__URL: postgres://order:order@postgres-order:5432/order_db
      REDIS__URL: redis://redis:6379
      USER_SERVICE_URL: http://user-service:3001
      NATS_URL: nats://nats:4222
      RUST_LOG: info
    depends_on:
      postgres-order:
        condition: service_healthy
      nats:
        condition: service_started
    expose: ["3002"]

  gateway:
    build:
      context: .
      dockerfile: services/gateway/Dockerfile
    environment:
      HTTP__ADDR: 0.0.0.0:8080
      USER_SERVICE_URL: http://user-service:3001
      ORDER_SERVICE_URL: http://order-service:3002
      CATALOG_SERVICE_URL: http://catalog-service:3003
      REDIS__URL: redis://redis:6379
      JWT_SECRET: dev-secret-change-me
      RUST_LOG: info
    ports: ["8080:8080"]
    depends_on: [user-service, order-service]

volumes:
  pg_user:
  pg_order:
```

외부에 여는 포트는 **gateway(8080)** 와 개발용 DB/Redis 정도만.

---

## 3. 멀티스테이지 Dockerfile

```dockerfile
# services/user-service/Dockerfile
FROM rust:1.85-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# 의존성 캐시 트릭 (선택): 먼저 Cargo.toml만 복사
COPY Cargo.toml Cargo.toml
COPY crates ./crates
COPY services/user-service ./services/user-service
# workspace members를 맞추기 위해 다른 서비스 stub이 필요할 수 있음
# → 초보 팀은 context 전체를 COPY 해도 됨

COPY . .
RUN cargo build -p user-service --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/user-service /usr/local/bin/user-service
ENV RUST_LOG=info
EXPOSE 3001
CMD ["user-service"]
```

워크스페이스 전체를 매번 빌드하면 느립니다.  
실무에서는 **cargo-chef** 또는 **스크래치 의존성 레이어**로 캐시를 짭니다.

이미지 슬림: `cargo build --release`, 불필요 features 제거, 가능하면 `distroless`/`static` 검토.

---

## 4. `.env.example`

```bash
# gateway
HTTP__ADDR=0.0.0.0:8080
JWT_SECRET=change-me
JWT_ISSUER=platform
USER_SERVICE_URL=http://127.0.0.1:3001
ORDER_SERVICE_URL=http://127.0.0.1:3002
CATALOG_SERVICE_URL=http://127.0.0.1:3003
REDIS__URL=redis://127.0.0.1:6379

# user-service
DATABASE__URL=postgres://user:user@127.0.0.1:5432/user_db

# order-service
# DATABASE__URL=postgres://order:order@127.0.0.1:5433/order_db
# NATS_URL=nats://127.0.0.1:4222

RUST_LOG=info,tower_http=info,sqlx=warn
```

시크릿은 `.env`에만, gitignore 필수.

---

## 5. 마이그레이션 전략

| 환경 | 방법 |
|---|---|
| 로컬 | 앱 기동 시 `sqlx::migrate!` |
| CI | `sqlx migrate run` job |
| Prod | 배포 Job이 앱보다 먼저 migrate (권장) |

프로덕션에서 앱 인스턴스 N개가 동시에 migrate 하지 않게  
**한 번만 실행되는 migrate Job** 을 두는 편이 안전합니다.  
sqlx는 advisory lock을 쓰지만, 정책은 명확히.

---

## 6. 스모크 테스트 스크립트

```bash
#!/usr/bin/env bash
set -euo pipefail
BASE=http://127.0.0.1:8080

curl -sf "$BASE/health" >/dev/null

curl -sf -X POST "$BASE/v1/auth/register" \
  -H 'content-type: application/json' \
  -d '{"email":"a@b.com","name":"Ada","password":"password123"}'

TOKEN=$(curl -sf -X POST "$BASE/v1/auth/login" \
  -H 'content-type: application/json' \
  -d '{"email":"a@b.com","password":"password123"}' | jq -r .access_token)

curl -sf -X POST "$BASE/v1/orders" \
  -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -H "idempotency-key: $(uuidgen)" \
  -d '{"items":[{"product_id":"...","qty":1}]}'

echo "smoke ok"
```

---

## 7. 프로덕션으로 가는 체크리스트

```
[ ] 이미지 non-root USER
[ ] JWT 키 RS256 + 시크릿 매니저
[ ] DB TLS / Redis AUTH
[ ] 서비스는 ClusterIP, Gateway/Ingress만 외부
[ ] HPA: CPU/RPS 기반
[ ] PDB / 롤링 업데이트
[ ] 로그 수집 (JSON → Loki/ELK)
[ ] /metrics 스크rape
[ ] 백업: PG PITR, Redis AOF/백업 정책
[ ] 부하 테스트 (gateway p99 예산)
```

K8s 전환 시 각 서비스 = Deployment + Service + ConfigMap/Secret.  
Ingress가 Compose의 gateway 포트 역할을 일부 대체할 수 있으나,  
**앱 레벨 gateway(JWT/rate limit)** 는 여전히 유용합니다.

---

## 8. 구축 로드맵 (다시 한 번에)

```
Week 1  workspace + user-service + PG migrate + /health
Week 2  Redis 캐시 + JWT login + gateway 프록시
Week 3  order-service + UserClient + outbox
Week 4  NATS + notification 소비 + request-id/metrics
Week 5  docker compose 전체 + smoke + 부하 기초
이후    gRPC 핫패스, Kafka, K8s, 멀티리전…
```

Nest/Spring으로 이미 MSA를 굴려본 팀이면 Week 2~3이면  
“같은 모양의 플랫폼”이 Rust로 서 있습니다.  
병목은 언어보다 **경계·데이터소유·관측성**입니다.

---

## 관련 문서

- [readme](./readme.md) — 인덱스
- [01_architecture](./01_architecture.md) — 경계
- [04_database](./04_database.md) · [05_redis](./05_redis.md)
- [06_gateway_auth](./06_gateway_auth.md) · [07_messaging](./07_messaging.md)
- [08_observability](./08_observability.md)

이 리포의 Axum 송수신 예제: [../reqwest/01.md](../reqwest/01.md)
