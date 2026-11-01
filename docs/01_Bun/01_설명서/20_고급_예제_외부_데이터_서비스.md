# 20. 고급 예제: 외부 데이터 서비스

이 장은 Bun이 패키지 없이 제공하는 S3 호환 스토리지, Redis/Valkey, PostgreSQL·MySQL 연결을 다룹니다. 계정·서버가 필요한 기능이므로, 환경 변수가 없으면 예제는 안내를 출력하고 안전하게 끝납니다.

| 예제 | 파일 | 필요한 환경 변수 |
|---|---|---|
| 47. S3 호환 스토리지 | `examples/47-s3-storage.ts` | `S3_BUCKET`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY` |
| 48. Redis/Valkey | `examples/48-redis.ts` | `REDIS_URL` |
| 49. 외부 SQL | `examples/49-external-sql.ts` | `DATABASE_URL` |

## 예제 47. S3 호환 스토리지

`Bun.S3Client`는 AWS S3, Cloudflare R2, MinIO처럼 S3 API를 지원하는 저장소에 공통으로 연결합니다. 예제는 기본으로 파일을 바꾸지 않고, 60초짜리 다운로드 URL만 만듭니다. 실제 파일을 읽으려면 `--read`를 붙입니다.

```bash
S3_BUCKET=my-bucket \
S3_ACCESS_KEY_ID=... \
S3_SECRET_ACCESS_KEY=... \
S3_ENDPOINT=https://... \
bun examples/47-s3-storage.ts --read
```

| API | 역할 |
|---|---|
| `new S3Client(설정)` | 자격 증명·버킷·S3 호환 endpoint를 묶음 |
| `client.file(키)` | 아직 네트워크 요청을 보내지 않는 원격 파일 참조 |
| `file.presign()` | 네트워크 요청 없이 제한 시간이 있는 URL 생성 |
| `Bun.write(file, 내용)` | 업로드. 이 예제에서는 의도적으로 호출하지 않음 |

```ts
// examples/47-s3-storage.ts
import { S3Client } from "bun";

const bucket = Bun.env.S3_BUCKET;
const accessKeyId = Bun.env.S3_ACCESS_KEY_ID;
const secretAccessKey = Bun.env.S3_SECRET_ACCESS_KEY;

if (!bucket || !accessKeyId || !secretAccessKey) {
  console.log("S3_BUCKET, S3_ACCESS_KEY_ID, S3_SECRET_ACCESS_KEY를 설정하면 실행합니다.");
  process.exit(0);
}

const s3 = new S3Client({ bucket, accessKeyId, secretAccessKey, endpoint: Bun.env.S3_ENDPOINT });
const file = s3.file("bun-report/health.json");
const url = file.presign({ expiresIn: 60 });
console.log("1) 60초 다운로드 URL 생성:", new URL(url).hostname);

if (Bun.argv.includes("--read")) console.log("2) health.json:", await file.text());
else console.log("2) 읽기는 --read를 붙여 실행합니다.");
```

## 예제 48. Redis/Valkey

Redis 예제는 충돌하지 않는 UUID 키만 만들고, `finally`에서 바로 지웁니다. 서비스 데이터에 영향을 주지 않습니다.

```bash
REDIS_URL=redis://localhost:6379 bun examples/48-redis.ts
```

| API | 역할 |
|---|---|
| `new RedisClient(url)` | 독립 연결 생성 |
| `connect()` | 서버 연결 확인 |
| `set(key, value, "EX", 초)` | 만료 시간을 가진 키 저장 |
| `ttl(key)` | 남은 만료 시간 확인 |
| `close()` | 연결 종료 |

구독(Pub/Sub)은 일반 명령을 보내는 연결과 분리하세요. 구독한 클라이언트는 구독 명령 계열만 처리하므로, 발행·조회용으로는 별도 `RedisClient`를 만듭니다.

```ts
// examples/48-redis.ts
import { RedisClient } from "bun";

const url = Bun.env.REDIS_URL;
if (!url) {
  console.log("REDIS_URL을 설정하면 실행합니다. 예: redis://localhost:6379");
  process.exit(0);
}

const redis = new RedisClient(url);
const key = `bun-report:example:${crypto.randomUUID()}`;
try {
  await redis.connect();
  await redis.set(key, "hello", "EX", 60);
  console.log("1) 임시 키 값:", await redis.get(key));
  console.log("2) 남은 TTL(초):", await redis.ttl(key));
} finally {
  await redis.del(key).catch(() => 0);
  redis.close();
}
```

## 예제 49. 외부 SQL

예제 33은 SQLite 메모리 DB로 `Bun.SQL` 문법을 연습했습니다. 이 예제는 `DATABASE_URL`을 사용해 PostgreSQL 또는 MySQL에 **읽기 전용** 연결 확인만 합니다.

```bash
DATABASE_URL=postgres://user:password@host:5432/app bun examples/49-external-sql.ts
```

운영 DB에는 예제 계정이 아닌 최소 권한 계정을 쓰고, 연결 문자열을 Git·문서·브라우저에 남기지 마세요. TLS가 필요한 서비스는 연결 문자열과 SQL 옵션에서 인증서를 설정합니다.

```ts
// examples/49-external-sql.ts
import { SQL } from "bun";

const url = Bun.env.DATABASE_URL;
if (!url) {
  console.log("DATABASE_URL을 설정하면 PostgreSQL 또는 MySQL 연결을 확인합니다.");
  process.exit(0);
}

const sql = new SQL(url);
try {
  const rows = await sql`SELECT 1 AS connected`;
  console.log("연결 확인:", rows[0]);
} finally {
  await sql.close();
}
```
