# Bun.serve 종합 보고서

> Bun 내장 HTTP/WebSocket 서버 API를 **공식 문서** 기준으로 정리한 예시 보고서.  
> 외부 프레임워크(Express/Hono/Elysia) 없이 `Bun.serve`만으로 구성한다.

| 항목 | 내용 |
|---|---|
| 작성일 | 2026-08-03 |
| 검증 런타임 | Bun **1.3.14** (로컬에서 일부 API 동작 재확인) |
| 범위 | `Bun.serve` · routing · WebSocket · TLS · cookies · SSE · HTML · hot reload · metrics |
| 로컬 예제 | [`server.ts`](./server.ts) — HTML Hello World |
| 미니 프레임워크 | [`mini/`](./mini/) — 의존성 0 Express식 구현 ([§17](#17-외부-의존성-0-express식-미니-프레임워크)) |

### 근거 문서 (공식)

| 주제 | URL |
|---|---|
| Server | https://bun.com/docs/runtime/http/server |
| Routing | https://bun.com/docs/runtime/http/routing |
| WebSockets | https://bun.com/docs/runtime/http/websockets |
| TLS | https://bun.com/docs/runtime/http/tls |
| Cookies | https://bun.com/docs/runtime/http/cookies |
| Error Handling | https://bun.com/docs/runtime/http/error-handling |
| Metrics | https://bun.com/docs/runtime/http/metrics |
| Watch / Hot | https://bun.com/docs/runtime/watch-mode |
| Hot HTTP guide | https://bun.com/docs/guides/http/hot |
| WS simple | https://bun.com/docs/guides/websocket/simple |
| WS pub/sub | https://bun.com/docs/guides/websocket/pubsub |

---

## 목차

1. [한 줄 요약](#1-한-줄-요약)
2. [Hello World (HTML)](#2-hello-world-html)
3. [Bun.serve 기본](#3-bunserve-기본)
4. [라우팅 (`routes`)](#4-라우팅-routes)
5. [fetch 핸들러 · fallback](#5-fetch-핸들러--fallback)
6. [요청/응답 · 파일 · 스트리밍 · SSE](#6-요청응답--파일--스트리밍--sse)
7. [쿠키](#7-쿠키)
8. [WebSocket](#8-websocket)
9. [TLS · HTTPS · HTTP/3](#9-tls--https--http3)
10. [Unix 소켓 · 포트 · idleTimeout](#10-unix-소켓--포트--idletimeout)
11. [에러 처리 · development](#11-에러-처리--development)
12. [메트릭 · Server API](#12-메트릭--server-api)
13. [Hot reload (`--hot`) · `server.reload`](#13-hot-reload---hot--serverreload)
14. [HTML import · 풀스택](#14-html-import--풀스택)
15. [실전 통합 예시](#15-실전-통합-예시)
16. [체크리스트 · 한계](#16-체크리스트--한계)
17. [외부 의존성 0 Express식 미니 프레임워크](#17-외부-의존성-0-express식-미니-프레임워크)

---

## 1. 한 줄 요약

`Bun.serve`는 Bun 바이너리에 포함된 고성능 HTTP 서버다.  
**라우팅 · WebSocket(pub/sub) · TLS · 쿠키 · 파일 전송 · HTML 번들**까지 내장하며, 핸들러는 Web 표준 `Request` / `Response`를 쓴다.

```bash
bun --hot server.ts
```

공식 벤치(문서 기준): 단순 응답 기준 Bun이 Node `http` 대비 대략 **2.5×** 요청/초. WebSocket 채팅룸 수치는 문서에 Bun **v0.2.1** 시절 벤치로 실려 있어, 최신 상대 성능은 별도 측정이 필요하다.  
(출처: [Server](https://bun.com/docs/runtime/http/server), [WebSockets](https://bun.com/docs/runtime/http/websockets))

---

## 2. Hello World (HTML)

이 폴더의 [`server.ts`](./server.ts)는 단일 파일 HTML Hello World다.

```ts
// bun --hot server.ts
const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <title>Hello World</title>
</head>
<body>
  <h1>Hello World</h1>
  <p>Bun.serve</p>
</body>
</html>`;

const server = Bun.serve({
  port: 3000,
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
  },
  fetch: () => new Response("Not Found", { status: 404 }),
});

console.log(`Listening on ${server.url}`);
```

확인:

```bash
bun --hot server.ts
curl -i http://localhost:3000/
```

---

## 3. Bun.serve 기본

### 3.1 최소 서버

출처: [Server — Basic Setup](https://bun.com/docs/runtime/http/server)

```ts
const server = Bun.serve({
  // routes 는 Bun v1.2.3+
  routes: {
    "/api/status": new Response("OK"),
    "/users/:id": (req) => new Response(`Hello User ${req.params.id}!`),
  },
  fetch(req) {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`Server running at ${server.url}`);
```

`import { serve } from "bun"` 후 `serve({...})` 도 동일하다.

### 3.2 `export default` 문법

파일에 `fetch`가 있는 default export를 두면 Bun이 자동으로 `Bun.serve`에 넘긴다.

```ts
import type { Serve } from "bun";

export default {
  fetch(req) {
    return new Response("Bun!");
  },
} satisfies Serve.Options<undefined>;
```

### 3.3 주요 옵션 맵

| 옵션 | 역할 |
|---|---|
| `port` / `hostname` | 리슨 주소 |
| `unix` | Unix domain socket |
| `routes` | 선언형 라우팅 |
| `fetch` | 미매칭/수동 핸들러 |
| `websocket` | WS 라이프사이클 |
| `tls` | HTTPS (BoringSSL) |
| `http3` | 실험적 HTTP/3 (TLS 필요) |
| `idleTimeout` | HTTP 유휴 연결 초 (기본 **10**, 최대 255, `0`=비활성). WS용은 `websocket.idleTimeout`(기본 120)과 별개 |
| `development` | 개발 모드(에러 페이지 등). **미지정 시** `NODE_ENV=production`이면 `false`, 그 외(미설정 포함)면 `true`인 경우가 많다(1.3.14 확인) |
| `error` | 서버 에러 → Response |
| `maxRequestBodySize` | 요청 바디 최대 크기(바이트). 초과하면 핸들러 도달 전 **자동 413**(Payload Too Large) 응답 — 1.3.14 확인 |
| `reusePort` | SO_REUSEPORT 클러스터 |

---

## 4. 라우팅 (`routes`)

출처: [Routing](https://bun.com/docs/runtime/http/routing)

라우터는 요청 경로를 미리 컴파일해 매칭하는 트리 기반 구현으로, 파라미터 디코딩까지 네이티브에서 처리한다(내부 구조는 버전에 따라 달라질 수 있음).  
핸들러는 `BunRequest`(=`Request` + `params` + `cookies`)를 받고 `Response | Promise<Response>`를 반환한다.

### 4.1 기본

```ts
Bun.serve({
  routes: {
    "/": () => new Response("Home"),
    "/api": () => Response.json({ success: true }),
    "/users": async () => Response.json({ users: [] }),
  },
  fetch() {
    return new Response("Unmatched route");
  },
});
```

### 4.2 우선순위 (specificity)

1. Exact — `/users/all`
2. Parameter — `/users/:id`
3. Wildcard — `/users/*`
4. Global catch-all — `/*`

```ts
Bun.serve({
  routes: {
    "/api/users/me": () => new Response("Current user"),
    "/api/users/:id": (req) => new Response(`User ${req.params.id}`),
    "/api/*": () => new Response("API catch-all"),
    "/*": () => new Response("Global catch-all"),
  },
});
```

### 4.3 타입 안전한 params

경로 문자열 리터럴이면 TypeScript가 `req.params` 형태를 추론한다.

```ts
import type { BunRequest } from "bun";

Bun.serve({
  routes: {
    "/orgs/:orgId/repos/:repoId": (req) => {
      const { orgId, repoId } = req.params;
      return Response.json({ orgId, repoId });
    },
    "/orgs/:orgId/repos/:repoId/settings": (
      req: BunRequest<"/orgs/:orgId/repos/:repoId/settings">,
    ) => {
      const { orgId, repoId } = req.params;
      return Response.json({ orgId, repoId });
    },
  },
});
```

퍼센트 인코딩·유니코드 파라미터는 자동 디코딩된다. 잘못된 유니코드는 `\uFFFD`로 치환된다.

### 4.4 HTTP 메서드별 핸들러

```ts
Bun.serve({
  routes: {
    "/api/posts": {
      GET: () => new Response("List posts"),
      POST: async (req) => {
        const body = await req.json();
        return Response.json({ created: true, ...body });
      },
    },
  },
});
```

### 4.5 Static Response (제로할당 디스패치)

`Response` 객체를 그대로 넣으면 초기화 후 추가 할당 없이 디스패치한다. 헬스체크·리다이렉트·고정 JSON에 적합.  
문서상 수동 `return new Response(...)` 대비 대략 **15%+** 성능 이점을 기대할 수 있다.

```ts
Bun.serve({
  routes: {
    "/health": new Response("OK"),
    "/ready": new Response("Ready", { headers: { "X-Ready": "1" } }),
    "/blog": Response.redirect("https://bun.com/blog"),
    "/api/config": Response.json({ version: "1.0.0", env: "production" }),
  },
});
```

정적 라우트는 서버 객체 수명 동안 캐시된다. 바꾸려면 `server.reload(options)`.

### 4.6 파일 응답: Static vs File

| 방식 | 동작 | 적합한 경우 |
|---|---|---|
| `new Response(await Bun.file(p).bytes())` | 시작 시 메모리 버퍼, ETag/`If-None-Match` | 작은 에셋, 자주 접근 |
| `new Response(Bun.file(p))` | 요청마다 파일 읽기, 404/Range/`Last-Modified` | 큰 파일, 자주 바뀌는 파일 |

```ts
Bun.serve({
  routes: {
    "/logo.png": new Response(await Bun.file("./logo.png").bytes()),
    "/download.zip": new Response(Bun.file("./download.zip")),
    "/favicon.ico": Bun.file("./favicon.ico"), // Server 문서 예: BunFile 직접
  },
});
```

파일 스트리밍 시 가능하면 `sendfile(2)` 제로카피를 사용한다. ([Routing — Streaming files](https://bun.com/docs/runtime/http/routing))

### 4.7 비동기 라우트

공식 Routing 문서 예. `sql`은 Postgres 등 **DB 연결**(`DATABASE_URL` 등)이 있어야 동작한다. DB 없이 비동기만 보려면 `await Bun.sleep(10)` 정도로 대체하면 된다.

```ts
import { sql, serve } from "bun";

serve({
  port: 3001,
  routes: {
    "/api/version": async () => {
      const [version] = await sql`SELECT version()`;
      return Response.json(version);
    },
  },
});
```

---

## 5. fetch 핸들러 · fallback

`routes`에 안 걸린 요청이 `fetch`로 온다. (Bun &lt; 1.2.3에서는 `fetch`가 필수에 가깝다.)

```ts
Bun.serve({
  async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === "/") return new Response("Home page!");
    if (url.pathname === "/blog") return new Response("Blog!");
    return new Response("404!");
  },
});
```

두 번째 인자로 `Server`를 받는다.

```ts
const server = Bun.serve({
  fetch(req, server) {
    const ip = server.requestIP(req);
    return new Response(`Your IP is ${ip?.address}`);
  },
});
```

프록시처럼 다른 `fetch`를 그대로 반환할 수도 있다.

```ts
Bun.serve({
  fetch(req) {
    return fetch("https://example.com");
  },
});
```

---

## 6. 요청/응답 · 파일 · 스트리밍 · SSE

### 6.1 바디 읽기

```ts
Bun.serve({
  async fetch(req) {
    // 상황에 맞게 하나만 사용
    const text = await req.text();
    // const json = await req.json();
    // const buf = await req.arrayBuffer();
    // const form = await req.formData();
    return Response.json({ len: text.length });
  },
});
```

### 6.2 Range / slice

공식 Routing 문서의 파싱 스케치를 약간 보강한 형태.  
`bytes=0-`처럼 end가 비면 `Number("") === 0`이 되어 기본값 `Infinity`가 적용되지 않으므로, 빈 구간은 별도 처리한다.

```ts
Bun.serve({
  fetch(req) {
    const raw = (req.headers.get("Range") ?? "bytes=0-").split("=").at(-1)!;
    const [a, b] = raw.split("-");
    const start = a === "" ? 0 : Number(a);
    const end = b === "" || b == null ? Infinity : Number(b);

    const bigFile = Bun.file("./big-video.mp4");
    return new Response(bigFile.slice(start, end));
  },
});
```

### 6.3 ReadableStream 응답

```ts
Bun.serve({
  fetch() {
    const stream = new ReadableStream({
      async start(controller) {
        const enc = new TextEncoder();
        for (const chunk of ["a", "b", "c"]) {
          controller.enqueue(enc.encode(chunk));
          await Bun.sleep(100);
        }
        controller.close();
      },
    });
    return new Response(stream, {
      headers: { "Content-Type": "text/plain; charset=utf-8" },
    });
  },
});
```

### 6.4 Server-Sent Events (SSE)

공식 문서: 스트리밍이 오래 조용하면 `idleTimeout`(기본 10초)에 끊긴다.  
요청별로 `server.timeout(req, 0)`으로 비활성화한다. ([Server — Per-Request Controls](https://bun.com/docs/runtime/http/server))

```ts
Bun.serve({
  routes: {
    "/events": (req, server) => {
      server.timeout(req, 0);

      let timer: ReturnType<typeof setInterval> | undefined;
      const stream = new ReadableStream({
        start(controller) {
          const enc = new TextEncoder();
          timer = setInterval(() => {
            controller.enqueue(
              enc.encode(`data: ${JSON.stringify({ t: Date.now() })}\n\n`),
            );
          }, 1000);
        },
        cancel() {
          if (timer) clearInterval(timer);
        },
      });

      return new Response(stream, {
        headers: {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          Connection: "keep-alive",
        },
      });
    },
  },
});
```

문서에 나온 async generator 패턴:

```ts
Bun.serve({
  routes: {
    "/events": (req, server) => {
      server.timeout(req, 0);
      return new Response(
        async function* () {
          yield "data: hello\n\n";
        },
        { headers: { "Content-Type": "text/event-stream" } },
      );
    },
  },
});
```

### 6.5 multipart 업로드

```ts
Bun.serve({
  async fetch(req) {
    if (req.method !== "POST") return new Response("POST only", { status: 405 });
    const form = await req.formData();
    const file = form.get("file");
    if (!(file instanceof File)) {
      return new Response("file required", { status: 400 });
    }
    const out = `./uploads/${file.name}`;
    await Bun.write(out, file);
    return Response.json({ saved: out, size: file.size, type: file.type });
  },
});
```

### 6.6 자주 쓰는 서버 패턴 (의존성 0)

프레임워크 없이 `Bun.serve`만으로 반복해서 쓰게 되는 조각들.

**쿼리 파라미터 파싱 + 검증**

```ts
Bun.serve({
  routes: {
    "/search": (req) => {
      const url = new URL(req.url); // BunRequest 도 쿼리는 new URL(req.url) 로
      const term = url.searchParams.get("q")?.trim();
      const page = Number(url.searchParams.get("page") ?? "1");
      if (!term) return Response.json({ error: "q required" }, { status: 400 });
      if (!Number.isInteger(page) || page < 1) {
        return Response.json({ error: "bad page" }, { status: 400 });
      }
      return Response.json({ term, page });
    },
  },
});
```

**CORS (프리플라이트 포함)**

```ts
const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET,POST,PUT,DELETE,OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

Bun.serve({
  async fetch(req) {
    if (req.method === "OPTIONS") return new Response(null, { status: 204, headers: CORS });
    return Response.json({ ok: true }, { headers: CORS });
  },
});
```

**Bearer 토큰 인증 가드**

```ts
function requireBearer(req: Request, token: string): Response | null {
  const auth = req.headers.get("authorization") ?? "";
  if (auth === `Bearer ${token}`) return null; // 통과
  return new Response("Unauthorized", {
    status: 401,
    headers: { "WWW-Authenticate": "Bearer" },
  });
}

Bun.serve({
  routes: {
    "/admin": (req) => requireBearer(req, "s3cret") ?? Response.json({ secret: 42 }),
  },
});
```

**JSON 바디 안전 파싱** (잘못된 JSON → 400)

```ts
async function readJson<T>(req: Request): Promise<{ data: T } | { error: string }> {
  if (!req.headers.get("content-type")?.includes("application/json")) {
    return { error: "expected application/json" };
  }
  try {
    return { data: (await req.json()) as T };
  } catch {
    return { error: "invalid JSON" };
  }
}

Bun.serve({
  routes: {
    "/api/echo": {
      POST: async (req) => {
        const r = await readJson<{ name: string }>(req);
        if ("error" in r) return Response.json(r, { status: 400 });
        return Response.json({ hello: r.data.name });
      },
    },
  },
});
```

**정적 파일 디렉터리 서버** (경로 이탈 방지 포함)

```ts
Bun.serve({
  async fetch(req) {
    const url = new URL(req.url);
    const rel = decodeURIComponent(url.pathname.slice(1)) || "index.html";
    if (rel.includes("..") || rel.includes("\0")) {
      return new Response("Bad path", { status: 400 });
    }
    const file = Bun.file(`./public/${rel}`);
    if (await file.exists()) return new Response(file); // sendfile 제로카피
    return new Response("Not Found", { status: 404 });
  },
});
```

**간단 미들웨어 합성** (프레임워크 없이)

```ts
type Mw = (req: Request, next: () => Promise<Response>) => Promise<Response>;

const withLog: Mw = async (req, next) => {
  const t = performance.now();
  const res = await next();
  console.log(`${req.method} ${new URL(req.url).pathname} ${res.status} ${(performance.now() - t).toFixed(1)}ms`);
  return res;
};

const compose = (mws: Mw[], handler: (req: Request) => Promise<Response>) =>
  (req: Request) =>
    mws.reduceRight<() => Promise<Response>>(
      (next, mw) => () => mw(req, next),
      () => handler(req),
    )();

Bun.serve({
  fetch: compose([withLog], async (req) => new Response("ok")),
});
```

> 미들웨어 체인·라우터·에러 핸들러까지 갖춘 완성형 구현은 [§17](#17-외부-의존성-0-express식-미니-프레임워크) 참고.

---

## 7. 쿠키

출처: [Cookies](https://bun.com/docs/runtime/http/cookies)

공식 문서: **`routes` 사용 시** `req.cookies`(`CookieMap`)로 읽고, `set`/`delete`하면 응답 Set-Cookie에 자동 반영된다.

주의 (Bun 1.3.14 확인): 일반 `fetch(req)`의 `Request`에는 `cookies` 프로퍼티가 없다. 자동 Set-Cookie 추적은 **route 핸들러의 `BunRequest`** 기준이다. `fetch`만 쓸 때는 `Set-Cookie` 헤더를 직접 달거나 `Bun.CookieMap`으로 파싱한다.

### 7.1 읽기

```ts
Bun.serve({
  routes: {
    "/profile": (req) => {
      const userId = req.cookies.get("user_id");
      const theme = req.cookies.get("theme") || "light";
      return Response.json({ userId, theme });
    },
  },
});
```

### 7.2 설정

```ts
Bun.serve({
  routes: {
    "/login": (req) => {
      req.cookies.set("user_id", "12345", {
        maxAge: 60 * 60 * 24 * 7,
        httpOnly: true,
        secure: true,
        path: "/",
      });
      req.cookies.set("theme", "dark");
      return new Response("Login successful");
    },
  },
});
```

### 7.3 삭제

```ts
Bun.serve({
  routes: {
    "/logout": (req) => {
      req.cookies.delete("user_id", { path: "/" });
      return new Response("Logged out successfully");
    },
  },
});
```

삭제 시 `Expires`가 과거인 Set-Cookie가 붙는다.

### 7.4 `Bun.CookieMap` (수동)

WebSocket upgrade 등에서 헤더를 직접 파싱할 때:

```ts
const cookies = new Bun.CookieMap(req.headers.get("cookie") ?? "");
const token = cookies.get("X-Token");
```

---

## 8. WebSocket

출처: [WebSockets](https://bun.com/docs/runtime/http/websockets), [simple](https://bun.com/docs/guides/websocket/simple), [pubsub](https://bun.com/docs/guides/websocket/pubsub)

특징:

- `Bun.serve`에 `websocket` 핸들러 객체 **하나**를 서버 단위로 등록 (소켓마다 리스너 부착 X)
- 내부적으로 uWebSockets
- 네이티브 **pub/sub** (`subscribe` / `publish`)
- TLS·압축(`perMessageDeflate`) 지원
- **`websocket.message`는 필수** — `websocket: {}`만 두면 런타임 에러 (`WebSocketServerContext expects a message handler`, 1.3.14). 공식 문서의 빈 `{}`는 자리표시용 스케치다.

### 8.1 최소 업그레이드 + echo

```ts
const server = Bun.serve({
  fetch(req, server) {
    const success = server.upgrade(req);
    if (success) {
      // 101 Switching Protocols — Response 반환하지 않음
      return undefined;
    }
    return new Response("Hello world!");
  },
  websocket: {
    // message 필수
    async message(ws, message) {
      console.log(`Received ${message}`);
      ws.send(`You said: ${message}`);
    },
  },
});

console.log(`Listening on ${server.hostname}:${server.port}`);
```

업그레이드 실패 시만 Response를 돌려준다. `websocket` 객체가 없으면 `upgrade` 호출 자체가 에러다.

```ts
Bun.serve({
  fetch(req, server) {
    if (server.upgrade(req)) return;
    return new Response("Upgrade failed", { status: 500 });
  },
  websocket: {
    message(ws, message) {
      ws.send(message);
    },
  },
});
```

### 8.2 이벤트 핸들러

```ts
Bun.serve({
  fetch(req, server) {
    if (server.upgrade(req)) return;
    return new Response("Upgrade failed", { status: 500 });
  },
  websocket: {
    message(ws, message) {}, // 필수
    open(ws) {},
    close(ws, code, reason) {},
    drain(ws) {}, // 백프레셔 해소
    // ping / pong 도 타입 정의에 존재 (버전 참고)
  },
});
```

### 8.3 전송 · 헤더 · contextual data

```ts
type WebSocketData = {
  createdAt: number;
  channelId: string | null;
  authToken: string | null;
};

Bun.serve({
  fetch(req, server) {
    const cookies = new Bun.CookieMap(req.headers.get("cookie") ?? "");

    const ok = server.upgrade(req, {
      headers: {
        // 필요 시 업그레이드 응답에 Set-Cookie 등
        // "Set-Cookie": `SessionId=${sessionId}`,
      },
      data: {
        createdAt: Date.now(),
        channelId: new URL(req.url).searchParams.get("channelId"),
        authToken: cookies.get("X-Token"),
      } satisfies WebSocketData,
    });

    if (ok) return undefined;
    return new Response("Upgrade failed", { status: 400 });
  },
  websocket: {
    data: {} as WebSocketData,
    message(ws, message) {
      ws.send("Hello world"); // string
      // ws.send(buf); TypedArray / ArrayBuffer / Blob
      console.log(ws.data.authToken, message);
    },
  },
});
```

브라우저 연결:

```js
const socket = new WebSocket("ws://localhost:3000/chat");
socket.addEventListener("message", (event) => console.log(event.data));
```

페이지에 심은 쿠키는 WS 업그레이드 요청에 포함된다(표준 동작).

### 8.4 Pub/Sub 채팅

```ts
const server = Bun.serve({
  fetch(req, server) {
    const url = new URL(req.url);
    if (url.pathname === "/chat") {
      const username = url.searchParams.get("user") ?? "anon";
      const success = server.upgrade(req, { data: { username } });
      return success
        ? undefined
        : new Response("WebSocket upgrade error", { status: 400 });
    }
    return new Response("Hello world");
  },
  websocket: {
    data: {} as { username: string },
    open(ws) {
      const msg = `${ws.data.username} has entered the chat`;
      ws.subscribe("the-group-chat");
      server.publish("the-group-chat", msg);
    },
    message(ws, message) {
      server.publish("the-group-chat", `${ws.data.username}: ${message}`);
      console.log(ws.subscriptions); // ["the-group-chat"]
    },
    close(ws) {
      const msg = `${ws.data.username} has left the chat`;
      ws.unsubscribe("the-group-chat");
      server.publish("the-group-chat", msg);
    },
  },
});
```

| API | 의미 |
|---|---|
| `ws.subscribe(topic)` | 토픽 구독 |
| `ws.unsubscribe(topic)` | 구독 해제 |
| `ws.publish(topic, data)` | **본인 제외** 브로드캐스트 |
| `server.publish(topic, data)` | 토픽 **전원** |
| `ws.isSubscribed(topic)` | 구독 여부 |
| `server.subscriberCount(topic)` | 구독자 수 |

`ws.publish`는 호출한 소켓을 제외한다. 전체에게 보내려면 `server.publish`.

### 8.5 압축 · 타임아웃 · 페이로드 · 백프레셔

```ts
Bun.serve({
  fetch(req, server) {
    if (!server.upgrade(req)) {
      return new Response("Upgrade failed", { status: 400 });
    }
    // 성공 시 Response 없이 return
  },
  websocket: {
    perMessageDeflate: true,
    idleTimeout: 60, // 기본 120초 (HTTP idleTimeout 10초와 별개)
    maxPayloadLength: 1024 * 1024, // 기본 16MB
    // backpressureLimit, closeOnBackpressureLimit, sendPings, publishToSelf
    message(ws, message) {
      const status = ws.send(String(message), true); // 두 번째 인자: compress
      // -1: 큐잉(백프레셔), 0: 드롭, 1+: 전송 바이트
    },
  },
});
```

### 8.6 클라이언트 (`new WebSocket`)

```ts
const socket = new WebSocket("ws://localhost:3000");
const socket2 = new WebSocket("ws://localhost:3000", ["soap", "wamp"]);

// Bun 전용: 브라우저에선 불가한 커스텀 헤더
const socket3 = new WebSocket("ws://localhost:3000", {
  headers: { Authorization: "Bearer …" },
});

socket.addEventListener("open", () => {});
socket.addEventListener("message", () => {});
socket.addEventListener("close", () => {});
socket.addEventListener("error", () => {});
```

### 8.7 ServerWebSocket 참고 API

공식 Reference 요약:

```ts
interface ServerWebSocket {
  readonly data: any;
  readonly readyState: number;
  readonly remoteAddress: string;
  readonly subscriptions: string[];
  send(message: string | ArrayBuffer | Uint8Array | Blob, compress?: boolean): number;
  close(code?: number, reason?: string): void;
  subscribe(topic: string): boolean;
  unsubscribe(topic: string): boolean;
  publish(topic: string, message: string | ArrayBuffer | Uint8Array | Blob): void;
  isSubscribed(topic: string): boolean;
  cork(cb: (ws: ServerWebSocket) => void): void;
}
```

---

## 9. TLS · HTTPS · HTTP/3

출처: [TLS](https://bun.com/docs/runtime/http/tls), [Server — HTTP/3](https://bun.com/docs/runtime/http/server)

BoringSSL 기반. `key`/`cert`는 **경로 문자열이 아니라 내용**(string / BunFile / TypedArray / Buffer).

```ts
Bun.serve({
  tls: {
    key: Bun.file("./key.pem"),
    cert: Bun.file("./cert.pem"),
    // passphrase: "my-secret-passphrase",
    // ca: Bun.file("./ca.pem"),
    // dhParamsFile: "/path/to/dhparams.pem",
    // serverName: "my-server.com",
  },
  fetch: () => new Response("https ok"),
});
```

다중 SNI:

```ts
Bun.serve({
  tls: [
    {
      key: Bun.file("./key1.pem"),
      cert: Bun.file("./cert1.pem"),
      serverName: "my-server1.com",
    },
    {
      key: Bun.file("./key2.pem"),
      cert: Bun.file("./cert2.pem"),
      serverName: "my-server2.com",
    },
  ],
});
```

### HTTP/3 (실험적)

TLS 필수. 같은 포트에서 TCP(HTTP/1.1) + UDP(HTTP/3).  
`http1: false`면 HTTP/3만.

```ts
Bun.serve({
  tls: {
    key: Bun.file("./key.pem"),
    cert: Bun.file("./cert.pem"),
  },
  http3: true,
  // http1: false,
  fetch(req) {
    return new Response("Hello over HTTP/3!");
  },
});
```

Unix 소켓과는 함께 쓸 수 없다(QUIC=UDP).

---

## 10. Unix 소켓 · 포트 · idleTimeout

### 포트 / 호스트

```ts
Bun.serve({
  port: 8080, // 생략 시 $BUN_PORT → $PORT → $NODE_PORT → 3000
  hostname: "0.0.0.0", // 문서 기본값. 미지정 시 server.url이 localhost로 보이기도 함(1.3.14)
  fetch: () => new Response("ok"),
});

// 랜덤 포트
const server = Bun.serve({
  port: 0,
  fetch: () => new Response("ok"),
});
console.log(server.port, server.url);
```

CLI:

```bash
bun --port=4002 server.ts
BUN_PORT=4002 bun server.ts
PORT=4002 bun server.ts
```

### Unix domain socket

```ts
Bun.serve({
  unix: "/tmp/my-socket.sock",
  fetch: () => new Response("unix ok"),
});
```

Linux abstract namespace:

```ts
Bun.serve({
  unix: "\0my-abstract-socket",
  fetch: () => new Response("ok"),
});
```

### idleTimeout

HTTP 연결 기준 기본 **10초** 무활동 시 종료. 최대 255, `0`이면 비활성.  
(WebSocket 쪽 기본 idle은 `websocket.idleTimeout` **120초** — [§8.5](#85-압축--타임아웃--페이로드--백프레셔))

```ts
Bun.serve({
  idleTimeout: 30,
  fetch: () => new Response("Bun!"),
});
```

요청 단위 오버라이드:

```ts
Bun.serve({
  async fetch(req, server) {
    server.timeout(req, 60);
    await req.text();
    return new Response("Done!");
  },
});
```

---

## 11. 에러 처리 · development

출처: [Error Handling](https://bun.com/docs/runtime/http/error-handling)

`development: true`이면 브라우저에 내장 에러 페이지가 뜬다.  
Bun 1.3.14 기준: 옵션을 안 주면 `NODE_ENV=production`일 때 `development === false`, 그 외에는 `true`인 경우가 많다. 프로덕션에서는 명시적으로 `development: false`를 권장한다.

```ts
Bun.serve({
  development: true, // 브라우저에 내장 에러 페이지
  fetch(req) {
    throw new Error("woops!");
  },
});
```

커스텀 `error` 콜백(development면 기본 에러 페이지를 대체):

```ts
Bun.serve({
  fetch(req) {
    throw new Error("woops!");
  },
  error(error) {
    return new Response(`<pre>${error}\n${error.stack}</pre>`, {
      headers: { "Content-Type": "text/html" },
    });
  },
});
```

---

## 12. 메트릭 · Server API

출처: [Metrics](https://bun.com/docs/runtime/http/metrics), [Server Lifecycle](https://bun.com/docs/runtime/http/server)

```ts
const server = Bun.serve({
  fetch(req, server) {
    return new Response(
      `Active requests: ${server.pendingRequests}\n` +
        `Active WebSockets: ${server.pendingWebSockets}`,
    );
  },
  websocket: {
    open(ws) {
      ws.subscribe("chat");
    },
    message() {},
  },
});

// 토픽 구독자
server.subscriberCount("chat");
```

수명주기:

```ts
await server.stop();     // in-flight 대기 후 종료
await server.stop(true); // 강제 종료

server.unref(); // 서버만으로 프로세스 keep-alive 안 함
server.ref();

// 테스트/내부 호출
const res = await server.fetch("http://localhost/health");

server.reload({
  fetch: () => new Response("v2"),
  // routes, error, websocket 갱신 가능
  // port/hostname 변경은 무시
});
```

`server.requestIP(req)` → `{ address, family, port } | null`  
(닫힌 요청·Unix 소켓에서는 `null`)

---

## 13. Hot reload (`--hot`) · `server.reload`

출처: [Watch mode](https://bun.com/docs/runtime/watch-mode), [Hot reload HTTP](https://bun.com/docs/guides/http/hot)

### `--hot` vs `--watch`

| | `--hot` | `--watch` |
|---|---|---|
| 프로세스 | soft reload (재시작 최소화) | 하드 리스타트에 가깝다 |
| `globalThis` | 유지 | 프로세스 재시작 시 초기화 |
| `Bun.serve` | fetch/routes 등을 거의 즉시 교체 | 서버 재바인딩 |

```bash
bun --hot server.ts
# 또는
bun --hot run index.ts
```

동작 요약 (공식):

1. 엔트리부터 import 그래프를 레지스트리로 구성 (`node_modules` 제외)
2. 파일 변경 시 soft reload — 모듈 재평가, **프로세스·포트 유지**
3. `Bun.serve`를 쓰면 핸들러를 재시작 없이 갱신
4. **일반 서버 코드의 `--hot`은 브라우저 자동 새로고침이 아니다** (서버 soft reload). 브라우저 HMR은 **HTML import**를 `routes`에 넣고 `bun --hot`으로 돌릴 때의 풀스택 경로다 ([§14](#14-html-import--풀스택), [Server — HTML imports](https://bun.com/docs/runtime/http/server)). 인라인 HTML 문자열(`server.ts`)만으로는 HMR이 없다.

상태 유지 예:

```ts
globalThis.count ??= 0;
globalThis.count++;

Bun.serve({
  port: 3000,
  fetch() {
    return new Response(`Reloaded ${globalThis.count} times`);
  },
});
```

### 프로그래밍 방식 `server.reload`

```ts
const server = Bun.serve({
  routes: {
    "/api/version": () => Response.json({ version: "1.0.0" }),
  },
});

server.reload({
  routes: {
    "/api/version": () => Response.json({ version: "2.0.0" }),
  },
});
```

갱신 가능: `fetch`, `error`, `routes`, `websocket`.  
`port`/`hostname` 등은 바꿔도 적용되지 않는다.

`--hot`일 때 서버 `id`로 pending request/WS를 끊지 않고 리로드하는 데 쓰인다. ([Bun.serve reference](https://bun.com/reference/bun/serve))

---

## 14. HTML import · 풀스택

출처: [Server — HTML imports](https://bun.com/docs/runtime/http/server)

```ts
import myReactSinglePageApp from "./index.html";

Bun.serve({
  routes: {
    "/": myReactSinglePageApp,
  },
});
```

| 모드 | 동작 |
|---|---|
| `bun --hot` + HTML import | 런타임 온디맨드 번들 + **브라우저 HMR** (문서: Server HTML imports) |
| `bun --hot` + 일반 TS 서버만 | 서버 soft reload. 브라우저 자동 갱신 없음 ([watch-mode](https://bun.com/docs/runtime/watch-mode)) |
| `bun build --target=bun` | import가 프리빌드 매니페스트로 해석, 런타임 번들 없음 |

HTML import는 HTML만 서빙하는 게 아니라 번들러·TS·CSS 파이프라인을 탄다.  
자세한 풀스택 가이드: https://bun.com/docs/bundler/fullstack

문자열 HTML(이 폴더 `server.ts`) vs 파일 import:

```ts
// A) 인라인 (의존성 0, HMR 없음)
return new Response(html, { headers: { "Content-Type": "text/html" } });

// B) 파일 직접
return new Response(Bun.file("./index.html"));

// C) HTML import (풀스택/HMR)
import page from "./index.html";
// routes: { "/": page }
```

---

## 15. 실전 통합 예시

아래는 **복사해 바로 돌릴 수 있는** 올인원 스케치다. (의존성 0, SQLite 선택)

### 15.1 REST + routes + cookies + metrics

```ts
// bun --hot rest-demo.ts
const users = [
  { id: "1", name: "ada" },
  { id: "2", name: "grace" },
];

const server = Bun.serve({
  port: 3000,
  routes: {
    "/": () =>
      new Response("<h1>API</h1><p>see /api/users</p>", {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),

    "/health": Response.json({ ok: true }),

    "/api/users": {
      GET: () => Response.json(users),
      POST: async (req) => {
        const body = (await req.json()) as { name?: string };
        if (!body.name) {
          return Response.json({ error: "name required" }, { status: 400 });
        }
        const user = { id: crypto.randomUUID(), name: body.name };
        users.push(user);
        return Response.json(user, { status: 201 });
      },
    },

    "/api/users/:id": (req) => {
      const user = users.find((u) => u.id === req.params.id);
      if (!user) return new Response("Not Found", { status: 404 });
      return Response.json(user);
    },

    "/api/login": {
      POST: (req) => {
        req.cookies.set("sid", crypto.randomUUID(), {
          httpOnly: true,
          sameSite: "lax",
          path: "/",
          maxAge: 60 * 60 * 24,
        });
        return Response.json({ ok: true });
      },
    },

    "/api/me": (req) => {
      const sid = req.cookies.get("sid");
      if (!sid) return new Response("Unauthorized", { status: 401 });
      return Response.json({ sid });
    },

    "/metrics": (_req, server) =>
      Response.json({
        pendingRequests: server.pendingRequests,
        pendingWebSockets: server.pendingWebSockets,
      }),
  },
  fetch: () => new Response("Not Found", { status: 404 }),
  error(error) {
    console.error(error);
    return new Response("Internal Server Error", { status: 500 });
  },
});

console.log(server.url);
```

### 15.2 HTTP + WebSocket 채팅 (한 서버)

```ts
// bun --hot chat.ts
type WsData = { username: string; room: string };

const server = Bun.serve({
  port: 3000,
  routes: {
    "/": () =>
      new Response(
        `<!doctype html>
<html><body>
  <h1>Chat</h1>
  <pre id="log"></pre>
  <input id="msg" /><button id="send">send</button>
  <script>
    const room = new URLSearchParams(location.search).get("room") || "lobby";
    const user = new URLSearchParams(location.search).get("user") || "anon";
    const ws = new WebSocket(\`ws://\${location.host}/ws?room=\${room}&user=\${user}\`);
    const log = document.getElementById("log");
    ws.onmessage = (e) => { log.textContent += e.data + "\\n"; };
    document.getElementById("send").onclick = () => {
      ws.send(document.getElementById("msg").value);
    };
  </script>
</body></html>`,
        { headers: { "Content-Type": "text/html; charset=utf-8" } },
      ),
  },
  fetch(req, server) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      const ok = server.upgrade(req, {
        data: {
          username: url.searchParams.get("user") ?? "anon",
          room: url.searchParams.get("room") ?? "lobby",
        } satisfies WsData,
      });
      return ok ? undefined : new Response("Upgrade failed", { status: 400 });
    }
    return new Response("Not Found", { status: 404 });
  },
  websocket: {
    data: {} as WsData,
    open(ws) {
      ws.subscribe(ws.data.room);
      server.publish(ws.data.room, `* ${ws.data.username} joined`);
    },
    message(ws, message) {
      server.publish(ws.data.room, `${ws.data.username}: ${message}`);
    },
    close(ws) {
      ws.unsubscribe(ws.data.room);
      server.publish(ws.data.room, `* ${ws.data.username} left`);
    },
  },
});

console.log(`open ${server.url}?user=ada&room=lobby`);
```

### 15.3 SQLite REST (공식 실무 예 축약)

출처 패턴: [Server — Practical example](https://bun.com/docs/runtime/http/server)

```ts
import { Database } from "bun:sqlite";

const db = new Database("posts.db");
db.exec(`
  CREATE TABLE IF NOT EXISTS posts (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
  )
`);

Bun.serve({
  routes: {
    "/api/posts": {
      GET: () => Response.json(db.query("SELECT * FROM posts").all()),
      POST: async (req) => {
        const post = await req.json();
        const id = crypto.randomUUID();
        db.query(
          `INSERT INTO posts (id, title, content, created_at) VALUES (?, ?, ?, ?)`,
        ).run(id, post.title, post.content, new Date().toISOString());
        return Response.json({ id, ...post }, { status: 201 });
      },
    },
    "/api/posts/:id": (req) => {
      const post = db.query("SELECT * FROM posts WHERE id = ?").get(req.params.id);
      if (!post) return new Response("Not Found", { status: 404 });
      return Response.json(post);
    },
  },
  error(error) {
    console.error(error);
    return new Response("Internal Server Error", { status: 500 });
  },
});
```

### 15.4 Graceful shutdown (신호 처리)

`SIGINT`/`SIGTERM`을 받아 in-flight 요청을 마치고 종료한다. 컨테이너·PM 환경 필수 패턴.

```ts
// bun graceful.ts
const server = Bun.serve({
  fetch: async () => {
    await Bun.sleep(50); // 처리 중인 요청 흉내
    return new Response("ok");
  },
});

let closing = false;
async function shutdown(signal: string) {
  if (closing) return;
  closing = true;
  console.log(`\n${signal} 수신 — 새 연결 차단, in-flight 대기…`);
  await server.stop();          // 인자 없이: 진행 중 요청 완료까지 대기
  console.log("종료 완료");
  process.exit(0);
}

process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));

console.log(`${server.url} (Ctrl+C 로 graceful stop)`);
```

`server.stop(true)`는 진행 중 요청까지 즉시 끊는 강제 종료다.

### 15.5 라우트별 rate limit (인메모리 토큰버킷)

의존성 없이 IP 기준 고정 윈도우 제한. (단일 프로세스 기준 — 클러스터면 공유 저장소 필요)

```ts
const hits = new Map<string, { count: number; resetAt: number }>();
const LIMIT = 5, WINDOW_MS = 10_000;

function rateLimit(ip: string): boolean {
  const now = Date.now();
  const rec = hits.get(ip);
  if (!rec || now > rec.resetAt) {
    hits.set(ip, { count: 1, resetAt: now + WINDOW_MS });
    return true;
  }
  if (rec.count >= LIMIT) return false;
  rec.count++;
  return true;
}

Bun.serve({
  fetch(req, server) {
    const ip = server.requestIP(req)?.address ?? "unknown";
    if (!rateLimit(ip)) {
      return new Response("Too Many Requests", {
        status: 429,
        headers: { "Retry-After": "10" },
      });
    }
    return new Response("ok");
  },
});
```

---

## 16. 체크리스트 · 한계

### 할 수 있는 것 (Bun 자체)

- [x] HTTP 라우팅(정적·`:param`·`*`·메서드별)
- [x] WebSocket + pub/sub + 압축
- [x] TLS / (실험) HTTP/3
- [x] 쿠키 자동 Set-Cookie (**`routes`의 BunRequest** 기준)
- [x] 파일·Range·sendfile
- [x] SSE/스트리밍 (`timeout` 주의)
- [x] HTML import + HMR (`bun --hot`)
- [x] `server.reload` / `--hot`
- [x] 메트릭(`pendingRequests` 등)

WebSocket 사용 시 `websocket.message` 핸들러는 반드시 둔다.

### 이 문서 범위 밖 / 별도

| 항목 | 비고 |
|---|---|
| Express식 미들웨어 체인 | 직접 조합 또는 래퍼 — 완성 예시는 [§17](#17-외부-의존성-0-express식-미니-프레임워크) |
| JWT 풀스택 라이브러리 | Web Crypto로 직접 구현 가능 |
| gRPC / GraphQL 서버 스택 | 외부 필요 |
| Redis/Postgres **서버 프로세스** | Bun은 클라만 내장 (`Bun.redis` / `Bun.sql`) |

### 빠른 명령

```bash
# Hello World
bun --hot server.ts

# 포트 지정
bun --port=4000 --hot server.ts

# 프로덕션식 실행(핫리로드 없음)
bun server.ts
```

---

## 17. 외부 의존성 0 Express식 미니 프레임워크

이 폴더의 [`mini/`](./mini/)에 **런타임 의존성 0**으로 Express 스타일 API를 `Bun.serve` 위에 구현했다.  
전부 로컬에서 요청을 실제로 날려 검증했다(라우팅·파라미터·미들웨어 체인·에러 핸들러·쿠키·CORS·인증).

| 파일 | 내용 |
|---|---|
| [`mini/mini.ts`](./mini/mini.ts) | 본체 — `Req` / `Res` / `Router` / `App` |
| [`mini/middleware.ts`](./mini/middleware.ts) | logger · cors · bodyParser · staticDir · requireBearer · errorHandler |
| [`mini/example.ts`](./mini/example.ts) | 데모 앱 |
| [`mini/README.md`](./mini/README.md) | 사용법 · Express 대비 차이 |

### 17.1 설계

- 핸들러 시그니처는 **Express와 동일한 `(req, res, next)`**, 에러 핸들러는 인자 4개 `(err, req, res, next)`.
- 내부는 Web 표준 `Request`/`Response`. `Res`는 상태·헤더·쿠키·바디를 축적하다 터미널 메서드(`json`/`send`/`html`/`redirect`) 호출 시 확정한다.
- 라우트 매칭은 경로를 정규식으로 컴파일(`:param` → `([^/]+)`, `*` → `(.*)`). `use(path, ...)`는 프리픽스 매칭.
- `App.handle`은 `(Request) => Promise<Response>`라서 `Bun.serve`의 `fetch`에 그대로 꽂힌다 → §8의 WebSocket과도 한 서버에서 공존 가능.

### 17.2 사용 예

```ts
import { mini, router } from "./mini/mini.ts";
import { logger, cors, bodyParser, requireBearer, errorHandler } from "./mini/middleware.ts";

const app = mini();

app.use(logger());
app.use(cors());
app.use(bodyParser()); // req.body 채움

app.get("/hello/:name", (req, res) => res.json({ hello: req.params.name }));

const api = router();
api.get("/users", (_req, res) => res.json([{ id: "1", name: "ada" }]));
api.post("/users", (req, res) => {
  const body = req.body as { name?: string };
  if (!body?.name) return void res.status(400).json({ error: "name required" });
  res.status(201).json({ id: crypto.randomUUID(), name: body.name });
});
api.delete("/users/:id", requireBearer("secret"), (req, res) => {
  res.status(204).send("");
});
app.use("/api", api); // 서브 라우터 마운트

app.get("/boom", () => { throw Object.assign(new Error("kaboom"), { status: 418 }); });
app.use(errorHandler()); // 항상 마지막

app.listen(3000, (s) => console.log(`mini on ${s.url}`));
```

```bash
bun --hot mini/example.ts
```

### 17.3 핵심 구현 발췌 — 미들웨어 체인

Express의 `next()` 기반 onion(양파 껍질) 흐름을 인덱스 재귀로 구현한다. 핸들러가 `next()`를 부르면 다음 레이어로, `next(err)`거나 throw면 에러 핸들러 체인으로 점프한다.

```ts
const run = async (i: number): Promise<void> => {
  if (res.finished) return;
  const entry = matched[i];
  if (!entry) return; // 매칭 소진 → (아무도 응답 안 하면) 404
  req.params = entry.params;
  let nextErr: unknown, nextCalled = false;
  const next = (e?: unknown) => { nextCalled = true; nextErr = e; };
  try { await entry.layer.handler(req, res, next); }
  catch (e) { return runError(0, e); }         // throw → 에러 핸들러
  if (res.finished) return;                    // 응답 확정 → 종료
  if (nextCalled) {
    if (nextErr !== undefined) return runError(0, nextErr); // next(err)
    return run(i + 1);                         // 다음 레이어
  }
  // next 도 응답도 없으면 여기서 멈춤
};
```

> ⚠️ 구현 중 실제로 밟은 버그: `use("/")` 같은 루트 프리픽스가 정규식 `^/(?=/|$)`로 컴파일되어 `/api/x` 하위 경로에 매칭되지 않았다(전역 미들웨어가 하위 라우트에서 안 돎). 프리픽스의 트레일링 슬래시를 먼저 제거(`"/" → ""`)해 `^(?=/|$)`로 만들면 모든 경로에 매칭된다. — [`mini.ts`](./mini/mini.ts)의 `compile()` 참고.

### 17.4 검증한 동작

| 케이스 | 결과 |
|---|---|
| `GET /hello/ada` | `{ "hello": "ada" }` |
| `POST /api/users {name:"lin"}` | `201` + 생성 객체 |
| `POST /api/users {}` | `400 name required` |
| `DELETE /api/users/2` (토큰 없음) | `401` |
| `DELETE /api/users/2` (`Bearer secret`) | `204` |
| `OPTIONS /api/users` (CORS preflight) | `204` + `Access-Control-Allow-Origin: *` |
| `GET /boom` (throw, `status:418`) | `418` + `{ "error": "kaboom" }` |
| 미매칭 경로 | `404` |

### 17.5 여기서 더 나아간다면

- **WebSocket 통합**: `app.handle`을 `fetch`에 두고, 같은 `Bun.serve`에 `websocket` 핸들러를 추가하면 HTTP+WS 단일 서버 ([§8](#8-websocket), [§15.2](#152-http--websocket-채팅-한-서버)).
- **정적 서빙**: `middleware.ts`의 `staticDir()`가 이미 `BunFile` 제로카피로 처리.
- **검증·인증 강화**: Web Crypto(`crypto.subtle`)로 JWT 서명/검증을 의존성 없이 추가 가능.
- **성능**: 정규식 매칭 대신 §4의 네이티브 `routes`로 핫패스를 옮기면 제로할당 디스패치 이점을 얻는다(미들웨어가 불필요한 라우트에 한해).