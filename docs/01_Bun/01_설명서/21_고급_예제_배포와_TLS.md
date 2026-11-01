# 21. 고급 예제: 배포와 TLS

| 예제 | 파일 | 실행 |
|---|---|---|
| 50. 단일 실행 파일과 내장 자산 | `examples/50-standalone-assets.ts` | `bun examples/50-standalone-assets.ts` |
| 51. TLS HTTPS 서버 | `examples/51-tls-server.ts` | 인증서 설정 뒤 실행 |

## 예제 50. 단일 실행 파일과 내장 자산

`bun build --compile`은 Bun 런타임과 애플리케이션을 하나의 실행 파일로 만듭니다. `with { type: "file" }`로 import한 자산은 컴파일 결과에 포함되며, 실행 중에는 `Bun.file()`로 읽습니다.

```bash
bun build --compile --outfile dist/bun-assets examples/50-standalone-assets.ts
./dist/bun-assets
```

| API | 역할 |
|---|---|
| `Bun.isStandaloneExecutable` | 현재 프로세스가 `--compile` 결과인지 확인 |
| `Bun.embeddedFiles` | 실행 파일에 포함된 자산 Blob 목록 |
| `import ... with { type: "file" }` | 파일 내용을 바로 읽지 않고, 컴파일러가 추적할 파일 경로를 import |

실행 파일에 비밀값을 넣지 마세요. 포함된 자산은 실행 파일을 가진 사람이 추출할 수 있다고 가정해야 합니다.

```ts
// examples/50-standalone-assets.ts
import poemPath from "./data/poem.txt" with { type: "file" };

const poem = Bun.file(poemPath);
console.log("1) 단일 실행 파일인가:", Bun.isStandaloneExecutable);
console.log("2) 포함한 파일:", poem.name, " / 첫 줄:", (await poem.text()).split("\n")[0]);

if (Bun.isStandaloneExecutable) {
  console.log("3) 실행 파일에 포함된 자산 수:", Bun.embeddedFiles.length);
  console.log("   자산 정보:", Bun.embeddedFiles.map((file) => `${file.type || "unknown"} ${file.size}bytes`).join(", "));
} else {
  console.log("3) --compile로 빌드하면 Bun.embeddedFiles에서 포함된 자산을 볼 수 있습니다.");
}
```

## 예제 51. TLS HTTPS 서버

TLS는 `Bun.serve` 설정에 key와 certificate 내용을 전달해 켭니다. 예제는 인증서 경로가 없거나 `--serve`를 붙이지 않으면 서버를 시작하지 않습니다.

```bash
TLS_KEY=certs/localhost-key.pem \
TLS_CERT=certs/localhost-cert.pem \
bun examples/51-tls-server.ts --serve
```

개발용 인증서는 `mkcert`처럼 신뢰할 수 있는 도구로 만들고, 운영 인증서는 호스팅 플랫폼·리버스 프록시·인증 기관에서 자동 갱신하도록 구성하세요. 개인 키를 Git에 넣거나 문서에 복사하지 마세요.

```ts
// examples/51-tls-server.ts
const keyPath = Bun.env.TLS_KEY;
const certPath = Bun.env.TLS_CERT;

if (!keyPath || !certPath || !Bun.argv.includes("--serve")) {
  console.log("TLS_KEY와 TLS_CERT에 PEM 경로를 설정하고 --serve를 붙이면 HTTPS 서버를 시작합니다.");
  process.exit(0);
}

const server = Bun.serve({
  hostname: "127.0.0.1",
  port: Number(Bun.env.PORT ?? 3443),
  tls: { key: Bun.file(keyPath), cert: Bun.file(certPath) },
  fetch: () => Response.json({ ok: true, protocol: "https" }),
});

console.log(`HTTPS 서버: ${server.url}`);
```
