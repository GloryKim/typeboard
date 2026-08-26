# Bun.serve 예제 전체 보고서

> - 대상: `01_hello.ts` ~ `41_ws_publish.ts` (41개) + 부속 파일(`_mini/mini.ts`, `_html_import/*`, `_public/*`, `_static/*`, `_data/*`, `_certs/*`)
> - 검증 런타임: **Bun 1.3.14** (`bun --version` 확인)
> - 외부 프레임워크·npm 패키지 없음. `bun:sqlite` · Web Crypto · HTML import 는 모두 Bun 내장.

---

## 목차

- [0. 개요와 공통 개념](#0-개요와-공통-개념)
- [초급 (01–16)](#초급-0116)
  - [01 hello](#01_hellots--hello-world) · [02 html](#02_htmlts--html-응답) · [03 json](#03_jsonts--json-응답) · [04 routes](#04_routests--다중-경로--경로-파라미터) · [05 methods](#05_methodsts--http-메서드별-핸들러) · [06 query](#06_queryts--쿼리-파라미터) · [07 body](#07_bodyts--요청-바디) · [08 static](#08_staticts--정적-파일)
  - [09 cookies](#09_cookiests--쿠키) · [10 websocket](#10_websocketts--websocket-에코) · [11 cors](#11_corsts--cors) · [12 redirect](#12_redirectts--정적-response--리다이렉트--헬스체크) · [13 error](#13_errorts--에러-처리) · [14 upload](#14_uploadts--파일-업로드) · [15 sse](#15_ssets--server-sent-events) · [16 auth](#16_authts--bearer-토큰-인증-가드)
- [중급 (17–32)](#중급-1732)
  - [17 middleware](#17_middlewarets--미들웨어-합성) · [18 static_dir](#18_static_dirts--정적-디렉터리-서버) · [19 stream](#19_streamts--readablestream-응답) · [20 ws_pubsub](#20_ws_pubsubts--websocket-pubsub-채팅) · [21 crud](#21_crudts--rest-crud) · [22 sqlite](#22_sqlitets--sqlite-rest) · [23 metrics](#23_metricsts--메트릭--requestip) · [24 ratelimit](#24_ratelimitts--rate-limit)
  - [25 graceful](#25_gracefults--graceful-shutdown) · [26 reload](#26_reloadts--serverreload) · [27 unix](#27_unixts--unix-domain-socket) · [28 range](#28_rangets--range-요청) · [29 proxy](#29_proxyts--리버스-프록시) · [30 timeout](#30_timeoutts--idletimeout--servertimeout) · [31 session](#31_sessionts--쿠키-세션) · [32 export_default](#32_export_defaultts--export-default--라우트-우선순위)
- [고급 (33–41)](#고급-3341)
  - [33 tls](#33_tlsts--tls--https) · [34 http3](#34_http3ts--http3-실험적) · [35 html_import](#35_html_importts--html-import--hmr) · [36 ws_advanced](#36_ws_advancedts--websocket-고급) · [37 jwt](#37_jwtts--jwt-hmac-sha256) · [38 mini_app](#38_mini_appts--express식-미니-프레임워크) · [39 production](#39_productionts--실전-통합) · [40 hardening](#40_hardeningts--프로덕션-하드닝) · [41 ws_publish](#41_ws_publishts--wspublish-vs-serverpublish)
- [부속 파일](#부속-파일)
- [전체 검토 요약표](#전체-검토-요약표)

---

## 0. 개요와 공통 개념

### 실행 방법 (readme.md 원문)

```bash
bun --hot 01_hello.ts
bun --hot 17_middleware.ts
bun --hot 33_tls.ts          # curl -k https://localhost:3033/
bun --hot 35_html_import.ts  # 브라우저 HMR
```

포트 배치: `01_hello.ts`(3001) ~ `41_ws_publish.ts`(3041) — **파일 번호 + 3000 = 포트**. 겹치지 않으므로 여러 개를 동시에 켜서 비교 학습할 수 있습니다.

### Bun.serve 핵심 API (예제 전반에서 반복되는 개념)

| 개념 | 요약 |
|---|---|
| `routes` | 경로 → 핸들러 맵. `"/path": handler` 또는 메서드별 `{ GET, POST, ... }`. 값이 `Response` 객체면 **제로할당 정적 디스패치**. |
| 경로 파라미터 | `"/users/:id"` → 핸들러의 `req.params.id`. 이때 `req` 는 확장 타입 **BunRequest**(`params`, `cookies` 보유). |
| 라우트 우선순위 | **exact > `:param` > `/prefix/*` > `/*`** (예제 32 참고). |
| `fetch(req, server)` | `routes` 에 안 걸린 모든 요청의 최종 폴백. WebSocket 업그레이드도 보통 여기서. |
| `server.upgrade(req, { data })` | HTTP→WS 승격. 성공 시 `true`, 이후 `fetch` 반환값은 무시(관례상 `undefined`). |
| `websocket: { open, message, close, drain }` | WS 콜백. `server.publish` / `ws.publish` / `ws.subscribe` 로 pub/sub. |
| `server.requestIP(req)` | `{ address, family, port }` 클라이언트 소켓 정보. |
| `server.timeout(req, sec)` | **요청 단위** 유휴 타임아웃 (0=비활성). SSE/롱폴링 필수. |
| `idleTimeout` | 서버 전역 HTTP 유휴 타임아웃(초). 기본 10, 최대 255, 0=비활성. |
| `error(err)` | 라우트/`fetch` 에서 throw 된 에러 → `Response` 로 변환. |
| `development` | `true` 면 내장 에러 페이지/스택 노출. 프로덕션은 `false`. |
| `maxRequestBodySize` | 초과 바디는 **핸들러 도달 전 자동 413**. |
| `server.reload(opts)` | 포트 유지한 채 핸들러 교체. |
| `server.stop(force?)` | graceful(기본) / 강제(`true`) 종료. |

> **참고 — `bun --hot` vs `bun`**: `--hot` 은 파일 수정 시 상태를 최대한 보존하며 리로드(HMR). SIGINT/graceful 종료(예제 25)나 `server.reload` 타이밍(예제 26)을 관찰할 때는 `--hot` 없이 순수 `bun` 으로 돌리는 게 명확합니다.

---

# 초급 (01–16)

## `01_hello.ts` — Hello World

포트 **3001**. Bun.serve 최소 구성. `fetch` 하나만 두면 모든 요청에 같은 응답.

```ts
// 01. Hello World — 최소 Bun.serve
// 실행: bun --hot 01_hello.ts
// 확인: curl http://localhost:3001/

const server = Bun.serve({
  port: 3001,
  fetch() {
    return new Response("Hello, Bun.serve!");
  },
});

console.log(`01 hello → ${server.url}`);
```

**해설** — `Bun.serve` 는 `server` 객체를 반환하고 `server.url` 로 실제 바인딩 주소를 얻습니다. `fetch` 는 인자를 안 써서 생략했습니다.

**검토** — 문제 없음(의도된 최소 예제). 보충: `port: 0` 을 주면 OS가 빈 포트를 자동 할당하며, 그 실제 포트는 `server.port` 로 알 수 있습니다(테스트에서 유용).

---

## `02_html.ts` — HTML 응답

포트 **3002**. 문자열 HTML을 `Content-Type: text/html` 로 반환.

```ts
// 02. HTML 응답
// 실행: bun --hot 02_html.ts
// 확인: 브라우저에서 http://localhost:3002/

const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Bun.serve HTML</title>
  <style>
    body {
      min-height: 100dvh;
      display: grid;
      place-items: center;
      font-family: ui-sans-serif, system-ui, sans-serif;
      background: #0f1115;
      color: #e8eaed;
    }
    h1 { font-weight: 600; letter-spacing: -0.03em; }
    p { color: #9aa0a6; margin-top: 0.5rem; }
  </style>
</head>
<body>
  <main>
    <h1>Hello HTML</h1>
    <p>Content-Type: text/html</p>
  </main>
</body>
</html>`;

const server = Bun.serve({
  port: 3002,
  fetch() {
    return new Response(html, {
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
  },
});

console.log(`02 html → ${server.url}`);
```

**해설** — `Content-Type` 을 명시하지 않으면 문자열 Response 는 `text/plain` 이 되어 브라우저가 태그를 그대로 보여줍니다. `charset=utf-8` 을 붙여 한글 깨짐을 방지.

**검토** — 문제 없음. 보충: 정적 HTML을 번들·HMR과 함께 서빙하려면 문자열이 아니라 **HTML 파일 import**(예제 35)가 낫습니다.

---

## `03_json.ts` — JSON 응답

포트 **3003**. `Response.json()` 헬퍼로 JSON 직렬화 + `Content-Type` 자동 설정.

```ts
// 03. JSON 응답 — Response.json
// 실행: bun --hot 03_json.ts
// 확인: curl http://localhost:3003/api/status

const server = Bun.serve({
  port: 3003,
  routes: {
    "/": () =>
      Response.json({
        message: "Bun.serve JSON 예제",
        tip: "GET /api/status",
      }),
    "/api/status": () =>
      Response.json({
        ok: true,
        runtime: "bun",
        time: new Date().toISOString(),
      }),
  },
  fetch() {
    return Response.json({ error: "Not Found" }, { status: 404 });
  },
});

console.log(`03 json → ${server.url}`);
```

**해설** — `routes` 에 정의된 경로만 매칭되고, 나머지는 `fetch` 폴백이 JSON 404를 반환합니다. `Response.json` 은 두 번째 인자로 `{ status, headers }` 를 받습니다.

**검토** — 문제 없음. 이 구조(“routes = 정상 경로, fetch = 404 폴백”)가 이후 대부분 예제의 골격입니다.

---

## `04_routes.ts` — 다중 경로 + 경로 파라미터

포트 **3004**. `:id` 같은 동적 세그먼트를 `req.params` 로 받습니다.

```ts
// 04. routes — 다중 경로 + 경로 파라미터 (:id)
// 실행: bun --hot 04_routes.ts
// 확인:
//   curl http://localhost:3004/
//   curl http://localhost:3004/users/42
//   curl http://localhost:3004/orgs/acme/repos/api

const server = Bun.serve({
  port: 3004,
  routes: {
    "/": () => new Response("홈"),
    "/about": () => new Response("소개 페이지"),
    "/users/:id": (req) => {
      const { id } = req.params;
      return Response.json({ userId: id });
    },
    "/orgs/:orgId/repos/:repoId": (req) => {
      const { orgId, repoId } = req.params;
      return Response.json({ orgId, repoId });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`04 routes → ${server.url}`);
```

**해설** — 여러 파라미터(`:orgId`, `:repoId`)도 동시에 지원. 여기서 `req` 는 `params` 를 가진 BunRequest이므로 별도 파싱 없이 구조분해할 수 있습니다.

**검토** — 문제 없음. 보충: `req.params.id` 값은 URL 디코딩된 문자열입니다. 숫자로 쓰려면 예제 06처럼 `Number()` + 검증을 직접 해야 합니다.

---

## `05_methods.ts` — HTTP 메서드별 핸들러

포트 **3005**. 같은 경로에서 `GET`/`POST` 를 분기하고 요청 바디를 검증.

```ts
// 05. HTTP 메서드별 핸들러 (GET / POST)
// 실행: bun --hot 05_methods.ts
// 확인:
//   curl http://localhost:3005/api/posts
//   curl -X POST http://localhost:3005/api/posts \
//     -H 'Content-Type: application/json' \
//     -d '{"title":"첫 글"}'

type Post = { id: number; title: string };

const posts: Post[] = [
  { id: 1, title: "Hello Bun" },
  { id: 2, title: "routes 배우기" },
];

const server = Bun.serve({
  port: 3005,
  routes: {
    "/api/posts": {
      GET: () => Response.json({ posts }),
      POST: async (req) => {
        const body = (await req.json()) as { title?: string };
        if (!body.title?.trim()) {
          return Response.json({ error: "title required" }, { status: 400 });
        }
        const post: Post = { id: posts.length + 1, title: body.title.trim() };
        posts.push(post);
        return Response.json({ created: true, post }, { status: 201 });
      },
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`05 methods → ${server.url}`);
```

**해설** — 라우트 값에 객체를 주면 메서드별 핸들러가 됩니다. 정의 안 된 메서드(예: `DELETE`)는 Bun이 자동으로 **405 Method Not Allowed** 를 냅니다. `POST` 는 `title` 공백 검증 후 201 생성.

**검토 / 보충**
- ⚠️ **ID 발급이 `posts.length + 1`**: 배열이므로 삭제 기능이 생기면 ID가 충돌할 수 있습니다(예: 3개 중 2번 삭제 → length 2 → 다음 id 3, 이미 존재). 이 예제엔 삭제가 없어 안전하지만, 실무에선 단조 증가 카운터나 `crypto.randomUUID()`(예제 21)를 쓰는 게 안전합니다.
- ⚠️ `await req.json()` 이 잘못된 JSON을 받으면 throw → 여기선 `error()` 핸들러가 없어 Bun 기본 500. 예제 07처럼 `try/catch` 로 400을 주는 편이 사용자 친화적입니다.

```ts
// 개선안: JSON 파싱 실패를 400으로, ID는 단조 카운터로
let nextId = posts.length + 1;
// ...
POST: async (req) => {
  let body: { title?: string };
  try {
    body = (await req.json()) as { title?: string };
  } catch {
    return Response.json({ error: "invalid JSON" }, { status: 400 });
  }
  if (!body.title?.trim()) {
    return Response.json({ error: "title required" }, { status: 400 });
  }
  const post: Post = { id: nextId++, title: body.title.trim() };
  posts.push(post);
  return Response.json({ created: true, post }, { status: 201 });
},
```

---

## `06_query.ts` — 쿼리 파라미터

포트 **3006**. `new URL(req.url).searchParams` 로 쿼리 스트링을 읽고 검증.

```ts
// 06. 쿼리 파라미터 — new URL(req.url).searchParams
// 실행: bun --hot 06_query.ts
// 확인:
//   curl 'http://localhost:3006/search?q=bun&page=2'
//   curl 'http://localhost:3006/search'   # 400 기대

const server = Bun.serve({
  port: 3006,
  routes: {
    "/": () =>
      new Response("예: GET /search?q=키워드&page=1"),
    "/search": (req) => {
      const url = new URL(req.url);
      const term = url.searchParams.get("q")?.trim();
      const page = Number(url.searchParams.get("page") ?? "1");

      if (!term) {
        return Response.json({ error: "q required" }, { status: 400 });
      }
      if (!Number.isInteger(page) || page < 1) {
        return Response.json({ error: "bad page" }, { status: 400 });
      }

      return Response.json({
        term,
        page,
        results: [`${term} 결과 #${page}`],
      });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`06 query → ${server.url}`);
```

**해설** — `searchParams.get()` 는 없으면 `null`. `?? "1"` 기본값 처리 후 `Number.isInteger` + 범위로 검증. `page=abc` → `NaN` → `isInteger(NaN)` false → 400. `page=2.5` → 400. 깔끔한 검증 패턴.

**검토** — 문제 없음. 보충: 쿼리를 여러 번 파싱한다면 `URL` 객체를 한 번만 만들어 재사용하세요(이 예제는 이미 그렇게 함).

---

## `07_body.ts` — 요청 바디

포트 **3007**. `text()` / `json()` / `formData()` 세 가지 바디 파싱.

```ts
// 07. 요청 바디 — text / json / formData
// 실행: bun --hot 07_body.ts
// 확인:
//   curl -X POST http://localhost:3007/echo/text -d 'hello'
//   curl -X POST http://localhost:3007/echo/json \
//     -H 'Content-Type: application/json' -d '{"name":"bun"}'
//   curl -X POST http://localhost:3007/echo/form -d 'name=bun&age=1'

const server = Bun.serve({
  port: 3007,
  routes: {
    "/": () =>
      new Response(
        "POST /echo/text | /echo/json | /echo/form 을 호출해 보세요",
      ),
    "/echo/text": {
      POST: async (req) => {
        const text = await req.text();
        return Response.json({ type: "text", length: text.length, text });
      },
    },
    "/echo/json": {
      POST: async (req) => {
        try {
          const data = await req.json();
          return Response.json({ type: "json", data });
        } catch {
          return Response.json({ error: "invalid JSON" }, { status: 400 });
        }
      },
    },
    "/echo/form": {
      POST: async (req) => {
        const form = await req.formData();
        const entries = Object.fromEntries(form.entries());
        return Response.json({ type: "form", entries });
      },
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`07 body → ${server.url}`);
```

**해설** — `/echo/json` 이 예제 05의 개선안처럼 `try/catch` 로 잘못된 JSON을 400 처리하는 **모범 사례**입니다. `formData()` 는 `application/x-www-form-urlencoded` 와 `multipart/form-data` 를 모두 파싱.

**검토 / 보충**
- `Object.fromEntries(form.entries())` 는 **같은 이름의 필드가 여러 개면 마지막 값만** 남습니다(예: `a=1&a=2` → `{a:"2"}`). 다중 값이 필요하면 아래처럼 처리하세요.

```ts
// 다중 값 보존
const entries: Record<string, string | string[]> = {};
for (const [k, v] of form.entries()) {
  const val = typeof v === "string" ? v : `[file ${v.name}]`;
  if (k in entries) entries[k] = ([] as string[]).concat(entries[k], val);
  else entries[k] = val;
}
```

- 바디 본문(body stream)은 **한 번만** 읽을 수 있습니다. `req.text()` 후 `req.json()` 을 또 부르면 에러. 하나만 고르세요.

---

## `08_static.ts` — 정적 파일

포트 **3008**. `Bun.file()` 로 디스크 파일을 스트리밍 서빙.

```ts
// 08. 정적 파일 — Bun.file + Response
// 실행: bun --hot 08_static.ts
// 확인:
//   curl http://localhost:3008/
//   curl http://localhost:3008/readme.txt
//   curl -o /tmp/out.html http://localhost:3008/page.html

const ROOT = import.meta.dir;

// 예제용 파일 (같은 폴더에 없으면 실행 시 생성)
const readmePath = `${ROOT}/_static/readme.txt`;
const pagePath = `${ROOT}/_static/page.html`;

await Bun.write(
  readmePath,
  "Bun.file 로 정적 파일을 서빙하는 초급 예제입니다.\n",
);
await Bun.write(
  pagePath,
  `<!DOCTYPE html>
<html lang="ko">
<head><meta charset="UTF-8" /><title>Static</title></head>
<body><h1>정적 HTML</h1><p>Bun.file → Response</p></body>
</html>
`,
);

const server = Bun.serve({
  port: 3008,
  routes: {
    "/": () =>
      Response.json({
        files: ["/readme.txt", "/page.html"],
      }),
    // 요청마다 파일 읽기 (큰 파일·자주 바뀌는 파일에 적합)
    "/readme.txt": () => new Response(Bun.file(readmePath)),
    "/page.html": () =>
      new Response(Bun.file(pagePath), {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`08 static → ${server.url}`);
```

**해설** — `new Response(Bun.file(path))` 는 파일을 메모리에 다 올리지 않고 스트리밍하며, `Content-Type` 을 확장자로 자동 추론(`.txt`→text/plain 등)합니다. `.html` 은 이 예제에서 굳이 헤더를 직접 지정했는데, Bun이 `.html` 도 자동 추론하므로 없어도 동작합니다(명시가 더 안전).

**검토 / 보충**
- 이 예제는 **경로를 하드코딩**해서 안전합니다. 임의 경로를 받는 디렉터리 서버는 경로 이탈(`..`) 방어가 필요 → 예제 18 참고.
- 파일이 없을 때 `new Response(Bun.file(없는경로))` 는 응답 시점에 빈 바디/에러가 될 수 있으니, 동적 경로라면 `await file.exists()` 로 먼저 확인하세요(예제 18).

---

## `09_cookies.ts` — 쿠키

포트 **3009**. BunRequest의 `req.cookies` 로 쿠키 읽기/설정/삭제. 방문 횟수 카운터.

```ts
// 09. 쿠키 — req.cookies (BunRequest)
// 실행: bun --hot 09_cookies.ts
// 확인:
//   curl -i -c /tmp/bun-c.txt -b /tmp/bun-c.txt http://localhost:3009/visit
//   curl -i -c /tmp/bun-c.txt -b /tmp/bun-c.txt http://localhost:3009/visit
//   curl -i -c /tmp/bun-c.txt -b /tmp/bun-c.txt http://localhost:3009/logout

const server = Bun.serve({
  port: 3009,
  routes: {
    "/": () =>
      new Response("GET /visit → 방문 횟수 쿠키 | GET /logout → 쿠키 삭제"),
    "/visit": (req) => {
      const raw = req.cookies.get("visits") ?? "0";
      const visits = Number(raw) + 1;
      req.cookies.set("visits", String(visits), {
        path: "/",
        httpOnly: true,
        sameSite: "lax",
      });
      return Response.json({ visits, message: `${visits}번째 방문` });
    },
    "/logout": (req) => {
      req.cookies.delete("visits", { path: "/" });
      return Response.json({ ok: true, message: "visits 쿠키 삭제" });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`09 cookies → ${server.url}`);
```

**해설** — `req.cookies` 는 `CookieMap` 입니다. `.set()`/`.delete()` 로 변경하면 Bun이 응답에 `Set-Cookie` 헤더를 **자동으로** 붙여줍니다(직접 헤더를 만들 필요 없음). `httpOnly`+`sameSite:"lax"` 는 기본 보안 옵션.

**검토 / 보충**
- ⚠️ **`req.cookies` 는 `routes` 핸들러(BunRequest)에서만** 제공됩니다. `fetch(req, ...)` 폴백의 `req` 는 표준 `Request` 라 `cookies` 가 없습니다. 폴백에서 쿠키를 다루려면 `req.headers.get("cookie")` 를 직접 파싱해야 합니다.
- 값이 신뢰 못 할 클라이언트에서 오므로 `Number(raw)` 가 `NaN` 이 될 수 있습니다(변조 시). `Number(raw) || 0` 로 방어하면 더 견고합니다.
- 실제 로그인 세션은 서버측 세션 스토어(예제 31)와 결합해야 합니다.

---

## `10_websocket.ts` — WebSocket 에코

포트 **3010**. HTTP→WS 업그레이드 + 에코 서버 + 테스트용 HTML 페이지.

```ts
// 10. WebSocket — 에코 서버
// 실행: bun --hot 10_websocket.ts
// 확인 (브라우저 콘솔 또는 아래 스크립트):
//   const ws = new WebSocket("ws://localhost:3010/ws");
//   ws.onmessage = (e) => console.log(e.data);
//   ws.onopen = () => ws.send("안녕");
//
// 또는: bun -e '
//   const ws = new WebSocket("ws://localhost:3010/ws");
//   ws.addEventListener("message", (e) => { console.log(e.data); ws.close(); });
//   ws.addEventListener("open", () => ws.send("ping"));
// '

const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <title>WS Echo</title>
  <style>
    body { font-family: ui-sans-serif, system-ui, sans-serif; padding: 2rem; }
    #log { white-space: pre-wrap; background: #f4f4f5; padding: 1rem; }
  </style>
</head>
<body>
  <h1>WebSocket Echo</h1>
  <input id="msg" value="hello" />
  <button id="send">보내기</button>
  <pre id="log"></pre>
  <script>
    const log = document.getElementById("log");
    const ws = new WebSocket("ws://" + location.host + "/ws");
    ws.onmessage = (e) => { log.textContent += "← " + e.data + "\\n"; };
    ws.onopen = () => { log.textContent += "연결됨\\n"; };
    document.getElementById("send").onclick = () => {
      const v = document.getElementById("msg").value;
      ws.send(v);
      log.textContent += "→ " + v + "\\n";
    };
  </script>
</body>
</html>`;

const server = Bun.serve({
  port: 3010,
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
  },
  fetch(req, server) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      // HTTP → WebSocket 업그레이드
      if (server.upgrade(req)) return undefined as never;
      return new Response("WebSocket upgrade failed", { status: 400 });
    }
    return new Response("Not Found", { status: 404 });
  },
  websocket: {
    open(ws) {
      ws.send("connected");
    },
    message(ws, message) {
      ws.send(`echo: ${message}`);
    },
    close(ws) {
      console.log("client closed");
    },
  },
});

console.log(`10 websocket → ${server.url}`);
```

**해설** — 업그레이드는 `routes` 가 아니라 `fetch` 안에서 `server.upgrade(req)` 로 합니다. 성공 시 응답을 반환하면 안 되므로 관례상 `undefined as never`. 이후 통신은 `websocket.{open,message,close}` 콜백이 담당. `open` 에서 `"connected"`, `message` 마다 `echo: ...`.

**검토 / 보충**
- `close(ws)` 의 `ws` 파라미터가 미사용이라 TS `noUnusedParameters` 설정 시 경고. `close()` 로 비우거나 `_ws` 로 두면 깔끔합니다(동작엔 무관).
- `return undefined as never` 는 타입 트릭입니다. Bun 최신 타입에선 `server.upgrade` 성공 시 `undefined` 반환이 허용되므로 `return;` 만 써도 됩니다.

---

## `11_cors.ts` — CORS

포트 **3011**. 프리플라이트(OPTIONS) 포함 CORS 헤더 처리.

```ts
// 11. CORS — 프리플라이트(OPTIONS) 포함
// 실행: bun --hot 11_cors.ts
// 확인:
//   curl -i -X OPTIONS http://localhost:3011/api/hello \
//     -H 'Origin: http://localhost:5173' \
//     -H 'Access-Control-Request-Method: GET'
//   curl -i http://localhost:3011/api/hello -H 'Origin: http://localhost:5173'

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET,POST,PUT,DELETE,OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

const server = Bun.serve({
  port: 3011,
  async fetch(req) {
    if (req.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS });
    }

    const url = new URL(req.url);
    if (url.pathname === "/api/hello") {
      return Response.json({ message: "cors ok" }, { headers: CORS });
    }

    return Response.json({ error: "Not Found" }, { status: 404, headers: CORS });
  },
});

console.log(`11 cors → ${server.url}`);
```

**해설** — 프리플라이트(OPTIONS)에 204 + CORS 헤더로 응답하고, 실제 요청 응답에도 같은 헤더를 붙입니다. `Access-Control-Allow-Origin: *` 는 모든 출처 허용.

**검토 / 보충**
- ⚠️ **와일드카드 `*` + 자격증명(쿠키) 조합은 브라우저가 거부**합니다. 쿠키 세션(예제 31/39)과 CORS를 함께 쓰려면 `Allow-Origin` 을 **구체 출처**로 반사하고 `Allow-Credentials: true` 를 추가해야 합니다.

```ts
// 자격증명 허용 CORS (허용 목록 반사 방식)
const ALLOW = new Set(["http://localhost:5173", "http://localhost:3000"]);
function corsFor(origin: string | null) {
  const h: Record<string, string> = {
    "Access-Control-Allow-Methods": "GET,POST,PUT,DELETE,OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization",
    Vary: "Origin", // 캐시 오염 방지
  };
  if (origin && ALLOW.has(origin)) {
    h["Access-Control-Allow-Origin"] = origin;
    h["Access-Control-Allow-Credentials"] = "true";
  }
  return h;
}
```

- 프리플라이트 응답에 `Access-Control-Max-Age` 를 주면 브라우저가 결과를 캐시해 OPTIONS 왕복을 줄입니다.

---

## `12_redirect.ts` — 정적 Response · 리다이렉트 · 헬스체크

포트 **3012**. 라우트 값에 **함수가 아닌 Response 객체**를 직접 두는 제로할당 패턴.

```ts
// 12. 정적 Response · 리다이렉트 · 헬스체크
// 실행: bun --hot 12_redirect.ts
// 확인:
//   curl -i http://localhost:3012/health
//   curl -i http://localhost:3012/blog          # 302 Location
//   curl -i http://localhost:3012/api/config

const server = Bun.serve({
  port: 3012,
  routes: {
    // Response 객체를 그대로 두면 제로할당 디스패치 (고정 응답에 적합)
    "/health": new Response("OK"),
    "/ready": new Response("Ready", {
      headers: { "X-Ready": "1" },
    }),
    "/blog": Response.redirect("https://bun.com/blog"),
    "/api/config": Response.json({
      version: "1.0.0",
      env: "development",
    }),
    "/": () =>
      Response.json({
        tip: ["GET /health", "GET /ready", "GET /blog", "GET /api/config"],
      }),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`12 redirect → ${server.url}`);
```

**해설** — 고정 응답은 매 요청 새 객체를 만들 필요가 없으므로 라우트에 **미리 만든 Response** 를 두면 Bun이 그대로 재사용(제로할당). 헬스체크(`/health`, `/ready`)나 리다이렉트에 적합. `Response.redirect` 는 기본 302.

**검토 / 보충**
- 정적 Response 는 “불변”이라는 전제입니다. 시간 등 **동적 값**이 필요하면 반드시 함수(`() => ...`)로 두세요. 예: `/api/config` 는 고정값이라 괜찮지만, 만약 `time: new Date()...` 를 넣으면 서버 시작 시각에 고정됩니다.
- 영구 이동은 `Response.redirect(url, 301)` 처럼 상태코드를 명시하세요.

---

## `13_error.ts` — 에러 처리

포트 **3013**. `error()` 콜백으로 throw된 예외를 Response로 변환 + `development:true`.

```ts
// 13. 에러 처리 — error 핸들러 + development
// 실행: bun --hot 13_error.ts
// 확인:
//   curl -i http://localhost:3013/ok
//   curl -i http://localhost:3013/boom          # error() 가 잡음
//   curl -i http://localhost:3013/missing      # 404

const server = Bun.serve({
  port: 3013,
  development: true,
  routes: {
    "/ok": () => Response.json({ ok: true }),
    "/boom": () => {
      throw new Error("의도적으로 터뜨린 에러");
    },
  },
  fetch() {
    return Response.json({ error: "Not Found" }, { status: 404 });
  },
  // 라우트/fetch 안에서 던진 에러 → 여기서 Response 로 변환
  error(err) {
    console.error("[error]", err);
    return Response.json(
      {
        error: "Internal Server Error",
        message: err instanceof Error ? err.message : String(err),
      },
      { status: 500 },
    );
  },
});

console.log(`13 error → ${server.url}`);
```

**해설** — 핸들러에서 throw 된 예외는 `error(err)` 로 모입니다. 커스텀 500 JSON을 반환하고 로그도 남깁니다. `development:true` 면 `error()` 를 안 뒀을 때 Bun이 상세 스택 페이지를 보여줍니다.

**검토 / 보충**
- ⚠️ **에러 메시지를 그대로 클라이언트에 노출**(`message: err.message`)하는 것은 개발용입니다. 프로덕션에선 내부 정보 유출이 될 수 있으니 `development` 플래그로 분기하세요.

```ts
error(err) {
  console.error("[error]", err);
  const dev = process.env.NODE_ENV !== "production";
  return Response.json(
    dev
      ? { error: "Internal Server Error", message: String(err) }
      : { error: "Internal Server Error" },
    { status: 500 },
  );
},
```

- 비동기 핸들러 내부의 rejected Promise도 `error()` 로 잡힙니다(`async` 핸들러를 await 하므로).

---

## `14_upload.ts` — 파일 업로드

포트 **3014**. `multipart/form-data` 파일을 `Bun.write` 로 저장.

```ts
// 14. 파일 업로드 — multipart formData + File + Bun.write
// 실행: bun --hot 14_upload.ts
// 확인:
//   echo 'hello upload' > /tmp/demo.txt
//   curl -F 'file=@/tmp/demo.txt' http://localhost:3014/upload
//   ls ./_uploads

const UPLOAD_DIR = `${import.meta.dir}/_uploads`;

await Bun.write(`${UPLOAD_DIR}/.keep`, "");

const server = Bun.serve({
  port: 3014,
  routes: {
    "/": () =>
      new Response(
        "POST /upload (multipart, field name: file)\n예: curl -F 'file=@./readme.md' http://localhost:3014/upload\n",
      ),
    "/upload": {
      POST: async (req) => {
        const form = await req.formData();
        const file = form.get("file");

        if (!(file instanceof File)) {
          return Response.json(
            { error: "file field required (multipart)" },
            { status: 400 },
          );
        }

        // 간단 안전: 파일명에서 경로 분리자 제거
        const safeName = file.name.replace(/[/\\]/g, "_") || "upload.bin";
        const out = `${UPLOAD_DIR}/${Date.now()}_${safeName}`;
        await Bun.write(out, file);

        return Response.json({
          saved: out,
          name: file.name,
          size: file.size,
          type: file.type || "application/octet-stream",
        });
      },
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`14 upload → ${server.url}`);
```

**해설** — `form.get("file")` 이 `File` 인스턴스인지 확인 후 저장. `Bun.write(path, file)` 은 File/Blob을 바로 디스크에 씁니다. 파일명 접두사로 `Date.now()` 를 붙여 충돌 방지, 경로 분리자(`/`, `\`)를 `_` 로 치환해 디렉터리 이탈 방어.

**검토 / 보충**
- ⚠️ **파일명 방어가 부족**합니다. `..` 자체는 남으므로 `....//` 같은 조합이나 선행 `.`(숨김파일) 위험이 있습니다. 더 견고하게:

```ts
const base = file.name.split(/[/\\]/).pop() ?? "upload.bin"; // 마지막 세그먼트만
const safeName = base.replace(/[^\w.\-]/g, "_").replace(/^\.+/, "") || "upload.bin";
```

- ⚠️ **업로드 크기 제한**이 없습니다. `maxRequestBodySize`(예제 40) 를 설정해 대용량 업로드로 인한 메모리/디스크 고갈을 막으세요.
- 동시에 같은 밀리초에 같은 이름이 오면 `Date.now()` 만으로는 충돌 가능 → `crypto.randomUUID()` 를 접두사에 함께 쓰면 안전.

---

## `15_sse.ts` — Server-Sent Events

포트 **3015**. `ReadableStream` + `text/event-stream` 으로 실시간 이벤트 푸시.

```ts
// 15. SSE (Server-Sent Events) — 실시간 이벤트 스트림
// 실행: bun --hot 15_sse.ts
// 확인:
//   curl -N http://localhost:3015/events
//   브라우저: http://localhost:3015/

const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <title>SSE</title>
  <style>
    body { font-family: ui-sans-serif, system-ui, sans-serif; padding: 2rem; }
    #log { white-space: pre-wrap; background: #f4f4f5; padding: 1rem; }
  </style>
</head>
<body>
  <h1>SSE Demo</h1>
  <pre id="log"></pre>
  <script>
    const log = document.getElementById("log");
    const es = new EventSource("/events");
    es.onmessage = (e) => { log.textContent += e.data + "\\n"; };
  </script>
</body>
</html>`;

const server = Bun.serve({
  port: 3015,
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
    "/events": (req, srv) => {
      // 기본 idleTimeout(10s)에 끊기지 않도록 비활성
      srv.timeout(req, 0);

      let timer: ReturnType<typeof setInterval> | undefined;
      const stream = new ReadableStream({
        start(controller) {
          const enc = new TextEncoder();
          let n = 0;
          timer = setInterval(() => {
            n += 1;
            controller.enqueue(
              enc.encode(`data: ${JSON.stringify({ n, t: Date.now() })}\n\n`),
            );
            if (n >= 10) {
              clearInterval(timer);
              controller.close();
            }
          }, 500);
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
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`15 sse → ${server.url}`);
```

**해설** — SSE 포맷은 `data: <내용>\n\n`. `srv.timeout(req, 0)` 로 이 요청의 유휴 타임아웃을 꺼 장시간 스트림이 끊기지 않게 합니다. **핵심은 `cancel()`**: 클라이언트가 연결을 끊으면 `setInterval` 을 정리해 타이머 누수를 막습니다.

**검토 / 보충**
- ✅ `cancel()` 로 정리하는 부분이 잘 되어 있습니다. 다만 브라우저 `EventSource` 는 연결이 끊기면 **자동 재연결**하므로, `controller.close()` 후에도 클라이언트가 계속 재접속을 시도합니다. 종료를 알리려면 커스텀 이벤트나 상태를 보내고 클라이언트에서 `es.close()` 하세요.
- 다중 라인 데이터는 각 줄마다 `data:` 를 붙여야 합니다. 이벤트 이름은 `event: name\n`, 재연결 ID는 `id: 123\n` 로 지정 가능.

---

## `16_auth.ts` — Bearer 토큰 인증 가드

포트 **3016**. `Authorization: Bearer <token>` 검사로 보호 라우트.

```ts
// 16. Bearer 토큰 인증 가드
// 실행: bun --hot 16_auth.ts
// 확인:
//   curl -i http://localhost:3016/public
//   curl -i http://localhost:3016/admin
//   curl -i http://localhost:3016/admin -H 'Authorization: Bearer s3cret'

const TOKEN = "s3cret";

function requireBearer(req: Request, token: string): Response | null {
  const auth = req.headers.get("authorization") ?? "";
  if (auth === `Bearer ${token}`) return null; // 통과
  return new Response("Unauthorized", {
    status: 401,
    headers: { "WWW-Authenticate": "Bearer" },
  });
}

const server = Bun.serve({
  port: 3016,
  routes: {
    "/": () =>
      new Response(
        "GET /public (공개) | GET /admin (Bearer s3cret 필요)\n",
      ),
    "/public": () => Response.json({ message: "누구나 볼 수 있음" }),
    "/admin": (req) =>
      requireBearer(req, TOKEN) ??
      Response.json({ secret: 42, message: "인증 성공" }),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`16 auth → ${server.url}`);
```

**해설** — `requireBearer` 는 통과 시 `null`, 실패 시 401 Response를 반환합니다. 그래서 `requireBearer(...) ?? 실제응답` 이라는 간결한 가드 패턴이 됩니다(null이면 실제 응답으로 진행). 401에 `WWW-Authenticate` 헤더도 규격대로 첨부.

**검토 / 보충**
- ⚠️ **타이밍 공격**: `auth === \`Bearer ${token}\`` 문자열 비교는 조기 반환(early-return)이라 실행 시간이 토큰과의 일치 길이에 비례합니다. 고보안 환경에선 상수 시간 비교를 쓰세요.

```ts
function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}
// if (safeEqual(auth, `Bearer ${token}`)) return null;
```

- 실무에선 이 정적 토큰 대신 **서명된 토큰(JWT, 예제 37)** 이나 서버 세션(예제 31)을 사용하세요. 토큰은 코드가 아니라 환경변수(`Bun.env.API_TOKEN`)로 관리.

---

# 중급 (17–32)

## `17_middleware.ts` — 미들웨어 합성

포트 **3017**. 프레임워크 없이 미들웨어를 `reduceRight` 로 합성(로깅·요청ID).

```ts
// 17. 미들웨어 합성 — 로깅 · 타이밍 · 헤더 (프레임워크 없이)
// 실행: bun --hot 17_middleware.ts
// 확인: curl -i http://localhost:3017/api/hello

type Mw = (
  req: Request,
  next: () => Promise<Response>,
) => Promise<Response>;

const withLog: Mw = async (req, next) => {
  const t = performance.now();
  const res = await next();
  const ms = (performance.now() - t).toFixed(1);
  console.log(`${req.method} ${new URL(req.url).pathname} ${res.status} ${ms}ms`);
  return res;
};

const withRequestId: Mw = async (req, next) => {
  const id = crypto.randomUUID().slice(0, 8);
  const res = await next();
  const headers = new Headers(res.headers);
  headers.set("X-Request-Id", id);
  return new Response(res.body, { status: res.status, headers });
};

const compose =
  (mws: Mw[], handler: (req: Request) => Promise<Response>) =>
  (req: Request) =>
    mws.reduceRight<() => Promise<Response>>(
      (next, mw) => () => mw(req, next),
      () => handler(req),
    )();

const handler = async (req: Request) => {
  const url = new URL(req.url);
  if (url.pathname === "/api/hello") {
    await Bun.sleep(5); // 처리 지연 흉내
    return Response.json({ message: "middleware ok" });
  }
  return new Response("Not Found", { status: 404 });
};

const server = Bun.serve({
  port: 3017,
  fetch: compose([withLog, withRequestId], handler),
});

console.log(`17 middleware → ${server.url}`);
```

**해설** — `Mw` 는 `(req, next) => Response` 형태. `compose` 는 `reduceRight` 로 미들웨어 배열을 **양파 구조**(withLog → withRequestId → handler → …되돌아옴)로 감쌉니다. `withRequestId` 는 응답에 헤더를 추가하기 위해 Response를 재생성.

**검토 / 보충**
- ⚠️ `new Response(res.body, ...)` 로 재생성하면 **원본 Response의 `statusText`, 일부 특성이 유실**될 수 있고, 이미 소비된 스트림이면 문제가 됩니다. 헤더만 바꾸려면 원본을 재활용하는 게 더 안전합니다.

```ts
const withRequestId: Mw = async (req, next) => {
  const id = crypto.randomUUID().slice(0, 8);
  const res = await next();
  res.headers.set("X-Request-Id", id); // 헤더만 추가 (스트림 보존)
  return res;
};
```
> 주: Bun의 Response headers 는 대개 가변이라 위처럼 직접 set 이 동작합니다. 불변인 경우엔 원 코드처럼 재생성이 필요합니다.

- `handler` 가 라우팅까지 담당하는데, `Bun.serve` 의 `routes` 와 병행하면 더 깔끔합니다. 다만 이 예제의 목적은 “프레임워크 없이 미들웨어 원리”라 의도적으로 `fetch` 하나로 처리한 것.

---

## `18_static_dir.ts` — 정적 디렉터리 서버

포트 **3018**. 디렉터리 전체를 서빙하며 경로 이탈(`..`) 방어.

```ts
// 18. 정적 디렉터리 서버 — 경로 이탈(..) 방지
// 실행: bun --hot 18_static_dir.ts
// 확인:
//   curl http://localhost:3018/
//   curl http://localhost:3018/about.html
//   curl http://localhost:3018/%2e%2e/secret.txt     # 400 (.. 인코딩)

const PUBLIC = `${import.meta.dir}/_public`;

await Bun.write(
  `${PUBLIC}/index.html`,
  `<!DOCTYPE html><html lang="ko"><body><h1>Public</h1><a href="/about.html">about</a></body></html>\n`,
);
await Bun.write(
  `${PUBLIC}/about.html`,
  `<!DOCTYPE html><html lang="ko"><body><h1>About</h1></body></html>\n`,
);
await Bun.write(`${PUBLIC}/hello.txt`, "static dir ok\n");

const server = Bun.serve({
  port: 3018,
  async fetch(req) {
    const url = new URL(req.url);
    let rel = decodeURIComponent(url.pathname.slice(1)) || "index.html";

    if (rel.includes("..") || rel.includes("\0")) {
      return new Response("Bad path", { status: 400 });
    }

    const file = Bun.file(`${PUBLIC}/${rel}`);
    if (await file.exists()) return new Response(file);
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`18 static_dir → ${server.url}`);
```

**해설** — 핵심 방어: (1) `decodeURIComponent` 로 `%2e%2e`(=`..`) 인코딩 우회를 먼저 풀고, (2) `..` 와 널바이트(`\0`)를 차단, (3) `file.exists()` 로 존재 확인 후 서빙. `Content-Type` 은 확장자 자동 추론.

**검토 / 보충**
- ✅ 인코딩된 `..` 를 decode 후에 검사하는 순서가 정확합니다. 다만 `..` 문자열 자체를 막는 방식은 `foo..bar.txt` 같은 **정상 파일명도 400** 이 됩니다. 더 정밀한 방어는 **정규화 후 루트 접두사 확인**입니다.

```ts
import { resolve, sep } from "node:path";
const rootResolved = resolve(PUBLIC) + sep;
const target = resolve(PUBLIC, rel);
if (!target.startsWith(rootResolved)) {
  return new Response("Bad path", { status: 400 }); // 루트 밖 → 차단
}
```

- 심볼릭 링크로 루트 밖을 가리키는 경우까지 막으려면 `realpath` 검증이 추가로 필요합니다. 정적 서빙이 주목적이라면 아예 Bun의 **내장 정적 서빙(HTML import·`routes` 정적 Response)** 을 쓰는 편이 안전합니다.

---

## `19_stream.ts` — ReadableStream 응답

포트 **3019**. 청크를 순차 enqueue 하여 스트리밍 전송.

```ts
// 19. ReadableStream 응답 — 청크 스트리밍
// 실행: bun --hot 19_stream.ts
// 확인: curl -N http://localhost:3019/stream

const server = Bun.serve({
  port: 3019,
  routes: {
    "/": () => new Response("GET /stream 으로 청크 스트림을 받아보세요"),
    "/stream": () => {
      const stream = new ReadableStream({
        async start(controller) {
          const enc = new TextEncoder();
          for (const chunk of ["하나\n", "둘\n", "셋\n", "끝\n"]) {
            controller.enqueue(enc.encode(chunk));
            await Bun.sleep(200);
          }
          controller.close();
        },
      });
      return new Response(stream, {
        headers: { "Content-Type": "text/plain; charset=utf-8" },
      });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`19 stream → ${server.url}`);
```

**해설** — SSE(예제 15)와 달리 일반 텍스트 스트림. `start` 안에서 `await Bun.sleep` 으로 200ms 간격 전송. `curl -N`(버퍼링 끄기)으로 청크가 도착하는 걸 실시간 확인.

**검토 / 보충**
- SSE와 달리 여기엔 `cancel()` 이 없는데, 이 스트림은 짧고(4청크) `Bun.sleep` 기반이라 큰 문제는 아닙니다. 하지만 클라이언트가 중간에 끊으면 루프는 계속 돌다 `enqueue` 에서 에러가 날 수 있으니, 장시간/무한 스트림이라면 `cancel()` 로 중단 플래그를 세우는 패턴을 권장합니다.

```ts
let aborted = false;
const stream = new ReadableStream({
  async start(controller) { /* 루프 안에서 if (aborted) break; */ },
  cancel() { aborted = true; },
});
```

---

## `20_ws_pubsub.ts` — WebSocket Pub/Sub 채팅

포트 **3020**. 토픽(방) 기반 pub/sub 채팅. `ws.data` 로 연결별 상태 저장.

```ts
// 20. WebSocket Pub/Sub — 방(room) 채팅
// 실행: bun --hot 20_ws_pubsub.ts
// 확인: 브라우저 http://localhost:3020/?user=ada&room=lobby
//       다른 탭에서 ?user=bob&room=lobby

type WsData = { username: string; room: string };

const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <title>WS Pub/Sub</title>
  <style>
    body { font-family: ui-sans-serif, system-ui, sans-serif; padding: 2rem; max-width: 40rem; }
    #log { height: 16rem; overflow: auto; background: #f4f4f5; padding: 1rem; white-space: pre-wrap; }
    .row { display: flex; gap: 0.5rem; margin-top: 0.75rem; }
    input { flex: 1; padding: 0.5rem; }
  </style>
</head>
<body>
  <h1>Room Chat</h1>
  <p id="meta"></p>
  <pre id="log"></pre>
  <div class="row">
    <input id="msg" placeholder="메시지" />
    <button id="send">보내기</button>
  </div>
  <script>
    const q = new URLSearchParams(location.search);
    const room = q.get("room") || "lobby";
    const user = q.get("user") || "anon";
    document.getElementById("meta").textContent = user + " @ " + room;
    const log = document.getElementById("log");
    const ws = new WebSocket("ws://" + location.host + "/ws?room=" + encodeURIComponent(room) + "&user=" + encodeURIComponent(user));
    ws.onmessage = (e) => { log.textContent += e.data + "\\n"; log.scrollTop = log.scrollHeight; };
    document.getElementById("send").onclick = () => {
      const v = document.getElementById("msg").value;
      if (!v) return;
      ws.send(v);
      document.getElementById("msg").value = "";
    };
  </script>
</body>
</html>`;

const server = Bun.serve({
  port: 3020,
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
    "/stats": (_req, srv) =>
      Response.json({
        lobby: srv.subscriberCount("lobby"),
        pendingWebSockets: srv.pendingWebSockets,
      }),
  },
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      const ok = srv.upgrade(req, {
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
      // server.publish → 토픽 구독자 전원
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

console.log(`20 ws_pubsub → ${server.url}?user=ada&room=lobby`);
```

**해설** — 업그레이드 시 `data` 로 연결별 상태(`username`, `room`)를 넘기고, `open` 에서 `ws.subscribe(room)` 으로 토픽 구독. 메시지는 `server.publish(room, msg)` 로 그 토픽 구독자 **전원**(보낸 사람 포함)에게 전달. `/stats` 는 `subscriberCount` 로 방 인원 조회.

**검토 / 보충**
- ⚠️ `websocket.data = {} as WsData` 는 **불필요한 no-op** 입니다. 연결별 데이터는 `upgrade(req, { data })` 로 주입되며, 핸들러 객체의 `data` 필드는 Bun이 사용하지 않습니다(타입 힌트 용도로 착각한 흔적). 지워도 동작 동일합니다. — 예제 36/39/41에도 동일 패턴이 반복됩니다.
- `open`/`message`/`close` 에서 `server.publish` 대신 `ws.publish` 를 쓰면 “본인 제외” 브로드캐스트가 됩니다(차이는 예제 41에서 상세). 채팅 입장/퇴장 알림은 본인 포함이 자연스러워 `server.publish` 가 맞습니다.
- XSS: 클라이언트가 받은 메시지를 `textContent` 로 넣어 안전합니다(`innerHTML` 아님). 👍

---

## `21_crud.ts` — REST CRUD

포트 **3021**. `Map` 기반 인메모리 저장소로 GET/POST/PUT/PATCH/DELETE 전부.

```ts
// 21. REST CRUD — GET/POST/PUT/PATCH/DELETE
// 실행: bun --hot 21_crud.ts
// 확인:
//   curl http://localhost:3021/api/items
//   curl -X POST http://localhost:3021/api/items -H 'Content-Type: application/json' -d '{"name":"pen"}'
//   curl -X PUT http://localhost:3021/api/items/1 -H 'Content-Type: application/json' -d '{"name":"pencil","qty":10}'
//   curl -X PATCH http://localhost:3021/api/items/1 -H 'Content-Type: application/json' -d '{"qty":3}'
//   curl -X DELETE http://localhost:3021/api/items/1

type Item = { id: string; name: string; qty: number };

const items = new Map<string, Item>([
  ["1", { id: "1", name: "notebook", qty: 2 }],
]);

const server = Bun.serve({
  port: 3021,
  routes: {
    "/api/items": {
      GET: () => Response.json([...items.values()]),
      POST: async (req) => {
        const body = (await req.json()) as { name?: string; qty?: number };
        if (!body.name?.trim()) {
          return Response.json({ error: "name required" }, { status: 400 });
        }
        const item: Item = {
          id: crypto.randomUUID().slice(0, 8),
          name: body.name.trim(),
          qty: body.qty ?? 1,
        };
        items.set(item.id, item);
        return Response.json(item, { status: 201 });
      },
    },
    "/api/items/:id": {
      GET: (req) => {
        const item = items.get(req.params.id);
        if (!item) return new Response("Not Found", { status: 404 });
        return Response.json(item);
      },
      PUT: async (req) => {
        if (!items.has(req.params.id)) {
          return new Response("Not Found", { status: 404 });
        }
        const body = (await req.json()) as { name?: string; qty?: number };
        if (!body.name?.trim() || typeof body.qty !== "number") {
          return Response.json(
            { error: "name and qty required" },
            { status: 400 },
          );
        }
        const item: Item = {
          id: req.params.id,
          name: body.name.trim(),
          qty: body.qty,
        };
        items.set(item.id, item);
        return Response.json(item);
      },
      PATCH: async (req) => {
        const cur = items.get(req.params.id);
        if (!cur) return new Response("Not Found", { status: 404 });
        const body = (await req.json()) as Partial<Pick<Item, "name" | "qty">>;
        const next = {
          ...cur,
          ...(body.name !== undefined ? { name: body.name } : {}),
          ...(body.qty !== undefined ? { qty: body.qty } : {}),
        };
        items.set(next.id, next);
        return Response.json(next);
      },
      DELETE: (req) => {
        if (!items.delete(req.params.id)) {
          return new Response("Not Found", { status: 404 });
        }
        return new Response(null, { status: 204 });
      },
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`21 crud → ${server.url}`);
```

**해설** — REST 시맨틱을 정확히 지킵니다: **PUT=전체 교체**(name+qty 둘 다 필수), **PATCH=부분 수정**(있는 필드만 병합), **DELETE=204 No Content**, **POST=201 Created**. ID는 `crypto.randomUUID().slice(0,8)`.

**검토 / 보충**
- ✅ REST 의미론이 교과서적으로 정확합니다. 특히 PATCH의 조건부 스프레드가 좋습니다.
- ⚠️ PATCH가 빈 바디(`{}`)나 잘못된 JSON을 받으면? `{}` 는 그대로 통과(변경 없음)하지만, 깨진 JSON은 throw → `error()` 미설정이라 500. `try/catch` 로 400을 주는 게 낫습니다.
- ⚠️ PATCH의 `body.qty` 타입 검증이 없어 `{"qty":"열개"}` 같은 문자열도 그대로 저장됩니다. PUT처럼 `typeof` 검증을 추가하세요.
- 재시작하면 데이터가 사라집니다(인메모리). 영속화는 예제 22(SQLite).

---

## `22_sqlite.ts` — SQLite REST

포트 **3022**. `bun:sqlite` 내장 모듈 + prepared statement 로 영속 CRUD.

```ts
// 22. SQLite REST — bun:sqlite (내장, npm 패키지 아님)
// 실행: bun --hot 22_sqlite.ts
// 확인:
//   curl http://localhost:3022/api/posts
//   curl -X POST http://localhost:3022/api/posts \
//     -H 'Content-Type: application/json' \
//     -d '{"title":"hello","content":"world"}'
//   curl http://localhost:3022/api/posts/<id>

import { Database } from "bun:sqlite";

const dbPath = `${import.meta.dir}/_data/posts.db`;
await Bun.write(`${import.meta.dir}/_data/.keep`, "");

const db = new Database(dbPath);
db.exec(`
  CREATE TABLE IF NOT EXISTS posts (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
  )
`);

const listPosts = db.query("SELECT * FROM posts ORDER BY created_at DESC");
const getPost = db.query("SELECT * FROM posts WHERE id = ?");
const insertPost = db.query(
  `INSERT INTO posts (id, title, content, created_at) VALUES (?, ?, ?, ?)`,
);
const deletePost = db.query("DELETE FROM posts WHERE id = ?");

const server = Bun.serve({
  port: 3022,
  routes: {
    "/api/posts": {
      GET: () => Response.json(listPosts.all()),
      POST: async (req) => {
        const body = (await req.json()) as { title?: string; content?: string };
        if (!body.title?.trim() || !body.content?.trim()) {
          return Response.json(
            { error: "title and content required" },
            { status: 400 },
          );
        }
        const id = crypto.randomUUID();
        const created_at = new Date().toISOString();
        insertPost.run(id, body.title.trim(), body.content.trim(), created_at);
        return Response.json(
          { id, title: body.title.trim(), content: body.content.trim(), created_at },
          { status: 201 },
        );
      },
    },
    "/api/posts/:id": {
      GET: (req) => {
        const post = getPost.get(req.params.id);
        if (!post) return new Response("Not Found", { status: 404 });
        return Response.json(post);
      },
      DELETE: (req) => {
        const r = deletePost.run(req.params.id);
        if (r.changes === 0) return new Response("Not Found", { status: 404 });
        return new Response(null, { status: 204 });
      },
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
  error(err) {
    console.error(err);
    return new Response("Internal Server Error", { status: 500 });
  },
});

console.log(`22 sqlite → ${server.url} (db: ${dbPath})`);
```

**해설** — `bun:sqlite` 는 npm 설치 없이 바로 씁니다. **prepared statement 를 서버 시작 시 한 번 준비**(`db.query(...)`)해 재사용하므로 빠르고 SQL 인젝션에 안전(파라미터 바인딩). `run()` 은 `{ changes }` 를 반환해 DELETE의 존재 여부 판단에 활용. `error()` 콜백도 갖춰 견고.

**검토 / 보충**
- ✅ prepared statement 재사용 + 파라미터 바인딩 = 모범 사례.
- 💡 **동시성 성능**: 웹 서버라면 WAL 모드를 켜 읽기/쓰기 병행성을 높이세요.

```ts
db.exec("PRAGMA journal_mode = WAL;");
db.exec("PRAGMA busy_timeout = 5000;"); // 잠금 대기
```

- 종료 시 `db.close()` 를 graceful shutdown(예제 25/39)과 결합하면 WAL 체크포인트가 안전하게 마무리됩니다. 이 예제엔 shutdown 훅이 없습니다(예제 39에 통합되어 있음).
- `.keep` 을 매 실행 `Bun.write` 하는 건 `_data/` 디렉터리 존재 보장을 위한 트릭입니다.

---

## `23_metrics.ts` — 메트릭 · requestIP

포트 **3023**. Server 객체의 관측 API(`requestIP`, `pendingRequests` 등) 노출.

```ts
// 23. 메트릭 · requestIP · Server API
// 실행: bun --hot 23_metrics.ts
// 확인:
//   curl http://localhost:3023/whoami
//   curl http://localhost:3023/metrics

const server = Bun.serve({
  port: 3023,
  routes: {
    "/": () =>
      new Response("GET /whoami | GET /metrics | WS /ws (메트릭용)\n"),
    "/whoami": (req, srv) => {
      const ip = srv.requestIP(req);
      return Response.json({
        ip: ip?.address ?? null,
        family: ip?.family ?? null,
        port: ip?.port ?? null,
      });
    },
    "/metrics": (_req, srv) =>
      Response.json({
        pendingRequests: srv.pendingRequests,
        pendingWebSockets: srv.pendingWebSockets,
        hostname: srv.hostname,
        port: srv.port,
        url: String(srv.url),
        topicDemo: srv.subscriberCount("demo"),
      }),
  },
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      if (srv.upgrade(req)) return undefined as never;
      return new Response("Upgrade failed", { status: 400 });
    }
    return new Response("Not Found", { status: 404 });
  },
  websocket: {
    open(ws) {
      ws.subscribe("demo");
    },
    message() {},
    close(ws) {
      ws.unsubscribe("demo");
    },
  },
});

console.log(`23 metrics → ${server.url}`);
```

**해설** — 라우트 핸들러의 두 번째 인자 `srv`(=server)로 런타임 통계를 얻습니다. `requestIP(req)` 는 소켓의 `{address, family, port}`. `/metrics` 는 대기 요청/WS 수, 토픽 구독자 수 등 모니터링용 지표.

**검토 / 보충**
- ⚠️ **프록시 뒤에서는 `requestIP` 가 프록시 IP** 를 줍니다. 실제 클라이언트 IP가 필요하면 `X-Forwarded-For` / `X-Real-IP` 헤더를 신뢰 가능한 프록시 한정으로 파싱하세요.

```ts
function clientIp(req: Request, srv: Bun.Server): string {
  const xff = req.headers.get("x-forwarded-for");
  if (xff) return xff.split(",")[0]!.trim(); // 신뢰된 프록시 뒤에서만
  return srv.requestIP(req)?.address ?? "unknown";
}
```

- `/metrics` 는 인증 없이 내부 상태를 노출하므로 실제 배포에선 내부망 한정 또는 인증(예제 16)으로 보호하세요. Prometheus 포맷으로 뽑으려면 텍스트 exposition 형식으로 바꾸면 됩니다.

---

## `24_ratelimit.ts` — Rate Limit

포트 **3024**. IP 기준 고정 윈도우(fixed window) 인메모리 레이트 리미터.

```ts
// 24. Rate limit — IP 기준 고정 윈도우 (인메모리)
// 실행: bun --hot 24_ratelimit.ts
// 확인: for i in $(seq 1 8); do curl -s -o /dev/null -w "%{http_code}\n" http://localhost:3024/; done
//       (LIMIT=5 → 앞 5개는 200, 이후 429)

const hits = new Map<string, { count: number; resetAt: number }>();
const LIMIT = 5;
const WINDOW_MS = 10_000;

function rateLimit(ip: string): { ok: boolean; remaining: number; resetAt: number } {
  const now = Date.now();
  let rec = hits.get(ip);
  if (!rec || now > rec.resetAt) {
    rec = { count: 0, resetAt: now + WINDOW_MS };
    hits.set(ip, rec);
  }
  if (rec.count >= LIMIT) {
    return { ok: false, remaining: 0, resetAt: rec.resetAt };
  }
  rec.count++;
  return {
    ok: true,
    remaining: LIMIT - rec.count,
    resetAt: rec.resetAt,
  };
}

const server = Bun.serve({
  port: 3024,
  fetch(req, srv) {
    const ip = srv.requestIP(req)?.address ?? "unknown";
    const { ok, remaining, resetAt } = rateLimit(ip);
    const retryAfter = Math.max(1, Math.ceil((resetAt - Date.now()) / 1000));

    if (!ok) {
      return new Response("Too Many Requests", {
        status: 429,
        headers: {
          "Retry-After": String(retryAfter),
          "X-RateLimit-Limit": String(LIMIT),
          "X-RateLimit-Remaining": "0",
        },
      });
    }

    return Response.json(
      { ok: true, ip, remaining },
      {
        headers: {
          "X-RateLimit-Limit": String(LIMIT),
          "X-RateLimit-Remaining": String(remaining),
        },
      },
    );
  },
});

console.log(`24 ratelimit → ${server.url} (limit ${LIMIT}/${WINDOW_MS}ms)`);
```

**해설** — IP별로 `{count, resetAt}` 를 저장하고, 윈도우가 지나면 리셋. 표준 헤더 `Retry-After`, `X-RateLimit-*` 를 붙여 클라이언트가 백오프할 수 있게 합니다.

**검토 / 보충**
- ⚠️ **메모리 누수**: `hits` 맵은 IP가 늘수록 무한 증가합니다(만료 항목을 지우지 않음). 주기적 청소가 필요합니다.

```ts
setInterval(() => {
  const now = Date.now();
  for (const [ip, rec] of hits) if (now > rec.resetAt) hits.delete(ip);
}, WINDOW_MS).unref?.(); // 이벤트 루프 유지 안 함
```

- ⚠️ **고정 윈도우의 경계 버스트**: 윈도우 경계에서 순간적으로 `2×LIMIT` 이 통과할 수 있습니다. 정밀하려면 슬라이딩 윈도우/토큰 버킷을 쓰세요.
- ⚠️ 프록시 뒤 IP 문제는 예제 23 보충과 동일. `X-Forwarded-For` 신뢰 처리 필요.
- 다중 인스턴스 배포에선 인메모리로 부족 → Redis 등 공유 스토어로.

---

## `25_graceful.ts` — Graceful Shutdown

포트 **3025**. SIGINT/SIGTERM 수신 시 진행 중 요청을 마치고 종료.

```ts
// 25. Graceful shutdown — SIGINT/SIGTERM → server.stop()
// 실행: bun 25_graceful.ts
// 확인:
//   터미널1: bun 25_graceful.ts
//   터미널2: curl http://localhost:3025/slow   # 0.8초 걸리는 요청
//   터미널1: Ctrl+C  — in-flight 끝나면 종료

const server = Bun.serve({
  port: 3025,
  routes: {
    "/": () => new Response("GET /slow 후 Ctrl+C 로 graceful stop 테스트"),
    "/slow": async () => {
      await Bun.sleep(800);
      return Response.json({ done: true, at: Date.now() });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

let closing = false;

async function shutdown(signal: string) {
  if (closing) return;
  closing = true;
  console.log(`\n${signal} 수신 — 새 연결 차단, in-flight 대기…`);
  await server.stop(); // 인자 없음: 진행 중 요청 완료까지 대기
  // await server.stop(true); // 강제 종료
  console.log("종료 완료");
  process.exit(0);
}

process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));

console.log(`25 graceful → ${server.url} (Ctrl+C)`);
```

**해설** — `closing` 플래그로 중복 시그널 방지. `await server.stop()`(인자 없음)은 새 연결을 막고 **진행 중 요청이 끝날 때까지** 기다립니다. `server.stop(true)` 는 즉시 강제 종료. `bun`(--hot 없이)으로 돌려야 시그널 흐름이 명확합니다.

**검토 / 보충**
- ✅ 프로덕션 필수 패턴을 정확히 담았습니다. `void shutdown(...)` 로 async 함수의 Promise를 명시적으로 버려 unhandled rejection 경고를 피합니다.
- 💡 무한정 대기를 막는 **강제 종료 타임아웃**을 더하면 더 견고합니다.

```ts
async function shutdown(signal: string) {
  if (closing) return;
  closing = true;
  const force = setTimeout(() => { console.error("타임아웃 — 강제 종료"); process.exit(1); }, 10_000);
  await server.stop();
  clearTimeout(force);
  process.exit(0);
}
```

- DB/리소스가 있으면 `server.stop()` 후 `db.close()` 등 정리를 추가하세요(예제 39가 그 예).

---

## `26_reload.ts` — server.reload

포트 **3026**. 포트를 유지한 채 라우트 핸들러를 런타임에 교체.

```ts
// 26. server.reload — 포트 유지한 채 핸들러 교체
// 실행: bun 26_reload.ts
// 확인:
//   curl http://localhost:3026/api/version   # v1
//   # 3초 후 자동 reload
//   curl http://localhost:3026/api/version   # v2
//
// bun --hot 과 병행 시: globalThis 상태는 프로세스에 남음

const g = globalThis as typeof globalThis & { __reloadCount?: number };
g.__reloadCount ??= 0;

const server = Bun.serve({
  port: 3026,
  routes: {
    "/api/version": () =>
      Response.json({
        version: "1.0.0",
        reloadCount: g.__reloadCount,
      }),
    "/": () =>
      new Response("GET /api/version — 3초 뒤 v2 로 reload 됩니다\n"),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`26 reload → ${server.url} (v1)`);

setTimeout(() => {
  g.__reloadCount = (g.__reloadCount ?? 0) + 1;
  server.reload({
    routes: {
      "/api/version": () =>
        Response.json({
          version: "2.0.0",
          reloadCount: g.__reloadCount,
          note: "server.reload 로 교체됨",
        }),
      "/": () => new Response("이제 v2 입니다\n"),
    },
    fetch() {
      return new Response("Not Found", { status: 404 });
    },
  });
  console.log("→ reloaded to v2 (port unchanged)");
}, 3000);
```

**해설** — `server.reload(opts)` 는 **소켓/포트를 유지**하며 핸들러만 바꿉니다(무중단 배포의 기초). `globalThis` 에 상태를 두면 `--hot` 리로드 사이에도 값이 보존됨을 보여줍니다. `??=` 는 최초 1회만 0으로 초기화.

**검토 / 보충**
- `server.reload` 로는 `routes`/`fetch`/`error` 등 핸들러류는 교체되지만 `port` 는 바뀌지 않습니다(의도된 동작). 포트를 바꾸려면 서버를 새로 띄워야 합니다.
- `--hot` 개발 모드는 파일 저장 시 자동으로 유사한 리로드를 수행합니다. 이 예제는 그걸 **코드로 명시적으로** 보여주는 것.

---

## `27_unix.ts` — Unix Domain Socket

포트 대신 **유닉스 소켓**으로 바인딩. 리버스 프록시(nginx 등) 뒤 로컬 통신.

```ts
// 27. Unix domain socket — 리버스 프록시 뒤 로컬 통신
// 실행: bun --hot 27_unix.ts
// 확인:
//   curl --unix-socket /tmp/bunserve-27.sock http://localhost/
//   (macOS/Linux curl 지원)

const sock = "/tmp/bunserve-27.sock";

try {
  const { unlinkSync } = await import("node:fs");
  unlinkSync(sock);
} catch {
  // 없으면 무시
}

const server = Bun.serve({
  unix: sock,
  routes: {
    "/": () =>
      Response.json({
        ok: true,
        via: "unix",
        tip: `curl --unix-socket ${sock} http://localhost/`,
      }),
    "/health": new Response("OK"),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`27 unix → unix:${sock}`);
// server.url 은 unix 모드에서 의미가 다를 수 있음
void server;
```

**해설** — `port` 대신 `unix: 경로` 를 주면 TCP 포트 없이 유닉스 소켓으로 서빙합니다(프록시-앱 간 통신에서 TCP 오버헤드/포트 관리 제거). 시작 전 **낡은 소켓 파일을 `unlinkSync` 로 제거**해야 “address in use” 를 피합니다.

**검토 / 보충**
- ✅ 낡은 소켓 정리 패턴이 정확합니다. 다만 **종료 시에도 소켓 파일을 지우는 것**이 깔끔합니다(다음 실행을 위해). graceful shutdown(예제 25)과 결합해 `unlinkSync(sock)` 를 추가하세요.
- 소켓 파일 권한(퍼미션)으로 접근 제어가 됩니다. 프록시만 접근하도록 디렉터리/권한을 관리하세요.
- `void server;` 는 “변수 미사용” 린트를 억제하는 관용구(서버는 백그라운드로 계속 동작).

---

## `28_range.ts` — Range 요청

포트 **3028**. `Range` 헤더를 파싱해 부분 응답(206)으로 반환(미디어 seek/이어받기).

```ts
// 28. Range 요청 — Bun.file.slice (부분 다운로드 / 미디어 seek)
// 실행: bun --hot 28_range.ts
// 확인:
//   curl -i http://localhost:3028/file
//   curl -i -H 'Range: bytes=0-11' http://localhost:3028/file
//   curl -i -H 'Range: bytes=12-' http://localhost:3028/file

const FILE = `${import.meta.dir}/_data/range-demo.txt`;
await Bun.write(
  FILE,
  "Hello Range!\nSecond line here.\nThird line end.\n",
);

const server = Bun.serve({
  port: 3028,
  routes: {
    "/": () =>
      new Response(
        "GET /file  (전체)\nGET /file  + Header Range: bytes=0-11\n",
      ),
    "/file": async (req) => {
      const file = Bun.file(FILE);
      const size = file.size;
      const range = req.headers.get("Range");

      if (!range) {
        return new Response(file, {
          headers: {
            "Content-Type": "text/plain; charset=utf-8",
            "Accept-Ranges": "bytes",
            "Content-Length": String(size),
          },
        });
      }

      // bytes=start-end | bytes=start- | bytes=-suffix
      const raw = range.split("=").at(-1) ?? "0-";
      const [a, b] = raw.split("-");
      let start: number;
      let end: number;

      if (a === "") {
        // bytes=-N → 끝에서 N바이트
        const suffix = Number(b);
        start = Math.max(0, size - suffix);
        end = size;
      } else {
        start = Number(a);
        end = b === "" || b == null ? size : Number(b) + 1; // end inclusive → slice exclusive
      }

      if (
        !Number.isFinite(start) ||
        !Number.isFinite(end) ||
        start < 0 ||
        start >= size ||
        end <= start
      ) {
        return new Response("Range Not Satisfiable", {
          status: 416,
          headers: { "Content-Range": `bytes */${size}` },
        });
      }

      end = Math.min(end, size);
      const slice = file.slice(start, end);

      return new Response(slice, {
        status: 206,
        headers: {
          "Content-Type": "text/plain; charset=utf-8",
          "Accept-Ranges": "bytes",
          "Content-Range": `bytes ${start}-${end - 1}/${size}`,
          "Content-Length": String(end - start),
        },
      });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`28 range → ${server.url}`);
```

**해설** — 세 가지 Range 형식(`start-end`, `start-`, `-suffix`)을 모두 처리. **end inclusive → slice exclusive** 변환(`Number(b)+1`)과 `Content-Range: bytes start-end/total` 헤더, 잘못된 범위는 **416 + `Content-Range: bytes */size`** 규격을 정확히 지킵니다. `Bun.file.slice` 로 부분만 스트리밍.

**검토 / 보충**
- ✅ 표준(RFC 7233)을 충실히 구현했습니다. 검증 로직도 견고합니다.
- 다중 범위(`bytes=0-1,4-5`)는 미지원(단일 범위만). 대부분의 미디어 재생/다운로드엔 단일 범위로 충분합니다.
- 파일이 자주 바뀌면 요청 시점에 `size` 를 다시 읽는 현재 방식이 맞습니다(캐시하면 stale 위험).

---

## `29_proxy.ts` — 리버스 프록시

포트 **3029**. 들어온 요청을 업스트림(`httpbin.org`)으로 전달하고 응답을 스트리밍.

```ts
// 29. 리버스 프록시 — fetch 결과를 그대로 전달
// 실행: bun --hot 29_proxy.ts
// 확인:
//   curl -i http://localhost:3029/proxy/get
//   curl -i http://localhost:3029/proxy/status/418
//
// 업스트림: https://httpbin.org  (네트워크 필요)
// 오프라인 테스트용 /local 도 제공

const UPSTREAM = "https://httpbin.org";

const server = Bun.serve({
  port: 3029,
  routes: {
    "/": () =>
      new Response(
        "GET /proxy/* → httpbin.org 로 프록시\nGET /local → 로컬 에코 (오프라인용)\n",
      ),
    "/local": () =>
      Response.json({ via: "local", message: "프록시 없이 로컬 응답" }),
    "/proxy/*": async (req) => {
      const url = new URL(req.url);
      // /proxy/get → https://httpbin.org/get
      const path = url.pathname.replace(/^\/proxy/, "") || "/";
      const target = `${UPSTREAM}${path}${url.search}`;

      try {
        const upstream = await fetch(target, {
          method: req.method,
          headers: {
            accept: req.headers.get("accept") ?? "*/*",
          },
          body:
            req.method === "GET" || req.method === "HEAD"
              ? undefined
              : await req.arrayBuffer(),
          redirect: "manual",
        });

        // 응답 헤더 일부만 전달
        const headers = new Headers();
        const pass = ["content-type", "content-length", "cache-control"];
        for (const key of pass) {
          const v = upstream.headers.get(key);
          if (v) headers.set(key, v);
        }
        headers.set("X-Proxied-From", target);

        return new Response(upstream.body, {
          status: upstream.status,
          headers,
        });
      } catch (err) {
        return Response.json(
          {
            error: "upstream unreachable",
            detail: err instanceof Error ? err.message : String(err),
            tip: "오프라인이면 GET /local 사용",
          },
          { status: 502 },
        );
      }
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`29 proxy → ${server.url}`);
```

**해설** — `/proxy/*` 와일드카드로 하위 경로 전체를 받아 업스트림 URL로 재구성. **응답 바디를 `upstream.body`(스트림)로 그대로 전달**해 메모리에 다 담지 않습니다. 화이트리스트 헤더만 전달, 실패 시 502. `redirect:"manual"` 로 프록시가 리다이렉트를 대신 따라가지 않게 함.

**검토 / 보충**
- ✅ 업스트림이 **고정 상수**라 SSRF(내부망 스캔 등) 위험이 없습니다. 만약 클라이언트가 대상 URL을 지정하게 만든다면 반드시 목적지 화이트리스트/사설IP 차단이 필요합니다.
- 요청 본문을 `arrayBuffer()` 로 버퍼링합니다. 대용량 업로드 프록시라면 요청도 스트림으로 넘기는 편이 좋지만(그러면 `duplex: "half"` 필요), 예제 목적엔 충분.
- `content-length` 를 그대로 전달하는데, 만약 본문을 변형한다면 `content-length` 는 빼야 합니다(불일치 방지). 여기선 무변형 전달이라 OK.

---

## `30_timeout.ts` — idleTimeout · server.timeout

포트 **3030**. 서버 전역 유휴 타임아웃과 요청별 타임아웃 제어를 대비.

```ts
// 30. idleTimeout · server.timeout — 요청별 타임아웃 제어
// 실행: bun --hot 30_timeout.ts
// 확인:
//   curl -m 5 http://localhost:3030/fast          # 즉시
//   curl -m 5 http://localhost:3030/slow-ok       # timeout(req,0) → 완료
//   curl -m 5 http://localhost:3030/slow-default  # 기본 idle 에 걸릴 수 있음
//
// 서버 기본 idleTimeout 을 2초로 낮춰 차이를 보기 쉽게 함

const server = Bun.serve({
  port: 3030,
  idleTimeout: 2, // 초 (HTTP 유휴). 기본은 10, 최대 255, 0=비활성
  routes: {
    "/": () =>
      new Response(
        [
          "GET /fast",
          "GET /slow-ok       — server.timeout(req, 0) 후 3초 대기",
          "GET /slow-default — timeout 미설정, idleTimeout=2 에 끊길 수 있음",
          "",
        ].join("\n"),
      ),
    "/fast": () => Response.json({ ok: true }),
    "/slow-ok": async (req, srv) => {
      // 이 요청만 유휴 타임아웃 비활성
      srv.timeout(req, 0);
      await Bun.sleep(3000);
      return Response.json({ ok: true, waitedMs: 3000, timeout: "disabled" });
    },
    "/slow-default": async () => {
      // idleTimeout(2s) 동안 응답이 없으면 연결이 끊길 수 있음
      await Bun.sleep(3000);
      return Response.json({ ok: true, waitedMs: 3000 });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`30 timeout → ${server.url} (idleTimeout=2s)`);
```

**해설** — `idleTimeout: 2` 로 전역 유휴 타임아웃을 낮춘 상태에서, `/slow-ok` 는 `srv.timeout(req, 0)` 로 **그 요청만** 타임아웃을 꺼 3초 작업을 완주합니다. `/slow-default` 는 설정 안 해 2초 유휴 제한에 걸려 끊길 수 있습니다. SSE(예제 15)에서 이 API를 실제로 활용했습니다.

**검토 / 보충**
- `idleTimeout` 은 **유휴(idle)** 시간 기준입니다. 데이터가 오가면 리셋되므로, 스트리밍처럼 주기적으로 데이터를 보내면 굳이 0으로 안 꺼도 유지됩니다. 반대로 “긴 침묵” 구간이 있으면 `srv.timeout(req, 0)` 필요.
- 값 범위: 0(비활성)~255초. 무한정 요청을 허용하면 슬로로리스(slowloris)류 공격에 취약하므로 프로덕션에선 신중히.

---

## `31_session.ts` — 쿠키 세션

포트 **3031**. 서버측 세션 스토어(`Map`) + 세션 쿠키로 보호 라우트.

```ts
// 31. 쿠키 세션 — 로그인 쿠키로 보호 라우트
// 실행: bun --hot 31_session.ts
// 확인:
//   curl -i -c /tmp/s.txt -b /tmp/s.txt -X POST http://localhost:3031/login \
//     -H 'Content-Type: application/json' -d '{"user":"ada"}'
//   curl -i -c /tmp/s.txt -b /tmp/s.txt http://localhost:3031/me
//   curl -i -c /tmp/s.txt -b /tmp/s.txt -X POST http://localhost:3031/logout
//   curl -i -c /tmp/s.txt -b /tmp/s.txt http://localhost:3031/me

type Session = { user: string; createdAt: number };

const sessions = new Map<string, Session>();

const server = Bun.serve({
  port: 3031,
  routes: {
    "/": () =>
      new Response(
        "POST /login {user} → POST /logout → GET /me (세션 필요)\n",
      ),
    "/login": {
      POST: async (req) => {
        const body = (await req.json()) as { user?: string };
        if (!body.user?.trim()) {
          return Response.json({ error: "user required" }, { status: 400 });
        }
        const sid = crypto.randomUUID();
        sessions.set(sid, { user: body.user.trim(), createdAt: Date.now() });
        req.cookies.set("sid", sid, {
          httpOnly: true,
          sameSite: "lax",
          path: "/",
          maxAge: 60 * 60,
        });
        return Response.json({ ok: true, user: body.user.trim() });
      },
    },
    "/logout": {
      POST: (req) => {
        const sid = req.cookies.get("sid");
        if (sid) sessions.delete(sid);
        req.cookies.delete("sid", { path: "/" });
        return Response.json({ ok: true });
      },
    },
    "/me": (req) => {
      const sid = req.cookies.get("sid");
      const session = sid ? sessions.get(sid) : undefined;
      if (!session) {
        return Response.json({ error: "Unauthorized" }, { status: 401 });
      }
      return Response.json({ user: session.user, sid });
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`31 session → ${server.url}`);
```

**해설** — 예제 09(쿠키)+16(인증)의 결합판. 로그인 시 랜덤 `sid`(UUID)를 서버 `Map` 에 저장하고 `httpOnly` 쿠키로 클라이언트에 심습니다. `/me` 는 쿠키의 sid로 세션을 조회해 인증. 로그아웃은 서버 세션 삭제 + 쿠키 삭제. **세션 값이 서버에만 있어** 쿠키 변조로 남의 세션이 될 수 없습니다.

**검토 / 보충**
- ⚠️ **세션 만료 청소 없음**: `maxAge`(쿠키)와 `createdAt`(세션)이 있지만 서버 `Map` 에서 만료 세션을 지우지 않아 메모리가 누적됩니다. 주기적 청소를 추가하세요.

```ts
const TTL = 60 * 60 * 1000;
setInterval(() => {
  const now = Date.now();
  for (const [sid, s] of sessions) if (now - s.createdAt > TTL) sessions.delete(sid);
}, 60_000).unref?.();
```
그리고 `/me` 에서도 `createdAt` 을 확인해 만료면 401 처리하는 게 정확합니다.

- 프로덕션 HTTPS에선 쿠키에 `secure: true` 를 추가하세요(HTTPS에서만 전송). CSRF가 걱정되면 `sameSite: "strict"` 또는 CSRF 토큰 병행.
- 다중 인스턴스면 세션 스토어를 Redis 등 외부로 빼야 합니다(예제 24와 동일 한계).

---

## `32_export_default.ts` — export default · 라우트 우선순위

포트 **3032**. `Bun.serve()` 대신 **default export 로 서버 설정**을 내보내고, 라우트 우선순위를 실증.

```ts
// 32. export default · 와일드카드 라우트 · 우선순위
// 실행: bun 32_export_default.ts
//   (또는: bun --hot 32_export_default.ts)
// 확인:
//   curl http://localhost:3032/
//   curl http://localhost:3032/api/users/me     # exact 우선
//   curl http://localhost:3032/api/users/42     # :id
//   curl http://localhost:3032/api/other        # /api/*
//   curl http://localhost:3032/anything         # /*

import type { Serve } from "bun";

export default {
  port: 3032,
  routes: {
    "/": () =>
      Response.json({
        tip: [
          "exact > param > wildcard > global",
          "/api/users/me",
          "/api/users/:id",
          "/api/*",
          "/*",
        ],
      }),
    // 1) Exact
    "/api/users/me": () => Response.json({ user: "current" }),
    // 2) Parameter
    "/api/users/:id": (req) => Response.json({ userId: req.params.id }),
    // 3) Wildcard
    "/api/*": (req) => {
      const url = new URL(req.url);
      return Response.json({ catch: "api", path: url.pathname });
    },
    // 4) Global catch-all
    "/*": (req) => {
      const url = new URL(req.url);
      return Response.json({ catch: "global", path: url.pathname }, { status: 404 });
    },
  },
  fetch() {
    // routes 의 /* 가 대부분 잡음. 여기까지 오면 진짜 미매칭
    return new Response("fallback", { status: 404 });
  },
} satisfies Serve.Options<undefined>;

console.log("32 export_default → http://localhost:3032/");
```

**해설** — `export default { ... }` 형태면 `bun 파일.ts` 실행 시 Bun이 자동으로 서버로 띄웁니다(별도 `Bun.serve()` 호출 불필요). 매칭 우선순위는 **exact(`/api/users/me`) > param(`/api/users/:id`) > wildcard(`/api/*`) > global(`/*`)**. `satisfies Serve.Options<undefined>` 로 타입 안전성 확보.

**검토 / 보충**
- ✅ 우선순위 규칙을 실행 가능한 형태로 잘 보여줍니다. `/*` 가 모든 걸 잡으므로 하단 `fetch` 폴백은 사실상 도달하지 않습니다(설명 주석대로).
- default export 방식은 `server.url` 참조/시그널 훅을 코드에서 직접 잡기 어렵습니다(서버 객체를 손에 못 쥠). graceful shutdown 등이 필요하면 명시적 `const server = Bun.serve(...)` 방식이 낫습니다.
- `console.log` 의 URL을 하드코딩(`http://localhost:3032/`)한 이유는 default export 라 `server` 객체가 없기 때문입니다.

---

# 고급 (33–41)

## `33_tls.ts` — TLS / HTTPS

포트 **3033**. 자체 서명 인증서로 HTTPS 서버.

```ts
// 33. TLS / HTTPS — 자체 서명 인증서
// 실행: bun --hot 33_tls.ts
// 확인:
//   curl -k https://localhost:3033/
//   브라우저: https://localhost:3033/  (경고 무시 — 개발용 인증서)
//
// 인증서: ./_certs/{key,cert}.pem
// 재발급: openssl req -x509 -newkey rsa:2048 -nodes \
//   -keyout _certs/key.pem -out _certs/cert.pem -days 365 \
//   -subj "/CN=localhost" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

const CERT_DIR = `${import.meta.dir}/_certs`;

const server = Bun.serve({
  port: 3033,
  tls: {
    // 경로 문자열이 아니라 BunFile / 내용
    key: Bun.file(`${CERT_DIR}/key.pem`),
    cert: Bun.file(`${CERT_DIR}/cert.pem`),
  },
  routes: {
    "/": () =>
      Response.json({
        ok: true,
        protocol: "https",
        tip: "curl -k https://localhost:3033/",
      }),
    "/whoami": (req, srv) =>
      Response.json({
        url: req.url,
        ip: srv.requestIP(req)?.address ?? null,
      }),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`33 tls → ${server.url}  (curl -k ${server.url})`);
```

**해설** — `tls: { key, cert }` 에 **경로 문자열이 아니라 `Bun.file(경로)`**(BunFile)를 넘기는 게 포인트입니다. 자체 서명이라 `curl -k`(검증 무시)나 브라우저 경고 무시가 필요. 인증서 재발급 명령이 주석에 포함되어 재현 가능.

**검토 / 보충**
- ✅ 개발용 HTTPS의 정석. 프로덕션은 Let's Encrypt 등 신뢰된 CA 인증서를 쓰고, 갱신 시 `server.reload({ tls })` 로 무중단 교체할 수 있습니다.
- 여러 도메인은 `tls` 를 배열로 주어 SNI(도메인별 인증서)를 구성할 수 있습니다.
- 인증서 파일은 저장소에 커밋하지 않는 게 원칙입니다(이 예제는 학습용이라 `_certs/` 에 포함).

---

## `34_http3.ts` — HTTP/3 (실험적)

포트 **3034**. QUIC 기반 HTTP/3. TLS 필수, 같은 포트에서 TCP+UDP.

```ts
// 34. HTTP/3 (실험적) — TLS 필수, 같은 포트 TCP+UDP
// 실행: bun --hot 34_http3.ts
// 확인:
//   curl -k --http3-only https://localhost:3034/     # curl 이 HTTP/3 지원할 때
//   curl -k https://localhost:3034/                  # HTTP/1.1 로도 응답 (http1 기본 on)
//
// 주의: Unix 소켓과 함께 쓸 수 없음 (QUIC=UDP)

const CERT_DIR = `${import.meta.dir}/_certs`;

const server = Bun.serve({
  port: 3034,
  tls: {
    key: Bun.file(`${CERT_DIR}/key.pem`),
    cert: Bun.file(`${CERT_DIR}/cert.pem`),
  },
  http3: true,
  // http1: false, // 켜면 HTTP/3 만
  routes: {
    "/": (req) =>
      Response.json({
        ok: true,
        http3: true,
        // 클라이언트가 실제로 h3 로 왔는지는 프록시/환경에 따라 다름
        url: req.url,
      }),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`34 http3 → ${server.url}`);
console.log("  try: curl -k --http3-only " + server.url);
```

**해설** — `http3: true` 로 QUIC(UDP 기반) HTTP/3를 켭니다. **TLS 필수**이며, 기본적으로 HTTP/1.1도 함께 서빙(`http1: false` 로 끄면 h3 전용). QUIC은 UDP라 유닉스 소켓과 병행 불가.

**검토 / 보충**
- ⚠️ **실험적 기능**입니다. Bun 버전/플랫폼에 따라 동작이 다를 수 있고, 클라이언트(curl)도 HTTP/3 빌드여야 `--http3-only` 가 됩니다.
- 브라우저는 보통 HTTP/2로 먼저 붙은 뒤 `Alt-Svc` 헤더를 보고 HTTP/3로 승격합니다. 로컬 자체 서명 환경에선 브라우저가 h3를 안 쓸 수 있습니다.
- 방화벽에서 해당 UDP 포트를 열어야 QUIC이 통과합니다.

---

## `35_html_import.ts` — HTML import + HMR

포트 **3035**. HTML 파일을 import 하면 Bun 번들러를 타고, `--hot` 에서 브라우저 HMR.

```ts
// 35. HTML import — 풀스택 번들 + bun --hot 브라우저 HMR
// 실행: bun --hot 35_html_import.ts
// 확인: 브라우저 http://localhost:3035/
//       _html_import/app.ts 를 수정·저장하면 브라우저가 자동 갱신(HMR)
//
// 인라인 HTML 문자열과 달리, HTML 파일 import 는 Bun 번들러를 탄다.

import homepage from "./_html_import/index.html";

const server = Bun.serve({
  port: 3035,
  routes: {
    "/": homepage,
    "/api/time": () => Response.json({ now: Date.now() }),
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
  development: true,
});

console.log(`35 html_import → ${server.url}`);
console.log("  edit _html_import/app.ts then save → browser HMR");
```

**해설** — `import homepage from "./index.html"` 는 문자열이 아니라 **번들 엔트리**입니다. Bun이 `<script src="./app.ts">` 같은 참조를 자동으로 번들/트랜스파일하고, `development:true` + `--hot` 이면 소스 수정 시 **브라우저가 자동 새로고침(HMR)** 됩니다. 인라인 HTML 문자열(예제 02/10 등)과의 결정적 차이.

**검토 / 보충**
- ✅ 이것이 Bun의 “풀스택” 강점입니다. 프론트 빌드 도구(Vite 등) 없이 TS/TSX/CSS를 번들.
- 프로덕션 배포 시엔 `development: false` + 사전 빌드(`bun build ./index.html`)로 정적 산출물을 만들어 서빙하는 편이 좋습니다.
- 관련 파일: [`_html_import/index.html`](#_html_importindexhtml), [`_html_import/app.ts`](#_html_importappts) — 아래 부속 파일 절 참고.

---

## `36_ws_advanced.ts` — WebSocket 고급

포트 **3036**. perMessageDeflate(압축)·백프레셔·cork·연결별 data 등 WS 고급 옵션.

```ts
// 36. WebSocket 고급 — perMessageDeflate · 백프레셔 · cork · data
// 실행: bun --hot 36_ws_advanced.ts
// 확인: 브라우저 http://localhost:3036/

type WsData = { id: string; joinedAt: number };

const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <title>WS Advanced</title>
  <style>
    body { font-family: ui-sans-serif, system-ui, sans-serif; padding: 2rem; }
    #log { white-space: pre-wrap; background: #f4f4f5; padding: 1rem; height: 14rem; overflow: auto; }
  </style>
</head>
<body>
  <h1>WS Advanced</h1>
  <button id="burst">100개 연속 전송 (백프레셔 관찰)</button>
  <pre id="log"></pre>
  <script>
    const log = document.getElementById("log");
    const ws = new WebSocket("ws://" + location.host + "/ws");
    ws.binaryType = "arraybuffer";
    ws.onmessage = (e) => {
      log.textContent += (typeof e.data === "string" ? e.data : "[bin " + e.data.byteLength + "]") + "\\n";
    };
    document.getElementById("burst").onclick = () => {
      for (let i = 0; i < 100; i++) ws.send("burst-" + i);
    };
  </script>
</body>
</html>`;

const server = Bun.serve({
  port: 3036,
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
  },
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      const ok = srv.upgrade(req, {
        data: {
          id: crypto.randomUUID().slice(0, 8),
          joinedAt: Date.now(),
        } satisfies WsData,
      });
      return ok ? undefined : new Response("Upgrade failed", { status: 400 });
    }
    return new Response("Not Found", { status: 404 });
  },
  websocket: {
    data: {} as WsData,
    perMessageDeflate: true,
    idleTimeout: 60,
    maxPayloadLength: 1024 * 1024,
    backpressureLimit: 1024 * 1024,
    closeOnBackpressureLimit: false,
    open(ws) {
      ws.send(`welcome ${ws.data.id}`);
    },
    message(ws, message) {
      // cork: 여러 send 를 한 번에 플러시
      ws.cork((socket) => {
        const status = socket.send(`echo: ${message}`, true); // compress
        // status: -1 큐잉(백프레셔), 0 드롭, 1+ 전송 바이트
        if (status === -1) {
          socket.send("(server backpressure)");
        }
      });
    },
    drain(ws) {
      // 백프레셔 해소 시
      ws.send("(drain: backpressure cleared)");
    },
  },
});

console.log(`36 ws_advanced → ${server.url}`);
```

**해설** — WS 튜닝 옵션 총집합:
- `perMessageDeflate: true` — 메시지 압축.
- `maxPayloadLength` — 수신 메시지 최대 크기(초과 시 연결 종료).
- `backpressureLimit` / `closeOnBackpressureLimit` — 송신 버퍼가 한계를 넘으면 큐잉/종료 정책.
- `ws.cork(fn)` — 콜백 안의 여러 `send` 를 **한 번에 플러시**(TCP 패킷 효율).
- `socket.send(msg, true)` 반환값 — `-1`(백프레셔로 큐잉), `0`(드롭), `1+`(전송 바이트).
- `drain(ws)` — 백프레셔 해소 시 호출(그때 밀린 데이터 재전송).

**검토 / 보충**
- ⚠️ `websocket.data = {} as WsData` 는 예제 20과 동일하게 **no-op**(실 데이터는 upgrade의 `data`). 제거 가능.
- `drain` 콜백은 백프레셔 기반 흐름제어의 핵심입니다. 대량 브로드캐스트 서버라면 `send` 반환값이 `-1` 일 때 생산을 멈추고 `drain` 에서 재개하는 패턴을 반드시 구현해야 메모리 폭증을 막습니다.
- `perMessageDeflate` 는 CPU를 더 쓰므로, 작은 메시지가 많으면 오히려 손해일 수 있어 워크로드에 따라 선택.

---

## `37_jwt.ts` — JWT (HMAC-SHA256)

포트 **3037**. npm 라이브러리 없이 **Web Crypto만으로** JWT 발급/검증.

```ts
// 37. JWT (HMAC-SHA256) — Web Crypto만 사용, npm 0
// 실행: bun --hot 37_jwt.ts
// 확인:
//   TOKEN=$(curl -s -X POST http://localhost:3037/login \
//     -H 'Content-Type: application/json' -d '{"sub":"ada"}' | bun -e 'console.log((await Bun.stdin.json()).token)')
//   curl -i http://localhost:3037/secret -H "Authorization: Bearer $TOKEN"

const SECRET = "dev-only-change-me";

function b64url(data: ArrayBuffer | string): string {
  const bytes =
    typeof data === "string" ? new TextEncoder().encode(data) : new Uint8Array(data);
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function b64urlJson(obj: unknown): string {
  return b64url(JSON.stringify(obj));
}

async function hmacKey() {
  return crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(SECRET),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign", "verify"],
  );
}

async function signJwt(payload: Record<string, unknown>, expiresInSec = 3600) {
  const header = { alg: "HS256", typ: "JWT" };
  const body = {
    ...payload,
    iat: Math.floor(Date.now() / 1000),
    exp: Math.floor(Date.now() / 1000) + expiresInSec,
  };
  const h = b64urlJson(header);
  const p = b64urlJson(body);
  const key = await hmacKey();
  const sig = await crypto.subtle.sign(
    "HMAC",
    key,
    new TextEncoder().encode(`${h}.${p}`),
  );
  return `${h}.${p}.${b64url(sig)}`;
}

function fromB64url(s: string): Uint8Array {
  const pad = "=".repeat((4 - (s.length % 4)) % 4);
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/") + pad;
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
}

async function verifyJwt(token: string): Promise<Record<string, unknown> | null> {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const [h, p, s] = parts;
  const key = await hmacKey();
  const ok = await crypto.subtle.verify(
    "HMAC",
    key,
    fromB64url(s!),
    new TextEncoder().encode(`${h}.${p}`),
  );
  if (!ok) return null;
  const payload = JSON.parse(
    new TextDecoder().decode(fromB64url(p!)),
  ) as Record<string, unknown>;
  if (typeof payload.exp === "number" && payload.exp < Date.now() / 1000) {
    return null;
  }
  return payload;
}

const server = Bun.serve({
  port: 3037,
  routes: {
    "/": () =>
      new Response("POST /login {sub} → GET /secret (Bearer JWT)\n"),
    "/login": {
      POST: async (req) => {
        const body = (await req.json()) as { sub?: string };
        if (!body.sub?.trim()) {
          return Response.json({ error: "sub required" }, { status: 400 });
        }
        const token = await signJwt({ sub: body.sub.trim() }, 60 * 60);
        return Response.json({ token, token_type: "Bearer" });
      },
    },
    "/secret": {
      GET: async (req) => {
        const auth = req.headers.get("authorization") ?? "";
        const m = /^Bearer\s+(.+)$/i.exec(auth);
        if (!m) return new Response("Unauthorized", { status: 401 });
        const payload = await verifyJwt(m[1]!);
        if (!payload) return new Response("Invalid token", { status: 401 });
        return Response.json({ ok: true, payload });
      },
    },
  },
  fetch() {
    return new Response("Not Found", { status: 404 });
  },
});

console.log(`37 jwt → ${server.url}`);
```

**해설** — JWT 3파트(`header.payload.signature`)를 직접 조립합니다:
- `b64url` — base64url 인코딩(`+/=` → `-_` 제거).
- `signJwt` — `iat`/`exp` 를 넣고 HMAC-SHA256 서명.
- `verifyJwt` — **`crypto.subtle.verify`** 로 서명 검증(상수 시간, 안전) 후 `exp` 만료 확인.
- HMAC 키는 `SECRET` 에서 `importKey`.

**검토 / 보충**
- ✅ 서명 검증에 `crypto.subtle.verify` 를 써서 **타이밍 세이프**합니다(수동 문자열 비교보다 안전).
- ✅ **alg 혼동 공격 안전**: 검증을 항상 HMAC로 고정하므로, 공격자가 `{"alg":"none"}` 헤더를 넣어도 서명 검증에서 걸립니다. 다만 명시적으로 `header.alg === "HS256"` 를 확인하면 방어가 더 분명해집니다.
- ⚠️ **`SECRET` 하드코딩**: 반드시 환경변수(`Bun.env.JWT_SECRET`)로 빼고, 충분히 긴 랜덤 값을 쓰세요. 코드/저장소에 노출 금지.
- ⚠️ `verifyJwt` 의 `JSON.parse` 는 payload가 깨졌을 때 throw 할 수 있습니다. `try/catch` 로 감싸 `null` 반환하면 더 견고합니다.

```ts
// verifyJwt payload 파싱 방어
let payload: Record<string, unknown>;
try {
  payload = JSON.parse(new TextDecoder().decode(fromB64url(p!)));
} catch { return null; }
```

---

## `38_mini_app.ts` — Express식 미니 프레임워크

포트 **3038**. 부속 파일 [`_mini/mini.ts`](#_miniminits) 의 초소형 라우터를 사용.

```ts
// 38. Express식 미니 프레임워크 — app.get/post + 미들웨어 + :id
// 실행: bun --hot 38_mini_app.ts
// 확인:
//   curl http://localhost:3038/
//   curl http://localhost:3038/api/users
//   curl -X POST http://localhost:3038/api/users \
//     -H 'Content-Type: application/json' -d '{"name":"lin"}'
//   curl http://localhost:3038/api/users/<id>

import { mini } from "./_mini/mini.ts";

type User = { id: string; name: string };
const users: User[] = [{ id: "1", name: "ada" }];

const app = mini();

app.use(async (req, _res, next) => {
  const t = performance.now();
  await next();
  console.log(
    `${req.method} ${req.path} ${(performance.now() - t).toFixed(1)}ms`,
  );
});

app.get("/", (_req, res) => {
  res.json({ tip: ["GET /api/users", "POST /api/users", "GET /api/users/:id"] });
});

app.get("/api/users", (_req, res) => {
  res.json(users);
});

app.post("/api/users", async (req, res) => {
  const body = (await req.raw.json()) as { name?: string };
  if (!body.name?.trim()) {
    res.status(400).json({ error: "name required" });
    return;
  }
  const user = { id: crypto.randomUUID().slice(0, 8), name: body.name.trim() };
  users.push(user);
  res.status(201).json(user);
});

app.get("/api/users/:id", (req, res) => {
  const user = users.find((u) => u.id === req.params.id);
  if (!user) {
    res.status(404).json({ error: "Not Found" });
    return;
  }
  res.json(user);
});

const server = app.listen(3038);
console.log(`38 mini_app → ${server.url}`);
```

**해설** — Express의 `app.use`/`app.get`/`res.status().json()` 감각을 Bun.serve 위에 얇게 재현. 원 요청은 `req.raw`(원본 Request)로 접근. 미들웨어(`app.use`)는 `next()` 로 체이닝. 실제 라우팅/응답 변환 로직은 부속 파일 `mini.ts` 에 있습니다.

**검토 / 보충** (프레임워크 본체 검토는 [부속 파일 절](#_miniminits)에서 상세)
- ⚠️ `mini.ts` 의 라우트 매칭이 **메서드는 맞지만 경로가 다른 경우와, 경로는 맞지만 메서드가 다른 경우를 구분하지 않아** 무조건 404를 냅니다(405가 아님). 학습용 축소판이라 감안.
- `req.raw.json()` 을 쓰는데, 편의상 `req.json()` 헬퍼를 `MiniReq` 에 추가하면 더 Express답습니다.

---

## `39_production.ts` — 실전 통합

포트 **3039**. REST + 쿠키세션 + WS pub/sub + SQLite + metrics + graceful 을 한 서버에.

```ts
// 39. 실전 통합 — REST + 쿠키세션 + WS pub/sub + metrics + graceful
// 실행: bun --hot 39_production.ts
// 확인:
//   curl http://localhost:3039/health
//   curl -c /tmp/p.txt -b /tmp/p.txt -X POST http://localhost:3039/api/login \
//     -H 'Content-Type: application/json' -d '{"user":"ada"}'
//   curl -c /tmp/p.txt -b /tmp/p.txt http://localhost:3039/api/me
//   브라우저: http://localhost:3039/

import { Database } from "bun:sqlite";

type WsData = { user: string; room: string };

const dbPath = `${import.meta.dir}/_data/prod.db`;
await Bun.write(`${import.meta.dir}/_data/.keep`, "");
const db = new Database(dbPath);
db.exec(`
  CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    user TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL
  )
`);

const sessions = new Map<string, string>(); // sid → user
const insertNote = db.query(
  `INSERT INTO notes (id, user, body, created_at) VALUES (?, ?, ?, ?)`,
);
const listNotes = db.query(
  `SELECT * FROM notes WHERE user = ? ORDER BY created_at DESC`,
);

const html = `<!DOCTYPE html>
<html lang="ko"><head><meta charset="UTF-8" /><title>Prod Demo</title>
<style>body{font-family:system-ui;padding:2rem}#log{background:#f4f4f5;padding:1rem;height:10rem;overflow:auto;white-space:pre-wrap}</style>
</head><body>
<h1>통합 데모</h1>
<p>REST + session cookie + WS room</p>
<pre id="log"></pre>
<input id="msg" /><button id="send">WS send</button>
<script>
  const log=document.getElementById("log");
  const ws=new WebSocket("ws://"+location.host+"/ws?user=guest&room=lobby");
  ws.onmessage=e=>{log.textContent+=e.data+"\\n"};
  document.getElementById("send").onclick=()=>ws.send(document.getElementById("msg").value);
</script>
</body></html>`;

const server = Bun.serve({
  port: 3039,
  development: false,
  maxRequestBodySize: 1024 * 1024, // 1MB — 초과 시 자동 413
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
    "/health": Response.json({ ok: true }),
    "/metrics": (_req, srv) =>
      Response.json({
        pendingRequests: srv.pendingRequests,
        pendingWebSockets: srv.pendingWebSockets,
        sessions: sessions.size,
      }),
    "/api/login": {
      POST: async (req) => {
        const body = (await req.json()) as { user?: string };
        if (!body.user?.trim()) {
          return Response.json({ error: "user required" }, { status: 400 });
        }
        const sid = crypto.randomUUID();
        sessions.set(sid, body.user.trim());
        req.cookies.set("sid", sid, {
          httpOnly: true,
          sameSite: "lax",
          path: "/",
          maxAge: 3600,
        });
        return Response.json({ ok: true, user: body.user.trim() });
      },
    },
    "/api/me": (req) => {
      const sid = req.cookies.get("sid");
      const user = sid ? sessions.get(sid) : undefined;
      if (!user) return new Response("Unauthorized", { status: 401 });
      return Response.json({ user });
    },
    "/api/notes": {
      GET: (req) => {
        const sid = req.cookies.get("sid");
        const user = sid ? sessions.get(sid) : undefined;
        if (!user) return new Response("Unauthorized", { status: 401 });
        return Response.json(listNotes.all(user));
      },
      POST: async (req) => {
        const sid = req.cookies.get("sid");
        const user = sid ? sessions.get(sid) : undefined;
        if (!user) return new Response("Unauthorized", { status: 401 });
        const body = (await req.json()) as { body?: string };
        if (!body.body?.trim()) {
          return Response.json({ error: "body required" }, { status: 400 });
        }
        const id = crypto.randomUUID();
        const created_at = new Date().toISOString();
        insertNote.run(id, user, body.body.trim(), created_at);
        return Response.json(
          { id, user, body: body.body.trim(), created_at },
          { status: 201 },
        );
      },
    },
  },
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      const ok = srv.upgrade(req, {
        data: {
          user: url.searchParams.get("user") ?? "anon",
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
      server.publish(ws.data.room, `* ${ws.data.user} joined`);
    },
    message(ws, message) {
      server.publish(ws.data.room, `${ws.data.user}: ${message}`);
    },
    close(ws) {
      ws.unsubscribe(ws.data.room);
      server.publish(ws.data.room, `* ${ws.data.user} left`);
    },
  },
  error(err) {
    console.error(err);
    return new Response("Internal Server Error", { status: 500 });
  },
});

let closing = false;
async function shutdown(sig: string) {
  if (closing) return;
  closing = true;
  console.log(`\n${sig} — graceful stop`);
  await server.stop();
  db.close();
  process.exit(0);
}
process.on("SIGINT", () => void shutdown("SIGINT"));
process.on("SIGTERM", () => void shutdown("SIGTERM"));

console.log(`39 production → ${server.url}`);
```

**해설** — 앞선 예제들의 종합판:
- **REST**(예제 21/22) + **세션 쿠키 인증**(예제 31) + **WS pub/sub**(예제 20) + **SQLite 영속**(예제 22) + **metrics**(예제 23) + **graceful shutdown + `db.close()`**(예제 25).
- `development: false`(내장 에러 페이지 끔) + `maxRequestBodySize`(자동 413) + `error()` 콜백 = 프로덕션 하드닝(예제 40).
- 노트 API는 세션 사용자별로 격리(`WHERE user = ?`).

**검토 / 보충**
- ✅ 각 조각이 앞 예제와 일관되며 통합이 자연스럽습니다. **graceful 종료에서 `db.close()`** 를 호출하는 부분이 특히 좋습니다(SQLite WAL 안전 종료).
- ⚠️ 세션 만료 청소가 여전히 없습니다(예제 31 보충 참조). 장기 구동 시 `sessions` 누수.
- ⚠️ `sessions` 는 인메모리라 재시작하면 전원 로그아웃되고, 다중 인스턴스 확장 불가. 프로덕션이라면 세션/pub-sub을 외부 스토어(Redis)로.
- WS 인증이 없습니다. `/ws` 업그레이드 시 쿠키의 `sid` 를 검증해 인증된 사용자만 방에 넣는 편이 실전적입니다.
- `websocket.data = {} as WsData` no-op은 여기도 동일(제거 가능).

---

## `40_hardening.ts` — 프로덕션 하드닝

포트 **3040**. `maxRequestBodySize`(자동 413) · `development:false` · `error()` 핵심 3종.

```ts
// 40. maxRequestBodySize · development:false — 프로덕션 하드닝 포인트
// 실행: bun --hot 40_hardening.ts
// 확인:
//   curl -i -X POST http://localhost:3040/echo -H 'Content-Type: text/plain' -d 'hi'
//   # 큰 바디 → 413 (핸들러 도달 전 자동)
//   dd if=/dev/zero bs=1024 count=20 2>/dev/null | \
//     curl -i -X POST http://localhost:3040/echo \
//       -H 'Content-Type: application/octet-stream' --data-binary @-

const server = Bun.serve({
  port: 3040,
  development: false, // 내장 에러 페이지 비활성 (프로덕션 권장)
  maxRequestBodySize: 8 * 1024, // 8KB — 초과 시 자동 413
  routes: {
    "/": () =>
      new Response(
        [
          "POST /echo  (max body 8KB)",
          "development: false",
          "error() 커스텀 JSON",
          "",
        ].join("\n"),
      ),
    "/echo": {
      POST: async (req) => {
        const buf = await req.arrayBuffer();
        return Response.json({
          bytes: buf.byteLength,
          note: "8KB 넘기면 여기까지 도달하지 않음 (413)",
        });
      },
    },
    "/boom": () => {
      throw new Error("prod error");
    },
  },
  fetch() {
    return Response.json({ error: "Not Found" }, { status: 404 });
  },
  error(err) {
    // development:false 여도 error 콜백은 동작
    console.error("[error]", err);
    return Response.json(
      { error: "Internal Server Error" },
      { status: 500 },
    );
  },
});

console.log(`40 hardening → ${server.url}`);
```

**해설** — 프로덕션 3대 스위치를 압축 시연:
1. `maxRequestBodySize` — 한도를 넘는 바디는 **핸들러 도달 전 자동 413**(메모리 보호).
2. `development: false` — 내장 스택 트레이스 페이지 비활성(정보 유출 방지).
3. `error()` — 그래도 커스텀 500 JSON은 콜백으로 제어(내부 메시지 노출 없이).

**검토 / 보충**
- ✅ 세 스위치의 조합이 정확합니다. 예제 13(개발용 상세 에러)과 정확히 대비됩니다.
- 추가 하드닝 아이디어: 보안 헤더(`Content-Security-Policy`, `X-Content-Type-Options: nosniff`, `Strict-Transport-Security` — HTTPS 시), 요청 타임아웃(예제 30), 레이트 리밋(예제 24)을 함께.

---

## `41_ws_publish.ts` — ws.publish vs server.publish

포트 **3041**. 두 브로드캐스트 API의 **수신자 차이**를 실증.

```ts
// 41. ws.publish vs server.publish
// 실행: bun --hot 41_ws_publish.ts
// 확인: 브라우저 탭 2개
//   http://localhost:3041/?user=ada
//   http://localhost:3041/?user=bob
//
// | API | 수신자 |
// |---|---|
// | server.publish(topic, msg) | 토픽 구독자 **전원** (보낸 사람 포함) |
// | ws.publish(topic, msg)     | 토픽 구독자 중 **본인 제외** |

type WsData = { user: string };

const TOPIC = "demo";

const html = `<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <title>publish 비교</title>
  <style>
    body { font-family: ui-sans-serif, system-ui, sans-serif; padding: 2rem; max-width: 36rem; }
    #log { height: 14rem; overflow: auto; background: #f4f4f5; padding: 1rem; white-space: pre-wrap; }
    .row { display: flex; gap: 0.5rem; margin-top: 0.75rem; flex-wrap: wrap; }
    button { padding: 0.5rem 0.75rem; cursor: pointer; }
    code { background: #e4e4e7; padding: 0.1rem 0.35rem; }
  </style>
</head>
<body>
  <h1>publish 비교</h1>
  <p id="meta"></p>
  <p>
    <code>server</code> = 전원(나 포함) ·
    <code>ws</code> = 남만(나는 안 받음)
  </p>
  <pre id="log"></pre>
  <div class="row">
    <button id="serverPub">server.publish</button>
    <button id="wsPub">ws.publish</button>
  </div>
  <script>
    const user = new URLSearchParams(location.search).get("user") || "anon";
    document.getElementById("meta").textContent = "나: " + user + "  (다른 탭을 하나 더 여세요)";
    const log = document.getElementById("log");
    const ws = new WebSocket("ws://" + location.host + "/ws?user=" + encodeURIComponent(user));
    ws.onmessage = (e) => {
      log.textContent += e.data + "\\n";
      log.scrollTop = log.scrollHeight;
    };
    document.getElementById("serverPub").onclick = () => ws.send(JSON.stringify({ mode: "server" }));
    document.getElementById("wsPub").onclick = () => ws.send(JSON.stringify({ mode: "ws" }));
  </script>
</body>
</html>`;

const server = Bun.serve({
  port: 3041,
  routes: {
    "/": () =>
      new Response(html, {
        headers: { "Content-Type": "text/html; charset=utf-8" },
      }),
  },
  fetch(req, srv) {
    const url = new URL(req.url);
    if (url.pathname === "/ws") {
      const ok = srv.upgrade(req, {
        data: { user: url.searchParams.get("user") ?? "anon" } satisfies WsData,
      });
      return ok ? undefined : new Response("Upgrade failed", { status: 400 });
    }
    return new Response("Not Found", { status: 404 });
  },
  websocket: {
    data: {} as WsData,
    open(ws) {
      ws.subscribe(TOPIC);
      // 입장 알림은 전원에게 (본인 포함)
      server.publish(TOPIC, `* ${ws.data.user} joined (subscribers=${server.subscriberCount(TOPIC)})`);
    },
    message(ws, raw) {
      let mode = "server";
      try {
        mode = (JSON.parse(String(raw)) as { mode?: string }).mode ?? "server";
      } catch {
        /* plain text → server */
      }

      if (mode === "ws") {
        // 본인 제외 → 보낸 탭 로그에는 안 뜸 (다른 탭만)
        ws.publish(TOPIC, `[ws.publish] ${ws.data.user} → others only`);
        // 보낸 사람에게만 로컬 확인용
        ws.send(`[local] ws.publish 호출함 (나는 토픽으로 안 받음)`);
      } else {
        // 전원 → 보낸 탭에도 동일 메시지 수신
        server.publish(TOPIC, `[server.publish] ${ws.data.user} → everyone`);
      }
    },
    close(ws) {
      ws.unsubscribe(TOPIC);
      server.publish(TOPIC, `* ${ws.data.user} left`);
    },
  },
});

console.log(`41 ws_publish → ${server.url}?user=ada`);
console.log(`  다른 탭: ${server.url}?user=bob`);
```

**해설** — 핵심 대비:

| API | 수신자 |
|---|---|
| `server.publish(topic, msg)` | 토픽 구독자 **전원**(보낸 사람 포함) |
| `ws.publish(topic, msg)` | 토픽 구독자 중 **본인 제외** |

채팅에서 “내가 보낸 메시지를 나도 화면에 그려야” 하면 `server.publish`(전원), “남에게만 알림”이면 `ws.publish`(본인 제외). 예제는 버튼 두 개로 이 차이를 직접 눈으로 확인하게 합니다. `ws.publish` 시엔 보낸 사람에게 `ws.send` 로 로컬 확인 메시지를 따로 줍니다.

**검토 / 보충**
- ✅ 예제 20의 채팅이 왜 `server.publish` 를 썼는지(본인 메시지도 표시) 이 예제로 명확해집니다. 실무 채팅에선 보통 `server.publish` 로 전원 브로드캐스트 후 클라이언트가 자기 메시지를 구분 표시.
- `websocket.data = {} as WsData` no-op 동일(제거 가능).

---

# 부속 파일

## `_mini/mini.ts`

예제 38이 사용하는 Express식 초소형 라우터(의존성 0). 학습용 축소판.

```ts
// _mini/mini.ts — Express식 초소형 라우터 (의존성 0)
// 고급 예제 38에서 사용. 학습용 축소판.

export type Next = (err?: unknown) => void;
export type Handler = (
  req: MiniReq,
  res: MiniRes,
  next: Next,
) => unknown | Promise<unknown>;

type Route = { method: string; pattern: string; handlers: Handler[] };

export class MiniReq {
  raw: Request;
  method: string;
  url: URL;
  params: Record<string, string> = {};
  body: unknown;
  constructor(raw: Request) {
    this.raw = raw;
    this.method = raw.method;
    this.url = new URL(raw.url);
  }
  get path() {
    return this.url.pathname;
  }
  get query() {
    return this.url.searchParams;
  }
}

export class MiniRes {
  statusCode = 200;
  headers = new Headers();
  finished = false;
  private _body: BodyInit | null = null;

  status(code: number) {
    this.statusCode = code;
    return this;
  }
  json(data: unknown) {
    this.headers.set("Content-Type", "application/json; charset=utf-8");
    this._body = JSON.stringify(data);
    return this.end();
  }
  send(text: string) {
    if (!this.headers.has("Content-Type")) {
      this.headers.set("Content-Type", "text/plain; charset=utf-8");
    }
    this._body = text;
    return this.end();
  }
  end() {
    this.finished = true;
    return this;
  }
  toResponse() {
    return new Response(this._body, {
      status: this.statusCode,
      headers: this.headers,
    });
  }
}

function match(pattern: string, path: string): Record<string, string> | null {
  const pp = pattern.split("/").filter(Boolean);
  const sp = path.split("/").filter(Boolean);
  if (pp.length !== sp.length) return null;
  const params: Record<string, string> = {};
  for (let i = 0; i < pp.length; i++) {
    const a = pp[i]!;
    const b = sp[i]!;
    if (a.startsWith(":")) params[a.slice(1)] = decodeURIComponent(b);
    else if (a !== b) return null;
  }
  return params;
}

export function mini() {
  const routes: Route[] = [];
  const middlewares: Handler[] = [];

  const add =
    (method: string) =>
    (pattern: string, ...handlers: Handler[]) => {
      routes.push({ method, pattern, handlers });
      return api;
    };

  const api = {
    use(mw: Handler) {
      middlewares.push(mw);
      return api;
    },
    get: add("GET"),
    post: add("POST"),
    put: add("PUT"),
    patch: add("PATCH"),
    delete: add("DELETE"),
    listen(port: number) {
      return Bun.serve({
        port,
        async fetch(raw) {
          const req = new MiniReq(raw);
          const res = new MiniRes();

          const stack: Handler[] = [...middlewares];
          const found = routes.find(
            (r) =>
              r.method === req.method &&
              match(r.pattern, req.path) !== null,
          );
          if (found) {
            req.params = match(found.pattern, req.path)!;
            stack.push(...found.handlers);
          } else {
            stack.push((_q, r) => {
              r.status(404).send("Not Found");
            });
          }

          let i = 0;
          const next: Next = async (err) => {
            if (err) {
              console.error(err);
              if (!res.finished) res.status(500).json({ error: "Internal" });
              return;
            }
            const h = stack[i++];
            if (!h || res.finished) return;
            try {
              await h(req, res, next);
            } catch (e) {
              await next(e);
            }
          };
          await next();
          return res.toResponse();
        },
      });
    },
  };

  return api;
}
```

**해설** — 구조:
- `MiniReq` — 원 Request를 감싸 `path`/`query`/`params` 게터 제공.
- `MiniRes` — `status().json()/.send()` 체이닝, 최종 `toResponse()` 로 Bun Response 변환.
- `match()` — `/` 세그먼트 단위 비교, `:param` 캡처(디코딩 포함).
- `mini()` — 라우트/미들웨어 등록 + `listen()` 안에서 미들웨어→핸들러 스택을 `next()` 재귀로 실행(에러는 `next(err)` 로 전파).

**검토 / 보충**
- ⚠️ **`match()` 를 두 번 호출**합니다(`find` 에서 한 번, `req.params` 채우려고 또 한 번). 라우트가 많으면 낭비. 한 번만 계산해 재사용하세요.

```ts
let matched: { route: Route; params: Record<string, string> } | undefined;
for (const r of routes) {
  if (r.method !== req.method) continue;
  const p = match(r.pattern, req.path);
  if (p) { matched = { route: r, params: p }; break; }
}
if (matched) { req.params = matched.params; stack.push(...matched.route.handlers); }
else stack.push((_q, res) => { res.status(404).send("Not Found"); });
```

- ⚠️ **405 미구분**: 경로는 맞고 메서드만 다른 경우도 404가 됩니다. 경로 매칭과 메서드 매칭을 분리하면 405를 낼 수 있습니다(학습용이라 생략된 부분).
- ⚠️ 라우트 우선순위가 **등록 순서(선착순)** 입니다. `/api/users/:id` 를 `/api/users/me` 보다 먼저 등록하면 `me` 가 `:id` 로 잡힙니다. 예제 32의 Bun 내장 라우터(exact > param)와 다른 점이니 등록 순서에 주의.
- `next()` 를 여러 번 부르는 오용에 대한 방어는 없습니다(Express도 유사). `res.finished` 체크가 일부 완충.

---

## `_html_import/index.html`

예제 35의 번들 엔트리 HTML. `<script type="module" src="./app.ts">` 로 TS를 참조.

```html
<!DOCTYPE html>
<html lang="ko">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>HTML Import</title>
  <style>
    body {
      min-height: 100dvh;
      display: grid;
      place-items: center;
      font-family: ui-sans-serif, system-ui, sans-serif;
      background: #0f1115;
      color: #e8eaed;
    }
    #count { font-size: 3rem; font-weight: 600; }
    button {
      margin-top: 1rem;
      padding: 0.6rem 1.2rem;
      border: 0;
      background: #3b82f6;
      color: white;
      font-size: 1rem;
      cursor: pointer;
    }
  </style>
</head>
<body>
  <main>
    <p>HTML import + bun --hot → 브라우저 HMR</p>
    <div id="count">0</div>
    <button id="inc">+1</button>
  </main>
  <script type="module" src="./app.ts"></script>
</body>
</html>
```

**해설** — 평범한 HTML이지만 `src="./app.ts"`(TypeScript!)를 그대로 참조합니다. Bun이 import 시 이 TS를 자동 트랜스파일·번들합니다. 브라우저는 트랜스파일된 JS를 받습니다.

## `_html_import/app.ts`

예제 35의 클라이언트 스크립트. 저장 시 HMR 대상.

```ts
// bun --hot 로 돌리면 이 파일 수정 시 브라우저 HMR
const el = document.getElementById("count")!;
const btn = document.getElementById("inc")!;

let n = 0;
const render = () => {
  el.textContent = String(n);
};

btn.addEventListener("click", () => {
  n += 1;
  render();
});

render();
console.log("[app.ts] loaded — 이 파일을 저장하면 HMR");
```

**해설** — 카운터 로직. `bun --hot` 상태에서 이 파일을 수정·저장하면 브라우저가 자동 갱신됩니다. TS의 non-null 단언(`!`)으로 DOM 요소를 가져옵니다.

**검토** — 학습용으로 충분. 실무라면 `getElementById` 결과가 `null` 일 가능성(요소 없음)을 방어하는 게 안전하지만, 자기 소유 HTML이라 `!` 로 단언한 것.

## 정적 산출물/데이터 파일

이 파일들은 **예제 실행 시 코드가 자동 생성**합니다(`Bun.write`). 저장소에 남아 있는 현재 내용:

| 파일 | 생성 예제 | 내용 |
|---|---|---|
| `_static/readme.txt` | 08 | `Bun.file 로 정적 파일을 서빙하는 초급 예제입니다.` |
| `_static/page.html` | 08 | 정적 HTML(`<h1>정적 HTML</h1>`) |
| `_public/index.html` | 18 | `<h1>Public</h1><a href="/about.html">about</a>` |
| `_public/about.html` | 18 | `<h1>About</h1>` |
| `_public/hello.txt` | 18 | `static dir ok` |
| `_data/range-demo.txt` | 28 | `Hello Range!\nSecond line here.\nThird line end.` (45바이트) |
| `_data/posts.db` | 22 | SQLite DB(posts 테이블) |
| `_data/prod.db` | 39 | SQLite DB(notes 테이블) |
| `_data/.keep`, `_uploads/.keep` | 여러 예제 | 디렉터리 유지용 빈 파일 |
| `_uploads/1785769774054_demo.txt` | 14 | 업로드 테스트 산출물(`hi`) — `Date.now()` 접두사 |
| `_certs/cert.pem`, `_certs/key.pem` | (openssl 수동) | 개발용 자체 서명 인증서(예제 33/34) |

> **참고** — `.db` 는 바이너리, `.pem` 은 개인키를 포함하므로 본문에는 원문을 싣지 않았습니다. `_certs/key.pem` 은 **개발용 개인키**이니 실서비스에 재사용하지 말고, 저장소 공개 시 노출에 유의하세요.

---

# 전체 검토 요약표

| # | 파일 | 포트 | 핵심 주제 | 상태 | 주요 검토/보충 포인트 |
|---|---|---|---|---|---|
| 01 | hello | 3001 | 최소 서버 | ✅ | `port:0` 자동할당 팁 |
| 02 | html | 3002 | HTML 응답 | ✅ | Content-Type 명시 |
| 03 | json | 3003 | `Response.json` | ✅ | routes+fetch 폴백 골격 |
| 04 | routes | 3004 | 경로 파라미터 | ✅ | params는 디코딩된 문자열 |
| 05 | methods | 3005 | 메서드 분기 | ⚠️ | ID=length+1 충돌 위험, JSON 파싱 400 처리 |
| 06 | query | 3006 | 쿼리 검증 | ✅ | 견고한 검증 패턴 |
| 07 | body | 3007 | 바디 파싱 3종 | ✅ | 다중 필드/바디 1회성 주의 |
| 08 | static | 3008 | `Bun.file` | ✅ | 동적경로는 exists 확인 |
| 09 | cookies | 3009 | 쿠키 | ⚠️ | cookies는 routes에서만, `Number||0` 방어 |
| 10 | websocket | 3010 | WS 에코 | ✅ | 미사용 파라미터 |
| 11 | cors | 3011 | CORS | ⚠️ | `*`+쿠키 불가, credentials CORS 보충 |
| 12 | redirect | 3012 | 정적 Response | ✅ | 동적값은 함수로 |
| 13 | error | 3013 | 에러 핸들러 | ⚠️ | 메시지 노출은 dev 한정 |
| 14 | upload | 3014 | 업로드 | ⚠️ | 파일명 방어 강화·크기 제한 |
| 15 | sse | 3015 | SSE | ✅ | cancel 정리 우수, 자동재연결 |
| 16 | auth | 3016 | Bearer 가드 | ⚠️ | 상수시간 비교·env 토큰 |
| 17 | middleware | 3017 | 미들웨어 합성 | ⚠️ | Response 재생성 대신 헤더 직접 set |
| 18 | static_dir | 3018 | 디렉터리 서빙 | ⚠️ | resolve 접두사 검증이 더 정밀 |
| 19 | stream | 3019 | 스트리밍 | ✅ | 장시간이면 cancel 추가 |
| 20 | ws_pubsub | 3020 | pub/sub 채팅 | ⚠️ | `data:{}` no-op |
| 21 | crud | 3021 | REST CRUD | ⚠️ | PATCH 타입검증·JSON 400 |
| 22 | sqlite | 3022 | SQLite | ✅ | WAL·busy_timeout 권장 |
| 23 | metrics | 3023 | Server API | ⚠️ | 프록시 뒤 IP·/metrics 보호 |
| 24 | ratelimit | 3024 | 레이트리밋 | ⚠️ | 맵 누수 청소·경계버스트 |
| 25 | graceful | 3025 | graceful stop | ✅ | 강제종료 타임아웃 보충 |
| 26 | reload | 3026 | `server.reload` | ✅ | 포트는 불변 |
| 27 | unix | 3027* | 유닉스 소켓 | ✅ | 종료 시 소켓 정리 보충 |
| 28 | range | 3028 | Range 206 | ✅ | RFC 충실, 단일범위 |
| 29 | proxy | 3029 | 리버스 프록시 | ✅ | 고정 업스트림=SSRF 안전 |
| 30 | timeout | 3030 | 타임아웃 | ✅ | idle=유휴 기준 |
| 31 | session | 3031 | 쿠키 세션 | ⚠️ | 세션 만료 청소·secure |
| 32 | export_default | 3032 | 라우트 우선순위 | ✅ | server 객체 없음 |
| 33 | tls | 3033 | HTTPS | ✅ | 프로덕션은 CA 인증서 |
| 34 | http3 | 3034 | HTTP/3 | ⚠️ | 실험적·환경 의존 |
| 35 | html_import | 3035 | 번들+HMR | ✅ | 배포는 사전빌드 |
| 36 | ws_advanced | 3036 | WS 튜닝 | ⚠️ | drain 흐름제어·`data:{}` no-op |
| 37 | jwt | 3037 | JWT HMAC | ⚠️ | SECRET env화·payload parse 방어 |
| 38 | mini_app | 3038 | 미니 프레임워크 | ⚠️ | (mini.ts) match 중복·405 미구분·등록순 우선순위 |
| 39 | production | 3039 | 실전 통합 | ✅ | 세션청소·WS인증 보충 |
| 40 | hardening | 3040 | 하드닝 3종 | ✅ | 보안헤더 추가 |
| 41 | ws_publish | 3041 | publish 차이 | ✅ | server=전원 / ws=본인제외 |

> `*` 예제 27은 TCP 포트가 아니라 유닉스 소켓(`/tmp/bunserve-27.sock`)을 씁니다.

### 반복되는 공통 개선 포인트 (여러 예제 공통)

1. **`websocket.data = {} as WsData` 는 no-op** — 예제 20/36/39/41 공통. 실제 연결 데이터는 `upgrade(req, { data })` 에서 주입되므로 삭제해도 무방(타입 힌트 목적이었던 것으로 보임).
2. **인메모리 상태의 만료 청소 부재** — 예제 24(레이트리밋)·31/39(세션). 장기 구동 시 메모리 누수 → `setInterval(...).unref()` 청소 권장.
3. **프록시 뒤 실제 클라이언트 IP** — 예제 23/24. `requestIP` 는 프록시 IP이므로 신뢰된 프록시 한정 `X-Forwarded-For` 처리 필요.
4. **비밀값 하드코딩** — 예제 16(토큰)/37(JWT SECRET). 환경변수(`Bun.env.*`)로 분리.
5. **잘못된 JSON 바디** — 예제 05/21 등. `try/catch` 로 400을 주면 예제 07 수준의 견고함.
6. **다중 인스턴스 확장** — 인메모리 세션/pub-sub/레이트리밋은 단일 프로세스 전제. 수평 확장 시 Redis 등 공유 스토어 필요.

---
