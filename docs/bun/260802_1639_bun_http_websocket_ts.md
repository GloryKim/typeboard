# Bun 단독 HTTP · WebSocket 상세 교재 (TypeScript · 외부 패키지 0)

> 전제: **`bun add` / npm 패키지 없음.** Bun 바이너리에 포함된 API만 사용한다.  
> 언어: **TypeScript** (`bun run server.ts` — 별도 `tsc`/`ts-node` 불필요).  
> 핵심 API: `Bun.serve`, `server.upgrade`, `ServerWebSocket`, `fetch`, `Response`, `Bun.file`.  
> 함께 두는 실행 예제: [`server.ts`](./server.ts), [`client.ts`](./client.ts) (7절).  
> 선수: HTTP 기초, WebSocket 개념(핸드셰이크·프레임), TS 타입 기초.  
> 관련: [Go std vs Bun 내장 범위](./260802_1629_go-std-vs-bun-native-scope.md)

실행:

```bash
# 최소 WS 페어 (이 폴더)
bun run server.ts
bun run client.ts

# 또는 교재 안의 다른 예제 파일을 만들어
bun run chat-server.ts
bun --hot server.ts   # 파일 변경 시 핫 리로드
```

---

## 목차

1. [왜 Bun만으로 가능한가](#1-왜-bun만으로-가능한가)
2. [최소 HTTP 서버](#2-최소-http-서버)
3. [라우팅 — fetch vs routes](#3-라우팅--fetch-vs-routes)
4. [JSON REST API](#4-json-rest-api)
5. [정적 파일 · HTML 한 방에](#5-정적-파일--html-한-방에)
6. [WebSocket 업그레이드 핵심](#6-websocket-업그레이드-핵심)
7. [저장소 최소 페어 — server.ts · client.ts](#7-저장소-최소-페어--serverts--clientts)
8. [에코 · 채팅방 (subscribe / publish)](#8-에코--채팅방-subscribe--publish)
9. [실전 통합: HTTP + WS + 브라우저 클라이언트](#9-실전-통합-http--ws--브라우저-클라이언트)
10. [Bun 쪽 WebSocket 클라이언트 (심화)](#10-bun-쪽-websocket-클라이언트-심화)
11. [옵션 · 백프레셔 · 타임아웃](#11-옵션--백프레셔--타임아웃)
12. [함정 · 자주 하는 실수](#12-함정--자주-하는-실수)
13. [체크리스트](#13-체크리스트)
14. [부록: 추가로 알아둘 개념](#14-부록-추가로-알아둘-개념)

---

## 1. 왜 Bun만으로 가능한가

### 개념

| 다른 런타임에서 흔히 쓰는 것 | Bun 내장 대체 |
|---|---|
| `express` / `hono`(패키지) | `Bun.serve` + `routes` / `fetch` |
| `ws` 패키지 | `Bun.serve({ websocket })` + `server.upgrade` |
| `cors` 미들웨어 | 헤더를 `Response`에 직접 |
| 별도 TS 트랜스파일 | Bun이 `.ts` 직접 실행 |

한 줄:

> **HTTP와 WebSocket이 같은 `Bun.serve` 프로세스 안에서** 동작한다.  
> WS는 별도 포트가 아니라 **같은 포트에서 HTTP 업그레이드**로 연다.

```text
브라우저 / 클라
   │  HTTP  GET /api/hello
   │  WS    GET /chat  + Upgrade
   ▼
Bun.serve  ── fetch / routes ──▶ Response
           └── upgrade 성공 ──▶ websocket.open/message/close
```

### 체크포인트

- [ ] “외부 패키지 0” = `package.json` dependencies 비어 있어도 됨 (없어도 됨)
- [ ] `bun run *.ts`만으로 실행됨을 확인

---

## 2. 최소 HTTP 서버

### 코드

```ts
// hello-server.ts
const server = Bun.serve({
  port: 3000,
  fetch(_req) {
    return new Response("hello from bun", {
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  },
});

console.log(`listening on ${server.url}`);
```

```bash
bun run hello-server.ts
# curl http://127.0.0.1:3000
```

### 설명

| 항목 | 기본/동작 |
|---|---|
| `port` 생략 | `BUN_PORT` / `PORT` / `NODE_PORT` 또는 **3000** |
| `hostname` 생략 | `0.0.0.0` (모든 인터페이스) |
| `server.url` | `http://localhost:3000` 형태 |
| `port: 0` | OS가 빈 포트 할당 → `server.port`로 확인 |

`export default` 문법도 된다 (`bun server.ts`가 `fetch`를 보고 serve):

```ts
// server.ts
export default {
  port: 3000,
  fetch() {
    return new Response("ok");
  },
};
```

### 체크포인트

- [ ] `curl`로 본문 확인
- [ ] `server.url` / `server.port` 출력

---

## 3. 라우팅 — fetch vs routes

### fetch만 (호환성 넓음)

```ts
Bun.serve({
  port: 3000,
  fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/health") {
      return Response.json({ ok: true });
    }

    if (url.pathname === "/hello" && req.method === "GET") {
      const name = url.searchParams.get("name") ?? "world";
      return new Response(`hello ${name}`);
    }

    return new Response("Not Found", { status: 404 });
  },
});
```

### routes (Bun 1.2.3+ 권장 스타일)

```ts
Bun.serve({
  port: 3000,
  routes: {
    "/health": new Response("OK"),

    "/users/:id": (req) => {
      return Response.json({ id: req.params.id });
    },

    "/api/posts": {
      GET: () => Response.json([{ id: 1, title: "hi" }]),
      POST: async (req) => {
        const body = (await req.json()) as { title?: string };
        return Response.json(
          { created: true, title: body.title ?? null },
          { status: 201 },
        );
      },
    },

    "/api/*": Response.json({ error: "not found" }, { status: 404 }),
  },

  fetch() {
    return new Response("fallback 404", { status: 404 });
  },
});
```

### 설명

- `routes`는 정적 `Response`, 함수, 메서드 맵(`GET`/`POST`…)을 지원한다.
- 와일드카드 `*` 로 API 접두 404를 묶을 수 있다.
- 학습·구버전 호환은 `fetch`만으로도 충분하다.
- **WebSocket 업그레이드는 `server` 핸들이 필요**하다. 보통 `fetch(req, server)`에서 `server.upgrade`를 호출한다.  
  `routes`만 쓰고 `fetch`를 안 두면 WS 경로를 넣기 어렵다 → HTTP는 `routes`, WS는 `fetch`에 두는 조합이 흔하다.

### 체크포인트

- [ ] `/users/42` 동적 파라미터 응답 확인
- [ ] 없는 경로 404

---

## 4. JSON REST API

### 코드

```ts
// rest.ts
type Item = { id: string; name: string };

const items = new Map<string, Item>();

const cors = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET,POST,DELETE,OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
};

function json(data: unknown, init: ResponseInit = {}) {
  const headers = new Headers(init.headers);
  for (const [k, v] of Object.entries(cors)) headers.set(k, v);
  headers.set("content-type", "application/json; charset=utf-8");
  return new Response(JSON.stringify(data), { ...init, headers });
}

Bun.serve({
  port: 3000,
  async fetch(req) {
    const url = new URL(req.url);

    if (req.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: cors });
    }

    if (url.pathname === "/api/items" && req.method === "GET") {
      return json([...items.values()]);
    }

    if (url.pathname === "/api/items" && req.method === "POST") {
      let body: { name?: string };
      try {
        body = (await req.json()) as { name?: string };
      } catch {
        return json({ error: "invalid json" }, { status: 400 });
      }
      if (!body.name?.trim()) {
        return json({ error: "name required" }, { status: 400 });
      }
      const item: Item = { id: crypto.randomUUID(), name: body.name.trim() };
      items.set(item.id, item);
      return json(item, { status: 201 });
    }

    const m = url.pathname.match(/^\/api\/items\/([^/]+)$/);
    if (m && req.method === "DELETE") {
      const ok = items.delete(m[1]!);
      return ok
        ? new Response(null, { status: 204, headers: cors })
        : json({ error: "not found" }, { status: 404 });
    }

    return json({ error: "not found" }, { status: 404 });
  },
});
```

### 설명

- `crypto.randomUUID()` — Web Crypto, Bun 내장.
- CORS는 미들웨어 없이 **헤더만** 붙이면 된다 (브라우저에서 다른 오리진일 때).
- 바디: `req.json()` / `req.text()` / `req.formData()` / `req.arrayBuffer()`.

### 체크포인트

- [ ] `POST` → `GET` 목록에 반영
- [ ] 잘못된 JSON에 400

---

## 5. 정적 파일 · HTML 한 방에

```ts
Bun.serve({
  port: 3000,
  async fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/" || url.pathname === "/index.html") {
      const f = Bun.file("./public/index.html");
      if (!(await f.exists())) {
        return new Response("missing index.html", { status: 404 });
      }
      return new Response(f);
    }

    if (url.pathname === "/app.css") {
      return new Response(Bun.file("./public/app.css"), {
        headers: { "content-type": "text/css; charset=utf-8" },
      });
    }

    return new Response("Not Found", { status: 404 });
  },
});
```

`Bun.file`은 경로를 **게으르게** 읽어 `Response`로 흘린다.  
HTML을 **문자열로 인라인**하면 `public/` 없이 한 파일 서버가 된다 → 8절.

### 체크포인트

- [ ] 브라우저로 `/` HTML 로드

---

## 6. WebSocket 업그레이드 핵심

### 개념

1. 클라이언트가 `ws://host/path` 로 연결 (브라우저는 HTTP Upgrade 요청).
2. 서버 `fetch`에서 `server.upgrade(req)` 호출.
3. **성공하면 `true`** → Bun이 `101 Switching Protocols` 처리.  
   **`Response`를 또 보내면 안 된다** → `return` / `return undefined`.
4. 실패하면 `false` → 일반 HTTP `Response`로 에러 반환.

### 최소 에코

```ts
// ws-echo.ts
type SocketData = {
  id: string;
};

const server = Bun.serve({
  port: 3000,
  fetch(req, server) {
    const url = new URL(req.url);

    if (url.pathname === "/ws") {
      const ok = server.upgrade(req, {
        data: { id: crypto.randomUUID() },
      });
      if (ok) return; // 중요: Response 금지
      return new Response("Upgrade failed", { status: 400 });
    }

    return new Response("WS endpoint: /ws");
  },
  websocket: {
    // 공식 권장: 여기에 data 타입을 두면 ws.data가 전 핸들러에서 타이핑됨
    data: {} as SocketData,
    open(ws) {
      console.log("open", ws.data.id);
      ws.send(JSON.stringify({ type: "welcome", id: ws.data.id }));
    },
    message(ws, message) {
      // message: string | ArrayBuffer | Uint8Array
      const text =
        typeof message === "string"
          ? message
          : new TextDecoder().decode(message);
      ws.send(`echo: ${text}`);
    },
    close(ws, code, reason) {
      console.log("close", ws.data.id, code, reason);
    },
  },
});

console.log(`ws://localhost:${server.port}/ws`);
```

`upgrade` 때 넘긴 `data`는 핸들러의 `ws.data`로 붙는다.  
타입은 위처럼 `websocket.data: {} as T` 가 문서화된 방식이다.

업그레이드 응답(101)에 헤더를 더 붙이려면:

```ts
server.upgrade(req, {
  data: { id: crypto.randomUUID() },
  headers: {
    "Set-Cookie": `sid=${crypto.randomUUID()}; Path=/; HttpOnly`,
  },
});
```

쿠키 읽기는 `new Bun.CookieMap(req.headers.get("cookie") ?? "")` (Bun 내장).

### 경로를 가리는 패턴

업그레이드를 **모든 요청**에 걸면 HTML/API가 망가진다.  
**반드시 pathname(또는 헤더)으로 분기**한다.

### 체크포인트

- [ ] `/ws`만 업그레이드, `/`는 텍스트 응답
- [ ] upgrade 성공 분기에서 `return`만 하는지 확인

---

## 7. 저장소 최소 페어 — server.ts · client.ts

이 디렉터리에 있는 실행 예제와 **동일한** 코드다.

| 파일 | 역할 |
|---|---|
| [`server.ts`](./server.ts) | 모든 요청을 WS로 upgrade + 에코 |
| [`client.ts`](./client.ts) | 2초마다 메시지 전송 · 수신 로그 |

```bash
# 터미널 1
bun run server.ts

# 터미널 2
bun run client.ts
```

### server.ts

학습용으로 **경로 분기 없이** 들어오는 요청을 전부 WebSocket으로 올린다.  
(실서비스에서는 6절처럼 `/ws`만 upgrade 하는 편이 안전하다.)

```ts
Bun.serve({
  port: 3000,
  fetch(req, server) {
    // HTTP 요청을 웹소켓 연결로 전환(업그레이드)
    if (server.upgrade(req)) {
      return;
    }
    return new Response("HTTP 요청은 지원하지 않습니다.", { status: 400 });
  },
  websocket: {
    // 클라이언트가 연결되었을 때
    open(ws) {
      console.log("[서버] 새로운 클라이언트가 연결되었습니다.");
      ws.send("서버: 연결을 환영합니다!");
    },
    // 메시지를 받았을 때
    message(ws, message) {
      console.log(`[서버] 받은 메시지: ${message}`);
      // 받은 메시지 그대로 클라이언트에게 반환 (에코)
      ws.send(`서버 에코: ${message}`);
    },
    // 연결이 끊어졌을 때
    close(ws, code, reason) {
      console.log("[서버] 클라이언트 연결이 종료되었습니다.");
    },
  },
});

console.log("🚀 Bun 웹소켓 서버가 3000번 포트에서 실행 중입니다...");
```

### client.ts

브라우저 `WebSocket`과 같은 API를 Bun이 제공한다. 이벤트는 `onopen` / `addEventListener` 둘 다 가능.

```ts
// client.ts  — docs/bun/client.ts 와 동일
const ws = new WebSocket("ws://localhost:3000");

// 서버와 연결이 완료되었을 때
ws.onopen = () => {
  console.log("[클라이언트] 서버에 연결 성공!");

  // 2초마다 서버로 메시지 전송
  let count = 1;
  setInterval(() => {
    const msg = `안녕하세요! (${count++}번째 메시지)`;
    console.log(`[클라이언트] 보냄: ${msg}`);
    ws.send(msg);
  }, 2000);
};

// 서버로부터 메시지를 받았을 때
ws.onmessage = (event) => {
  console.log(`[클라이언트] 받음: ${event.data}`);
};

// 서버와 연결이 끊어졌을 때
ws.onclose = () => {
  console.log("[클라이언트] 서버와 연결이 끊어졌습니다.");
};

// 에러 발생 시
ws.onerror = (error) => {
  console.error("[클라이언트] 에러 발생:", error);
};
```

### 동작 흐름

```text
client.ts                     server.ts
   │  WS handshake ──────────► upgrade → open
   │  ◄──── "서버: 연결을 환영합니다!"
   │  2초마다 send ──────────► message → echo send
   │  ◄──── "서버 에코: …"
```

예상 로그 (요지):

```text
# server
[서버] 새로운 클라이언트가 연결되었습니다.
[서버] 받은 메시지: 안녕하세요! (1번째 메시지)

# client
[클라이언트] 서버에 연결 성공!
[클라이언트] 받음: 서버: 연결을 환영합니다!
[클라이언트] 보냄: 안녕하세요! (1번째 메시지)
[클라이언트] 받음: 서버 에코: 안녕하세요! (1번째 메시지)
```

### 체크포인트

- [ ] `bun run server.ts` 후 `bun run client.ts`로 에코 확인
- [ ] 브라우저에서 `ws://localhost:3000` 은 이 서버가 **모든 path를 WS로** 받기 때문에, 일반 HTTP 페이지는 400 문구만 본다

---

## 8. 에코 · 채팅방 (subscribe / publish)

Bun은 **토픽 pub/sub**이 내장이다. API 모양은 MQTT/Redis pub-sub과 비슷하지만,  
**기본 구현은 이 Bun 프로세스 메모리 안**이다 (멀티 인스턴스·다른 머신과는 공유되지 않음).

| API | 역할 |
|---|---|
| `ws.subscribe("room")` | 이 소켓을 토픽에 등록 |
| `ws.unsubscribe("room")` | 해제 |
| `ws.publish("room", data)` | 그 토픽의 **다른** 구독자에게 전송 (**자기 제외**) |
| `server.publish("room", data)` | 토픽의 **모든** 구독자에게 전송 (호출 주체가 소켓이 아님) |
| `server.subscriberCount("room")` | 구독 수 |
| `ws.subscriptions` | 이 소켓이 구독 중인 토픽 목록 (`string[]`) |
| `ws.isSubscribed("room")` | 구독 여부 |

```ts
type ChatData = {
  nick: string;
  room: string;
};

const server = Bun.serve({
  port: 3000,
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname !== "/chat") {
      return new Response("use /chat?nick=kim&room=lobby");
    }

    const nick = url.searchParams.get("nick")?.trim() || "anon";
    const room = url.searchParams.get("room")?.trim() || "lobby";

    const ok = srv.upgrade(req, {
      data: { nick, room },
    });
    if (!ok) return new Response("upgrade failed", { status: 400 });
    return;
  },
  websocket: {
    data: {} as ChatData,
    open(ws) {
      ws.subscribe(ws.data.room);
      // server.publish → 방금 subscribe한 본인 포함 전원
      server.publish(
        ws.data.room,
        JSON.stringify({ type: "join", nick: ws.data.nick }),
      );
      console.log("peers", server.subscriberCount(ws.data.room));
      console.log("subs", ws.subscriptions);
    },
    message(ws, message) {
      const text =
        typeof message === "string"
          ? message
          : new TextDecoder().decode(message);
      // 공식 채팅 예제도 server.publish로 전원(본인 포함) 재방송
      server.publish(
        ws.data.room,
        JSON.stringify({
          type: "chat",
          nick: ws.data.nick,
          text,
          at: Date.now(),
        }),
      );
    },
    close(ws) {
      ws.unsubscribe(ws.data.room);
      server.publish(
        ws.data.room,
        JSON.stringify({ type: "leave", nick: ws.data.nick }),
      );
    },
  },
});
```

정리:

- **본인 제외 브로드캐스트** → `ws.publish`
- **전원(본인 포함)** → `server.publish` (또는 `publishToSelf: true` + `ws.publish`)

### 체크포인트

- [ ] 같은 `room` 두 클라가 서로 메시지를 받는지
- [ ] 다른 `room`은 격리되는지
- [ ] “프로세스 하나 안에서의 pub/sub”임을 설명

---

## 9. 실전 통합: HTTP + WS + 브라우저 클라이언트

**파일 하나**로 HTTP API · 인라인 HTML · 채팅 WS를 모두 제공한다.  
외부 JS 라이브러리·번들러·`bun add` 없음.

```ts
// chat-server.ts
type ClientData = {
  id: string;
  nick: string;
  room: string;
};

type Outgoing =
  | { type: "welcome"; id: string; room: string; peers: number }
  | { type: "join" | "leave"; nick: string; peers: number }
  | { type: "chat"; nick: string; text: string; at: number }
  | { type: "error"; message: string };

function sendJson(ws: { send(data: string): number | void }, msg: Outgoing) {
  ws.send(JSON.stringify(msg));
}

const PAGE = `<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Bun chat</title>
  <style>
    :root { font-family: ui-sans-serif, system-ui, sans-serif; }
    body { margin: 1.5rem; max-width: 40rem; }
    #log { border: 1px solid #ccc; height: 16rem; overflow: auto; padding: .5rem; }
    .meta { color: #666; font-size: .85rem; }
    form { display: flex; gap: .5rem; margin-top: .75rem; flex-wrap: wrap; }
    input, button { font: inherit; padding: .4rem .6rem; }
    input[name="text"] { flex: 1; min-width: 10rem; }
  </style>
</head>
<body>
  <h1>Bun WS chat</h1>
  <p class="meta">외부 패키지 없이 <code>Bun.serve</code>만 사용</p>
  <form id="join">
    <input name="nick" placeholder="닉네임" value="guest" required />
    <input name="room" placeholder="방" value="lobby" required />
    <button type="submit">접속</button>
  </form>
  <div id="log"></div>
  <form id="send">
    <input name="text" placeholder="메시지" autocomplete="off" disabled />
    <button type="submit" disabled>전송</button>
  </form>
  <script>
    const logEl = document.getElementById("log");
    const joinForm = document.getElementById("join");
    const sendForm = document.getElementById("send");
    const textInput = sendForm.querySelector('[name="text"]');
    const sendBtn = sendForm.querySelector("button");
    let ws;

    function line(text, cls) {
      const p = document.createElement("p");
      if (cls) p.className = cls;
      p.textContent = text;
      logEl.appendChild(p);
      logEl.scrollTop = logEl.scrollHeight;
    }

    joinForm.addEventListener("submit", (e) => {
      e.preventDefault();
      const fd = new FormData(joinForm);
      const nick = fd.get("nick");
      const room = fd.get("room");
      if (ws) ws.close();
      const proto = location.protocol === "https:" ? "wss:" : "ws:";
      const url =
        proto +
        "//" +
        location.host +
        "/chat?nick=" +
        encodeURIComponent(nick) +
        "&room=" +
        encodeURIComponent(room);
      ws = new WebSocket(url);
      ws.addEventListener("open", () => {
        textInput.disabled = false;
        sendBtn.disabled = false;
        line("connected", "meta");
      });
      ws.addEventListener("message", (ev) => {
        let msg;
        try {
          msg = JSON.parse(ev.data);
        } catch {
          line(String(ev.data));
          return;
        }
        if (msg.type === "welcome")
          line("welcome id=" + msg.id + " peers=" + msg.peers, "meta");
        else if (msg.type === "join")
          line(msg.nick + " joined (" + msg.peers + ")", "meta");
        else if (msg.type === "leave")
          line(msg.nick + " left (" + msg.peers + ")", "meta");
        else if (msg.type === "chat") line(msg.nick + ": " + msg.text);
        else if (msg.type === "error") line("error: " + msg.message, "meta");
      });
      ws.addEventListener("close", () => {
        textInput.disabled = true;
        sendBtn.disabled = true;
        line("disconnected", "meta");
      });
    });

    sendForm.addEventListener("submit", (e) => {
      e.preventDefault();
      if (!ws || ws.readyState !== WebSocket.OPEN) return;
      const text = textInput.value.trim();
      if (!text) return;
      ws.send(text);
      textInput.value = "";
    });
  </script>
</body>
</html>`;

const server = Bun.serve({
  port: Number(process.env.PORT ?? 3000),
  // 아래 idleTimeout은 HTTP 연결용(기본 10초). WS는 websocket.idleTimeout(기본 120초).

  fetch(req, srv) {
    const url = new URL(req.url);

    if (url.pathname === "/chat") {
      const nick = (url.searchParams.get("nick") ?? "anon").trim().slice(0, 32);
      const room =
        (url.searchParams.get("room") ?? "lobby").trim().slice(0, 32) || "lobby";
      const ok = srv.upgrade(req, {
        data: { id: crypto.randomUUID(), nick, room },
      });
      if (ok) return;
      return new Response("WebSocket upgrade failed", { status: 400 });
    }

    if (url.pathname === "/api/health") {
      return Response.json({
        ok: true,
        pendingRequests: srv.pendingRequests,
        pendingWebSockets: srv.pendingWebSockets,
      });
    }

    if (url.pathname === "/api/rooms" && req.method === "GET") {
      return Response.json({
        lobby: srv.subscriberCount("lobby"),
      });
    }

    if (url.pathname === "/api/announce" && req.method === "POST") {
      return (async () => {
        const body = (await req.json().catch(() => null)) as
          | { room?: string; text?: string }
          | null;
        const room = body?.room?.trim() || "lobby";
        const text = body?.text?.trim();
        if (!text) {
          return Response.json({ error: "text required" }, { status: 400 });
        }
        const payload: Outgoing = {
          type: "chat",
          nick: "server",
          text,
          at: Date.now(),
        };
        srv.publish(room, JSON.stringify(payload));
        return Response.json({
          published: true,
          peers: srv.subscriberCount(room),
        });
      })();
    }

    if (url.pathname === "/" || url.pathname === "/index.html") {
      return new Response(PAGE, {
        headers: { "content-type": "text/html; charset=utf-8" },
      });
    }

    return new Response("Not Found", { status: 404 });
  },

  websocket: {
    data: {} as ClientData,
    maxPayloadLength: 64 * 1024, // 기본값은 16MB. 채팅은 더 작게.
    idleTimeout: 120, // WS 유휴 기본도 120초. sendPings로 유지 가능.
    sendPings: true,

    open(ws) {
      ws.subscribe(ws.data.room);
      const peers = server.subscriberCount(ws.data.room);
      sendJson(ws, {
        type: "welcome",
        id: ws.data.id,
        room: ws.data.room,
        peers,
      });
      // 전원(본인 포함). join을 본인 UI에 안 보여도 되면 ws.publish 사용.
      server.publish(
        ws.data.room,
        JSON.stringify({
          type: "join",
          nick: ws.data.nick,
          peers,
        } satisfies Outgoing),
      );
    },

    message(ws, message) {
      const text = (
        typeof message === "string"
          ? message
          : new TextDecoder().decode(message)
      )
        .trim()
        .slice(0, 2000);
      if (!text) {
        sendJson(ws, { type: "error", message: "empty" });
        return;
      }
      const payload: Outgoing = {
        type: "chat",
        nick: ws.data.nick,
        text,
        at: Date.now(),
      };
      // server.publish → 송신자 포함 전원 (별도 echo send 불필요)
      server.publish(ws.data.room, JSON.stringify(payload));
    },

    close(ws) {
      ws.unsubscribe(ws.data.room);
      const peers = Math.max(0, server.subscriberCount(ws.data.room));
      server.publish(
        ws.data.room,
        JSON.stringify({
          type: "leave",
          nick: ws.data.nick,
          peers,
        } satisfies Outgoing),
      );
    },
  },
});

console.log(`HTTP  ${server.url}`);
console.log(`WS    ws://localhost:${server.port}/chat?nick=kim&room=lobby`);
```

실행:

```bash
bun run chat-server.ts
# 브라우저: http://127.0.0.1:3000
# curl -X POST http://127.0.0.1:3000/api/announce \
#   -H 'content-type: application/json' \
#   -d '{"room":"lobby","text":"hello from curl"}'
```

### 설명

| 경로 | 역할 |
|---|---|
| `GET /` | 인라인 HTML 채팅 UI |
| `WS /chat?nick=&room=` | 업그레이드 + 방 구독 |
| `GET /api/health` | 프로세스 메트릭 |
| `POST /api/announce` | HTTP에서 `server.publish` |

같은 포트에서 HTTP와 WS를 같이 쓰는 것이 Bun의 기본 모델이다.

### 체크포인트

- [ ] 탭 두 개로 같은 방 접속 → 메시지 왕복
- [ ] `curl` announce가 브라우저 로그에 보임
- [ ] `/api/health`의 `pendingWebSockets` 증가 확인

---

## 10. Bun 쪽 WebSocket 클라이언트 (심화)

7절 [`client.ts`](./client.ts)가 **최소 에코 클라**다 (`onopen` / `setInterval`).  
9절 채팅 서버(`chat-server.ts` 패턴)에 붙일 때는 쿼리로 nick/room을 넘긴다.

```ts
// chat-client.ts — 9절 통합 서버용
const ws = new WebSocket("ws://127.0.0.1:3000/chat?nick=bot&room=lobby");

ws.addEventListener("open", () => {
  ws.send("hello from bun client");
});

ws.addEventListener("message", (ev) => {
  console.log("recv", ev.data);
});

ws.addEventListener("close", () => {
  console.log("closed");
});
```

```bash
# 7절 최소 페어
bun run server.ts   
bun run client.ts 

# 9절 채팅 서버를 띄운 뒤
bun run chat-client.ts
```

### 체크포인트

- [ ] 7절 페어로 에코 확인
- [ ] 9절 서버 + 위 클라로 채팅 메시지 확인

---

## 11. 옵션 · 백프레셔 · 타임아웃

### HTTP `idleTimeout` (serve 최상위)

기본 약 **10초** 유휴 시 연결 종료. 단위는 **초**, 최대 255, `0`이면 비활성.

```ts
Bun.serve({
  idleTimeout: 30, // HTTP
  fetch() {
    return new Response("ok");
  },
});
```

긴 스트림/SSE는 요청마다 `server.timeout(req, 0)`.

### WebSocket `idleTimeout` / `maxPayloadLength`

| 옵션 | 기본 (공식 문서) | 의미 |
|---|---|---|
| `websocket.idleTimeout` | **120초** | 메시지/핑 없이 유휴면 WS 종료 |
| `websocket.maxPayloadLength` | **16MB** | 이보다 큰 수신 프레임이면 연결 종료 |

```ts
websocket: {
  idleTimeout: 60,
  maxPayloadLength: 1024 * 1024, // 1MB
  sendPings: true, // 유휴 끊김 완화에 도움
  message(ws, msg) {
    ws.send(msg);
  },
},
```

HTTP `idleTimeout`과 WS `idleTimeout`을 **헷갈리지 말 것**.

### `send` 반환값 (백프레셔)

`ws.send(message)`는 `number`를 반환한다.

| 값 | 의미 |
|---|---|
| `1+` | 전송(또는 큐에 넣은) 바이트 수 |
| `-1` | 큐에 넣었으나 **백프레셔** |
| `0` | 연결 문제 등으로 **드롭** |

```ts
websocket: {
  maxPayloadLength: 1024 * 1024,
  backpressureLimit: 1024 * 1024,
  closeOnBackpressureLimit: false,
  drain(ws) {
    // 송신 여유가 생겼을 때 재시도 지점
  },
  perMessageDeflate: true, // 또는 send(msg, true)로 메시지별 압축
  publishToSelf: false, // ws.publish 기본: 자기 제외
  message(ws, msg) {
    const n = ws.send(msg);
    if (n === -1) {
      // 백프레셔 — drain에서 이어가기
    }
  },
},
```

### TLS (참고 · 패키지 0)

```ts
Bun.serve({
  port: 443,
  tls: {
    key: Bun.file("./key.pem"),
    cert: Bun.file("./cert.pem"),
  },
  fetch(req, server) {
    if (server.upgrade(req)) return;
    return new Response("ok");
  },
  websocket: {
    message(ws, msg) {
      ws.send(msg);
    },
  },
});
```

### 체크포인트

- [ ] HTTP 10초 vs WS 120초 기본값을 구분
- [ ] `send`가 -1/0/양수를 돌려줌을 확인
- [ ] `maxPayloadLength` 기본 16MB·필요 시 축소

---

## 12. 함정 · 자주 하는 실수

1. **upgrade 성공 후 `return new Response(...)`** → 핸드셰이크 깨짐. `return`만.  
2. **모든 경로를 upgrade** → 페이지·API 불가. pathname 분기.  
   (7절 `server.ts`는 학습용으로 전부 upgrade — 실서비스에선 피한다.)  
3. **`ws` npm 패키지 습관** → Bun에선 불필요.  
4. **`ws.data` 타입 누락** → `websocket: { data: {} as T }` (공식).  
5. **큰 페이로드 무검증** → 기본 16MB라도 앱에서 길이 가드.  
6. **방 이름 무한 생성** → 길이·문자 제한.  
7. **CORS 없이 다른 오리진 fetch** → 브라우저만 실패.  
8. **SSE/스트리밍 + HTTP idleTimeout(기본 10초)** → `server.timeout(req, 0)`.  
9. **`ws.publish`만 쓰고 송신자 UI 없음** → `server.publish` 또는 본인 `send` / `publishToSelf`.  
10. **HTTP `idleTimeout`과 WS `idleTimeout` 혼동**.  
11. **pub/sub을 클러스터·멀티 서버 공유로 착각** → **프로세스 로컬**. 스케일아웃 시 Redis 등 필요(그때는 외부 의존).  
12. **`close`에서 `unsubscribe` 누락** → 토픽에 죽은 구독이 남을 수 있음(버전에 따라 자동 정리되더라도 명시 권장).  
13. **핫 리로드 중 WS 끊김** → `bun --hot` 개발 시 흔함.  
14. **`routes`만 있고 upgrade용 `fetch` 없음** → WS 경로 구현 곤란.

### 체크포인트

- [ ] 위 1·2·4·11을 코드 리뷰 체크리스트에 넣기

---

## 13. 체크리스트

- [ ] **7절** `server.ts` + `client.ts` 에코 왕복  
- [ ] `bun run`으로 TS HTTP 서버  
- [ ] `routes` 또는 `fetch` 라우팅 + WS는 `fetch`에서 upgrade  
- [ ] JSON POST/GET  
- [ ] `server.upgrade` + `return`만  
- [ ] `websocket.data`로 `ws.data` 타이핑  
- [ ] `subscribe` / `publish` / `unsubscribe` 채팅방  
- [ ] `ws.publish`(타인) vs `server.publish`(전원) 구분  
- [ ] 브라우저 내장 `WebSocket` 클라 (패키지 0)  
- [ ] HTTP에서 `server.publish`  
- [ ] HTTP vs WS idleTimeout·maxPayload 기본값 인지  
- [ ] pub/sub이 단일 프로세스임을 인지  

---

## 14. 부록: 추가로 알아둘 개념

### 바이너리 메시지

```ts
message(ws, message) {
  if (typeof message !== "string") {
    // ArrayBuffer | Uint8Array
    ws.send(message); // 에코
    ws.send(new Uint8Array([1, 2, 3]));
  }
},
```

### 서버·소켓 `error` 핸들러

```ts
Bun.serve({
  fetch(req, server) {
    if (server.upgrade(req)) return;
    return new Response("ok");
  },
  error(err) {
    console.error("serve error", err);
    return new Response("Internal Error", { status: 500 });
  },
  websocket: {
    message(ws, msg) {
      ws.send(msg);
    },
    error(ws, err) {
      console.error("ws error", ws.data, err);
    },
  },
});
```

### `server.stop` / 메트릭

```ts
await server.stop(); // 기본: 진행 중 요청·WS 종료 대기
await server.stop(true); // 즉시 끊기
console.log(server.pendingRequests, server.pendingWebSockets);
```

### `cork` (고급)

여러 `send`를 한 번에 묶을 때 `ws.cork(ws => { ws.send(...); ... })` — 고빈도 송신 최적화.

### 서브프로토콜

클라: `new WebSocket(url, ["chat", "json"])`.  
서버는 upgrade 헤더/`Sec-WebSocket-Protocol` 협상을 직접 맞춰야 하는 경우가 많다(버전·문서 확인).

### HTML import / fullstack (참고)

Bun 1.x는 `import page from "./index.html"` + `routes`로 풀스택 서빙이 가능하다.  
이 교재 범위(최소 의존·개념)에서는 **인라인 HTML**로 충분하다.

---

## 부록. 파일 구성 제안

이 문서와 같은 폴더:

```text
docs/bun/
  260802_1639_bun_http_websocket_ts.md   # 이 교재
  # (선택) server.ts                              # 7절 최소 WS 서버 (에코)
  # (선택) client.ts                              # 7절 최소 WS 클라
  # (선택) chat-server.ts                # 9절 통합 예제를 파일로 빼서 실행
```

```bash
cd docs/bun
bun run server.ts   # 터미널 1
bun run client.ts   # 터미널 2
```

`package.json`은 없어도 된다. 있다면 **dependencies는 비운다**:

```json
{
  "name": "bun-ws-lab",
  "private": true,
  "scripts": {
    "server": "bun run server.ts",
    "client": "bun run client.ts"
  }
}
```

---

## 공식 문서

- HTTP Server: https://bun.com/docs/runtime/http/server  
- WebSockets: https://bun.com/docs/runtime/http/websockets  
- 간단 WS 가이드: https://bun.com/docs/guides/websocket/simple  

---

끝. **Express도 `ws` 패키지도 없이**, TypeScript 파일 하나와 Bun만으로 HTTP API · UI · WebSocket 채팅 · 서버 푸시까지 같은 포트에 올릴 수 있다.
