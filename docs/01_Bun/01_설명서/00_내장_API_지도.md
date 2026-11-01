# 0. 내장 API 지도

Bun에 들어 있는 기능을 분야별로 모았습니다. "이런 일을 하고 싶은데 Bun에 있나?" 싶을 때 이 장에서 찾고, 오른쪽 열의 장과 예제로 가서 실행해 보세요. "대신하는 패키지"는 Node.js에서 같은 일을 할 때 흔히 설치하는 npm 패키지입니다. Bun에서는 설치하지 않아도 됩니다.

Bun 1.4.3의 `Bun` 객체에는 속성이 112개 있습니다(`Object.keys(Bun).length`). 그 밖에 `"bun"`, `bun:sqlite`, `bun:test`, `bun:ffi`, `bun:jsc` 모듈과 웹 표준 전역 객체(`fetch`, `Response`, `WebSocket`, `Worker` 등)가 있습니다.

## 실행과 환경

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 명령줄 인자, 환경 변수 | `Bun.argv`, `Bun.env`, `.env` 자동 로드 | `dotenv`, `minimist` | [4장](./04_%EC%BD%94%EB%93%9C_%EC%8B%A4%ED%96%89%EA%B3%BC_%EC%9E%90%EB%8F%99_%EC%9E%AC%EC%8B%A4%ED%96%89.md), [12장](./12_%EC%98%88%EC%A0%9C_%EC%8A%A4%ED%81%AC%EB%A6%BD%ED%8A%B8%EC%99%80_%ED%8C%8C%EC%9D%BC.md) 예제 1 |
| 파일 저장 시 다시 실행 | `bun --watch`, `bun --hot` | `nodemon` | [4장](./04_%EC%BD%94%EB%93%9C_%EC%8B%A4%ED%96%89%EA%B3%BC_%EC%9E%90%EB%8F%99_%EC%9E%AC%EC%8B%A4%ED%96%89.md) |
| 지금 실행 중인 파일, 버전, 스레드 | `Bun.main`, `import.meta.main`, `Bun.version`, `Bun.revision`, `Bun.isMainThread` | | [15장](./15_%EC%A4%91%EA%B8%89_%EC%98%88%EC%A0%9C_%EB%8D%B0%EC%9D%B4%ED%84%B0%EC%99%80_%EC%9C%A0%ED%8B%B8%EB%A6%AC%ED%8B%B0.md) 예제 24 |
| 프로그램 위치 찾기 | `Bun.which` | `which` | 15장 예제 24 |
| 모듈 경로 찾기 | `Bun.resolve`, `Bun.resolveSync`, `import.meta.resolve` | `resolve` | 15장 예제 24 |
| 파일 경로와 file URL 변환 | `Bun.pathToFileURL`, `Bun.fileURLToPath` | Node `url` | 15장 예제 24 |
| 기다리기, 시간 재기 | `Bun.sleep`, `Bun.sleepSync`, `Bun.nanoseconds` | | [12장](./12_%EC%98%88%EC%A0%9C_%EC%8A%A4%ED%81%AC%EB%A6%BD%ED%8A%B8%EC%99%80_%ED%8C%8C%EC%9D%BC.md) 예제 8, 15장 예제 24 |
| 버전 범위 비교 | `Bun.semver.satisfies`, `Bun.semver.order` | `semver` | 15장 예제 24 |
| 객체 깊은 비교 | `Bun.deepEquals`, `Bun.deepMatch` | `lodash.isequal` | 15장 예제 24 |
| 끝난 Promise 값 바로 꺼내기 | `Bun.peek` | | 15장 예제 24 |
| 실행 파일 하나로 만들기 | `bun build --compile`, `Bun.isStandaloneExecutable`, `Bun.embeddedFiles` | `pkg` | [10장](./10_%EB%B9%8C%EB%93%9C%EC%99%80_%EC%8B%A4%ED%96%89_%ED%8C%8C%EC%9D%BC.md), 21장 예제 50 |

## 파일과 입출력

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 파일 읽기·쓰기 | `Bun.file`, `Bun.write` | `fs-extra` | [6장](./06_%ED%8C%8C%EC%9D%BC%EA%B3%BC_%EC%85%B8_%EB%8B%A4%EB%A3%A8%EA%B8%B0.md), 12장 예제 2·3 |
| 파일 찾기 (와일드카드) | `Bun.Glob` | `glob`, `fast-glob` | 12장 예제 5 |
| 큰 파일 조금씩 쓰기·읽기 | `file.writer()`(FileSink), `file.stream()`, `file.slice()` | | 15장 예제 27 |
| 스트림을 한 번에 모으기 | `Bun.readableStreamToText`, `Bun.readableStreamToJSON`, `Bun.readableStreamToBytes`, `Bun.readableStreamToArrayBuffer`, `Bun.readableStreamToBlob`, `Bun.readableStreamToArray`, `Bun.readableStreamToFormData` | `get-stream` | 15장 예제 27 |
| 메모리에서 바이트 이어 붙이고 줄 찾기 | `Bun.ArrayBufferSink`, `Bun.concatArrayBuffers`, `Bun.indexOfLine`, `Bun.allocUnsafe` | | 15장 예제 27·43 |
| 파일을 메모리처럼 다루기 | `Bun.mmap` | `mmap-io` | 15장 예제 43 |
| 표준 입출력 | `Bun.stdin`, `Bun.stdout`, `Bun.stderr`, `console`의 `for await` | | 12장 예제 4, 15장 예제 43 |

## 셸과 프로세스

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 셸 명령 실행 (모든 OS) | `` $`명령` `` (Bun Shell) | `zx`, `execa`, `shelljs` | [6장](./06_%ED%8C%8C%EC%9D%BC%EA%B3%BC_%EC%85%B8_%EB%8B%A4%EB%A3%A8%EA%B8%B0.md), 12장 예제 6 |
| 다른 프로그램 실행 | `Bun.spawn`, `Bun.spawnSync` | `cross-spawn` | 12장 예제 7 |
| 가짜 터미널로 실행 | `Bun.Terminal`, `Bun.spawn({ terminal })` | `node-pty` | [17장](./17_%EC%A4%91%EA%B8%89_%EC%98%88%EC%A0%9C_%EB%8F%84%EA%B5%AC_%ED%94%84%EB%A1%9C%EC%84%B8%EC%8A%A4_%ED%85%8C%EC%8A%A4%ED%8A%B8.md) 예제 38 |
| 여러 CPU 코어 쓰기 | `Worker` | `piscina` | [16장](./16_%EC%A4%91%EA%B8%89_%EC%98%88%EC%A0%9C_%EC%84%9C%EB%B2%84%EC%99%80_%EB%84%A4%ED%8A%B8%EC%9B%8C%ED%81%AC.md) 예제 32 |
| 반복 예약 작업 | `Bun.cron`, `Bun.cron.parse`, `Bun.cron.remove` | `node-cron`, `cron-parser` | 16장 예제 34 |

## 웹 서버와 네트워크

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 웹 서버, 주소별 처리 | `Bun.serve({ routes })` | `express`, `fastify` | [7장](./07_%EC%9B%B9_%EC%84%9C%EB%B2%84_%EB%A7%8C%EB%93%A4%EA%B8%B0.md), [13장](./13_%EC%98%88%EC%A0%9C_%EC%9B%B9_%EC%84%9C%EB%B2%84.md) 예제 10·11 |
| 정적 파일, 캐시, 서버 교체 | 정적 `Response`, `{ dir }` 라우트, `server.reload`/`stop` | `serve-static`, 별도 프록시 설정 일부 | 16장 예제 44 |
| HTTPS·mTLS 서버 | `Bun.serve({ tls })`, `ca`, `requestCert` | `https`, 별도 TLS 서버 | 21장 예제 51, 23장 예제 54 (인증서 설정 후 실행) |
| WebSocket 서버 | `Bun.serve({ websocket })`, `subscribe`/`publish` | `ws`, `socket.io` | 13장 예제 13 |
| 실시간 알림 (SSE) | `ReadableStream` 응답 | | 13장 예제 14 |
| 파일 위치가 곧 주소 | `Bun.FileSystemRouter` | Next.js 라우터 | 16장 예제 30 |
| 쿠키 | `req.cookies`, `Bun.Cookie`, `Bun.CookieMap` | `cookie`, `cookie-parser` | 16장 예제 28 |
| CSRF 토큰 | `Bun.CSRF.generate`, `Bun.CSRF.verify` | `csrf-csrf` | 16장 예제 28 |
| 파일 업로드, 이어받기 | `req.formData()`, `new Response(Bun.file())` | `multer` | 16장 예제 29 |
| HTML·CSS·TS 함께 서빙 | HTML import, `development: true` | `vite` 개발 서버 | [14장](./14_%EC%98%88%EC%A0%9C_%EB%8D%B0%EC%9D%B4%ED%84%B0%EB%B2%A0%EC%9D%B4%EC%8A%A4_%ED%85%8C%EC%8A%A4%ED%8A%B8_%EB%B9%8C%EB%93%9C.md) 예제 19 |
| HTTP 요청 보내기 | `fetch`, `AbortSignal.timeout`, `FetchSession` | `axios`, `node-fetch`, `undici` | 13장 예제 12, 16장 예제 35 |
| 유닉스 소켓 | `Bun.serve({ unix })`, `fetch({ unix })` | | 16장 예제 35 |
| TCP·UDP | `Bun.listen`, `Bun.connect`, `Bun.udpSocket` | `net`, `dgram` | 16장 예제 31 |
| DNS 조회 | `Bun.dns.lookup`, `prefetch`, `getCacheStats` | | 16장 예제 31 |

## 데이터 형식

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| TOML | `TOML.parse`, `.toml` import | `@iarna/toml`, `smol-toml` | 15장 예제 20 |
| YAML | `YAML.parse`, `YAML.stringify`, `.yaml` import | `yaml`, `js-yaml` | 15장 예제 20 |
| 주석 있는 JSON | `JSONC.parse` | `jsonc-parser` | 15장 예제 20 |
| JSON5 | `JSON5.parse`, `JSON5.stringify` | `json5` | 15장 예제 20 |
| 한 줄에 JSON 하나 (JSONL) | `JSONL.parse`, `JSONL.parseChunk` | `ndjson` | 15장 예제 20 |
| XML | `XML.parse`, `XML.stringify` | `fast-xml-parser`, `xml2js` | 15장 예제 20 |
| Markdown → HTML·다른 형식 | `Bun.markdown.html`, `Bun.markdown.render` | `marked`, `markdown-it` | 15장 예제 25 |
| HTML 고치기·정보 뽑기 | `HTMLRewriter` | `cheerio` | 15장 예제 25 |
| 값 복제·직렬화 | `structuredClone`, `bun:jsc`의 `serialize`/`deserialize` | `v8.serialize` | 15장 예제 43 |

## 압축, 해시, 암호, ID

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| gzip·deflate | `Bun.gzipSync`, `gunzipSync`, `deflateSync`, `inflateSync`, `CompressionStream` | `pako` | 15장 예제 21 |
| zstd | `Bun.zstdCompress`, `Bun.zstdCompressSync`, `Bun.zstdDecompress`, `Bun.zstdDecompressSync` | `@mongodb-js/zstd` | 15장 예제 21 |
| tar 묶음 | `Bun.Archive` | `tar`, `archiver` | 15장 예제 21 |
| 빠른 해시 | `Bun.hash`(wyhash), `xxHash3`, `crc32`, `adler32` 등 | `xxhash-wasm`, `crc-32` | 15장 예제 22 |
| 암호 해시, HMAC | `Bun.CryptoHasher`, `Bun.sha`, `Bun.MD4`/`MD5`, `Bun.SHA1`/`SHA224`/`SHA256`/`SHA384`/`SHA512`/`SHA512_256`의 `hash` | | 12장 예제 9, 15장 예제 22·43 |
| 비밀번호 저장 | `Bun.password.hash`, `verify` (argon2id, bcrypt) | `bcrypt`, `argon2` | 12장 예제 9, 16장 예제 28 |
| UUID | `crypto.randomUUID`, `Bun.randomUUIDv7`, `Bun.randomUUIDv5` | `uuid` | 15장 예제 22 |
| 운영체제 비밀번호 보관함 | `Bun.secrets` | `keytar` | 17장 예제 40 |

## 데이터베이스

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| SQLite (동기) | `bun:sqlite`의 `Database`, `serialize`/`deserialize`, `loadExtension` | `better-sqlite3` | 14장 예제 15·16, 18장 예제 45, 23장 예제 53 |
| SQLite·PostgreSQL·MySQL (비동기) | `Bun.SQL`, `Bun.sql`, `Bun.postgres` | `pg`, `postgres`, `mysql2` | 16장 예제 33 (SQLite만 실행), 20장 예제 49 (외부 DB 연결 확인) |
| Redis | `Bun.redis`, `Bun.RedisClient` | `ioredis`, `redis` | 20장 예제 48 (설정 후 실행) |
| S3 저장소 | `Bun.s3`, `Bun.S3Client` | `@aws-sdk/client-s3` | 20장 예제 47 (설정 후 실행) |

## 터미널 출력

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 색 | `Bun.color`, `Bun.enableANSIColors` | `chalk`, `picocolors` | 15장 예제 23, 17장 예제 38 |
| 한글·이모지 폭 | `Bun.stringWidth` | `string-width` | 15장 예제 23 |
| 색 코드 지우기·자르기·줄 바꾸기 | `Bun.stripANSI`, `Bun.sliceAnsi`, `Bun.wrapAnsi` | `strip-ansi`, `slice-ansi`, `wrap-ansi` | 15장 예제 23 |
| 객체를 보기 좋게 | `Bun.inspect`, `Bun.inspect.table`, `Bun.inspect.custom` | `util.inspect` | 15장 예제 23 |
| HTML 이스케이프 | `Bun.escapeHTML` | `escape-html` | 15장 예제 23, 16장 예제 28 |

## 이미지와 브라우저

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 크기 변경, 회전, 형식 변환 | `Bun.Image`, `Bun.file().image()` | `sharp`, `jimp` | 15장 예제 26, 16장 예제 29 |
| 흐린 미리보기 | `image.placeholder()` | `blurhash`, `plaiceholder` | 15장 예제 26 |
| 클립보드 이미지·편집기 열기 | `Bun.Image.fromClipboard`, `Bun.openInEditor` | | 22장 예제 52 (명시적 플래그 필요) |
| 브라우저 조종, 스크린샷 | `Bun.WebView` | `playwright`, `puppeteer` | 17장 예제 41 |

## 개발 도구

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 타입 검사 | `bun check` | `tsc` | [9장](./09_%ED%83%80%EC%9E%85_%EA%B2%80%EC%82%AC.md) |
| 번들 | `Bun.build`, `bun build` | `esbuild`, `webpack`, `rollup` | [10장](./10_%EB%B9%8C%EB%93%9C%EC%99%80_%EC%8B%A4%ED%96%89_%ED%8C%8C%EC%9D%BC.md), 14장 예제 18 |
| 한 파일 변환, import 분석 | `Bun.Transpiler` (`transformSync`, `scan`, `scanImports`) | `@babel/core`, `es-module-lexer` | 17장 예제 36 |
| 새 파일 형식 import | `Bun.plugin`, `Bun.build({ plugins })` | esbuild 플러그인 | 17장 예제 37 |
| 빌드할 때 값 계산 | 매크로 (`with { type: "macro" }`) | `babel-plugin-macros` | 17장 예제 37 |
| C 함수 부르기 | `Bun.FFI`, `bun:ffi` (`dlopen`, `cc`) | `ffi-napi`, `node-gyp` | 17장 예제 39 |
| 메모리 측정, 누수 찾기 | `bun:jsc`의 `heapStats`, `Bun.gc`, `Bun.generateHeapSnapshot` | | 15장 예제 43 |
| 같은 앱 인스턴스 격리 | `Bun.ModuleGraph` | 별도 Worker·프로세스 일부 | 19장 예제 46 (실험적) |

## 테스트

| 하고 싶은 일 | Bun 기능 | 대신하는 패키지 | 어디서 |
|---|---|---|---|
| 테스트 실행, 비교 | `bun test`, `expect`, `test.each` | `jest`, `vitest` | [8장](./08_%ED%85%8C%EC%8A%A4%ED%8A%B8%ED%95%98%EA%B8%B0.md), 14장 예제 17 |
| 가짜 함수, 엿보기 | `mock`, `spyOn` | `sinon` | 8장, 17장 예제 42 |
| 모듈 바꿔 끼우기 | `mock.module` | `jest.mock`, `proxyquire` | 17장 예제 42 |
| 시각·타이머 조종 | `setSystemTime`, `jest.useFakeTimers` | `@sinonjs/fake-timers` | 17장 예제 42 |
| 스냅샷 | `toMatchSnapshot`, `--update-snapshots` | | 17장 예제 42 |
| 커버리지 | `bun test --coverage` | `c8`, `nyc` | 17장 예제 42 |

## 이 설명서에서 실행하지 않은 기능

아래 기능은 외부 서비스가 필요하거나, 이 설명서를 작성한 환경에서 확인할 수 없었거나, 초·중급 범위를 넘어서 예제를 만들지 않았습니다. 쓰기 전에 [Bun 공식 문서](https://bun.com/docs)를 확인하세요.

| 기능 | 하는 일 | 실행하지 않은 이유 |
|---|---|---|
| `Bun.s3`, `Bun.S3Client` | AWS S3, Cloudflare R2 등 S3 호환 저장소 읽기·쓰기. `s3.file("경로").text()` | 20장 예제 47에서 환경 변수 설정 후 실행 |
| `Bun.redis`, `Bun.RedisClient` | Redis 명령(`get`, `set`, `incr`, pub/sub) | 20장 예제 48에서 Redis 서버 주소 설정 후 실행 |
| `Bun.SQL`의 PostgreSQL·MySQL | 예제 33과 같은 문법으로 실제 DB 서버에 연결 | 20장 예제 49에서 DB 주소 설정 후 실행 |
| `Database.loadExtension`, `setCustomSQLite` | SQLite 네이티브 확장과 별도 SQLite 라이브러리 | 23장 예제 53에서 신뢰하는 절대 경로를 설정 후 실행. 임의 확장은 네이티브 코드를 실행함 |
| mTLS의 실제 클라이언트 인증 | 클라이언트 키·인증서를 통한 상호 TLS 인증 | 23장 예제 54에서 서버를 시작할 수 있으나, CA가 서명한 클라이언트 인증서는 환경마다 달라 자동 실행하지 않음 |
| `Bun.secrets`의 저장·읽기 | 예제 40 | 자동화 세션에서 키체인 확인 창을 띄울 수 없어 `ERR_SECRETS_INTERACTION_NOT_ALLOWED` |
| `Bun.shrink`, `Bun.unsafe`, `bun:jsc`의 나머지 | 엔진 내부 조정과 진단 | 일반 서비스 예제로는 부적절하여 제외. 안전한 사용 기준을 확인한 뒤 차후 업데이트 예정 |

## 1.4.3에서 확인한 주의점 모음

예제를 만들면서 문서나 예상과 다르게 동작한 것들입니다. 다음 버전에서 고쳐질 수 있습니다.

| 기능 | 동작 | 대신 이렇게 | 어디서 |
|---|---|---|---|
| `new Bun.Archive({ 경로: Bun.file() })` | 묶음 안의 파일이 0바이트 | `await Bun.file().bytes()`를 넣기 | 15장 예제 21 |
| `Bun.write(경로, archive)` | `compress: "gzip"`을 무시 | `Bun.write(경로, await archive.blob())` | 15장 예제 21 |
| `Bun.deflateSync(data, { windowBits })` | `windowBits`를 무시하고 항상 머리말 없는 DEFLATE | zlib 머리말과 adler32를 직접 붙이기 | 15장 예제 26 |
| `.size`를 읽은 `BunFile`의 `exists()` | 파일을 지운 뒤에도 `true` | `Bun.file(경로)`를 새로 만들어 확인 | 15장 예제 27 |
| `Bun.markdown.html`의 `headings: { ids: true }` | 한글 제목의 id가 `""` 또는 `"-1"` | 제목 id를 직접 만들기 | 15장 예제 25 |
| `.jsonl` 파일 import | 객체가 아니라 파일 경로가 들어옴 | `JSONL.parse(await Bun.file().text())` | 15장 예제 20 |
| `Bun.CSRF.verify("")` | `false`가 아니라 오류를 던짐 | 빈 토큰을 먼저 거르기 | 16장 예제 28 |
| 스트림 응답의 `Content-Length` | 헤더를 빼고 청크 전송 | 전체 크기를 다른 헤더로 알리기 | 16장 예제 35 |
| `Bun.Transpiler` (`define` 없음) | `process.env.NODE_ENV`를 `"development"`로 바꿔 넣음 | 배포용이면 `define`으로 `"production"` 지정 | 17장 예제 36 |
| 플러그인의 `loader: "object"` | `Bun.build`에서 기본값이 `exports` 객체 전체가 됨 | `contents: "export default ..."`, `loader: "js"` | 17장 예제 37 |
| `WebView`의 `type()` | `keydown` 이벤트 없음 | `<form>`의 `submit`으로 Enter 받기 | 17장 예제 41 |
| `WebView`의 `title` | `navigate()` 직후 빈 문자열 | `evaluate("document.title")` | 17장 예제 41 |
| `Bun.color(색, "ansi")` | 터미널이 아니거나 `NO_COLOR`면 빈 문자열 | 항상 색이 필요하면 `"ansi-16m"` | 15장 예제 23 |

## 모듈 이름 정리

| 가져오는 곳 | 들어 있는 것 |
|---|---|
| 전역 `Bun` | 이 장의 `Bun.*` 기능 대부분 |
| `"bun"` | `$`(셸), `SQL`, `YAML`, `TOML`, `JSON5`, `JSONC`, `JSONL`, `XML`, `secrets`, `Glob` 등. `Bun.*`와 같은 것을 이름으로 가져오기 |
| `bun:sqlite` | `Database` (동기 SQLite) |
| `bun:test` | `test`, `expect`, `mock`, `spyOn`, `setSystemTime`, `jest` 등 |
| `bun:ffi` | `dlopen`, `cc`, `ptr`, `CString` 등 |
| `bun:jsc` | 엔진(JavaScriptCore) 진단: `heapStats`, `serialize`, `estimateShallowMemoryUsageOf` 등 |
| `node:*` | Node.js 내장 모듈(`node:fs`, `node:path`, `node:crypto` …). Bun이 대부분 지원 |
| 전역 웹 표준 | `fetch`, `Request`, `Response`, `URL`, `WebSocket`, `Worker`, `crypto`, `CompressionStream`, `HTMLRewriter`, `structuredClone` 등 |
