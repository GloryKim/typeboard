# Go(Golang) vs TypeScript(Bun) 성능 벤치마크 요약 보고서

## 1. 개요

- **원문 출처**
  - 영상: [Go (Golang) vs TypeScript: Performance Benchmark](https://www.youtube.com/watch?v=3-XHVFVX1io) (Anton Putra)
  - 코드: [antonputra/tutorials `lessons/275`](https://github.com/antonputra/tutorials/tree/275/lessons/275)
- **핵심 질문**: 백엔드를 프런트와 같은 TypeScript(Bun 런타임)로 계속 갈 것인가, 아니면 서버 사이드에 최적화된 Go로 전환할 것인가.
- **비교 대상**
  - Go: `Fiber v3` 프레임워크 (`go 1.25.5`)
  - TypeScript: `Bun 1.3.5` 런타임 (Zig 기반, 2021년 등장)
- **결론 요약**: 정적(static) 테스트에서는 두 진영이 거의 대등(오히려 Bun이 근소 우위). 그러나 **DB가 개입되는 현실적 워크로드에서는 Go가 처리량·지연시간·안정성 모두에서 명확히 우세**. 다만 과거 대비 격차는 줄어드는 추세.

### 참고: JavaScript 서버 런타임의 흐름 (영상 배경 설명)

- 2009년 **Node.js** 출시 — 최초로 널리 채택된 JavaScript 서버 런타임.
- 2018년 **Deno** 발표 — 보안 강화, TypeScript 네이티브 지원 등 개선.
- 2021년 **Bun** 출시 — 성능 향상에 특화, Zig(저수준 언어)로 작성되어 정밀한 메모리 관리와 고속 처리 지향.
- 반대편에는 서버 사이드용으로 설계되고 네트워킹에 최적화된 **Go** 언어가 존재.

---

## 2. 테스트 환경 (재현 가능한 실제 수치)

| 항목 | 값 | 근거 파일 |
|---|---|---|
| 클러스터 | AWS EKS 관리형 Kubernetes | 영상 |
| 앱 인스턴스 수 | `replicas: 2` (각 파드 CPU 1코어 제한) | `deploy/*/1-deployment.yaml` |
| 파드 리소스 | requests/limits: `memory 3Gi`, cpu `750m`(req)/`1000m`(limit) | `deploy/*/1-deployment.yaml` |
| Go 동시성 설정 | `GOMAXPROCS`를 `limits.cpu`(=1)에서 주입 | `deploy/go-app/1-deployment.yaml` |
| Bun 환경 | `NODE_ENV=production` | `deploy/bun-app/1-deployment.yaml` |
| 부하 클라이언트 | `parallelism: 40` (독립 파드 40개), 전용 `clients` 노드에 배치 | `tests/*/go-client.yaml` |
| 클라이언트 리소스 | `memory 256Mi`, `cpu 1000m` | `tests/*/*-client.yaml` |
| 부하 도구 | `quay.io/aputra/load-tester:v26` | `tests/*/*-client.yaml` |
| DB | `postgres:18.1`, 별도 스토리지 최적화 EC2에 배치 | `compose.yaml`, `migration/*` |
| DB 커넥션 한도 | 앱당 `maxConnections: 250` (2인스턴스 → **총 500**) | `deploy/*/0-config.yaml` |

### 배포 세부 설정 (deployment 매니페스트)

- 배포 전략: `strategy.type: Recreate`, `terminationGracePeriodSeconds: 0`
- `readinessProbe` / `livenessProbe`: `GET /healthz`
- 스케줄링:
  - `podAffinity`(requiredDuringScheduling)로 같은 앱 파드를 `kubernetes.io/hostname` 기준 배치
  - `nodeAffinity`로 `node=general` + `kubernetes.io/arch=amd64` 노드에 배치
  - `tolerations`: 모든 taint 허용(`operator: Exists`, `effect: NoSchedule`)
- 이미지 태그: Go `aputra/go-app-275:v3`, Bun `aputra/bun-app-275:v3`
- 네임스페이스: `benchmark` (`monitoring: prometheus` 라벨) — `deploy/namespace.yaml`
- 모니터링: 앱/클라이언트별 `PodMonitor` 구성(`3-pod-monitor.yaml`, `4-client-pod-monitor.yaml`)

### 부하 프로파일 (`Tester.toml`)

- **Test 1 (정적 GET)** — `tests/1-test/config.yaml`
  - `request = "get"`, `protocol = "http1"`
  - `min_clients = 1` → `max_clients = 1000`
  - `stage_interval_s = 15`, `request_delay_ms = 40`, `request_timeout_ms = 1000`
- **Test 2 (POST + DB)** — `tests/2-test/config.yaml`
  - `request = "post"`, `protocol = "http1"`
  - 동일 범위(1 → 1000), 단 `stage_interval_s = 30` (단계 간격을 늘려 DB 워크로드 안정화)

즉, 클라이언트를 1개에서 1000개까지 단계적으로 늘려 **양쪽 앱이 실패(에러)를 내기 시작할 때까지** 밀어붙이는 방식.

---

## 3. 애플리케이션 구현 비교

두 앱은 기능적으로 동일한 엔드포인트를 제공하여 공정 비교를 지향.

| 엔드포인트 | 동작 |
|---|---|
| `GET /api/users` | 고정된 유저 3명 JSON 반환 (정적) |
| `POST /api/users` | 요청 바디를 PostgreSQL에 INSERT 후 생성된 `id` 반환 |
| `GET /healthz` | 헬스체크(200) |

- **Go**: `Fiber v3` + `pgx/v5` 커넥션 풀(`pgxpool`). 저장 시 `INSERT ... RETURNING id` 사용 (`go-app/server.go`, `go-app/user.go`).
  - 저장 시 이미지 키를 `user-go-<UnixMilli>.png` 형식으로 생성.
  - 주요 의존성: `github.com/gofiber/fiber/v3 v3.0.0-rc.3`, `github.com/jackc/pgx/v5 v5.7.6`, `github.com/antonputra/go-utils v0.1.6`.
- **Bun**: `Bun.serve`(`idleTimeout: 60`, `development: false`, `reusePort: true`) + Bun 내장 `SQL` 클라이언트(`bun-app/src/app.ts`, `db.ts`, `users.ts`).
  - 저장 시 이미지 키를 `user-bun-<getTime()>.png` 형식으로 생성.
  - `idleTimeout: 60`은 Node.js 기본 타임아웃과 맞추기 위한 설정(코드 주석 명시).
  - 설정 로딩에 `js-yaml` 사용(`config.yaml` 파싱).
- **빌드 방식(중요한 공정성 요소)**
  - Go: `scratch` 이미지로 정적 바이너리 (`CGO_ENABLED=0`, `GOARCH=amd64`, `GOOS=linux`, `GOAMD64=v2`, `-trimpath`, `-ldflags="-s -w -buildid="`) — `go-app/Dockerfile`
  - Bun: `bun build --compile --minify --sourcemap`로 단일 실행 바이너리 생성, 최종 런타임은 `oven/bun:1.3.5-slim` — `bun-app/Dockerfile`, `package.json`
- DB 스키마는 앱별로 분리(`go_app`, `bun_app` 테이블)하여 상호 간섭 제거 (`migration/0-sql.yaml`).
  - 마이그레이션 시 유휴 커넥션 정리(`pg_terminate_backend`), 테이블 DROP/CREATE 후 `VACUUM full` 수행.
  - 테이블 컬럼: `id(SERIAL PK)`, `name`, `address`, `phone`, `image`, `created_at`, `updated_at`.
  - 마이그레이션은 `postgres:18.1` 이미지로 `psql -a -f /init.sql` 실행(Job).

---

## 4. 실험 결과

### 4-1. Test 1 — 정적 HTTP GET (DB 없음)

| 지표 | 결과 |
|---|---|
| 처리량(RPS) | 거의 동등, **Bun이 근소 우위**. Bun은 2-CPU 인스턴스에서 **약 200,000 RPS** 도달(제작자가 "지금까지 본 최고 수준" 평가) |
| 지연시간(Latency) | 매우 유사 |
| CPU 사용량 | 매우 유사 |
| 메모리 사용량 | **Go가 부하 증가에 따라 메모리가 상승**하는 특이 패턴 관찰 |

> Test 1 종합: 순수 정적 응답에서는 Go와 Bun의 성능 프로파일이 사실상 대등하며, 처리량 피크는 오히려 Bun이 약간 높음.

### 4-2. Test 2 — POST + PostgreSQL 저장 (현실적 워크로드)

| 지표 | 결과 |
|---|---|
| 처리량(RPS) | **Go가 명확히 우세하고 더 안정적**, 약 **84,000 RPS** 도달 |
| 지연시간(Latency) | **Go가 현저히 우수** |
| DB 커넥션 풀 | **Bun은 시작 즉시 한도(500) 전부 생성**, **Go는 부하에 따라 점진적으로 확장** |
| Postgres CPU / 앱 CPU / 메모리 | 함께 계측(그래프 제시) |

> Test 2 종합: DB나 다른 마이크로서비스와 상호작용하는 실제 운영 시나리오에서는 Go가 처리량·지연시간·리소스 관리(특히 동적 커넥션 풀링)에서 우위. 단, "TypeScript 런타임과 서버 사이드 언어의 격차는 확실히 줄고 있다"는 것이 제작자의 총평.

### 4-3. 테스트 진행 방식(영상)

- 전체 테스트는 약 2시간 소요되었으며 영상에서는 1분으로 압축.
- DB용으로 스토리지 최적화 EC2 2대를 추가 프로비저닝(병목 회피 목적, 비용은 다소 상승).
- 제작자는 실제 클라이언트 대상 서비스와 동일한 프로덕션 셋업으로 AWS에서 테스트 수행.

---

## 5. 해석 및 시사점

1. **워크로드 의존성이 큼**: "무엇이 더 빠른가"는 단정할 수 없고, 순수 I/O·정적 응답이면 Bun도 충분히 경쟁력 있음(피크 약 200k RPS).
2. **DB 개입 시 Go 우위**: 84k RPS 안정 유지 + 낮은 지연시간 + 부하 기반 동적 커넥션 확장은 실서비스에서 중요한 차이.
3. **커넥션 풀 전략 차이**: Bun의 즉시 최대 커넥션 점유 방식은 DB 측 부담·자원 낭비 리스크가 될 수 있음. Go의 점진적 확장이 더 예측 가능.
4. **메모리 관찰 포인트**: 정적 테스트에서 Go 메모리가 부하와 함께 증가한 점은 추가 프로파일링이 필요한 흥미로운 예외.
5. **선택 가이드**
   - 풀스택 생산성/단일 언어 유지가 최우선이고 트래픽이 I/O 위주라면 → Bun(TypeScript) 합리적.
   - 고부하·DB 집약·낮은 지연시간이 핵심인 프로덕션 API라면 → Go가 여전히 우세.

---

## 6. 벤치마크의 한계 (해석 시 유의)

- 단일 프레임워크 조합(Fiber v3 vs Bun 내장 서버)의 결과이며 다른 프레임워크·ORM에서는 달라질 수 있음.
- `stage_interval` 등 부하 파라미터가 Test 1(15s)과 Test 2(30s)에서 달라 두 테스트를 직접 비교하기보다 각 테스트 내 상대 비교로 볼 것.
- 제작자 자체 벤치마크로, 하드웨어/버전 의존성이 크며 절대 수치는 환경별로 재현 필요.

---

## 부록 A. `lessons/275` 저장소 구조

```
lessons/275
├─ README.md                 # "Go (Golang) vs TypeScript: Performance Benchmark"
├─ compose.yaml              # 로컬 개발용 postgres:18.1 (5432)
├─ go-app/                   # Go(Fiber v3) 앱
│  ├─ Dockerfile             # scratch 정적 바이너리 빌드
│  ├─ main.go / server.go / db.go / user.go / config.go
│  ├─ config.yaml
│  └─ go.mod / go.sum
├─ bun-app/                  # Bun(TypeScript) 앱
│  ├─ Dockerfile             # bun build --compile 후 slim 런타임
│  ├─ src/app.ts / db.ts / users.ts / config.ts / types.ts
│  ├─ package.json / bun.lock / tsconfig.json
│  └─ config.yaml
├─ deploy/                   # Kubernetes 매니페스트
│  ├─ namespace.yaml
│  ├─ go-app/  (0-config, 1-deployment, 2-service, 3/4-pod-monitor)
│  └─ bun-app/ (0-config, 1-deployment, 2-service, 3/4-pod-monitor)
├─ migration/               # DB 초기화/마이그레이션 Job
│  ├─ 0-sql.yaml
│  ├─ 1-sql-bun-migration.yaml
│  └─ 2-sql-go-migration.yaml
└─ tests/                    # 부하 테스트
   ├─ 1-test/ (config.yaml=GET, go-client, bun-client, tls)
   └─ 2-test/ (config.yaml=POST, go-client, bun-client, tls)
```

## 부록 B. 참고 링크

- 영상: https://www.youtube.com/watch?v=3-XHVFVX1io
- 코드: https://github.com/antonputra/tutorials/tree/275/lessons/275
- 제작자 사이트: https://antonputra.com/
- 결과 요약(외부 정리): https://www.xugj520.cn/en/archives/go-typescript-backend-benchmark-2026.html
