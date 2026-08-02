# Go / Rust 폐쇄망(독립망) USB 이관·크로스빌드 팩트 정리

원본: `readme.md` (대화 로그)  
범위: 독립망 2대 PC 간 WebSocket, USB로 의존성/바이너리 이관, OS·아키텍처 이기종, Go vs Rust 차이

---

## 1. 결론 요약

| 항목 | 팩트 |
|------|------|
| 독립망 + USB로 Go + Gorilla WS 이관 | 가능. `go mod vendor`로 의존성 포함 후 소스/바이너리 이동 |
| OS·CPU가 달라도 WS 통신 | 가능. WebSocket은 HTTP/TCP 기반 표준 프로토콜 |
| aarch64 Linux에서 Windows x86용 크로스빌드 | 가능. `GOOS`/`GOARCH`로 `.exe` 생성 |
| 소스만 USB로 옮기고 타깃에서 빌드 | 가능. 타깃에 Go/Rust 툴체인이 있어야 함 |
| Rust `cargo vendor` | Go `go mod vendor`와 동일한 역할. 단 `.cargo/config.toml` 필수 |
| M1 Mac → aarch64 Linux (Axum, vendor) | 가능. 타깃에 Rust + (필요 시) C 빌드 도구 필요 |
| C 의존 크레이트(openssl 등) + vendor | Rust 소스는 vendor 가능. 시스템 C 라이브러리/헤더는 vendor로 해결 안 됨 |
| Go vs Rust 독립망 난이도 | Go가 훨씬 단순. Pure Go면 Go 컴파일러만으로 빌드 가능 |

---

## 2. 독립망에서 Go WebSocket이 되는 이유

1. Go는 빌드 시 필요 코드를 포함한 **단일 정적 바이너리**를 만든다.
2. 외부 인터넷이 없어도, **소스 + 모듈(vendor)** 을 USB로 물리 복사하면 폐쇄망에서 빌드·실행 가능.
3. 두 PC가 같은 독립망에 있고, IP·방화벽·바인딩 주소만 맞으면 Gorilla WebSocket으로 상호 WS 통신 가능.

---

## 3. Go: 의존성 USB 이관 (`go mod vendor`)

### 3.1 인터넷 PC에서의 준비

```bash
go mod init mywebsocket
go get github.com/gorilla/websocket   # (원문 예시 경로 정리)
go mod vendor
```

- `vendor/` 디렉터리에 외부 라이브러리 **전체 소스**가 저장됨.
- `vendor/` 포함 프로젝트 전체를 USB에 복사.

### 3.2 독립망 PC에서의 빌드/실행

```bash
go run -mod=vendor main.go
go build -mod=vendor -o app main.go
```

- `-mod=vendor`로 네트워크 대신 로컬 `vendor/`만 사용.
- 최신 Go는 `vendor/` 자동 감지 가능하나, **명시 플래그가 안전**.

---

## 4. 독립망 네트워크 체크리스트 (WS 통신)

라이브러리 이관과 별개로, 두 PC가 실제로 WS를 주고받으려면:

| 체크 | 내용 |
|------|------|
| 고정 IP / 서브넷 | 동일 서브넷(예: `192.168.1.x`), `ping` 통해야 함 |
| 서버 바인딩 | `127.0.0.1` 금지. `:8080` 또는 `0.0.0.0:8080` |
| 방화벽 | 서버 PC에서 해당 포트 인바운드 허용 |
| 클라이언트 URL | 서버 실제 IP 사용. 예: `ws://192.168.1.10:8080/ws` |

### OS별 방화벽 참고

- **Windows**: 최초 실행 시 보안 경고에서 개인/공용 네트워크 액세스 허용.
- **Linux (ufw 예시)**: `sudo ufw allow 8080/tcp`
- **포트**: `80`/`443` 등 시스템 포트는 관리자 권한·차단 이슈 가능 → **1024 초과 사설 포트**(8080, 9000 등) 권장.

---

## 5. OS·CPU 이기종과 WebSocket

### 5.1 통신 자체

- WebSocket은 OS/CPU에 종속되지 않는 **표준 네트워크 프로토콜**.
- aarch64 ↔ x86, Linux ↔ Windows 등 **이기종이어도 WS 통신 가능**.
- 엔디안(Big/Little) 이슈: WS는 텍스트(JSON 등) 또는 규격 바이너리 + 라이브러리 처리로 **실무상 신경 쓸 필요 거의 없음**.

### 5.2 빌드는 타깃에 맞게

Go는 소스 → 기계어 컴파일. 바이너리를 미리 만들면 **타깃 OS/ARCH를 지정**해야 함.

| 타깃 | 예시 |
|------|------|
| Windows x86_64 | `GOOS=windows GOARCH=amd64 go build -mod=vendor -o server.exe main.go` |
| Linux aarch64 | `GOOS=linux GOARCH=arm64 go build -mod=vendor -o client main.go` |
| 구형 32bit Windows | `GOARCH=386` |

**권장**: 소스 + `vendor/`만 USB로 옮기고, **각 독립망 PC에서 직접 `go build`** → 환경변수 없이 해당 머신용 바이너리 생성.

### 5.3 Linux 실행 권한

USB/크로스빌드로 온 실행 파일은 기본 실행 권한이 없을 수 있음.

```bash
chmod +x ./client
./client
```

---

## 6. aarch64 Linux → Windows x86 크로스빌드

### 방법 A: Linux에서 Windows `.exe` 빌드 (추천)

```bash
GOOS=windows GOARCH=amd64 go build -mod=vendor -o myapp.exe main.go
```

- USB에 `myapp.exe`만 옮겨 Windows에서 실행.
- **Windows에 Go가 없어도 실행 가능** (정적 바이너리).
- 일반 64bit Windows → `amd64`, 구형 32bit → `386`.

### 방법 B: 소스 폴더를 Windows로 옮겨 빌드

전제: Windows에 Go 설치됨 + `vendor/` 포함.

```cmd
go run -mod=vendor main.go
go build -mod=vendor -o myapp.exe main.go
```

### Windows 실행 시 주의

1. 방화벽 허용 (개인+공용).
2. 사설 포트 사용 권장.

---

## 7. 소스만 USB로 옮길 때 Go 체크리스트

| 항목 | 팩트 |
|------|------|
| 사전 작업 | 인터넷 PC에서 `go mod vendor` 필수 (독립망은 `go get` 불가) |
| 실행 | `-mod=vendor` 필수에 가깝게 권장 |
| 경로 구분자 | `/` vs `\` → `filepath.Join` 사용 시 OS 무관 |
| CGO | `import "C"`면 크로스/이기종 빌드 복잡도 증가 |
| Gorilla WebSocket | **100% Pure Go** → CGO 이슈 없음 |

---

## 8. Rust: `cargo vendor` (Go vendor 대응)

### 8.1 인터넷 PC

```bash
cargo vendor
```

- 루트에 `vendor/` 생성. `Cargo.toml`의 모든 crate 소스 다운로드.
- 터미널에 출력되는 `[source.crates-io]` 설정을 **`.cargo/config.toml`에 저장 필수**.

```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

- USB에 포함할 것: 소스 + `.cargo/` + `vendor/`.

### 8.2 독립망 타깃

```bash
cargo run --offline
# 또는
cargo build --release --offline
```

- `--offline`: crates.io 접속 시도 차단, 로컬 vendor만 사용.

---

## 9. Rust가 Go보다 까다로운 점

| 이슈 | 내용 |
|------|------|
| C 의존성 | 일부 WS/네트워크 crate가 OpenSSL 등 **시스템 C 라이브러리** 사용 |
| vendor 한계 | `cargo vendor`는 **Rust 소스만** 묶음. OS의 C 헤더/라이브러리는 미포함 |
| Windows 빌드 | `cargo build`에 **Visual Studio Build Tools (C++)** 필요. rustc만으로는 부족할 수 있음 |
| 크로스컴파일 | 타깃용 **링커/툴체인** 필요. Mac→Linux는 `cargo-cross`(Docker) 또는 GCC 링커 별도 설치 등 부담 |
| 권장 | Mac/이기종 → Linux Rust는 **바이너리 크로스빌드보다 소스+vendor를 타깃에서 빌드** |

순수 Rust TLS 예: `tungstenite` + `rustls` / handshake 등 C 비의존 피처.

---

## 10. Mac → Linux (x86 / aarch64)

메커니즘은 Go/Rust 모두 **vendor + 타깃 빌드**가 동일하게 적용.

### 방법 1: 소스 + vendor를 Linux에서 빌드 (권장)

| 언어 | Mac에서 | Linux에서 |
|------|---------|-----------|
| Go | `go mod vendor` | `go run -mod=vendor main.go` |
| Rust | `cargo vendor` + config.toml | `cargo run --offline` |

- Mac이 Intel이든 Apple Silicon이든, 타깃 Linux가 **자기 ARCH에 맞게** 컴파일 → 실패 확률 최저.

### 방법 2: Mac에서 Linux 바이너리 크로스빌드

**Go (쉬움)**

```bash
GOOS=linux GOARCH=amd64 go build -mod=vendor -o myapp_x86 main.go   # x86_64 Linux
GOOS=linux GOARCH=arm64 go build -mod=vendor -o myapp_arm main.go   # aarch64 Linux
```

**Rust (어려움)**

- 타깃용 링커 필요 → `cargo-cross` 또는 아키텍처별 GCC 링커.
- Mac→Linux Rust는 **방법 1(소스 이관) 강력 추천**.

### Mac→Linux 실전 복병

1. **`.DS_Store` 등 숨김 파일**: 빌드 실패 원인은 드묾. 정리만.
2. **`chmod +x`**: USB로 온 바이너리/빌드 산출물 실행 전 필수.
3. **개행**: Mac·Linux 모두 LF → Mac→Linux는 CRLF 충돌 거의 없음. (Windows는 CRLF 주의)

---

## 11. M1 Mac → aarch64 Linux (대규모 Axum)

### 기본 조건

- 타깃 aarch64 Linux에 **Rust(cargo) 사전 설치**.
- M1과 타깃이 둘 다 **aarch64(ARM64)** → 소스+vendor 이관·네이티브 빌드가 가장 안전.

### 빌드 전 체크리스트 (3가지)

#### (1) `.cargo/config.toml` + `--offline`

```bash
cargo build --release --offline
```

#### (2) C 라이브러리 의존성

- Axum 대규모 프로젝트에 JWT, SQLx, HTTPS/TLS 등이 있으면 OpenSSL 등 **시스템 C**를 끌어올 수 있음.
- 미설치 시 예: `Could not find directory of OpenSSL`.
- 대응: `native-tls` → **`rustls`** 피처로 Pure Rust 전환.

#### (3) build-essential / Development Tools

- Rust만 있고 C 컴파일러가 없으면 일부 crate·링크 단계에서 실패 가능.
- Ubuntu/Debian: `build-essential`  
- RHEL/Rocky: Development Tools  
→ **인터넷 차단 전에** 설치해 둘 것.

### 실행 순서

```bash
# 폴더 복사 후
cargo build --release --offline
chmod +x ./target/release/<바이너리명>
./target/release/<바이너리명>
```

산출물 위치: `target/release/`.

---

## 12. C 의존이 많은 Rust 프로젝트 + vendor

### 핵심 구분

| 구분 | 가능 여부 |
|------|-----------|
| Rust crate 소스를 `cargo vendor`로 묶기 | **가능** |
| 빌드에 필요한 OS C 헤더/시스템 라이브러리까지 vendor로 해결 | **불가** (기본) |
| 독립망에서 C lib 없으면 | **링크 에러** |

### 해결책 A: Pure Rust로 바꾸기 (권장)

```toml
# AS-IS
sqlx = { version = "0.7", features = [ "postgres", "runtime-tokio-native-tls" ] }
reqwest = { version = "0.11", features = [ "json" ] }  # 기본 native-tls

# TO-BE
sqlx = { version = "0.7", features = [ "postgres", "runtime-tokio-rustls" ] }
reqwest = { version = "0.11", features = [ "json", "rustls-tls" ], default-features = false }
```

- OpenSSL → **rustls**
- Crypto → **ring** / RustCrypto (`sha2`, `aes`, `hmac`, `bcrypt` 등)

### 해결책 B: 코드 못 고칠 때 `vendored` 피처

```toml
openssl = { version = "0.10", features = ["vendored"] }
```

- C OpenSSL **소스까지** vendor에 포함 → OS에 OpenSSL 없어도 빌드 가능.
- 단, 타깃에 **GCC 등 C 컴파일러**는 여전히 필요 (`build-essential` 등).

### 해결책 C: SQLx 오프라인 모드

SQLx는 컴파일 시 실제 DB 연결로 쿼리 검사 → 독립망에 개발 DB 없으면 실패.

**인터넷+DB 있는 Mac에서:**

```bash
cargo sqlx migrate run
cargo sqlx prepare
```

- `.sqlx-data.json`(또는 동등 캐시) 생성 → USB에 함께 복사.

**독립망 Linux:**

```bash
SQLX_OFFLINE=true cargo build --release --offline
```

### M1 → 독립망 Linux Rust 준비 순서 (최종)

1. `Cargo.toml`에서 `native-tls` → `rustls`로 변경.
2. 불가피한 `openssl`은 `features = ["vendored"]`.
3. SQLx면 `cargo sqlx prepare` → 캐시 파일 포함.
4. `cargo vendor` + `.cargo/config.toml` 반영.
5. USB 이관 후: `SQLX_OFFLINE=true cargo build --release --offline`.

---

## 13. Go로 하면 Rust급 신경을 거의 안 써도 되는가?

**예. 독립망 이관 복잡도에서 Go가 Rust보다 훨씬 유리.**

| 비교 | Rust | Go |
|------|------|-----|
| C 의존 | openssl 등 C 링크 흔함 | Pure Go 문화 강함. crypto/TLS/HTTP/WS 유명 라이브러리 다수 Pure Go |
| vendor 결과 | Rust 소스만 → 시스템 C는 별도 | vendor가 Pure Go면 **Go 컴파일러만**으로 빌드 |
| DB 라이브러리 | SQLx: 컴파일 시 DB·prepare 캐시 필요 | database/sql, GORM, sqlx(Go) 등은 **컴파일 시 DB 미요구** (런타임 검증) |
| 추가 빌드 도구 | build-essential, VS Build Tools, 링커 | Go 자체 링커. **go만 있으면 됨** |
| 크로스컴파일 | 링커·툴체인 부담 | `GOOS`/`GOARCH` 한 줄 |

### Go 독립망 이관 최소 절차

```bash
# M1 Mac
go mod vendor

# USB로 프로젝트 통째 복사

# 독립망 aarch64 Linux
go build -mod=vendor -o server main.go
```

C 호환·피처 세팅·sqlx prepare JSON 등 Rust 절차가 **대부분 생략**.

---

## 14. 명령어·설정 치트시트

### Go

```bash
go mod vendor
go run -mod=vendor main.go
go build -mod=vendor -o app main.go

GOOS=windows GOARCH=amd64 go build -mod=vendor -o app.exe main.go
GOOS=linux   GOARCH=amd64 go build -mod=vendor -o app_x86 main.go
GOOS=linux   GOARCH=arm64 go build -mod=vendor -o app_arm main.go
```

### Rust

```bash
cargo vendor
# → .cargo/config.toml에 source replace 설정

cargo run --offline
cargo build --release --offline
SQLX_OFFLINE=true cargo build --release --offline
```

### Linux

```bash
chmod +x ./바이너리
sudo ufw allow 8080/tcp   # 필요 시
```

---

## 15. 한 줄 팩트 모음

1. 독립망이라도 USB로 Go+vendor를 넣으면 두 PC WS 가능.
2. WS는 OS/ARCH 무관; 빌드 산출물은 OS/ARCH 종속.
3. Go 크로스컴파일은 `GOOS`/`GOARCH`로 충분.
4. 소스 이관 시 타깃에 툴체인만 있으면 네이티브 빌드가 가장 안전.
5. Rust도 `cargo vendor` 가능하나 config.toml + `--offline` 필수.
6. Rust vendor ≠ 시스템 C 라이브러리 해결.
7. OpenSSL/native-tls는 독립망 실패 1순위 → rustls 또는 openssl vendored.
8. SQLx는 `prepare` + `SQLX_OFFLINE=true` 없으면 독립망 빌드에서 터질 수 있음.
9. Mac→Linux Rust 크로스빌드는 까다로움 → vendor 소스 이관 권장.
10. M1→aarch64 Linux는 ARCH 동일 → Axum vendor 이관·로컬 빌드에 유리.
11. Go는 Pure Go + 자체 링커로 독립망 이관이 Rust보다 압도적으로 단순.
12. 네트워크 실패의 흔한 원인: `127.0.0.1` 바인딩, 방화벽, 잘못된 클라이언트 IP/포트.
