# 18. CI/CD — 회귀를 사람이 잡지 않게

[09_deploy](./09_deploy.md)는 **Docker Compose로 띄우는 법**까지 다룹니다.
그 다음 단계 — **코드가 어떻게 검증되고 어떻게 프로덕션에 도달하는가** — 가 통째로 비어 있습니다.

"Spring Boot급 MSA"를 자칭하려면 이 문서의 내용이 자동화돼 있어야 합니다.

---

## 1. 파이프라인 전경

```
PR 열림
  ├─ 정적 검사      fmt · clippy · deny · 시크릿 스캔      (~1분)
  ├─ 빌드/유닛      cargo nextest --lib                     (~3분)
  ├─ 통합           PG/Redis 컨테이너 + nextest             (~5분)
  ├─ 계약           소비자 계약 검증 (16 §6)                 (~1분)
  ├─ 스펙           openapi.json 최신성 + breaking 탐지      (~1분)
  └─ 마이그레이션    린트 + idempotent 검사                   (~1분)
       ↓ 전부 통과해야 머지 가능

main 머지
  ├─ 이미지 빌드 + SBOM + 취약점 스캔
  ├─ 레지스트리 푸시 (불변 태그)
  └─ 스테이징 자동 배포 → 스모크 → 카오스

수동 승인
  └─ 프로덕션 배포 (migrate Job → 카나리 → 전체) → 배포 마커
```

**PR 단계 전체가 10분을 넘지 않게 하세요.** 넘어가면 사람이 기다리지 않고 다른 일을 하다가 맥락을 잃습니다.

---

## 2. Rust 빌드 캐싱

Rust CI에서 가장 큰 비용은 컴파일입니다. 캐싱이 없으면 워크스페이스 전체 빌드에 10분 이상 갑니다.

```yaml
name: CI
on:
  pull_request:
  push: { branches: [main] }

env:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0        # CI에서는 증분 컴파일이 오히려 느리고 캐시를 키움
  RUSTFLAGS: "-D warnings"    # 경고를 에러로

jobs:
  static:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2       # target/ 와 ~/.cargo 캐싱
        with: { shared-key: "ci" }

      - name: Format
        run: cargo fmt --all --check

      - name: Clippy
        run: cargo clippy --workspace --all-targets --all-features -- -D warnings
```

`Swatinem/rust-cache`가 사실상 표준입니다. `shared-key`를 잡에서 공유하면 재사용률이 올라갑니다.

### sccache (선택)

여러 리포/러너가 있으면 분산 캐시가 더 효과적입니다.

```yaml
env:
  RUSTC_WRAPPER: sccache
  SCCACHE_GHA_ENABLED: "true"
```

---

## 3. 정적 검사

### 3-1. clippy를 실제 게이트로

```toml
# 루트 Cargo.toml — 워크스페이스 전체 린트 정책
[workspace.lints.rust]
unsafe_code = "forbid"
missing_debug_implementations = "warn"

[workspace.lints.clippy]
# 10_errata §12-8 — 요청 경로의 패닉 차단
unwrap_used = "deny"
expect_used = "warn"          # 초기화 코드에서는 허용하되 눈에 띄게
panic = "deny"
todo = "deny"
unimplemented = "deny"

# 실수하기 쉬운 것들
dbg_macro = "deny"
print_stdout = "warn"          # 로깅은 tracing으로
float_cmp = "deny"
integer_division = "warn"
indexing_slicing = "warn"      # slice[i] 대신 .get(i)
```

```toml
# 각 크레이트 Cargo.toml
[lints]
workspace = true
```

정당한 예외는 지역적으로 허용하되 **이유를 적게** 합니다.

```rust
#[allow(clippy::unwrap_used, reason = "정규식 리터럴은 컴파일 타임에 검증됨")]
static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d+$").unwrap());
```

### 3-2. cargo-deny

```toml
# deny.toml
[advisories]
db-path = "~/.cargo/advisory-db"
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "deny"
ignore = [
    # 예외는 반드시 만료일과 이유를 함께
    # { id = "RUSTSEC-2024-XXXX", reason = "미사용 경로, 2026-09-01까지 업스트림 대기" },
]

[licenses]
version = 2
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib"]
confidence-threshold = 0.9

[bans]
multiple-versions = "warn"     # 중복 버전 감지 (15 §2-1의 OTel 문제)
wildcards = "deny"             # "*" 버전 금지
deny = [
    { name = "openssl", reason = "rustls를 사용합니다" },
]
skip-tree = [
    # 빌드 전용 의존성의 중복은 무시
]

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

```yaml
      - uses: EmbarkStudios/cargo-deny-action@v2
        with: { command: check }
```

**`[bans] deny = openssl`이 실용적입니다.** [02 §2](./02_workspace.md)가 reqwest를
`rustls-tls`로 설정해뒀는데, 새 의존성이 openssl을 끌어오면 Docker 빌드에
`libssl-dev`가 필요해지고 이미지가 커집니다. 이 규칙이 그걸 막습니다.

### 3-3. 시크릿 스캔

```yaml
      - name: Secret scan
        uses: gitleaks/gitleaks-action@v2
        env: { GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
```

pre-commit 훅에도 넣으세요. **커밋된 시크릿은 히스토리에서 지워도 이미 유출된 것**으로 취급하고 회전해야 합니다.

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/gitleaks/gitleaks
    rev: v8.21.0
    hooks: [{ id: gitleaks }]
```

### 3-4. 프로덕션 기본값 검사

[10_errata §11](./10_errata.md)에서 다룬 "개발용 시크릿이 프로덕션까지 가는" 사고를 CI에서도 막습니다.

```yaml
      - name: No dev secrets in manifests
        run: |
          if grep -rn "change-me\|dev-secret\|password123" \
               --include="*.yaml" --include="*.yml" \
               --exclude-dir=".git" k8s/ helm/ 2>/dev/null; then
            echo "::error::프로덕션 매니페스트에 개발용 기본값이 있습니다"
            exit 1
          fi
```

---

## 4. 재현 가능한 빌드

### 4-1. `Cargo.lock`

[09 §3](./09_deploy.md)의 Dockerfile은 `Cargo.lock`을 복사하지 않습니다.
**바이너리 워크스페이스에서는 반드시 커밋하고 강제하세요.**

```bash
cargo build --locked --release    # lock과 다르면 실패
cargo test --locked
```

```yaml
      - name: Cargo.lock is committed and current
        run: |
          cargo metadata --locked --format-version 1 > /dev/null \
            || (echo "::error::Cargo.lock이 최신이 아닙니다. cargo update 후 커밋하세요." && exit 1)
```

없으면 **어제 성공한 빌드가 오늘 실패**합니다. 의존성의 패치 버전이 올라갔기 때문인데,
원인 파악에 몇 시간이 갑니다.

### 4-2. 툴체인 고정

```toml
# rust-toolchain.toml  ([02 §1](./02_workspace.md)이 "(선택)"이라 했지만 필수에 가깝습니다)
[toolchain]
channel = "1.85.0"       # 정확한 버전. "stable"은 어느 날 갑자기 바뀝니다
components = ["rustfmt", "clippy"]
profile = "minimal"
```

**로컬·CI·Docker가 같은 버전을 쓰게 됩니다.** Dockerfile의 `FROM rust:1.85`와 일치시키세요.

### 4-3. sqlx 오프라인 캐시

```yaml
      - name: sqlx offline cache is current
        run: |
          cargo sqlx prepare --workspace --check -- --all-targets
        env:
          DATABASE_URL: postgres://test:test@localhost:5432/test
```

이게 없으면 **"로컬은 되는데 CI가 깨진다"** 가 반복됩니다.
[13 §8-3](./13_data_evolution.md) 참고.

---

## 5. 테스트 잡

```yaml
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_USER: test
          POSTGRES_PASSWORD: test
          POSTGRES_DB: test
        ports: ["5432:5432"]
        options: >-
          --health-cmd "pg_isready -U test"
          --health-interval 5s --health-timeout 5s --health-retries 10
      redis:
        image: redis:7-alpine
        ports: ["6379:6379"]
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 5s --health-retries 10

    env:
      DATABASE_URL: postgres://test:test@localhost:5432/test
      REDIS_URL: redis://localhost:6379
      SQLX_OFFLINE: "true"

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with: { shared-key: "ci" }
      - uses: taiki-e/install-action@nextest

      - name: Test
        run: cargo nextest run --workspace --profile ci --locked

      - name: Migration lint
        run: cargo test --workspace --locked migration_

      - name: Consumer contracts        # 16 §6
        run: cargo nextest run --workspace --test contracts

      - name: OpenAPI spec is current   # 16 §3-2
        run: |
          cargo run --bin dump-openapi --locked
          git diff --exit-code '**/openapi.json' \
            || (echo "::error::스펙이 변경됨. dump-openapi 실행 후 커밋하세요." && exit 1)

      - uses: actions/upload-artifact@v4
        if: failure()
        with: { name: test-results, path: target/nextest/ci/ }
```

`--profile ci`가 [17 §9-1](./17_testing.md)의 `.config/nextest.toml` 설정(재시도, 타임아웃, fail-fast off)을 적용합니다.

---

## 6. Docker 빌드

### 6-1. cargo-chef로 의존성 캐싱

[09 §3](./09_deploy.md)이 *"실무에서는 cargo-chef"* 라고만 하고 넘어간 부분입니다.
워크스페이스 전체를 매번 빌드하면 5~10분, chef를 쓰면 코드만 바뀌었을 때 1분 이하가 됩니다.

```dockerfile
# services/user-service/Dockerfile
ARG RUST_VERSION=1.85

# ── 1. 레시피: 의존성 목록만 추출 ──────────────────────────
FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── 2. 의존성 빌드: 소스가 바뀌어도 이 레이어는 캐시됨 ──────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# ── 3. 애플리케이션 빌드 ───────────────────────────────────
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked -p user-service \
 && strip target/release/user-service

# ── 4. 런타임 ─────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=builder /app/target/release/user-service /usr/local/bin/user-service
USER nonroot
EXPOSE 3001
ENTRYPOINT ["/usr/local/bin/user-service"]
```

**핵심: `recipe.json`이 의존성 목록만 담으므로, 애플리케이션 코드가 바뀌어도 2단계가 캐시됩니다.**
`Cargo.toml`이 바뀔 때만 의존성을 다시 빌드합니다.

`SQLX_OFFLINE=true`가 필요합니다 — 빌드 컨테이너에서 DB에 접속할 수 없으니까요.

### 6-2. distroless 사용 시 주의

| 항목 | 영향 |
|---|---|
| 셸 없음 | `docker exec sh` 불가 → `kubectl debug`의 ephemeral container 사용 |
| `curl` 없음 | Dockerfile `HEALTHCHECK` 불가 → K8s probe로 대체 |
| CA 인증서 | `cc-debian12`에 포함됨 ✅ |
| 타임존 데이터 | 없음 → UTC만 쓰거나 `chrono-tz` 번들 |

타임존이 필요하면:

```dockerfile
COPY --from=builder /usr/share/zoneinfo /usr/share/zoneinfo
```

**애초에 앱 내부는 UTC로만 다루세요** ([16 §9-2](./16_api_contract.md)). 시간대 변환은 표현 계층에서.

### 6-3. 멀티 서비스 빌드

서비스마다 Dockerfile을 복사하지 말고 인자화하세요.

```dockerfile
ARG SERVICE
RUN cargo build --release --locked -p ${SERVICE}
RUN cp target/release/${SERVICE} /app/service    # 고정 경로로
...
COPY --from=builder /app/service /usr/local/bin/service
ENTRYPOINT ["/usr/local/bin/service"]
```

```yaml
  build:
    strategy:
      matrix:
        service: [gateway, user-service, order-service, catalog-service, notification-service]
    steps:
      - uses: docker/build-push-action@v6
        with:
          build-args: SERVICE=${{ matrix.service }}
          cache-from: type=gha,scope=${{ matrix.service }}
          cache-to: type=gha,mode=max,scope=${{ matrix.service }}
```

### 6-4. 변경된 서비스만 빌드

워크스페이스 전체를 매번 빌드하면 CI 시간이 서비스 수에 비례해 늘어납니다.

```yaml
      - uses: dorny/paths-filter@v3
        id: changes
        with:
          filters: |
            common: ['crates/**', 'Cargo.lock', 'Cargo.toml']
            user: ['services/user-service/**']
            order: ['services/order-service/**']
```

`crates/common`이 바뀌면 **전부** 빌드해야 합니다. 그렇지 않으면 변경된 것만.

---

## 7. 이미지 태깅과 서명

### 7-1. 불변 태그

```yaml
      - uses: docker/metadata-action@v5
        id: meta
        with:
          images: ghcr.io/${{ github.repository }}/${{ matrix.service }}
          tags: |
            type=sha,format=long,prefix=       # 커밋 SHA — 주 식별자
            type=ref,event=pr,prefix=pr-
            type=semver,pattern={{version}}
            type=raw,value=latest,enable={{is_default_branch}}
```

**배포에는 `latest`가 아니라 SHA 태그를 쓰세요.**

```
latest 로 배포하면:
- 지금 무엇이 돌고 있는지 모름
- 롤백 대상이 불명확
- 파드 재시작 시 다른 이미지가 뜰 수 있음
```

`latest`는 사람이 `docker pull`할 때의 편의용입니다.

### 7-2. SBOM과 취약점 스캔

```yaml
      - name: Generate SBOM
        uses: anchore/sbom-action@v0
        with:
          image: ${{ steps.meta.outputs.tags }}
          format: cyclonedx-json

      - name: Vulnerability scan
        uses: aquasecurity/trivy-action@master
        with:
          image-ref: ${{ steps.meta.outputs.tags }}
          format: sarif
          output: trivy.sarif
          severity: CRITICAL,HIGH
          exit-code: '1'
          ignore-unfixed: true       # 수정본이 없으면 막지 않음

      - uses: github/codeql-action/upload-sarif@v3
        if: always()
        with: { sarif_file: trivy.sarif }
```

`ignore-unfixed: true`가 실용적입니다. **수정할 수 없는 취약점으로 배포를 막으면
팀이 스캔 자체를 꺼버립니다.**

distroless 기반이면 OS 패키지가 거의 없어 스캔 결과가 매우 깨끗합니다.
Rust 의존성 취약점은 `cargo deny`(§3-2)가 담당합니다.

### 7-3. 이미지 서명 (선택)

```yaml
      - uses: sigstore/cosign-installer@v3
      - name: Sign
        run: cosign sign --yes ${{ steps.meta.outputs.tags }}
```

클러스터에서 서명된 이미지만 허용하도록 정책(Kyverno/Gatekeeper)을 걸면
**공급망 공격에 대한 방어**가 됩니다. 규제 환경이 아니면 나중에 해도 됩니다.

---

## 8. 배포

### 8-1. 순서

[13 §7](./13_data_evolution.md)의 결론을 파이프라인으로 옮깁니다.

```
1. migrate Job 실행 → 완료 대기
2. 새 이미지로 Deployment 업데이트
3. 롤아웃 상태 감시 (타임아웃 설정)
4. 스모크 테스트
5. 실패 시 자동 롤백
6. 성공 시 배포 마커 전송
```

```yaml
  deploy-staging:
    needs: [build]
    environment: staging
    steps:
      - name: Run migrations
        run: |
          kubectl apply -f k8s/migrate-job.yaml
          kubectl wait --for=condition=complete --timeout=600s job/migrate-${{ github.sha }}

      - name: Deploy
        run: |
          kubectl set image deployment/user-service \
            user-service=ghcr.io/${{ github.repository }}/user-service:${{ github.sha }}

      - name: Wait for rollout
        run: kubectl rollout status deployment/user-service --timeout=300s

      - name: Smoke test          # 09 §6의 스크립트
        run: ./scripts/smoke.sh https://staging.example.com

      - name: Rollback on failure
        if: failure()
        run: |
          kubectl rollout undo deployment/user-service
          kubectl rollout status deployment/user-service --timeout=300s

      - name: Deployment marker   # 15 §10-2
        if: success()
        run: |
          curl -sf -X POST "$GRAFANA_URL/api/annotations" \
            -H "Authorization: Bearer $GRAFANA_TOKEN" \
            -H 'content-type: application/json' \
            -d "{\"tags\":[\"deploy\",\"user-service\",\"staging\"],
                 \"text\":\"${{ github.sha }} by ${{ github.actor }}\"}"
```

**배포 마커를 빠뜨리지 마세요.** 장애 대응에서 "이게 배포 직후 시작됐나?"에
즉시 답할 수 있는 것이 평균 복구 시간을 크게 줄입니다.

### 8-2. 마이그레이션 Job의 멱등성

`kubectl apply`로 같은 이름의 Job을 다시 만들면 실패합니다.
**Job 이름에 SHA를 넣고, 오래된 Job은 TTL로 정리**하세요.

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: migrate-{{ .Values.image.tag }}
spec:
  ttlSecondsAfterFinished: 86400
  backoffLimit: 2
```

### 8-3. 카나리 배포

프로덕션에서는 전량 교체 전에 일부만 노출합니다.

```yaml
# Argo Rollouts
spec:
  strategy:
    canary:
      steps:
        - setWeight: 5
        - pause: { duration: 5m }      # 지표 관찰
        - analysis:
            templates: [{ templateName: error-rate }]
        - setWeight: 25
        - pause: { duration: 10m }
        - setWeight: 100
```

```yaml
# 자동 판정
apiVersion: argoproj.io/v1alpha1
kind: AnalysisTemplate
metadata: { name: error-rate }
spec:
  metrics:
    - name: error-rate
      interval: 1m
      count: 5
      successCondition: result[0] < 0.01
      failureLimit: 1
      provider:
        prometheus:
          address: http://prometheus:9090
          query: |
            sum(rate(http_requests_total{service="user-service",status=~"5.."}[2m]))
            / sum(rate(http_requests_total{service="user-service"}[2m]))
```

**[15](./15_observability_deep.md)의 메트릭이 여기서 실제 값을 합니다.**
관측성 없이는 카나리를 자동 판정할 수 없습니다. 두 문서가 이어지는 지점입니다.

### 8-4. 롤백 가능성

```
❌ 롤백 불가:  코드 롤백 + 스키마는 contract 완료됨
✅ 롤백 가능:  코드 롤백 + 스키마는 expand 상태 유지
```

[13 §7-3](./13_data_evolution.md)의 결론입니다.
**"이 배포는 롤백 가능한가?"** 를 PR 템플릿에 넣으세요.

```markdown
## 배포 안전성
- [ ] 스키마 변경이 옛 코드와 호환됨 (또는 스키마 변경 없음)
- [ ] contract(삭제) 단계라면 expand가 ___ 배포 전에 나갔음
- [ ] 이 배포를 되돌리려면: (절차)
```

---

## 9. 환경 관리

### 9-1. 이미지는 하나, 설정만 다르게

```
❌ user-service:staging / user-service:prod    (다른 이미지 = 다른 코드)
✅ user-service:abc123 + 환경별 ConfigMap/Secret
```

스테이징에서 검증한 **바로 그 바이너리**가 프로덕션에 가야 합니다.

### 9-2. GitOps

```
platform-config/          (별도 리포)
├── base/
│   └── user-service/
│       ├── deployment.yaml
│       └── service.yaml
└── overlays/
    ├── staging/
    │   ├── kustomization.yaml
    │   └── replicas.yaml
    └── production/
        ├── kustomization.yaml
        ├── replicas.yaml
        └── resources.yaml
```

CI는 오버레이의 이미지 태그만 갱신하고, ArgoCD/Flux가 동기화합니다.

| 이점 | 설명 |
|---|---|
| 배포 이력 = git 로그 | 누가 언제 무엇을 배포했는지 명확 |
| 롤백 = git revert | 절차가 단순 |
| 드리프트 감지 | 수동 `kubectl edit`이 자동 되돌려짐 |
| 클러스터 크레덴셜 불필요 | CI가 클러스터에 접근할 필요 없음 |

마지막이 보안상 중요합니다. **CI 토큰이 유출돼도 클러스터가 안전합니다.**

### 9-3. 환경별 설정 차이 최소화

```yaml
# 다른 것 (필수)
replicas, resources, DATABASE_URL, 도메인, 시크릿

# 같아야 하는 것
로그 레벨 구조, 타임아웃 값, 기능 플래그 기본값, 미들웨어 구성
```

**스테이징과 프로덕션의 설정이 크게 다르면 스테이징 검증이 무의미해집니다.**
"스테이징에서는 됐는데"의 대부분이 여기서 옵니다.

---

## 10. 릴리스 위생

### 10-1. 브랜치 보호

```
main:
  - PR 필수 (직접 push 금지)
  - 리뷰 1명 이상
  - 모든 상태 체크 통과
  - 머지 전 최신 main과 동기화
  - force push 금지
```

### 10-2. 커밋 규약

```
feat(order): 주문 취소 API 추가
fix(user): 이메일 중복 시 500 대신 409 반환
chore(deps): axum 0.8.9로 업데이트
docs(msa): 회복탄력성 문서 추가
```

Conventional Commits를 쓰면 CHANGELOG와 semver 범프를 자동화할 수 있습니다.
**강제하려면 CI에서 검사**하세요. 안 그러면 절반만 지켜집니다.

### 10-3. 의존성 업데이트

```yaml
# .github/dependabot.yml
version: 2
updates:
  - package-ecosystem: cargo
    directory: "/"
    schedule: { interval: weekly }
    open-pull-requests-limit: 5
    groups:
      # 함께 올려야 하는 것들을 묶음 (15 §2-1)
      opentelemetry:
        patterns: ["opentelemetry*", "tracing-opentelemetry"]
      serde:
        patterns: ["serde*"]
  - package-ecosystem: docker
    directory: "/services"
    schedule: { interval: weekly }
  - package-ecosystem: github-actions
    directory: "/"
    schedule: { interval: monthly }
```

**`groups`가 중요합니다.** OTel 크레이트를 하나씩 올리면 매번 컴파일이 깨집니다.

주 1회 "의존성 PR 처리" 시간을 정해두세요. 미루면 **한 번에 50개가 쌓이고 아무도 안 봅니다.**

### 10-4. 릴리스 체크리스트

```markdown
## 배포 전
- [ ] CI 전체 통과
- [ ] 마이그레이션이 롤링 호환 (13 §2)
- [ ] 계약 테스트 통과 — 소비자 영향 없음 (16 §6)
- [ ] OpenAPI 파괴적 변경 없음 (또는 소비자 합의됨)
- [ ] 부하 테스트 (아키텍처 변경 시)
- [ ] 롤백 절차 확인
- [ ] 온콜 인지

## 배포 후
- [ ] 스모크 테스트 통과
- [ ] 에러율 정상 (15분 관찰)
- [ ] p99 지연 정상
- [ ] outbox 밀림 없음 (14 §10)
- [ ] 배포 마커 기록됨
```

### 10-5. 배포하지 않는 시간

```
❌ 금요일 오후
❌ 연휴 직전
❌ 온콜이 자리에 없을 때
❌ 큰 이벤트/프로모션 직전
```

**"금요일 배포 금지"는 자신감 부족이 아니라 위험 관리입니다.**
자동 롤백이 잘 갖춰지고 팀이 익숙해지면 완화하세요. 처음부터 무리하지 않는 게 낫습니다.

---

## 11. 개발자 경험

CI/CD의 목적은 검증만이 아니라 **개발이 빠르게 돌아가는 것**입니다.

### 11-1. 로컬 명령 통일

```makefile
.DEFAULT_GOAL := help

help:            ## 명령 목록
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
	  awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

setup:           ## 최초 1회 개발 환경 준비
	rustup component add rustfmt clippy
	cargo install cargo-nextest cargo-deny sqlx-cli cargo-watch --locked
	cp -n .env.example .env || true
	$(MAKE) up migrate seed

up:              ## 인프라 기동 (09 §1 모드 A)
	docker compose up -d --wait postgres-user postgres-order redis nats

migrate:         ## 전 서비스 마이그레이션
	./scripts/migrate-all.sh

seed:            ## 개발용 시드 데이터
	cargo run -p dev-tools --bin seed

dev:             ## 전 서비스 감시 실행
	cargo watch -x 'run -p gateway' & \
	cargo watch -x 'run -p user-service' & \
	wait

check:           ## CI가 하는 검사를 로컬에서
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo nextest run --workspace
	cargo sqlx prepare --workspace --check -- --all-targets

logs:            ## 인프라 로그
	docker compose logs -f

reset:           ## 전부 초기화 (데이터 삭제)
	docker compose down -v && $(MAKE) up migrate seed
```

**`make check`가 CI와 같은 검사를 하는 것이 핵심입니다.**
"push하고 CI 기다렸다가 fmt 실패 발견"의 왕복을 없앱니다.

**`make setup`이 새 팀원의 첫날을 결정합니다.** 이게 잘 되면 30분, 안 되면 이틀입니다.

### 11-2. 시드 데이터

```rust
// crates/dev-tools/src/bin/seed.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ⚠️ 안전장치 — 프로덕션 DB에 실행되면 재앙
    let url = std::env::var("DATABASE_URL")?;
    anyhow::ensure!(
        url.contains("localhost") || url.contains("127.0.0.1"),
        "seed는 로컬 DB에서만 실행할 수 있습니다: {url}"
    );

    let pool = PgPool::connect(&url).await?;

    // 고정 UUID — 문서와 테스트에서 참조 가능
    let alice = Uuid::parse_str("018f0000-0000-7000-8000-000000000001")?;
    seed_user(&pool, alice, "alice@dev.local", "Alice", &["user"]).await?;
    seed_user(&pool, admin_id(), "admin@dev.local", "Admin", &["admin"]).await?;
    seed_products(&pool, 50).await?;
    seed_orders(&pool, alice, 10).await?;

    println!("✅ 시드 완료");
    println!("   alice@dev.local / password: devpassword123");
    println!("   admin@dev.local / password: devpassword123");
    Ok(())
}
```

**고정 UUID를 쓰세요.** API 문서의 예시, Postman 컬렉션, 수동 테스트가 전부 안정적으로 동작합니다.

### 11-3. 빠른 피드백 루프

```bash
cargo install bacon --locked
```

```toml
# bacon.toml
[jobs.check]
command = ["cargo", "check", "--workspace", "--all-targets"]
need_stdout = false
watch = ["services", "crates"]

[jobs.test]
command = ["cargo", "nextest", "run", "--workspace", "--lib"]
need_stdout = true
```

`cargo watch`보다 출력이 정리되어 있어 큰 워크스페이스에서 편합니다.

### 11-4. 컴파일 시간 줄이기

Rust MSA에서 가장 흔한 불만입니다.

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]     # 링커 교체 — 체감 효과가 큼

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=/opt/homebrew/bin/ld64.lld"]
```

```toml
# Cargo.toml — 개발 빌드는 최적화를 낮추되, 의존성은 최적화
[profile.dev]
opt-level = 0
debug = 1              # 2 대신 1 — 링크 시간 단축

[profile.dev.package."*"]
opt-level = 2          # 의존성은 한 번만 빌드되므로 최적화해도 됨
```

**측정부터 하세요.**

```bash
cargo build --timings          # target/cargo-timings/ 에 HTML 리포트
```

어떤 크레이트가 시간을 먹는지 보고 나서 대응하세요.
대개 `sqlx` 매크로, 무거운 proc-macro, 그리고 **`crates/common`이 비대해진 것**이 원인입니다
([02 §4](./02_workspace.md)가 경고한 그것 — 컴파일 시간으로도 대가를 치릅니다).

---

## 체크포인트

```
[ ] PR 파이프라인이 10분 이내다
[ ] rust-cache 또는 sccache로 빌드가 캐싱된다
[ ] clippy가 -D warnings로 게이트다
[ ] unwrap_used/panic이 deny다 (10_errata §12-8)
[ ] cargo-deny로 취약점·라이선스·중복버전을 검사한다
[ ] gitleaks가 CI와 pre-commit에 있다
[ ] Cargo.lock이 커밋되고 --locked로 빌드한다
[ ] rust-toolchain.toml로 버전이 고정됐다
[ ] cargo sqlx prepare --check가 CI에 있다
[ ] 통합 테스트가 실제 PG/Redis로 돈다
[ ] 계약 테스트가 CI 게이트다
[ ] openapi.json 최신성과 breaking 변경을 검사한다
[ ] 마이그레이션 린트가 CI에 있다
[ ] cargo-chef로 Docker 의존성 레이어가 캐싱된다
[ ] 이미지가 non-root(distroless 등)다
[ ] 배포에 SHA 태그를 쓴다 (latest 아님)
[ ] SBOM 생성 + trivy 스캔이 있다
[ ] migrate Job → 배포 → 스모크 → 실패 시 롤백 순서다
[ ] 배포 마커가 Grafana로 전송된다
[ ] 프로덕션은 카나리 + 자동 지표 판정이다
[ ] 스테이징과 프로덕션이 같은 이미지를 쓴다
[ ] GitOps로 배포 이력이 git에 남는다
[ ] Dependabot에 OTel 등 그룹이 설정됐다
[ ] make setup 한 번으로 개발 환경이 준비된다
[ ] make check가 CI와 같은 검사를 한다
[ ] 시드 데이터에 프로덕션 안전장치가 있다
[ ] 릴리스 체크리스트가 PR 템플릿에 있다
```

---

다음: [19_platform_decisions — 자작할 것과 사올 것](./19_platform_decisions.md)
