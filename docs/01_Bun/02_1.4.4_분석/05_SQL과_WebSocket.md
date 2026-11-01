# 5. SQL과 WebSocket

`Bun.SQL`, `bun:sqlite`, `node:sqlite`를 한 변경으로 다시 잡았습니다. 열린 풀 리퀘스트 149개를 대체하고 이슈 51개를 고친 작업이라고 합니다. WebSocket 쪽은 npm `ws` 8.22.0에 맞춘 재작성과, close 이벤트의 의미 변경이 핵심입니다. close 쪽은 업그레이드만으로 기존 코드의 분기가 달라집니다.

## 쿼리 비용

PostgreSQL에서 클라이언트 CPU 명령 수(Linux x64, 유저 스페이스)는 한 행 `SELECT`가 48%, 100행 `SELECT`가 30%, `INSERT`가 21%, 1,000행×10열 대량 insert가 38% 줄었습니다. 같은 대량 insert의 MySQL은 53% 줄었습니다.

`await sql\`select 1\`` 한 번이 가비지 컬렉션에 넘기는 객체 수는 PostgreSQL 84개에서 28개, MySQL 76개에서 27개, SQLite 52개에서 33개입니다.

## 취소, COPY, 이름 변환

`query.cancel()`은 타입과 문서에만 있었고 아무 일도 하지 않았습니다. 이제 PostgreSQL에 취소를 요청하고, 프로미스는 서버 오류로 거절됩니다. 아직 보내지 않은 쿼리는 보내지 않고 `ERR_POSTGRES_QUERY_CANCELLED`(MySQL·SQLite는 각각의 코드)로 거절됩니다. MySQL과 MariaDB에서 이미 서버에서 돌기 시작한 쿼리는 끝까지 실행됩니다.

```ts
const query = sql`SELECT pg_sleep(30)`.execute();
setTimeout(() => query.cancel(), 100);
await query; // canceling statement due to user request
```

PostgreSQL `COPY`는 웹 스트림입니다. `writable()`이 `COPY ... FROM STDIN`, `readable()`이 `COPY ... TO STDOUT`입니다. 역압이 있어서 테이블이 커져도 메모리 사용이 스트림 크기만큼 늘지 않습니다. 형식은 문이 지정한 `text`, `csv`, `binary`의 바이트 그대로입니다. 1,000MiB COPY가 초당 255MiB, 최대 RSS 59MiB라는 측정이 있습니다.

```ts
await Bun.file("users.csv").stream().pipeTo(
  sql`COPY users (id, name) FROM STDIN (FORMAT csv)`.writable(),
);

const body = sql`COPY users TO STDOUT (FORMAT csv, HEADER)`.readable();
```

`new SQL({ transform: SQL.camel })`은 양쪽을 바꿉니다. 읽을 때 `first_name`이 `firstName`이 되고, `sql({ firstName })`로 쓸 때 컬럼은 `first_name`이 됩니다. 어댑터 모두에서 동작합니다. `SQL.pascal`, `SQL.kebab`도 양방향이고, `SQL.toCamel`·`SQL.fromCamel`은 한방향입니다. `{ column: { from, to } }`에 함수를 직접 줄 수 있습니다. 이름은 postgres.js와 같습니다.

JavaScript 배열과 `Int32Array`를 PostgreSQL 배열 자리에 넣을 수 있습니다. `null`이 들어 있는 배열과 중첩 배열도 됩니다. `ANY(${[1, 2, 3]})`와 `tags: ["bun", "sql"]` insert는 1.4.3에서 실패했습니다. `uuid[]`, `int4range[]`, `tsvector[]` 등 내장 배열 타입 35개가 `"{...}"` 문자열 대신 배열로 돌아옵니다. 열거형 같은 사용자 타입의 배열은 `sql.array(values, "mood")`를 씁니다.

PostgreSQL·MySQL·MariaDB 결과에는 `result.columns`와 `result.statement`가 있습니다. `type`으로 `jsonb`와 `text[]`처럼 JavaScript 값이 비슷해 보이는 컬럼을 구분합니다. PostgreSQL 컬럼은 `{ name, type, table, number }`이고, MySQL·MariaDB는 `{ name, type, table, length, flags }`입니다. SQLite에서는 둘 다 `null`입니다. `statement.string`은 `$1` 자리표시자가 들어간, Bun이 보낸 문장입니다.

`sql.json(42)`는 모든 어댑터에서 JSON으로 저장합니다. 숫자·불리언·문자열·`null`에 씁니다. 값 없이 넘긴 `null`은 여전히 SQL `NULL`입니다.

## 연결과 핫 리로드

`bun --hot`으로 저장할 때마다 설정이 같은 `new SQL()`이 최대 `max`개의 PostgreSQL·MySQL 연결을 새로 열고 예전 연결을 닫지 않았습니다. `max: 2`로 다섯 번 리로드하면 서버에 세션이 12개 남던 것이 이제 2개입니다. `SET`과 임시 테이블은 유지되고, 재사용한 연결마다 `onconnect`가 다시 실행됩니다.

새로 받는 연결 옵션은 다음과 같습니다.

- `channelBinding`: `"disable"`(기본), `"prefer"`, `"require"`. PostgreSQL SCRAM을 서버 인증서에 묶습니다(SCRAM-SHA-256-PLUS). URL의 `channel_binding`, 환경 변수 `PGCHANNELBINDING`도 됩니다.
- `tls.checkServerIdentity(hostname, cert)`. `node:tls`와 같이 호스트 이름 검사를 바꿉니다. 이 함수를 주면 `verify-ca`보다 느슨한 SSL 모드는 `verify-full`로 올라갑니다.
- MySQL·MariaDB `foundRows: true`. `affectedRows`가 바뀐 행이 아니라 맞은 행을 셉니다. `mysql2`와 같습니다.
- URL의 `connect_timeout`, `idle_timeout`, `max_lifetime`, `max`.
- libpq 방식의 `host`, `dbname`, `sslrootcert`, `sslcert`, `sslkey` 등. 1.4.3은 이 값을 서버로 넘겨 연결이 거절되었습니다.
- `PGAPPNAME`, `PGOPTIONS`, `PGCONNECT_TIMEOUT`.

풀 동작에서 결과가 달라지는 수정:

- `idleTimeout`과 `maxLifetime`이 쿼리 중·예약 중·열린 트랜잭션 연결을 끊지 않습니다. 일이 끝난 뒤에 닫습니다.
- 연결 일부가 끊겨도 하나가 살아 있으면 `max`까지 다시 열지 않던 것이, 다시 찹니다.
- 수동 `BEGIN` 도중 풀에 반환된 연결은 다음 호출자에게 주지 않고 닫습니다.
- `sql.close({ timeout: 0 })`은 돌고 있는 쿼리를 기다리지 않습니다. `timeout: 2 ** 31`은 타이머가 넘쳐서 1ms 만에 쿼리를 죽이지 않습니다.
- `sql.begin()`과 `sql.reserve()`가 대기 중인 이전 쿼리를 새치기하지 않습니다.
- `sql.listen()` 연결에서도 `onconnect`와 `onclose`가 실행됩니다.
- PostgreSQL·MySQL 소켓에 TCP keepalive 60초가 걸려, 서버가 사라져도 쿼리가 무한히 기다리지 않습니다.

트랜잭션에서는 `sql.begin(tx => Promise.all(tasks))` 중 하나가 실패한 뒤, 다른 작업이 `ROLLBACK` 이후에 보낸 문이 autocommit으로 실행되어 행이 남던 것이 이제 거절됩니다. `release()`·`COMMIT`·`ROLLBACK` 이후에야 await되는 쿼리도 다른 호출자의 연결에서 실행되지 않고 거절됩니다. `tx.close({ timeout })`은 제때 끝나도 커밋하지 않고 롤백합니다. `sql.begin()` 콜백의 `AsyncLocalStorage`는 풀이 차갑거나 바쁠 때 비거나 다른 요청의 저장소를 보던 것이, 호출자의 저장소를 봅니다.

## 업그레이드 뒤에 값이 달라지는 쿼리

연결이 되어 있던 코드도 아래는 결과가 바뀝니다.

```ts
await sql`SELECT * FROM users WHERE id IN ${sql([])}`;
// 행 없음. 1.4.3은 PostgreSQL·MySQL에서 문법 오류

const written = await sql`UPDATE users SET name = ${"a"} WHERE false`;
written.affectedRows; // PostgreSQL·SQLite에서 1.4.3은 항상 null. 이제는 쓴 행 수, SELECT는 0
written.count;        // MySQL에서 쓰기 뒤에 0이던 것이 고쳐짐

await sql`INSERT INTO posts (tags) VALUES (${sql.array(["a", "b"])})`;
// 타입을 빼면 1.4.3은 JSON[]로 캐스트되어 원소가 JSON 문자열로 들어감
// text[] 컬럼은 sql.array(["a", "b"], "TEXT")

const [row] = await sql`SELECT ${(2n ** 52n) + 1n}::int8 AS n`;
row.n; // 문자열. 1.4.3은 이 범위의 정수를 float8로 보내 numeric이 어긋났음

await sql`INSERT INTO events (at) VALUES (${new Date(NaN)})`;
// 거절. 1.4.3은 PostgreSQL·MySQL에 1970-01-01로 저장
```

`Date`를 `text`에 묶으면 `toString()`이 아니라 ISO 8601이고, 일반 객체는 `[object Object]`가 아니라 JSON입니다. `date`·`time`에 묶은 `Date`는 더 이상 실패하지 않습니다. 배열이나 객체를 `bool`·`int4`에 넣으면 클라이언트에서 `[]`를 `true`로 바꾸지 않고, 서버가 검증하도록 텍스트로 보냅니다.

비밀번호는 `PASSWORD` 환경 변수를 더 이상 읽지 않습니다. URL, `password` 옵션, `PGPASSWORD`, `MYSQL_PASSWORD`를 씁니다.

```ts
new SQL({ adapter: "sqlite", filename: "./dev.db" });
// adapter 없이 filename만 주면 1.4.3은 PostgreSQL 접속을 시도
// SQLITE_URL=x.db 는 postgres://x.db 라는 파일을 열려고 했음
```

없는 유닉스 소켓 `path`는 TCP로 넘어가지 않고 `ERR_POSTGRES_CONNECTION_REFUSED` 또는 `ERR_MYSQL_CONNECTION_REFUSED`로 실패합니다. `mysql://user:pass@/db?socket=/path`는 `Invalid URL`이 아니라 그 소켓으로 접속합니다. `mariadb://`에 데이터베이스가 없으면 `mariadb`라는 데이터베이스를 고르지 않습니다. SQLite URL의 `?mode=rw`가 파일을 만들고, `?mode=RO`와 `{ readwrite: false }`가 읽기·쓰기로 열리던 것이 모드와 맞아졌습니다.

MySQL에서 `sql("db.table")`은 `` `db`.`table` ``입니다. 1.4.3은 `db.table` 전체를 식별자 하나로 묶었습니다. `ON DUPLICATE KEY UPDATE`가 소문자이거나 여러 줄이어도 `sql({ age: 31 })`를 받을 수 있습니다. TLS 없이 `caching_sha2_password`는 비밀번호가 20바이트 이상이면 errno 1045였고, 빠른 인증이 되지 않아 매번 `allowPublicKeyRetrieval: true`가 필요했습니다. `sha256_password`는 문서에만 있고 로그인이 항상 실패했습니다. 지연이 30ms일 때 같은 연결의 prepared 쿼리 4개는 파이프라인이 되어 124ms에서 31ms입니다. 1.2.22 이후에는 한 번에 하나씩이었습니다.

SQLite 어댑터는 `select*from t`, `/*c*/select`, `values (...)`처럼 붙여 쓴 문을 빈 배열로 끝내지 않습니다. 여러 문을 한 문자열로 보내면 첫 문만 실행되던 것이, 마이그레이션 파일 전체를 실행합니다. 다른 `sql.begin()`이 돌고 있으면 중첩 오류 대신 기다리고, 실패한 `COMMIT`이 트랜잭션을 연 채로 두지 않습니다.

```ts
await sql`PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS notes (id INTEGER);`;
```

`bun:sqlite`도 옵션과 바인딩이 달라집니다.

```ts
import { Database } from "bun:sqlite";

const db = new Database("app.db", { create: false });
// 이미 있는 파일인데 1.4.3은 SQLITE_MISUSE. {} 와 { readonly: false }도 같았음
new Database("app.db", { readonly: true, create: true }); // TypeError

db.query("SELECT $name AS name").get({ name: "살구" });
// 1.4.3 기본 모드는 NULL. { $name: "살구" }는 strict: true에서 예외였음. 둘 다 동작

db.transaction(async () => {
  throw new Error("첫 await 전에 실패");
})();
// 이미 거절된 프로미스라 1.4.3은 커밋. 1.4.4는 롤백
```

`db.run("a; b; c")`와 `db.exec()`는 마지막 문 외의 런타임 오류를 삼키지 않고, 실패한 문에서 던진 뒤 나머지를 건너뜁니다. `break`로 `stmt.iterate()`를 빠져나가면 커서가 닫혀서, 이후 `DROP TABLE`이 `database table is locked`가 되지 않습니다. `changes`는 트리거·외래 키·FTS5 그림자 테이블의 행을 세지 않고 그 문 자신의 행만 셉니다. `stmt.run(new Date())`나 인자 하나로 넘긴 `Map`·`Promise`는 `NULL`로 묶이지 않고 `TypeError`입니다. `ALTER TABLE` 뒤에 남아 있는 `Statement`는 새 컬럼 이름으로 행을 돌려줍니다.

## bun:sqlite 함수와 훅

`db.function("add2", (a, b) => a + b)`로 SQL에서 부르는 JavaScript 함수를 정의합니다. `db.aggregate("add_all", { start: 0, step: (total, value) => total + value })`는 집계 함수입니다. `varargs`, `deterministic`, `directOnly`, `safeIntegers`를 받고, `inverse`가 있으면 윈도우 함수로 쓸 수 있습니다. 함수가 던진 오류는 그 쿼리의 `get()`·`all()`·`run()`으로 올라갑니다. 시그니처는 better-sqlite3에 맞춥니다.

`db.onInsert`, `db.onUpdate`, `db.onDelete`는 이 `Database`가 바꾸는 행마다 호출됩니다. `null`을 넣으면 훅이 빠집니다. `stmt.reader`는 행을 돌려주는 문, `stmt.readonly`는 데이터베이스를 직접 고치지 않는 문입니다. `db[Symbol.for("sqlite-type")]`은 `"bun:sqlite"`라서 `node:sqlite`의 `DatabaseSync`와 구분할 수 있습니다.

## ws를 npm 패키지처럼

`import "ws"`는 Bun 내장 구현입니다. 1.4.4에서 `ws` 8.22.0을 기준으로 다시 옮겼고, 프레임·마스킹·UTF-8 검사·압축은 네이티브입니다.

| npm `ws` on Node.js와 비교 | 1.4.4 | 이전 |
|---|---:|---:|
| `ws` 자체 테스트 271개 통과 | 234 | 94 |
| 클라이언트 스크립트 212개 중 출력이 같은 수 | 179 | 18 |
| 서버 스크립트 159개 중 출력이 같은 수 | 150 | 67 |

남은 실패는 `ws._receiver`, `generateMask`처럼 네이티브 구현에 없는 내부에 의존합니다.

`createWebSocketStream()`은 역압과 함께 구현되었습니다. 전에는 `Not supported yet in Bun`이었습니다. `agent`, `createConnection`, `lookup`을 존중해서 SOCKS 프록시 에이전트가 동작합니다. 커스텀 `agent`는 무시되고 직접 연결했습니다. `handshakeTimeout`, `closeTimeout`, `followRedirects`, `maxRedirects`, `origin`, `auth`, `protocolVersion`, `autoPong`, `skipUTF8Validation`이 조용히 무시되지 않습니다. `maxPayload`는 클라이언트와 서버 모두에서 적용되어, socket.io의 `maxHttpBufferSize`가 기대대로 동작합니다. 기본 `maxPayload`는 16MiB에서 100MiB입니다.

`WebSocketServer`는 `perMessageDeflate`로 압축을 협상하고, 아무 `Duplex`나 받습니다. `http2.createSecureServer({ allowHTTP1: true })`(`vite --https`), `server.emit("connection", socket)`(socket.io 스티키 세션), fastify `injectWS`가 여기 해당합니다. 서버 소켓에 `pause()`, `resume()`, `isPaused`가 있고 `instanceof WebSocket`입니다.

이벤트 의미가 Node.js와 같아진 부분:

- `'error'`의 인자는 `ErrorEvent`가 아니라 `Error`이고 `error.code`는 `"ECONNREFUSED"` 같은 코드입니다. 리스너가 없으면 미처리 예외입니다. 클라이언트와 서버 소켓마다 `'error'` 리스너가 필요합니다.
- `'close'`는 연결이 닫힐 때 상대의 코드와 이유로 발생합니다. 이유는 `Buffer`입니다. 코드 없는 `close()`는 1005, 실패한 연결은 1006입니다.
- `send()`는 `CONNECTING` 동안 던집니다.
- `ws` 서버에는 유휴 시간 제한, 자동 ping, 역압 제한이 없습니다. 1.4.3은 `Bun.serve` 기본값을 물려받아 유휴 120초, 104초마다 ping, 16MiB 제한이 있었습니다. 오래 쉬는 소켓이 끊기던 서버는 이제 끊기지 않습니다.

## 브라우저 WebSocket의 close

`close` 이벤트는 브라우저·Node.js처럼 연결이 실제로 닫힌 뒤에 발생합니다. 1.4.3은 `close()`를 호출하자마자 내가 보낸 코드와 `wasClean: true`로 발생해서, 서버가 무엇을 보냈는지와 무관했습니다.

```ts
ws.onclose = (event) => {
  event.code; // 서버가 보낸 코드. Close에 코드가 없으면 1005
  event.wasClean; // 서버가 Close 프레임을 안 보내면 false
};
```

코드 없는 `close()`는 빈 Close 프레임을 보냅니다. 양쪽 모두 1000이 아니라 1005를 보고하고, `Bun.serve`의 `close(ws, code)` 핸들러도 마찬가지입니다. 스크립트가 `ws.close()`로 끝나면 왕복이 한 번 더 있어야 프로세스가 끝납니다. 서버가 30초 안에 응답하지 않으면 `close`는 1006입니다. `BUN_CONFIG_WS_CLOSE_TIMEOUT`으로 그 시간을 바꾸고, `ws.terminate()`는 즉시 닫습니다.

실패한 연결(나쁜 핸드셰이크, TLS·프록시 오류, 프로토콜 오류, 너무 큰 메시지, 잘못된 UTF-8)은 `error` 다음 `close` 1006입니다. `event.error.code`가 새로 생겼습니다. 1.4.3은 `close`가 1002·1007·1009·1011·1015와 영문 이유로 발생하고 `wasClean: true`인 경우도 있었으며, `error`가 없는 경우가 많았습니다. `event.message`에 예전 `reason`의 설명이 더 구체적으로 들어갑니다. `open` 이후 TCP가 끊긴 것은 실패로 치지 않고 `close` 1006만 발생합니다.

`Bun.serve`의 소켓은 `ws.pause()`와 `ws.resume()`로 읽는 속도를 멈출 수 있습니다. 일시 정지 중에는 TCP 역압이 걸리고 유휴 시간 제한도 멈춥니다. `websocket.error`는 `(error, ws)`를 받습니다. 1.4.3은 오류만 받아서, 어느 소켓인지 모르고 타입에도 없었습니다. `message`를 포함한 모든 핸들러는 생략할 수 있습니다.

`wss://`를 `https://` 프록시로 연결할 때 `tls`는 프록시와 대상 서버 양쪽에 적용됐습니다. `proxy.tls`를 주면 프록시는 그 설정만, 대상은 `tls`만 씁니다.

```ts
const ws = new WebSocket("wss://example.com", {
  proxy: {
    url: "https://proxy.internal:8443",
    tls: { ca: Bun.file("./proxy-ca.pem") },
  },
});
```

`Bun.serve`에서 1MB 비ASCII 텍스트를 그대로 돌려주는 WebSocket은 CPU 명령이 72% 줄었습니다. Safari가 `perMessageDeflate: true` 서버의 압축 메시지에 1002로 연결을 끊던 것, `server.stop(true)`가 1006으로 닫던 것(이제 1001 "Server closed"), 유휴 시간 0이 약 256초 뒤에 끊던 것도 WebSocket 서버 수정에 들어 있습니다.
