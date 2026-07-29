# 00. 보강판 인덱스 — 01~09를 실제로 운영 가능하게 만들기

기존 [readme](./readme.md) ~ [09_deploy](./09_deploy.md)는 **MSA의 뼈대**를 세웁니다.
이 보강판(10~19)은 그 뼈대를 **프로덕션에 올렸을 때 실제로 터지는 것들**을 채웁니다.

기존 문서는 수정하지 않습니다. 이 문서들이 덧붙는 레이어입니다.

---

## 1. 왜 보강이 필요한가

01~09를 그대로 구현하면 이런 상태가 됩니다.

```
✅ 서비스 경계가 나뉘고 DB가 분리됨
✅ JWT 로그인이 되고 gateway가 프록시함
✅ 캐시·이벤트·트레이스가 흐름
❌ 배포할 때마다 진행 중 요청이 끊김        → 11_resilience
❌ x-user-id 헤더를 위조하면 남의 계정이 됨  → 10_errata / 12_security
❌ 트래픽이 몰리면 큐가 쌓이다 OOM           → 11_resilience
❌ 컬럼 하나 추가하다 테이블 전체가 잠김      → 13_data_evolution
❌ outbox 테이블이 무한 증식하고 재시도가 없음 → 14_messaging_ops
❌ Jaeger에 트레이스가 홉마다 끊겨서 보임      → 15_observability_deep
❌ CI가 없어서 회귀를 사람이 잡음             → 18_cicd
```

"Rust라서 빠르다"의 이득은 위 7개 중 **아무것도 해결해주지 않습니다.**

---

## 2. 문서 지도

| 번호 | 문서 | 무엇을 채우나 | 대응 기존 문서 |
|---|---|---|---|
| 10 | [10_errata](./10_errata.md) | 01~09 코드 예제의 **버그·보안 결함 정정** | 전부 |
| 11 | [11_resilience](./11_resilience.md) | graceful shutdown · 과부하 방어 · 런타임 튜닝 | 03, 09 |
| 12 | [12_security](./12_security.md) | 헤더 신뢰 경계 · 키 회전 · 토큰 폐기 · 감사 | 06 |
| 13 | [13_data_evolution](./13_data_evolution.md) | 무중단 스키마 변경 · 풀 튜닝 · PgBouncer | 04 |
| 14 | [14_messaging_ops](./14_messaging_ops.md) | outbox 워커 운영 · inbox · DLQ · 순서 | 07 |
| 15 | [15_observability_deep](./15_observability_deep.md) | traceparent 전파 · 카디널리티 · SLO | 08 |
| 16 | [16_api_contract](./16_api_contract.md) | utoipa/OpenAPI · 클라이언트 생성 · 계약 테스트 | 01, 07 |
| 17 | [17_testing](./17_testing.md) | 통합/계약/부하/장애주입 테스트 실물 | 03 |
| 18 | [18_cicd](./18_cicd.md) | clippy·deny·nextest·이미지·롤백 | 09 |
| 19 | [19_platform_decisions](./19_platform_decisions.md) | gateway 자작 여부 · 스케줄러 · 업로드 · DX | 01, 06, 09 |

---

## 3. 읽는 순서 (우선순위)

### 🔴 지금 당장 — 그대로 두면 사고

```
[ ] 10_errata 전체       (복붙하면 터지는 12개 항목)
[ ] 12_security §1~3     (헤더 스푸핑 · 에러 노출 · 시크릿 로그)
```

이 둘은 **기능 추가가 아니라 결함 제거**입니다. 다른 작업보다 먼저입니다.

### 🟠 1주차 — 배포가 안전해지는 최소치

```
[ ] 11_resilience §1~3   (graceful shutdown · 드레인 · 패닉 격리)
[ ] 13_data_evolution §1 (expand/contract 마이그레이션)
```

이거 없이 롤링 업데이트를 하면 **배포마다 5xx가 납니다.** 원인을 코드에서 찾다가 며칠 씁니다.

### 🟡 2~3주차 — 운영이 시작되면 바로 필요

```
[ ] 11_resilience §4~6   (동시성 제한 · 타임아웃 예산 · bulkhead)
[ ] 14_messaging_ops     (재시도 · DLQ · outbox 정리)
[ ] 15_observability_deep (트레이스가 실제로 이어지게)
[ ] 18_cicd              (사람이 잡던 회귀를 CI로)
```

### 🟢 안정화 이후

```
[ ] 16_api_contract  (서비스가 5개 넘어가면 필수)
[ ] 17_testing       (팀이 2명 넘어가면 필수)
[ ] 19_platform_decisions (기술부채가 쌓이기 전에 한 번 읽기)
```

---

## 4. 기존 로드맵에 끼워넣기

[09_deploy §8](./09_deploy.md)의 주차별 로드맵을 이렇게 조정합니다.

```
Week 1  workspace + user-service + PG migrate + /health
        + 10_errata 반영 (처음부터 올바른 코드로 시작)
        + 11_resilience §1~3 (graceful shutdown을 첫 서비스부터)

Week 2  Redis 캐시 + JWT login + gateway 프록시
        + 12_security 전체 (인증을 붙이는 그 주에 같이)

Week 3  order-service + UserClient + outbox
        + 11_resilience §4~6 (동기 호출이 생기는 순간)
        + 14_messaging_ops (outbox를 만드는 그 주에 같이)

Week 4  NATS + notification 소비 + request-id/metrics
        + 15_observability_deep

Week 5  docker compose 전체 + smoke + 부하 기초
        + 18_cicd + 17_testing

Week 6  (신설) 13_data_evolution + 16_api_contract
        스키마가 굳기 전에 진화 규칙을 정하는 주
```

**핵심 변경:** 회복탄력성과 보안을 "나중에"로 미루지 않고, **해당 기능을 만드는 바로 그 주에** 같이 넣습니다. 나중에 넣으면 전 서비스를 다시 훑어야 합니다.

---

## 5. 이 보강판의 원칙

1. **기존 문서를 부정하지 않습니다.** 01~09의 판단(sqlx, 4서비스 시작, HTTP 우선)은 옳습니다. 여기서는 그 위에 얹습니다.
2. **"실무에서는~"으로 끝난 문장을 코드로 만듭니다.** 기존 문서가 `// 실무에서는 스트리밍·재시도 필요` 로 남긴 자리들이 이 문서들의 목차입니다.
3. **왜 필요한지의 장애 시나리오를 먼저 씁니다.** 패턴 이름만 외우면 잘못된 곳에 적용합니다.
4. **NestJS/Spring 대응을 유지합니다.** 넘어온 사람이 "그건 프레임워크가 해주던 건데"를 빨리 찾도록.

---

## 6. NestJS/Spring이 공짜로 해주던 것 (= 여기서 직접 해야 하는 것)

기존 readme의 대응표에 빠진, **"Rust에는 대응물이 없어서 직접 만들어야 하는"** 목록입니다.

| NestJS / Spring | Axum | 어디서 |
|---|---|---|
| `app.enableShutdownHooks()` / Spring graceful shutdown | 없음 — 직접 | [11](./11_resilience.md) |
| Spring Actuator (`/health`, `/info`, `/env`) | 부분(`/health`만 관례) | [15](./15_observability_deep.md) |
| Nest `ValidationPipe` 전역 에러 포맷 | 없음 — 커스텀 extractor | [10](./10_errata.md) §10 |
| Spring `@Transactional` | 없음 — 수동 `tx` 전달 | [13](./13_data_evolution.md) |
| Nest exception filter의 5xx 마스킹 | 없음 — 직접 | [10](./10_errata.md) §3 |
| JVM 스레드풀 ↔ 컨테이너 CPU limit 인식 | tokio는 인식 못 함 | [11](./11_resilience.md) §7 |
| Spring Cloud Gateway 재시도/서킷브레이커 | 없음 | [11](./11_resilience.md) §5 |
| Flyway/Liquibase의 baseline·repair | sqlx는 최소한만 | [13](./13_data_evolution.md) |
| SpringDoc 자동 스펙 | `utoipa` 수동 어노테이션 | [16](./16_api_contract.md) |
| `@Scheduled` + ShedLock | 없음 | [19](./19_platform_decisions.md) §3 |

이 표가 이 보강판의 존재 이유를 가장 잘 요약합니다.
**Rust는 프레임워크가 대신 해주던 운영 기능을 돌려주지 않습니다.** 대신 명시적으로 만들 수 있게 해줍니다.

---

## 7. 버전 기준

이 보강판의 코드는 아래 버전에서 API를 확인했습니다.

```toml
axum          = "0.8"    # 0.8.9 기준 검증
axum-core     = "0.5"
tower-http    = "0.6"    # 0.6.11 기준 검증
tower         = "0.5"
```

기존 문서의 일부 버전은 한 세대 뒤쳐져 있습니다 ([10_errata §12](./10_errata.md) 참고).
OpenTelemetry 계열은 특히 `opentelemetry` ↔ `tracing-opentelemetry` ↔ `opentelemetry-otlp` 버전이
서로 맞물려야 하므로, [15_observability_deep](./15_observability_deep.md)의 조합표를 쓰세요.

---

다음: [10_errata — 기존 예제의 정정 목록](./10_errata.md)
