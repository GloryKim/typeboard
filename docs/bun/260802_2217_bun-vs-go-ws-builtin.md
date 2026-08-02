# Bun vs Go+WS — 기본 지원 기능 비교

> 전제 (이 문서만의 가정):
> - **Bun** = 바이너리에 들어 있는 API만 (`bun add` / npm 없음)
> - **Go+WS** = Go 표준 라이브러리 + **설치 시 기본으로 따라온다고 가정한** WebSocket 패키지  
>   → `gorilla/websocket` **또는** `coder/websocket` (둘 중 하나·또는 둘 다 “기본 툴킷”으로 취급)
> - 그 외 `golang.org/x/...`, gin, chi, redis 클라 등은 **여전히 외부**로 두고 비교하지 않음
>
> 목적: **각자 “기본으로 쓸 수 있는 내부 함수·API”**만으로 무엇이 되는지 대조  
> 관련: 순수 std만 본 문서 → [`go-std-vs-bun-native-scope.md`](./go-std-vs-bun-native-scope.md) (거기선 Go WS = ❌)

범례:

| 기호 | 의미 |
|---|---|
| ✅ | 기본 툴킷으로 실무 사용 가능 |
| ⚠️ | 가능하지만 수동·제한·버전/패키지 차이 |
| ❌ | 이 전제만으로는 사실상 불가 |
| G | gorilla/websocket 기준 |
| C | coder/websocket 기준 |
| B | Bun 내장 |

---

## 목차

1. [한 줄 결론](#1-한-줄-결론)
2. [비교 범위](#2-비교-범위)
3. [WebSocket — 핵심 비교](#3-websocket--핵심-비교)
4. [WebSocket API 함수 대응표](#4-websocket-api-함수-대응표)
5. [HTTP · 파일 · 포맷 (WS 외 기본기)](#5-http--파일--포맷-ws-외-기본기)
6. [동시성 · 운영 · 배포](#6-동시성--운영--배포)
7. [gorilla vs coder (Go 쪽만)](#7-gorilla-vs-coder-go-쪽만)
8. [차이만 모아보기](#8-차이만-모아보기)
9. [선택 가이드](#9-선택-가이드)
10. [폐쇄망(Air-gapped) 적합성](#10-폐쇄망air-gapped-적합성)
11. [팩트체크 결과 (2026-08 기준)](#11-팩트체크-결과-2026-08-기준)

---

## 1. 한 줄 결론

| 질문 | 답 |
|---|---|
| Go에 WS를 “기본 설치”로 치면 Bun만의 WS 우위가 사라지나? | **양방향 실시간 자체는 동률 ✅.** 다만 **내장 pub/sub·백프레셔 UX**는 여전히 **Bun**이 두껍다. |
| “기본 함수만”으로 채팅방/브로드캐스트? | Bun: `subscribe`/`publish` 한 줄. Go+WS: **맵·뮤텍스·허브 직접** (라이브러리는 커넥션 I/O만). |
| HTTP JSON + 파일 + WS 한 서버? | **둘 다 ✅** |
| 그 외 기본기 (XML/CSV/이미지/zip/pprof/단일 바이너리) | **여전히 Go std가 두꺼움** |
| SQLite / YAML / bcrypt / TS 실행 | **여전히 Bun만** (이 전제에서 Go는 외부 추가 필요) |

```text
순수 Go std:     HTTP ✅  /  WS ❌
이 문서 Go+WS:   HTTP ✅  /  WS ✅ (커넥션 단위)
Bun 내장:        HTTP ✅  /  WS ✅ + 토픽 pub/sub 내장
```

---

## 2. 비교 범위

| 포함 | 제외 |
|---|---|
| Bun Runtime API (`Bun.serve`, `ServerWebSocket`, `fetch`, sqlite, YAML…) | npm / `bun add` |
| Go `std` | `golang.org/x/...` (명시하지 않는 한) |
| gorilla **또는** coder WebSocket의 **공개 API** | gin, echo, chi, socket.io, redis pub/sub |
| “함수·옵션으로 바로 되는가” | 벤치 RPS 절대값 (환경 의존) |

WS 패키지 가정은 이렇게 읽으면 됩니다.

```text
[Go 설치] ──가정──▶ std + (gorilla | coder) 가 “기본 도구상자”
[Bun 설치] ───────▶ 런타임에 HTTP+WS 이미 포함
```

---

## 3. WebSocket — 핵심 비교

### 3.1 서버: 연결·메시지·종료

| 기능 | Bun (B) | Go+gorilla (G) | Go+coder (C) | 비고 |
|---|---|---|---|---|
| HTTP → WS 업그레이드 | ✅ `server.upgrade(req)` | ✅ `Upgrader.Upgrade` | ✅ `websocket.Accept` | |
| 서버 핸들러 모델 | ✅ 서버당 공유 `open/message/close/drain` | ✅ 연결마다 goroutine+루프 | ✅ 동일 (context 루프) | Bun은 콜백 공유가 기본 |
| 텍스트 메시지 | ✅ | ✅ `TextMessage` | ✅ `MessageText` | |
| 바이너리 메시지 | ✅ | ✅ `BinaryMessage` | ✅ `MessageBinary` | |
| 송신 | ✅ `ws.send` / `sendText` / `sendBinary` | ✅ `WriteMessage` / `NextWriter` | ✅ `Write` / `WriteTimeout` | |
| 수신 | ✅ `message` 콜백 | ✅ `ReadMessage` / `NextReader` | ✅ `Read` | |
| 연결 종료 | ✅ `ws.close` + `close` 핸들러 | ✅ `Close` / `WriteControl` | ✅ `Close` / `CloseNow` | |
| 연결별 임의 데이터 | ✅ `upgrade(..., { data })` → `ws.data` | ⚠️ 직접 구조체/맵 | ⚠️ 동일 | Bun이 UX 단순 |
| Origin 검사 | ⚠️ 직접 | ✅ `CheckOrigin` | ✅ `OriginPatterns` | **Go WS가 옵션 명확** |
| Subprotocol | ⚠️ | ✅ | ✅ | Go |
| TLS 위 WS (`wss`) | ✅ (서버 TLS) | ✅ | ✅ | |
| 클라 WebSocket | ✅ 전역 `WebSocket` | ✅ `Dial` / `Dialer` | ✅ `Dial` | |

### 3.2 제어 프레임 · 압축 · 기한

| 기능 | Bun | gorilla | coder | 비고 |
|---|---|---|---|---|
| Ping / Pong | ⚠️ 런타임·버전 확인 (자동 keepalive 성격) | ✅ `WriteControl` + Ping/PongHandler | ✅ `Ping` (idiomatic) | **Go가 제어권 큼** |
| Read/Write deadline | ⚠️ | ✅ `SetReadDeadline` 등 | ✅ `context` deadline | **Go 운영 세밀** |
| permessage-deflate | ✅ 옵션·on-the-fly | ⚠️ no context takeover 위주 | ✅ 풀에 가깝게 | coder ≥ Bun 감각, gorilla는 제한 |
| 백프레셔 | ✅ `send` 반환값 + `drain` | ⚠️ 버퍼/직접 | ⚠️ | **Bun이 서버 API로 노출** |
| 동시 write 안전 | ✅ (런타임 관리) | ❌ 패닉 위험 → 락/허브 필수 | ✅ 내부 처리 | **gorilla 함정** |

### 3.3 브로드캐스트 · 룸 (결정적 차이)

| 기능 | Bun | Go+WS (G/C) |
|---|---|---|
| 토픽 구독 | ✅ `ws.subscribe(topic)` | ❌ → `map[topic]set(conn)` 직접 |
| 구독 해제 | ✅ `unsubscribe` | ❌ 직접 |
| 구독 여부 | ✅ `isSubscribed` | ❌ 직접 |
| 현재 구독 목록 | ✅ `ws.subscriptions` | ❌ 직접 |
| 토픽 발행 (자신 제외) | ✅ `ws.publish(topic, data)` | ❌ 직접 루프 |
| 토픽 전체 발행 | ✅ `server.publish(topic, data)` | ❌ 허브 패턴 |
| 구독자 수 | ✅ `server.subscriberCount(topic)` | ❌ 직접 |
| cork/배치 송신 | ✅ `ws.cork` | ⚠️ PreparedMessage(G) 등 | |

**요약:**  
- **1:1 에코·단일 클라 push** → Bun / Go+WS **동률**.  
- **채팅방·채널·룸** → Bun은 **내장 함수**, Go+WS는 **앱 코드로 허브 구현**이 “기본”.

```text
Bun:   upgrade → subscribe("room") → publish("room", msg)
Go+WS: Accept/Upgrade → 레지스트리.Register → hub.Broadcast (직접 작성)
```

---

## 4. WebSocket API 함수 대응표

“기본으로 있는 이름”만 대응. 완벽 1:1은 아님.

### 4.1 서버 생명주기

| 하고 싶은 일 | Bun | gorilla | coder |
|---|---|---|---|
| 업그레이드 | `server.upgrade(req, { data })` | `upgrader.Upgrade(w, r, hdr)` | `websocket.Accept(w, r, opts)` |
| 열림 | `websocket.open(ws)` | Upgrade 성공 직후 | Accept 성공 직후 |
| 메시지 | `websocket.message(ws, msg)` | `conn.ReadMessage()` 루프 | `conn.Read(ctx)` 루프 |
| 닫힘 | `websocket.close(ws, code, reason)` | read err / Close | `Close` / read err |
| 송신 가능 재개 | `websocket.drain(ws)` | (직접) | (직접) |
| 에러 | `websocket.error` (있으면) | `error` 반환 | `error` 반환 |

### 4.2 송수신

| 하고 싶은 일 | Bun | gorilla | coder |
|---|---|---|---|
| 텍스트 보내기 | `ws.send(str)` / `sendText` | `WriteMessage(TextMessage, b)` | `Write(ctx, MessageText, b)` |
| 바이너리 보내기 | `ws.send(buf)` / `sendBinary` | `WriteMessage(BinaryMessage, b)` | `Write(ctx, MessageBinary, b)` |
| JSON 보내기 | `JSON.stringify` + send | 수동 또는 헬퍼 | ✅ `wsjson.Write` |
| JSON 받기 | `JSON.parse` | 수동 | ✅ `wsjson.Read` |
| 스트림형 writer | — | ✅ `NextWriter` | ⚠️ / NetConn 래핑 |
| 준비된 메시지 재사용 | cork / publish | ✅ `PreparedMessage` | 버퍼 재사용 설계 |

### 4.3 Pub/Sub (Bun만 “함수”)

| 하고 싶은 일 | Bun | Go+WS |
|---|---|---|
| 룸 참가 | `ws.subscribe("room-1")` | 허브 `Join(room, conn)` 직접 |
| 룸 퇴장 | `ws.unsubscribe("room-1")` | `Leave` 직접 |
| 룸에 말하기 | `server.publish("room-1", msg)` | `Broadcast(room, msg)` 직접 |
| 자기 제외 전파 | `ws.publish(...)` | 허브에서 sender 스킵 직접 |

### 4.4 최소 예 (감각)

**Bun**

```ts
Bun.serve({
  fetch(req, server) {
    if (server.upgrade(req, { data: { room: "lobby" } })) return;
    return new Response("ok");
  },
  websocket: {
    open(ws) {
      ws.subscribe(ws.data.room);
    },
    message(ws, message) {
      ws.publish(ws.data.room, message);
    },
    close(ws) {
      ws.unsubscribe(ws.data.room);
    },
  },
});
```

**Go + coder (허브는 직접)**

```go
// Accept → Read 루프 → 브로드캐스트는 map+mutex 또는 hub 채널로 직접 구현
// WS 라이브러리가 주는 것: Accept, Read, Write, Close, Ping
// WS 라이브러리가 안 주는 것: subscribe/publish 토픽 API
```

**Go + gorilla** — 동일하게 I/O만 제공. 동시 `WriteMessage`는 락 필수.

---

## 5. HTTP · 파일 · 포맷 (WS 외 기본기)

WS를 기본으로 넣어도 **나머지는 기존 std vs Bun 내장과 동일**. 요약만.

| 대분류 | Bun | Go+WS (= std + WS) | 승자·메모 |
|---|---|---|---|
| HTTP 서버 listen | ✅ `Bun.serve` | ✅ `net/http` | 동률 |
| 라우팅 패턴 | ⚠️ 직접/버전 | ✅ Go1.22+ ServeMux | Go |
| Graceful / 타임아웃 필드 | ⚠️ | ✅ `Server`·`Shutdown` | Go |
| HTTP 클라 | ✅ `fetch` | ✅ `Client` | 동률 |
| JSON | ✅ | ✅ | 동률 |
| multipart 업/다운 | ✅ | ✅ | 동률 |
| Range 파일 | ✅ | ✅ `ServeContent` | Go 관례 |
| YAML | ✅ `Bun.YAML` | ❌ | **Bun** |
| XML / CSV | ⚠️ | ✅ | **Go** |
| SQLite | ✅ `bun:sqlite` | ❌ | **Bun** |
| bcrypt/argon2 | ✅ `Bun.password` | ❌ std | **Bun** |
| 이미지 JPEG/PNG 디코드 | ❌ | ✅ `image/*` | **Go** |
| zip/tar | ⚠️ | ✅ | **Go** |
| html/template | ❌ | ✅ | **Go** |
| pprof | ❌ | ✅ | **Go** |
| TS 실행·번들·`bun test` | ✅ | ❌ | **Bun** |
| 단일 바이너리·크로스컴파일 | ⚠️ | ✅ | **Go** |
| **WebSocket** | ✅ 내장+pub/sub | ✅ 라이브러리 I/O | **용도별로 갈림** |

자세한 비-WS 표는 [`go-std-vs-bun-native-scope.md`](./go-std-vs-bun-native-scope.md)와 같고, **§4 WebSocket 행만 ❌→✅로 바뀐 버전**이 이 문서다.

---

## 6. 동시성 · 운영 · 배포

| 항목 | Bun | Go+WS |
|---|---|---|
| 연결당 모델 | 이벤트 루프 + 네이티브 WS | goroutine(들) per conn |
| 방송 동시성 | 런타임 pub/sub | 허브 한 곳 또는 coder 동시 write |
| 취소·기한 | AbortSignal / 옵션 | `context` (특히 coder) |
| 관측 | console 위주 | `slog` + `net/http/pprof` |
| 배포물 | 플랫폼별 `bun` + 소스 | 타깃별 바이너리 (WS 패키지 링크됨) |
| 폐쇄망·의존 | Bun 바이너리만이면 WS 포함 | Go 툴체인 + 모듈 캐시(또는 vendor)에 WS 필요 — **“설치 기본” 가정 시 OK** |

속도 감각 (방향만, 절대값 비주장):

```text
고부하 장시간·메모리 예측:  종종 Go+WS
개발 속도·룸 기능 빨리:      Bun
단순 에코 RPS:               둘 다 상위권 (환경 의존)
```

HTTP만의 속도 논의는 [`bun-http-vs-go-speed.md`](./bun-http-vs-go-speed.md).

---

## 7. gorilla vs coder (Go 쪽만)

이 문서에서 Go+WS를 하나로 묶되, **기본 함수 차이**만 짧게.

| | gorilla/websocket | coder/websocket |
|---|---|---|
| 상태 | **2022-12 아카이브 → 2023-07 신규 메인테이너로 부활, 현재 활성 유지** (v1.5.x). 예제·튜토리얼 압도적으로 많음 | 신규에 많이 추천, context 중심 (과거 `nhooyr.io/websocket`에서 이관) |
| 핵심 API | `Upgrade`, `ReadMessage`, `WriteMessage` | `Accept`, `Dial`, `Read`, `Write` |
| context | ❌ deadline 수동 | ✅ 전반 |
| 동시 write | ❌ 동기화 필수 | ✅ |
| JSON 헬퍼 | 수동 | ✅ `wsjson` |
| Ping API | Handler 등록 패턴 | ✅ `Ping` |
| 압축 | 제한적 | 더 풀 |
| PreparedMessage | ✅ | — |
| 토픽 pub/sub | ❌ | ❌ |

**실무 가정:** “Go 설치 시 WS가 무조건 있다”면  
- 새 코드 → **coder**를 기본으로 상상하는 편이 함수 모델이 현대적  
- 기존 튜토리얼·허브 예제 → **gorilla**가 더 흔함  
둘 다 **룸 API는 없음**.

---

## 8. 차이만 모아보기

### Bun만 “기본 함수”로 두꺼운 것

| 기능 |
|---|
| WS **토픽** `subscribe` / `unsubscribe` / `publish` / `subscriberCount` |
| WS `drain` · `cork` · send 백프레셔 반환 |
| `ws.data`로 업그레이드 시 상태 주입 |
| `bun:sqlite`, `Bun.YAML`, `Bun.password` |
| TS/JSX 실행, 번들, `--watch`, `bun test` |

### Go+WS만 “기본”으로 두꺼운 것 (std + WS)

| 기능 |
|---|
| WS **Origin/Subprotocol/Deadline/Ping** 세밀 제어 (특히 gorilla 옵션·coder context) |
| `ServeContent`, Server 타임아웃, `Shutdown` |
| `encoding/xml`·`csv`, `image/*`, `archive/zip|tar` |
| `html/template`, `log/slog`, `net/http/pprof` |
| `embed`, `GOOS/GOARCH` 단일 바이너리 |
| channel · 풍부한 `sync` (허브 구현에 유리) |

### 이 전제에서 **동률 ✅**

| 기능 |
|---|
| HTTP JSON API |
| multipart 업로드 · 파일 저장·다운로드 |
| TLS / WSS |
| WebSocket **연결·텍스트/바이너리·에코** |
| 해시·HMAC·AES 등 기본 암호 |
| 서브프로세스 · env · 시그널 |

### 여전히 둘 다 ❌ (이 전제)

> “WS 커넥션 I/O + (Bun만) 토픽 pub/sub”까지가 기본 제공의 끝.  
> 아래는 **실시간 서버를 실제로 굴릴 때 반드시 필요해지지만, 둘 다 앱 코드나 외부 인프라로 직접 채워야 하는** 것들이다.

#### 확장 · 인프라

| 기능 | 메모 |
|---|---|
| 멀티 프로세스/멀티 노드 WS 동기화 | Bun pub/sub도 **프로세스 로컬(단일 프로세스)** → 노드 간 브로드캐스트는 Redis/NATS 등 백플레인 필요. Go 허브도 동일하게 한 프로세스 안에서만 유효 |
| 수평 확장 시 sticky session / 라우팅 | LB 레벨 설정이지 런타임 기능 아님 → 둘 다 밖에서 처리 |
| 백프레셔의 **클러스터 차원** 관리 | 느린 소비자를 노드 경계 너머로 흘려보내는 로직은 직접 |

#### 프로토콜 · 전송

| 기능 | 메모 |
|---|---|
| socket.io 호환 프로토콜 | 양쪽 다 별도 (프레이밍·핸드셰이크 다름) |
| **long-polling / SSE 폴백** | 방화벽·프록시가 WS를 끊는 폐쇄망/사내망에서 중요한데, 둘 다 자동 폴백 없음 (socket.io엔 있음) |
| **클라이언트 자동 재연결 + 상태 복원** | 브라우저·Bun·Go의 WS 클라 모두 끊기면 그걸로 끝. 재연결 백오프·resume은 앱이 구현 |
| **메시지 전달 보장(ACK/at-least-once)·순서 보장·재전송** | WS는 “TCP 위 프레임”만 보장. 앱 레벨 ack/dedup/재시도는 없음 |
| **오프라인 큐 / 히스토리 리플레이** | 끊겼던 클라가 놓친 메시지 받기 → 저장·재생 로직 직접 (Bun의 pub/sub는 “현재 접속자”에게만) |
| WebTransport / HTTP-3, RFC 8441(HTTP/2 위 WS) | 둘 다 기본 미지원 |

#### 애플리케이션 계층

| 기능 | 메모 |
|---|---|
| **Presence(온라인 여부·타이핑 표시)** | 접속/이탈 집계·전파 직접. Bun `subscriberCount`가 *부분* 도움일 뿐 사용자 단위 presence는 아님 |
| **연결별 rate limit / flood 제어** | 악성·폭주 클라 차단 로직 기본 없음 (Bun `maxPayloadLength`·`idleTimeout`은 크기/유휴만 커버) |
| **인증·인가 프레임워크** | 업그레이드 시 쿠키/헤더는 읽지만, 세션·권한·JWT 검증은 직접. HMAC/해시만 std/Bun 내장 |
| JWT/OAuth **풀 라이브러리** | 서명·검증을 수동 조립만 |
| **구조화된 RPC/요청-응답 상관(correlation)** | req↔res 매칭, 스트림 다중화(JSON-RPC 류) 프레이밍 직접 |
| gRPC / GraphQL(구독 포함) 풀세트 | |
| ffmpeg급 미디어(트랜스코딩·WebRTC SFU) | |

---

## 9. 선택 가이드

| 필요한 “기본” 능력 | 추천 |
|---|---|
| WS 에코·알림 push·소수 연결 | **Bun 또는 Go+WS** |
| 채팅 룸/채널을 **라이브러리 함수로** | **Bun** |
| WS + Origin·기한·pprof·단일 바이너리 운영 | **Go+WS** |
| WS + SQLite/YAML/TS를 패키지 없이 | **Bun** |
| WS + zip/이미지/HTML 템플릿을 패키지 없이 | **Go+WS** |
| “허브 코드 직접 짜기 싫다” | **Bun** |
| “런타임 없이 바이너리만 USB” | **Go+WS** (크로스컴파일) |

```text
가정: Go에 gorilla|coder 가 기본으로 따라온다

  실시간 연결 자체     →  무승부
  룸/브로드캐스트 UX   →  Bun
  운영·표준 라이브러리 두께 →  Go
  프로토타입·TS       →  Bun
```

---

## 10. 폐쇄망(Air-gapped) 적합성

> 결론부터: **배포물을 밖에서 만들어 “아티팩트만” 반입하는 진짜 폐쇄망이면 Go+WS가 더 적합**하다.  
> Bun은 “앱 전체가 내장 API로 끝나고(=이 문서의 전제) 타깃이 흔한 리눅스/맥 x64·arm64”라는 **좁은 조건에서만** 대등하다.

### 10.1 먼저 짚을 함정 — “Go에 WS가 기본 포함” 가정은 폐쇄망에선 특히 허구

- 순수 Go std엔 WebSocket이 **없다**. 실제로는 `go get github.com/coder/websocket`(또는 gorilla)가 필요하다.
- 폐쇄망엔 인터넷이 없으므로 이 `go get`이 **안 된다**. 해결책은 둘:
  1. **연결된 빌드 머신에서 `go mod vendor`** → `vendor/` 통째로 반입 → 타깃에선 네트워크 0으로 빌드, 또는
  2. 애초에 **연결된 머신에서 정적 바이너리로 컴파일해 그 바이너리만 반입**.
- 반대로 **Bun의 WS·pub/sub는 런타임 바이너리 안에 진짜로 내장**이라, WS 하나만 놓고 보면 Bun은 폐쇄망에서 **추가로 챙길 게 전혀 없다.** (npm 패키지를 안 쓰는 한)

즉 “WS 의존성 조달” 난이도는 **Bun이 유리**, “배포 아티팩트 형태”는 **Go가 유리**. 폐쇄망 판단은 이 둘의 무게 싸움이다.

### 10.2 시나리오 A — 밖에서 빌드하고 아티팩트만 반입 (전형적 폐쇄망)

| 항목 | Go+WS | Bun |
|---|---|---|
| 반입물 | **정적 바이너리 1개** (`CGO_ENABLED=0 go build`, WS는 vendor로 링크되어 이미 포함) | `bun` 런타임(수십 MB) + 소스(.ts/.js) |
| 타깃에 설치할 것 | **없음** (툴체인·런타임 불필요, `scratch` 컨테이너도 가능) | Bun 런타임 present 필요 |
| 크로스 아키텍처 | `GOOS/GOARCH`로 거의 모든 OS·CPU(예: linux/arm64, ppc64le 등) | 프리빌트 타깃 제한 (주로 linux/mac × x64/arm64). 특이 아키텍처면 곤란 |
| 공격 표면 | 정적 바이너리 최소 | 범용 JS 런타임 전체가 상주 → 더 큼 |
| 무결성 검증 | 바이너리 해시 1개만 서명·대조 | 런타임 + 소스 트리 다중 대조 |

→ **이 시나리오는 Go+WS 압승.** “런타임 없이 바이너리만 USB”가 정확히 폐쇄망의 이상적 형태다.

### 10.3 시나리오 B — 폐쇄망 안에서 직접 개발·빌드

| 항목 | Go+WS | Bun |
|---|---|---|
| 사전 반입 | Go 툴체인 + **모듈 캐시 또는 `vendor/`** (WS 포함) | `bun` 런타임만. **내장 API로 끝나면 패키지 0** |
| 오프라인 빌드 | `GOFLAGS=-mod=vendor`, `GOPROXY=off`로 완전 오프라인 | 내장만 쓰면 `bun add` 자체가 불필요 |
| 패키지가 필요해지면 | `go mod vendor` 재수행 후 재반입 (표준·일급 지원) | **사설 레지스트리 미러(Verdaccio 등) 또는 node_modules 통째 반입** 필요 → 운영 부담 |
| 반복 개발 속도 | 컴파일 사이클 | `--watch`·TS 즉시 실행으로 빠름 |

→ **앱이 “내장 API만으로” 끝난다면(이 문서의 전제 그대로) Bun이 가장 깔끔**하다: 챙길 패키지가 0. 단, 한 번이라도 npm 패키지가 끼면 폐쇄망 Bun은 미러 인프라가 필요해 급격히 번거로워진다. Go는 패키지가 생겨도 `vendor` 반입이 표준 절차라 예측 가능하다.

### 10.4 종합 권고

```text
진짜 폐쇄망(반입형)      →  Go+WS  (정적 바이너리 1개, 런타임·설치 0, 최소 공격표면, 최다 아키텍처)
폐쇄망 + 내장 API로 완결  →  Bun    (WS·pub/sub·SQLite·YAML·password까지 패키지 0으로 끝남)
폐쇄망 + npm 패키지 필요  →  Go 우세 (vendor는 일급 / Bun은 레지스트리 미러 필요)
특이 CPU·OS 타깃         →  Go     (크로스컴파일 폭이 넓음)
```

- **보안·운영이 최우선인 폐쇄망(정부·금융·OT/제조 내부망)** → **Go+WS**. 단일 정적 바이너리, 런타임 부재, 해시 하나로 무결성·감사, `scratch`/distroless 컨테이너까지 가능.
- **폐쇄망이지만 요구가 “내장 기능 안에서” 다 해결되고(WS 채팅·SQLite 저장·YAML 설정 등) 타깃이 표준 리눅스** → **Bun**도 충분히 실용적. 오히려 SQLite·password 해시까지 패키지 0으로 끝나 Go보다 조달할 게 적을 수 있다.
- **주의(양쪽 공통):** §8의 “둘 다 ❌” 항목(멀티노드 동기화·자동 재연결·전달 보장·presence 등)은 폐쇄망이라고 면제되지 않는다. 특히 폐쇄망 내부 프록시가 WS를 끊는 경우 **폴백이 없다는 점**을 설계 초기에 고려해야 한다.

---

## 11. 팩트체크 결과 (2026-08 기준)

공식 문서·저장소로 대조한 결과. **핵심 수정 1건(gorilla 상태), 나머지는 사실 확인·보강.**

| 문서 서술 | 검증 | 비고 |
|---|---|---|
| Bun `subscribe`/`unsubscribe`/`publish`/`isSubscribed`/`ws.subscriptions`/`server.publish`/`server.subscriberCount`/`cork`/`ws.data` | ✅ 정확 | 공식 API 레퍼런스에 모두 존재 (`ws.subscriptions`는 `readonly string[]` 프로퍼티) |
| Bun 백프레셔 `send` 반환값 + `drain` | ✅ 정확 | 반환값 `-1`(백프레셔로 큐잉)·`0`(연결 문제로 드롭)·`1+`(전송 바이트). `drain` 핸들러 존재 |
| Bun `ws.publish`는 **자신 제외** 전파 | ✅ 정확 | 서버 전체엔 `server.publish`. `publishToSelf`(기본 false) 옵션으로 자기 포함 전환 가능 |
| Bun Ping/Pong `⚠️` | ✅ 타당 | **자동 keepalive**(`sendPings` 기본 true, `idleTimeout` 기본 120초)만 제공. 수동 `ping()/pong()` 핸들러는 공식 노출 없음 → 세밀 제어는 Go 우위가 맞음 |
| Bun `perMessageDeflate` 압축 | ✅ 정확 | 서버 옵션 + `send(msg, true)`로 개별 압축. **기본 off** |
| coder permessage-deflate “coder ≥ Bun 감각” | ✅ 대체로 | `CompressionMode`(ContextTakeover / NoContextTakeover / **Disabled=기본**) 지원. 양쪽 다 기본 off라 “켜야 함”은 공통 |
| gorilla 동시 write 위험 → 락 필수 | ✅ 정확 | 동시 writer 1·reader 1 원칙. 동시 `WriteMessage`는 앱이 직렬화해야 함 |
| **gorilla “레거시·아카이브”** | ⚠️ **수정함** | 2022-12 아카이브는 맞지만 **2023-07 신규 메인테이너로 부활, 현재 활성 유지**. “죽은 프로젝트” 뉘앙스는 틀림 (§7 반영) |
| coder = 과거 `nhooyr.io/websocket` 이관 | ✅ 정확 | |
| 순수 Go std엔 WS 없음 / 실제론 `go get` 필요 | ✅ 정확 | 문서 말미 경고와 일치. 폐쇄망 함의는 §10에 보강 |

> 추가 확인용 상수(고정 버전에서 재검): Bun 기본 `idleTimeout` 120초, `maxPayloadLength` 16 MB, `backpressureLimit` 16 MB, `sendPings` true.

---

## 부록 — 기존 문서와의 관계

| 문서 | 전제 | Go WS |
|---|---|---|
| [`go-std-vs-bun-native-scope.md`](./go-std-vs-bun-native-scope.md) | Go **순수 std만** | ❌ |
| **이 문서** | Go std + **gorilla\|coder 기본 포함 가정** | ✅ (I/O) / pub/sub는 ❌ |
| [`bun-http-vs-go-speed.md`](./bun-http-vs-go-speed.md) | HTTP 서버 속도 | (WS 비초점) |
| [`bun-vs-go-cross-platform.md`](./bun-vs-go-cross-platform.md) | OS·CPU 이식 | |

> Bun WS·pub/sub API는 버전에 따라 옵션명이 늘어날 수 있음 → 고정 버전에서 `Bun.serve` / `ServerWebSocket` 재확인.  
> Go는 이 문서 가정과 달리 **실제로는** `go get`으로 WS를 받는 것이 정석이며, std만으로는 여전히 WS ❌.
