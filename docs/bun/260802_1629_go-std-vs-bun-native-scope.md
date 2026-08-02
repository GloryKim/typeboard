# Go 표준 vs Bun 내장 — 외부 디펜던시 0일 때 지원 범위 (상세)

> 전제: **외부 라이브러리 / 레지스트리 패키지 0**  
> - Go = **표준 라이브러리(`std`)만** (`github.com/...`, `golang.org/x/...` 없음)  
> - Bun = **Bun 바이너리에 포함된 API만** (`bun add` / npm import 없음)  
> 목적: **기능별 지원 가능 유무**를 최대한 상세히 대조

범례:

| 기호 | 의미 |
|---|---|
| ✅ | 네이티브로 실무 사용 가능 |
| ⚠️ | 가능하지만 제한·직접 구현·버전 의존 |
| ❌ | 네이티브만으로는 사실상 불가 (외부 패키지/도구 필요) |
| — | 해당 개념이 약하거나 해당 없음 |

---

## 목차

1. [한눈에 총표](#1-한눈에-총표)
2. [HTTP 서버](#2-http-서버)
3. [HTTP 클라이언트](#3-http-클라이언트)
4. [실시간 통신 (WS·SSE 등)](#4-실시간-통신-wssse-등)
5. [요청 바디 · 업로드 · 다운로드](#5-요청-바디--업로드--다운로드)
6. [데이터 포맷 입출력](#6-데이터-포맷-입출력)
7. [파일시스템 · 압축 · 아카이브](#7-파일시스템--압축--아카이브)
8. [이미지 · 오디오 · 비디오](#8-이미지--오디오--비디오)
9. [암호화 · 인증 · 비밀](#9-암호화--인증--비밀)
10. [데이터베이스 · 캐시 · 오브젝트 스토리지](#10-데이터베이스--캐시--오브젝트-스토리지)
11. [저수준 네트워크 · DNS · 메일](#11-저수준-네트워크--dns--메일)
12. [동시성 · 프로세스 · OS](#12-동시성--프로세스--os)
13. [텍스트 · 로케일 · 시간 · 정규식](#13-텍스트--로케일--시간--정규식)
14. [템플릿 · HTML · 이메일 본문](#14-템플릿--html--이메일-본문)
15. [관측 · 로그 · 프로파일](#15-관측--로그--프로파일)
16. [테스트 · 빌드 · 언어 도구](#16-테스트--빌드--언어-도구)
17. [차이만 모아보기 (Go만 / Bun만)](#17-차이만-모아보기-go만--bun만)
18. [둘 다 ❌ 인 것](#18-둘-다--인-것)
19. [선택 가이드](#19-선택-가이드)

---

## 1. 한눈에 총표

| 대분류 | 세부 | Go std | Bun 내장 | 비고 |
|---|---|---|---|---|
| HTTP 서버 | Listen, 라우팅, 헤더 | ✅ | ✅ | |
| HTTP 클라 | GET/POST JSON | ✅ | ✅ `fetch` | |
| HTTPS | TLS 종료/접속 | ✅ | ✅ | |
| HTTP/2 | | ✅ | ⚠️ 버전·경로 확인 | |
| HTTP/3 | | ❌ | ⚠️ 실험/버전 | |
| WebSocket | 서버 | ❌ | ✅ | **큰 차이** |
| WebSocket | 클라 | ❌ | ✅ | |
| SSE | | ⚠️ 직접 | ⚠️ 직접 | |
| JSON | | ✅ | ✅ | |
| multipart 업로드 | | ✅ | ✅ | |
| 파일 다운로드·Range | | ✅ 강함 | ✅ | Go `ServeContent` 성숙 |
| YAML | | ❌ | ✅ | **Bun만** |
| XML | | ✅ | ⚠️ | Go 우세 |
| CSV | | ✅ | ⚠️ 수동 | Go 우세 |
| SQLite | | ❌ | ✅ | **Bun만** |
| Postgres/MySQL 드라이버 | | ❌ | ⚠️ 버전 | |
| Redis 클라 | | ❌ | ⚠️ 내장/실험 | |
| S3 클라 | | ❌ | ⚠️ `Bun.s3` | |
| bcrypt/argon2 | | ❌ std | ✅ `Bun.password` | **Bun만** |
| 해시·HMAC·AES·RSA | | ✅ | ✅ | |
| JWT 풀세트 | | ❌ | ❌ | 수동 HMAC만 |
| JPEG/PNG/GIF 픽셀 | | ✅ | ❌~⚠️ | **Go만** (디코드) |
| 영상 트랜스코드 | | ❌ | ❌ | ffmpeg 등 |
| gzip/zip/tar | | ✅ | ⚠️~✅ | Go 아카이브 더 두꺼움 |
| HTML 템플릿 | | ✅ | ⚠️ | **Go만** 전용 엔진 |
| TS 실행·번들 | | ❌ | ✅ | **Bun만** |
| `go test` / `bun test` | | ✅ | ✅ | |
| 단일 바이너리 | | ✅ | ⚠️ 런타임 필요 | **Go 강점** |
| gRPC | | ❌ | ❌ | |
| FFI | | ⚠️ cgo | ✅ `bun:ffi` | |

---

## 2. HTTP 서버

| 기능 | Go | Bun | 상세 차이 |
|---|---|---|---|
| 포트 listen | ✅ `http.ListenAndServe` / `http.Server` | ✅ `Bun.serve({ port })` | 둘 다 프로세스=서버 |
| `0.0.0.0` 바인드 | ✅ `Addr: ":8080"` | ✅ `hostname: "0.0.0.0"` | |
| TLS 서버 | ✅ `ListenAndServeTLS`, `tls.Config` | ✅ `tls` 옵션·인증서 | Go가 `ClientAuth` 등 세밀 |
| 메서드·경로 라우팅 | ✅ Go1.22+ `ServeMux` 패턴 | ⚠️ `if`/`URL` 직접 또는 직접 라우터 | Go가 패턴 라우팅 내장 |
| 경로 파라미터 | ✅ `r.PathValue("id")` | ⚠️ URL 파싱 직접 | |
| 미들웨어 체인 | ⚠️ 함수로 감싸기 | ⚠️ 동일 | 프레임워크 없음 |
| 요청 헤더 읽기 | ✅ `r.Header` | ✅ `req.headers` | |
| 응답 헤더·상태코드 | ✅ `WriteHeader` | ✅ `Response` | |
| 쿠키 읽기/쓰기 | ✅ `r.Cookie`, `SetCookie` | ✅ Cookie / `Set-Cookie` | |
| Keep-Alive | ✅ | ✅ | |
| 타임아웃 Read/Write/Idle | ✅ `Server` 필드 **명확** | ⚠️ idleTimeout 등 — 문서 확인 | **Go가 운영 옵션 풍부** |
| `ReadHeaderTimeout` | ✅ | ⚠️ | Slowloris 방어에 Go 관례 강함 |
| Graceful shutdown | ✅ `Shutdown(ctx)` | ⚠️ 신호 후 서버 중단 패턴 | **Go가 정석 API** |
| HTTP/2 서버 | ✅ (TLS 등 조건) | ⚠️ | |
| 트레일러 헤더 | ✅ | ⚠️ | |
| Expect: 100-continue | ✅ | ⚠️ | |
| Hijack (원본 conn) | ✅ `http.Hijacker` | ⚠️ 제한적 | WS 직접 구현 시 Go만 해당 |
| HTTP/2 Push | ⚠️ deprecated 경향 | — | |
| 정적 디렉터리 | ✅ `FileServer` | ✅ 파일 Response 루프 | |
| CGI/FCGI | ✅ | ❌ | 거의 안 씀 |
| 가상 호스트 | ⚠️ 직접 | ⚠️ 직접 | |

**요약:** HTTP **기본 서버**는 둘 다 ✅. **운영 타임아웃·Graceful·ServeMux 패턴·Hijack**은 Go가 더 상세. **코딩 감각(fetch/Response)** 은 Bun이 웹 표준에 가까움.

---

## 3. HTTP 클라이언트

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| GET/POST | ✅ `http.Get` / `Client.Do` | ✅ `fetch` | Axios 불필요 |
| JSON 송신 | ✅ `bytes`+Header | ✅ `body: JSON.stringify` | |
| JSON 수신 | ✅ `json.NewDecoder` | ✅ `res.json()` | |
| 커스텀 헤더 | ✅ | ✅ | |
| 타임아웃 | ✅ `Client.Timeout`, `context` | ✅ `AbortSignal` / AbortController | |
| 연결 풀 | ✅ `Transport` | ✅ 런타임 관리 | Go가 튜닝 항목 많음 |
| 프록시 | ✅ `Proxy` / env | ⚠️ env·옵션 | |
| 쿠키 저장소(Jar) | ✅ `cookiejar` | ⚠️ | **Go 우세** |
| 리다이렉트 정책 | ✅ `CheckRedirect` | ⚠️ `redirect` 옵션 | |
| 클라이언트 인증서 | ✅ `tls.Config` | ⚠️ | Go 세밀 |
| HTTP/2 클라 | ✅ | ⚠️ | |
| 스트리밍 업로드 | ✅ `io.Reader` Body | ✅ stream / File | |
| multipart 클라 송신 | ✅ `multipart.Writer` | ✅ `FormData` | |

**요약:** 둘 다 Axios 없이 ✅. **Jar·Transport 튜닝·TLS 클라 세밀도**는 Go.

---

## 4. 실시간 통신 (WS·SSE 등)

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| WebSocket 서버 | ❌ | ✅ `Bun.serve` `websocket` | **결정적 차이** |
| WebSocket 클라 | ❌ | ✅ | |
| WS 바이너리 프레임 | ❌ (외부) | ✅ | |
| WS 압축 permessage-deflate | ❌ | ⚠️ | |
| SSE 서버 | ⚠️ `Flush`+루프 직접 | ⚠️ `ReadableStream` 등 직접 | 둘 다 “가능하나 수동” |
| SSE 클라 | ⚠️ | ⚠️ `EventSource`(환경) | |
| long-polling | ⚠️ HTTP로 구현 | ⚠️ | |
| MQTT | ❌ | ❌ | |
| gRPC streaming | ❌ | ❌ | |

**요약:** **실시간 양방향은 Bun만 네이티브.** Go는 std만이면 WS ❌ → 외부 또는 Hijack+직접 구현(비추).

---

## 5. 요청 바디 · 업로드 · 다운로드

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| raw body 읽기 | ✅ `io.ReadAll` / Copy | ✅ `req.arrayBuffer` / text | |
| body 크기 제한 | ✅ `MaxBytesReader` | ⚠️ 직접 가드 | **Go가 한줄 방어 명확** |
| JSON body | ✅ | ✅ `req.json()` | |
| `x-www-form-urlencoded` | ✅ `ParseForm` | ✅ `formData` / URLSearchParams | |
| `multipart/form-data` 파싱 | ✅ `ParseMultipartForm`, `FormFile` | ✅ `req.formData()`, `File` | 둘 다 ✅ |
| multipart 메모리/디스크 분기 | ✅ `maxMemory` | ⚠️ 런타임 동작 | Go API가 명시적 |
| 파일 저장 스트리밍 | ✅ `io.Copy` | ✅ `Bun.write(path, file)` | |
| 여러 파일 필드 | ✅ | ✅ | |
| 다운로드 한 파일 | ✅ `ServeFile` | ✅ `new Response(Bun.file)` | |
| Range / 부분 콘텐츠 | ✅ `ServeContent` **자동** | ✅ 되는 편 (스모크 권장) | **Go가 검증·관례 강함** |
| If-Modified-Since 등 | ✅ ServeContent | ⚠️ | Go |
| Content-Disposition | ✅ 수동 헤더 | ✅ 수동 | |
| HEAD 요청 | ✅ | ✅ | |
| chunked encoding | ✅ | ✅ | |

**요약:** JSON·업로드·다운로드 **둘 다 ✅**. 대용량·Range·캐시 헤더는 **Go ServeContent**가 더 안심.

---

## 6. 데이터 포맷 입출력

| 포맷 | Go | Bun | API·차이 |
|---|---|---|---|
| JSON encode/decode | ✅ `encoding/json` | ✅ `JSON` | 둘 다 충분 |
| JSON 스트리밍 Decoder | ✅ | ⚠️ | Go `Decoder` 토큰 단위 |
| `DisallowUnknownFields` | ✅ | ⚠️ 수동 | Go |
| XML | ✅ `encoding/xml` | ⚠️ | **Go만 두꺼움** |
| CSV | ✅ `encoding/csv` | ⚠️ split 수동 | **Go만** |
| Base64 | ✅ | ✅ | |
| Hex | ✅ `encoding/hex` | ✅ | |
| gob (Go 전용 바이너리) | ✅ | ❌ | Go만 |
| YAML | ❌ | ✅ `Bun.YAML` | **Bun만** |
| TOML | ❌ | ⚠️ 버전 | |
| Protobuf | ❌ | ❌ | |
| MessagePack / CBOR | ❌ | ❌ | |
| JSON Lines | ⚠️ 루프로 | ⚠️ | |
| query string | ✅ `url.Values` | ✅ `URLSearchParams` | |

**요약:** 웹 JSON은 동률. **XML/CSV/gob → Go. YAML → Bun.**

---

## 7. 파일시스템 · 압축 · 아카이브

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| 읽기/쓰기/삭제 | ✅ `os` | ✅ `Bun.file` / `fs` 호환 | |
| 디렉터리 순회 | ✅ `filepath.Walk` | ✅ | |
| 권한 chmod | ✅ | ✅ | |
| 임시 파일 | ✅ `os.CreateTemp` | ✅ | |
| 임베드(바이너리에 파일 포함) | ✅ `embed` | ⚠️ 번들 시 포함 | **Go embed가 명확** |
| 심볼릭 링크 | ✅ | ✅ | |
| 파일 락 | ⚠️ OS별 | ⚠️ | |
| gzip | ✅ `compress/gzip` | ✅ | |
| flate / zlib / lzw | ✅ | ⚠️ | Go |
| bzip2 | ✅ `compress/bzip2` | ⚠️ | Go |
| zip 읽기/쓰기 | ✅ `archive/zip` | ⚠️ | **Go 우세** |
| tar | ✅ `archive/tar` | ⚠️ | **Go 우세** |
| 스트림 복사 | ✅ `io.Copy` | ✅ | |
| 파일 watch | ❌ | ✅ 계열 | **Bun** |
| mmap | ❌ | ⚠️ | |

---

## 8. 이미지 · 오디오 · 비디오

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| 파일을 그냥 저장/전송 | ✅ | ✅ | **둘 다 OK (당신 핵심)** |
| JPEG 디코드→픽셀 | ✅ `image/jpeg` | ❌ | **Go만** |
| PNG | ✅ | ❌ | Go |
| GIF | ✅ | ❌ | Go |
| 이미지 크기 읽기(디코드) | ✅ | ❌ 메타만 어렵 | |
| 드로잉(사각형 등) | ✅ `image/draw` | ❌ | Go |
| 리사이즈 고품질 | ❌ std만으론 빈약 | ❌ | 둘 다 약함 |
| WebP/AVIF | ❌ | ❌ | |
| EXIF | ❌ | ❌ | |
| 오디오 디코드 | ❌ | ❌ | |
| 비디오 디코드/트랜스코드 | ❌ | ❌ | ffmpeg exec 우회 |
| MIME 추정 | ✅ `http.DetectContentType` (스니프) | ⚠️ | Go 간단 스니프 |

**요약:** “올리기·받기” ✅✅. “이미지 가공”은 **Go가 기본 포맷 디코드만** 네이티브, 그 이상 둘 다 ❌.

---

## 9. 암호화 · 인증 · 비밀

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| TLS | ✅ | ✅ | |
| SHA-1/256/512 | ✅ `crypto/sha*` | ✅ | |
| BLAKE2 등 | ✅ 일부 | ✅ hasher | |
| MD5 | ✅ (비권장 용도) | ✅ | |
| HMAC | ✅ `crypto/hmac` | ✅ | |
| AES-GCM 등 | ✅ `crypto/cipher` | ✅ Web Crypto | |
| ChaCha20-Poly1305 | ✅ | ⚠️ | |
| RSA | ✅ `crypto/rsa` | ✅ | |
| ECDSA/Ed25519 | ✅ | ✅ | |
| X.509 파싱 | ✅ `crypto/x509` | ⚠️ | **Go 강함** |
| 난수 | ✅ `crypto/rand` | ✅ | |
| bcrypt | ❌ std | ✅ `Bun.password` | **Bun만** |
| argon2 | ❌ std | ✅ `Bun.password` | **Bun만** |
| scrypt | ✅ `golang.org/x`는 외부 / std `crypto/scrypt` ✅ | ⚠️ | Go `crypto/scrypt`는 **std** |
| PBKDF2 | ✅ | ✅ | |
| JWT 생성/검증 풀 | ❌ | ❌ | HMAC 수동은 가능 |
| OAuth2/OIDC | ❌ | ❌ | |
| OTP(TOTP) | ❌ | ❌ | |

**요약:** 암호 프리미티브는 Go가 더 넓고 문서화됨. **비밀번호 해시 UX는 Bun.** JWT/OAuth는 둘 다 풀세트 ❌.

---

## 10. 데이터베이스 · 캐시 · 오브젝트 스토리지

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| DB 추상 API | ✅ `database/sql` | — | Go는 **드라이버 없으면 빈 껍데기** |
| SQLite | ❌ 드라이버 외부 | ✅ `bun:sqlite` | **Bun만 진짜 네이티브** |
| PostgreSQL | ❌ | ⚠️ `Bun.sql` 등 버전 | |
| MySQL | ❌ | ⚠️ | |
| 트랜잭션 API | ✅ (드라이버 있을 때) | ✅ sqlite | |
| Redis | ❌ | ⚠️ `Bun.redis` | Bun 버전 확인 |
| S3 호환 | ❌ | ⚠️ `Bun.s3` | Bun 버전 확인 |
| 로컬 KV(내장) | ❌ | ⚠️ | 파일/sqlite로 대체 |
| ORM | ❌ | ❌ | |
| 마이그레이션 툴 | ❌ | ❌ | |

**요약:** 폐쇄망·패키지 0이면 **로컬 DB는 Bun SQLite가 유일한 편한 길.** Go는 파일·직접 포맷 또는 드라이버 vendor 반입.

---

## 11. 저수준 네트워크 · DNS · 메일

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| TCP listen/dial | ✅ `net` | ✅ `Bun.listen` / `connect` | |
| UDP | ✅ | ✅ | |
| Unix domain socket | ✅ | ✅ | |
| DNS Lookup | ✅ | ✅ | |
| Resolver 커스텀 | ✅ | ⚠️ | Go |
| IP 파싱 | ✅ `net.ParseIP` | ✅ | |
| CIDR | ✅ | ⚠️ | Go |
| raw socket | ⚠️ OS·권한 | ⚠️ | |
| SMTP 송신 | ⚠️ `net/smtp` | ❌~⚠️ | Go만 제한적 내장 |
| POP3/IMAP | ❌ | ❌ | |
| FTP | ❌ | ❌ | |
| SSH 클라 | ❌ | ❌ | |
| QUIC 저수준 | ❌ | ⚠️ | |

---

## 12. 동시성 · 프로세스 · OS

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| 동시 요청 처리 | ✅ goroutine | ✅ 이벤트 루프+네이티브 | |
| Mutex/RWMutex | ✅ | ⚠️ Atomics·단일스레드 가정 | 모델이 다름 |
| Channel | ✅ | ❌ (언어에 없음) | **Go만** |
| WaitGroup | ✅ | ⚠️ Promise.all | |
| Context 취소·기한 | ✅ `context` | ✅ AbortSignal | |
| 타이머 | ✅ `time` | ✅ | |
| 워커 풀 패턴 | ✅ 직접 | ⚠️ | |
| Worker 스레드 | — (goroutine) | ✅ `Worker` | Bun/Web |
| 서브프로세스 | ✅ `os/exec` | ✅ `Bun.spawn` | |
| 파이프 stdin/out | ✅ | ✅ | |
| env | ✅ | ✅ | |
| 시그널 SIGINT 등 | ✅ | ✅ | |
| PID/UID | ✅ | ✅ | |
| cgo FFI | ⚠️ | — | 크로스컴파일↓ |
| bun:ffi | — | ✅ | **Bun이 FFI UX 단순** |
| plugin(.so) | ⚠️ Linux 등 | — | 비이식 |
| 사용자/그룹 | ✅ | ⚠️ | |

---

## 13. 텍스트 · 로케일 · 시간 · 정규식

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| UTF-8 | ✅ | ✅ | |
| strings / bytes | ✅ | ✅ JS string | |
| 정규식 | ✅ `regexp` RE2 | ✅ JS RegExp | 문법 차이 |
| 시간·타임존 | ✅ `time` | ✅ `Date` / Temporal 일부 | Go 타임존 DB 강함 |
| 기간 Duration | ✅ | ⚠️ | Go |
| strconv / 파싱 | ✅ | ✅ | |
| 유니코드 정규화 | ✅ `unicode/norm` | ⚠️ | Go |
| i18n 메시지 카탈로그 | ❌ | ❌ | |
| locale 정렬 고급 | ⚠️ | ⚠️ | |

---

## 14. 템플릿 · HTML · 이메일 본문

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| HTML 템플릿(이스케이프) | ✅ `html/template` | ❌ 전용 엔진 없음 | **Go만** |
| text/template | ✅ | ⚠️ 문자열 | Go |
| HTML DOM 파서 | ❌ (x/net 외부) | ⚠️ HTMLRewriter 등 | 순수 std면 Go도 ❌ |
| XSS 자동 이스케이프 | ✅ html/template | ⚠️ 수동 | Go |
| Markdown | ❌ | ❌ | |
| PDF 생성 | ❌ | ❌ | |

---

## 15. 관측 · 로그 · 프로파일

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| 구조화 로그 | ✅ `log/slog` | ⚠️ `console` / 직접 | **Go slog** |
| 전통 log | ✅ `log` | ✅ | |
| pprof HTTP | ✅ `net/http/pprof` | ❌ | **Go만** |
| 실행 트레이스 | ✅ `runtime/trace` | ⚠️ | Go |
| 메트릭 Prometheus | ❌ | ❌ | |
| OpenTelemetry | ❌ | ❌ | |

---

## 16. 테스트 · 빌드 · 언어 도구

| 기능 | Go | Bun | 상세 |
|---|---|---|---|
| 단위 테스트 | ✅ `testing` | ✅ `bun test` | |
| 벤치마크 | ✅ `testing.B` | ✅ | |
| 퍼즈 | ✅ | ⚠️ | Go |
| 커버리지 | ✅ | ✅ | |
| 포맷터 | ✅ `gofmt` | ✅ bun 포맷 | |
| 타입 검사 | ✅ 컴파일 | ✅ `tsc`급 내장 검사(TS) | |
| TS 실행 | ❌ | ✅ | **Bun만** |
| JSX 실행 | ❌ | ✅ | Bun |
| 번들러 | ❌ | ✅ | Bun |
| minify | ❌ | ✅ | |
| watch 재실행 | ❌ std | ✅ `--watch` | Bun |
| 크로스컴파일 | ✅ `GOOS/GOARCH` | — (런타임별 설치) | **Go 강점** |
| 단일 실행 파일 | ✅ | ⚠️ | Go |
| 모듈 다운로드 없이 std만 빌드 | ✅ | ✅ (npm 안 쓰면) | |

---

## 17. 차이만 모아보기 (Go만 / Bun만)

### Go만 ✅ (Bun은 ❌ 또는 확연히 약함)

| 기능 | 설명 |
|---|---|
| WebSocket **없이**도 Hijack 등 저수준 HTTP | |
| `ServeContent`급 Range/캐시 관례 | 파일 HTTP |
| `encoding/xml`, `encoding/csv`, `gob` | |
| `image/jpeg|png|gif` + draw | |
| `archive/zip|tar`, 다양한 `compress/*` | |
| `html/template` | |
| `crypto/x509` 등 PKI 세밀 | |
| `net/http/pprof`, `slog` | |
| `embed`, 크로스컴파일, 단일 바이너리 | |
| `database/sql` 인터페이스 자체 | 드라이버는 별개 |
| Channel · 풍부한 sync | |
| `net/smtp` (제한적) | |

### Bun만 ✅ (Go std는 ❌ 또는 확연히 약함)

| 기능 | 설명 |
|---|---|
| **WebSocket 서버/클라** | |
| **`bun:sqlite`** | |
| **`Bun.YAML`** | |
| **`Bun.password` (bcrypt/argon2)** | |
| **TS/JSX 직접 실행** | |
| **번들러·내장 테스트 러너·watch** | |
| Web `fetch` / `FormData` / `File` UX | Go도 가능하나 API 형태 다름 |
| `bun:ffi` UX | |
| 파일 watch | |
| (버전) S3 / Redis / SQL 헬퍼 | 문서 확인 |

### 동률 ✅ (당신 핵심 시나리오)

| 기능 |
|---|
| HTTP 서버·클라, TLS |
| JSON 주고받기 |
| multipart 업로드 · 파일 저장 |
| 파일 다운로드·스트리밍 |
| gzip |
| 서브프로세스, env, 시그널 |
| 해시·HMAC·AES 등 기본 암호 |

---

## 18. 둘 다 ❌ 인 것

네이티브(std / Bun 내장)만으로는 보통 포기:

| 기능 | 대안(폐쇄망) |
|---|---|
| gRPC | 반입 바이너리/미리 vendor (정책 위반 여지) |
| GraphQL 풀서버 | 직접 최소 구현 또는 포기 |
| JWT/OAuth 풀 라이브러리 | HMAC JWT 수동 or 반입 |
| ORM · 마이그레이션 프레임워크 | SQL 직접 |
| WebP/AVIF·ffmpeg급 처리 | OS에 ffmpeg 반입 후 exec |
| Prometheus/OTel SDK | 로그·간단 노출 직접 |
| Electron급 데스크톱 | 범위 밖 |

---

## 19. 선택 가이드

| 필요한 네이티브 능력 | 추천 |
|---|---|
| JSON + 파일 HTTP만 | **Go 또는 Bun** (동률) |
| + WebSocket | **Bun** |
| + SQLite (패키지 없이) | **Bun** |
| + YAML 설정 | **Bun** |
| + bcrypt 쉽게 | **Bun** |
| + 이미지 디코드/간단 처리 | **Go** |
| + zip/tar 서버 측 패킹 | **Go** |
| + HTML 템플릿 페이지 | **Go** |
| + pprof·운영 타임아웃 정석 | **Go** |
| + TS로 작성 | **Bun** |
| + USB에 실행파일만 | **Go** |

---

## 부록 — 웹서버 한 줄 결론

외부 디펜던시 0 기준:

- **둘 다 되는 것:** HTTP JSON API, 파일 업/다운, TLS, fetch/클라 호출.  
- **Bun만 쉽게 되는 것:** WebSocket, SQLite, YAML, password hash, TS.  
- **Go만 두꺼운 것:** XML/CSV, 이미지 기본 코덱, zip/tar, html/template, ServeContent·Server 타임아웃·pprof·단일 바이너리.

> Bun 내장 API는 버전마다 늘어납니다. 폐쇄망에 고정한 버전에서 `bun --revision`과 Runtime API로 SQLite/S3/Redis/SQL 항목을 재확인하세요.  
> Go는 `golang.org/x/...`를 쓰면 이미 “순수 std”가 아닙니다.
