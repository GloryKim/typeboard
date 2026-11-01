# Go / Bun / Rust — Vendor·크로스빌드·폐쇄망 비교 (상세 표)

원본: `back.md` (+ 기존 폐쇄망 팩트)  
범위: 의존성 로컬 격리, 다른 OS/CPU 빌드·배포, 주의점, 명령 예시

---

## 1. 한눈에 보는 총괄 비교

| 비교 항목 | Go (`go mod vendor`) | Bun (`bun build --compile`) | Rust (`cargo vendor`) |
|-----------|----------------------|-----------------------------|------------------------|
| **기본 개념** | 외부 라이브러리 **소스 전부**를 `vendor/`에 복사 | 소스 + npm 의존성 + **Bun 런타임**을 **단일 바이너리**로 합침 | 외부 crate **소스 전부**를 `vendor/`에 복사 |
| **Go vendor와 1:1 대응** | 본인 | **없음** (`node_modules` 모델). 대안은 단일 실행파일 또는 Yarn Zero-Installs | 거의 동일 (`cargo vendor`) |
| **산출물** | `vendor/` 소스 폴더 → (빌드 후) 단일 바이너리 | `.exe` / ELF 등 **단일 실행 파일** | `vendor/` 소스 폴더 → (빌드 후) 단일 바이너리 |
| **크로스컴파일 난이도** | ★☆☆ 매우 쉬움 (`GOOS`/`GOARCH`) | ★★☆ 쉬움 (`--target=...`) | ★★★★ 까다로움 (타깃 링커·C 툴체인) |
| **C/네이티브 의존** | 매우 낮음 (Pure Go 문화, `CGO_ENABLED=0` 가능) | 높음 (`.node` 네이티브 애드온이 OS/CPU 종속) | 매우 높음 (`*-sys`, OpenSSL 등) |
| **폐쇄망 빌드** | 완벽 (`-mod=vendor`) | 완벽 (호스트에서 빌드한 바이너리만 USB 이동) | 완벽 (단 `.cargo/config.toml` + `--offline`) |
| **타깃에 필요한 것** | Go 툴체인 **또는** 미리 빌드한 바이너리 | **아무것도 없음** (런타임 내장) | Rust 툴체인 **또는** 미리 빌드한 바이너리 (+ 종종 C 컴파일러) |
| **권장 배포** | 호스트에서 크로스빌드 후 바이너리만 배포 / 또는 vendor 소스→타깃 빌드 | 호스트에서 `--compile --target` 후 바이너리만 배포 | `cross`(Docker)로 바이너리 빌드 / 또는 vendor→타깃에서 네이티브 빌드 |
| **개방성 한 줄** | OS/CPU 이관·빌드에 **가장 열려 있음** | vendor보다 **단일 파일 배포**가 강점 | 소스는 묶을 수 있으나 **C/툴체인 장벽** |

---

## 2. 워크플로 비교 (무엇을 USB에 담는가)

| 단계 | Go | Bun | Rust |
|------|----|-----|------|
| **1. 인터넷 PC 준비** | `go mod vendor` | `bun install` 후 `bun build --compile --target=...` | `cargo vendor` + `.cargo/config.toml` |
| **2. USB에 담는 것** | A) `vendor/` 포함 프로젝트 전체<br>B) 타깃용 바이너리만 | **타깃용 단일 실행 파일만** (일반적) | A) 소스 + `vendor/` + `.cargo/`<br>B) `cross`로 만든 바이너리만 |
| **3. 독립망에서** | A) `go build -mod=vendor`<br>B) 바이너리 실행 | 바이너리 실행 (`chmod +x` Linux) | A) `cargo build --release --offline`<br>B) 바이너리 실행 |
| **타깃에 툴체인 필요?** | A: 필요 / B: 불필요 | 불필요 | A: 필요(+C툴체인 가능) / B: 불필요 |

---

## 3. Go — Vendor + 크로스빌드 (상세)

### 3.1 핵심 팩트

| 항목 | 내용 |
|------|------|
| 명령 | `go mod vendor` → `vendor/`에 전 플랫폼용 소스 포함 |
| 다른 OS/CPU에서 빌드 | **가능** (소스 레벨 크로스컴파일 지원) |
| Gorilla WebSocket | **Pure Go** → CGO 이슈 없음 |
| 빌드 강제 로컬 | `-mod=vendor` (Go 1.14+ 자동감지도 있으나 명시 권장) |

### 3.2 주의점 표

| # | 주의점 | 문제 | 대응 | 예시 |
|---|--------|------|------|------|
| 1 | **CGO** | C 코드/`import "C"`면 타깃용 C 컴파일러 필요. vendor만으로 부족 | Pure Go 유지, 크로스 시 `CGO_ENABLED=0` | 아래 §3.3 |
| 2 | **빌드 태그 / 파일 분기** | `//go:build linux`, `conn_windows.go` 등 OS별 파일 | vendor는 **전 OS 소스**를 담음. 다만 OS 전용 API만 쓰면 타깃에서 기능 제한·컴파일 에러 가능 | 라이브러리 플랫폼 지원 확인 |
| 3 | **`-mod=vendor`** | vendor를 무시하고 네트워크 `go get` 시도할 수 있음 | 폐쇄망에선 플래그 명시 | `go build -mod=vendor .` |
| 4 | **Go 버전** | 개발/빌드 Go 메이저 버전이 다르면 `vendor/modules.txt` 형식 거부 가능 | 양쪽 Go 버전 맞추기 또는 동일 버전 Docker | Go 1.22 ↔ 1.22 |

### 3.3 명령 예시

**인터넷 PC에서 vendor**

```bash
go mod init mywebsocket
go get github.com/gorilla/websocket
go mod vendor
# USB: 프로젝트 전체(vendor 포함) 복사
```

**타깃에서 네이티브 빌드 (소스 이관)**

```bash
go build -mod=vendor -o app .
# 또는
go run -mod=vendor .
```

**호스트에서 크로스빌드 (바이너리만 USB)**

```bash
# Pure Go 보장 + 크로스
CGO_ENABLED=0 GOOS=linux   GOARCH=amd64 go build -mod=vendor -o app_linux_amd64 .
CGO_ENABLED=0 GOOS=linux   GOARCH=arm64 go build -mod=vendor -o app_linux_arm64 .
CGO_ENABLED=0 GOOS=windows GOARCH=amd64 go build -mod=vendor -o app.exe .
CGO_ENABLED=0 GOOS=darwin  GOARCH=arm64 go build -mod=vendor -o app_mac_m1 .
```

| 타깃 | `GOOS` | `GOARCH` | 산출물 예 |
|------|--------|----------|-----------|
| Linux x86_64 | `linux` | `amd64` | `app_linux_amd64` |
| Linux aarch64 | `linux` | `arm64` | `app_linux_arm64` |
| Windows 64bit | `windows` | `amd64` | `app.exe` |
| Windows 32bit | `windows` | `386` | `app.exe` |
| macOS Apple Silicon | `darwin` | `arm64` | `app_mac_m1` |
| macOS Intel | `darwin` | `amd64` | `app_mac_intel` |

**Linux에서 실행**

```bash
chmod +x ./app_linux_arm64
./app_linux_arm64
```

---

## 4. Bun — `bun build --compile` (상세)

### 4.1 핵심 팩트

| 항목 | 내용 |
|------|------|
| Go vendor 대응 | **완전 일치 기능 없음** (`node_modules` 공유 폴더 모델) |
| 실질적 대안 | `bun build --compile` → 소스+의존성+런타임을 **한 파일** |
| 타깃에 Bun/Node 필요? | **불필요** |
| 소스만 오프라인 잠그기 | Bun 내장보다는 Yarn Zero-Installs(`.yarn/cache` 커밋) 쪽. 배포 편의는 `--compile`이 우위 |

### 4.2 지원 타겟 (`--target`)

| Target | 의미 |
|--------|------|
| `bun-linux-x64` | Linux x86_64 (glibc 계열 가정) |
| `bun-linux-arm64` | Linux aarch64 |
| `bun-linux-x64-musl` | Alpine 등 **musl** Linux |
| `bun-windows-x64` | Windows x64 |
| `bun-windows-arm64` | Windows ARM64 |
| `bun-darwin-x64` | macOS Intel |
| `bun-darwin-arm64` | macOS Apple Silicon |

### 4.3 주의점 표

| # | 주의점 | 문제 | 대응 | 예시 |
|---|--------|------|------|------|
| 1 | **네이티브 애드온 (`.node`)** | `sharp`, `bcrypt`, 일부 DB 드라이버 등 C++/Rust 바이너리는 **빌드한 OS/CPU에서만** 동작 | Pure JS/TS 위주, 또는 타깃 OS에서 재설치·타깃용 바이너리로 빌드 | Mac에서 빌드한 `.node`를 Linux 바이너리에 넣어도 Linux에서 실패 |
| 2 | **동적 `require` / 경로** | 단일 파일로 합치면 `__dirname`, `process.cwd()`, `require(dynamicPath)`가 깨지거나 파일 누락 | 정적 에셋은 `--asset`으로 임베드, 동적 require 최소화 | 아래 §4.4 |
| 3 | **libc (glibc vs musl)** | Ubuntu용으로 빌드한 바이너리가 Alpine에서 libc 에러 | Alpine이면 `bun-linux-x64-musl` 명시 | 아래 예시 |

### 4.4 명령 예시

```bash
# Linux ARM64 단일 실행파일
bun build ./index.ts --compile --target=bun-linux-arm64 --outfile=my-app-linux

# Windows x64
bun build ./index.ts --compile --target=bun-windows-x64 --outfile=my-app-win.exe

# Alpine (musl)
bun build ./index.ts --compile --target=bun-linux-x64-musl --outfile=my-app-alpine

# 정적 파일 임베드 (개념 예시)
bun build ./index.ts --compile --target=bun-linux-x64 --outfile=my-app --asset ./config.json
```

**폐쇄망 배포 흐름**

```text
[인터넷 Mac/PC]
  bun install
  bun build ./src/server.ts --compile --target=bun-linux-arm64 --outfile=ws-server
        │
        ▼ USB (파일 1개)
[독립망 aarch64 Linux]
  chmod +x ./ws-server
  ./ws-server
  # Bun 설치 불필요
```

---

## 5. Rust — `cargo vendor` (상세)

### 5.1 핵심 팩트

| 항목 | 내용 |
|------|------|
| Go와의 유사점 | `cargo vendor`로 crate 소스를 `vendor/`에 묶음 → 폐쇄망 빌드 가능 |
| Go와의 차이 | **C 툴체인·링커·build.rs** 때문에 “가져가서 빌드”가 Go만큼 열리지 않음 |
| 설정 필수 | `.cargo/config.toml`에 crates.io → vendor 리다이렉트 |
| 오프라인 플래그 | `cargo build --offline` / `cargo run --offline` |

### 5.2 Go와 결정적으로 다른 점

| # | 차이 | Go | Rust |
|---|------|----|------|
| 1 | **C 의존** | Pure Go 많고 `CGO_ENABLED=0`으로 끄기 쉬움 | `openssl-sys`, `libsqlite3-sys` 등 `-sys` 크레이트 흔함. vendor는 **Rust 소스만** 묶음 |
| 2 | **크로스컴파일** | `GOOS`/`GOARCH` 두 변수 | `rustup target add` + **타깃용 링커/크로스 GCC** 수동 설치. 초보에게 복잡 |
| 3 | **build.rs** | 해당 패턴 드묾 | 빌드 직전 스크립트가 호스트 C 도구·헤더를 탐색 → 환경 바뀌면 실패 잦음 |

### 5.3 주의점·해결 표

| # | 상황 | 문제 | 해결 |
|---|------|------|------|
| 1 | OpenSSL / native-tls | 타깃에 `libssl-dev` 없으면 링크 에러 | `rustls` 피처로 교체, 또는 `openssl` `features = ["vendored"]` |
| 2 | 크로스빌드 | 링커 미설정 | `cross build --target ...` (Docker) 권장 |
| 3 | Windows에서 cargo | VS Build Tools 없음 | Build Tools(C++) USB로 사전 설치 |
| 4 | SQLx | 컴파일 시 DB 연결 시도 | `cargo sqlx prepare` + `SQLX_OFFLINE=true` |
| 5 | Pure Rust만 | C 없음 | Go처럼 비교적 매끄럽게 vendor→타깃 빌드 |

### 5.4 명령·설정 예시

**vendor + config**

```bash
cargo vendor
```

```toml
# .cargo/config.toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

**독립망에서**

```bash
cargo build --release --offline
# SQLx 사용 시
SQLX_OFFLINE=true cargo build --release --offline
```

**TLS/의존성 TO-BE (Pure Rust 방향)**

```toml
# AS-IS (C/OpenSSL 경로)
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-native-tls"] }
reqwest = { version = "0.11", features = ["json"] }

# TO-BE
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio-rustls"] }
reqwest = { version = "0.11", features = ["json", "rustls-tls"], default-features = false }

# 코드를 못 고칠 때
openssl = { version = "0.10", features = ["vendored"] }
# ※ vendored여도 타깃에 GCC 등 C 컴파일러는 필요
```

**크로스빌드 (권장: cross)**

```bash
# cargo build 대신
cross build --release --target aarch64-unknown-linux-gnu
cross build --release --target x86_64-pc-windows-msvc
```

| 흔한 타깃 트리플 | 의미 |
|------------------|------|
| `x86_64-unknown-linux-gnu` | Linux x86_64 (glibc) |
| `aarch64-unknown-linux-gnu` | Linux ARM64 |
| `x86_64-pc-windows-msvc` | Windows x64 |
| `aarch64-apple-darwin` | macOS Apple Silicon |

---

## 6. 시나리오별 “뭐가 제일 편한가”

| 시나리오 | 추천 | 이유 | 예시 한 줄 |
|----------|------|------|------------|
| 폐쇄망 + USB + Gorilla WS | **Go** | vendor + Pure Go + `GOOS`/`GOARCH` | `go build -mod=vendor` |
| 타깃에 런타임 설치 금지, JS로 짜고 싶음 | **Bun `--compile`** | 런타임까지 단일 파일 | `--target=bun-linux-arm64` |
| 대규모 Axum + DB/TLS, C 의존 많음 | **Rust** (난이도↑) | rustls/vendored/`cross`/sqlx offline 준비 필수 | `SQLX_OFFLINE=true cargo build --offline` |
| M1 → aarch64 Linux, 소스 이관 | Go / Rust(ARCH 동일) | 타깃 네이티브 빌드가 안전 | vendor 폴더 USB → 타깃에서 빌드 |
| Mac → Linux, **바이너리만** 넘기기 | Go ≫ Bun > Rust | Go 환경변수 2개 / Bun target / Rust는 cross·링커 | Go: `GOOS=linux GOARCH=amd64` |
| Alpine Docker | Bun musl 타깃 / Go 정적 / Rust musl 타깃 | glibc 바이너리 Alpine에서 깨짐 | `bun-linux-x64-musl` |
| Windows 타깃, 툴체인 최소 | Go 또는 Bun 바이너리 | Rust는 VS Build Tools 부담 | Go `GOOS=windows` 또는 Bun `--target=bun-windows-x64` |

---

## 7. 실패 원인 빠른 대조표

| 증상 | Go에서 | Bun에서 | Rust에서 |
|------|--------|---------|----------|
| 빌드가 인터넷을 찾음 | `-mod=vendor` 누락 | (보통 바이너리만 옮김) | `config.toml` 또는 `--offline` 누락 |
| 다른 OS에서 컴파일 실패 | CGO 켜짐 / 버전 불일치 | — (보통 호스트에서 이미 컴파일) | C 헤더·링커·build.rs |
| 다른 OS에서 **실행** 실패 | ARCH 틀린 바이너리, `chmod` | `.node` 네이티브, glibc≠musl, 동적 경로 | ARCH/ABI 불일치, 동적 링크 lib 누락 |
| TLS/HTTPS 관련 | 드묾 (stdlib crypto) | 네이티브 패키지 의존 시 | OpenSSL / native-tls |
| DB 관련 컴파일 에러 | 드묾 (런타임 연결) | 네이티브 드라이버 | SQLx online 검사 |

---

## 8. 실전 미니 레시피 (복붙용)

### 8.1 Go: Mac → 독립망 Linux arm64 (바이너리만)

```bash
# Mac (인터넷)
go mod vendor
CGO_ENABLED=0 GOOS=linux GOARCH=arm64 go build -mod=vendor -o ws-server .

# USB로 ws-server만 이동

# Linux aarch64
chmod +x ./ws-server && ./ws-server
```

### 8.2 Go: vendor 소스 → 독립망 Windows에서 빌드

```bash
# 인터넷 PC
go mod vendor
# USB: 프로젝트 통째

# Windows (Go 설치됨)
go build -mod=vendor -o ws-server.exe .
ws-server.exe
```

### 8.3 Bun: Mac → Linux arm64 단일 파일

```bash
bun install
bun build ./index.ts --compile --target=bun-linux-arm64 --outfile=ws-server
# USB → Linux: chmod +x ./ws-server && ./ws-server
```

### 8.4 Rust: M1 → aarch64 Linux (vendor 소스 이관)

```bash
# M1 (인터넷) — 가능하면 rustls로 정리 후
cargo vendor
# .cargo/config.toml 설정 반영
# SQLx면: cargo sqlx prepare

# USB: 소스 + vendor + .cargo (+ .sqlx 캐시)

# Linux aarch64 (rustc + build-essential)
SQLX_OFFLINE=true cargo build --release --offline
chmod +x ./target/release/<crate이름>
```

### 8.5 Rust: 호스트에서 크로스 (cross)

```bash
cross build --release --target aarch64-unknown-linux-gnu
# target/aarch64-unknown-linux-gnu/release/<이름> 을 USB로
```

---

## 9. 세 줄 요약

| | |
|--|--|
| **Go** | 언어·툴이 크로스컴파일에 최적화. vendor로 다른 OS/CPU에 가져가 빌드하기에 **가장 열려 있음**. |
| **Bun** | vendor 디렉터리 격리보다 **런타임까지 묶은 단일 실행 파일** 배포가 강점. 타깃에 Bun 불필요. |
| **Rust** | vendor로 소스는 묶이지만, C/`-sys`/링커/`build.rs` 때문에 Go만큼 쉽지 않음. `cross`·rustls·vendored로 완화. |

---

## 10. 의사결정 플로우 (짧게)

```text
폐쇄망/USB로 다른 OS·CPU에 올려야 한다
│
├─ JS/TS로 짜고, 타깃에 런타임 설치 싫다 → Bun --compile (+ Pure JS, musl 주의)
├─ 최소 마찰·WS/서버·크로스빌드 최우선 → Go + vendor (+ CGO_ENABLED=0)
└─ 이미 Rust(Axum 등)다
     ├─ Pure Rust(rustls 등) → cargo vendor → 타깃 빌드 또는 cross
     └─ OpenSSL/SQLx/C 많음 → rustls·vendored·sqlx prepare·build-essential 준비
```
