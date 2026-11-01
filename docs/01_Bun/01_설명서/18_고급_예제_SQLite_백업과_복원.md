# 18. 고급 예제: SQLite 백업과 복원

예제 15·16은 SQLite에 데이터를 넣고 API로 꺼내는 방법을 다뤘습니다. 여기서는 운영 중 놓치기 쉬운 세 가지를 더 다룹니다.

1. JavaScript `number`로는 정확히 표현할 수 없는 큰 정수
2. 여러 줄을 가져오다 실패했을 때 전부 되돌리는 트랜잭션
3. 파일을 거치지 않고 데이터베이스 전체를 메모리 백업으로 만들고 읽기 전용으로 복원하기

| 예제 | 파일 | 실행 |
|---|---|---|
| 45. SQLite 백업과 복원 | `examples/45-sqlite-backup.ts` | `bun examples/45-sqlite-backup.ts` |

## 예제 45. SQLite 백업과 복원

`examples/45-sqlite-backup.ts` — 정산 원장처럼 큰 ID와 금액을 다루는 작은 프로그램입니다. 모든 데이터는 메모리에만 두므로 실행 뒤 파일을 남기지 않습니다.

```ts
import { Database } from "bun:sqlite";

type Ledger = { id: bigint; memo: string; won: bigint };

// safeIntegers를 켜면 SQLite INTEGER를 bigint로 받아 큰 금액·식별자가 조용히 틀어지지 않습니다.
using db = new Database(":memory:", { strict: true, safeIntegers: true });
db.run("CREATE TABLE ledger (id INTEGER PRIMARY KEY, memo TEXT NOT NULL, won INTEGER NOT NULL)");

const insert = db.query("INSERT INTO ledger (id, memo, won) VALUES ($id, $memo, $won)");
const importRows = db.transaction((rows: Ledger[]) => {
  for (const row of rows) insert.run(row);
  return rows.length;
});

const firstId = 9_007_199_254_740_993n;
console.log("1) 가져온 줄:", importRows([{ id: firstId, memo: "정산 시작", won: 1_200_000n }, { id: firstId + 1n, memo: "수수료", won: -35_000n }]));

const total = db.query("SELECT SUM(won) AS won FROM ledger").get() as { won: bigint };
console.log("2) 합계와 자료형:", total.won, typeof total.won);

try {
  importRows([{ id: firstId + 2n, memo: "임시 줄", won: 1n }, { id: firstId, memo: "중복", won: 1n }]);
} catch (error) {
  console.log("3) 잘못된 가져오기 취소:", (error as Error).message.includes("UNIQUE"));
}
const count = db.query("SELECT COUNT(*) AS count FROM ledger").get() as { count: bigint };
console.log("   남은 줄:", count.count);

const backup = db.serialize();
using restored = Database.deserialize(backup, { readonly: true, strict: true, safeIntegers: true });
const copied = restored.query("SELECT id, memo, won FROM ledger ORDER BY id").all() as Ledger[];
console.log("4) 메모리 백업:", backup.byteLength, "bytes / 복원한 첫 ID:", copied[0].id);
```

```bash
bun examples/45-sqlite-backup.ts
```

```text
1) 가져온 줄: 2
2) 합계와 자료형: 1165000n bigint
3) 잘못된 가져오기 취소: true
   남은 줄: 2n
4) 메모리 백업: 8192 bytes / 복원한 첫 ID: 9007199254740993n
```

### 세 가지 핵심

| 기능 | 하는 일 | 이 예제에서 확인하는 점 |
|---|---|---|
| `safeIntegers: true` | SQLite의 64비트 `INTEGER`를 `bigint`로 읽음 | `9007199254740993n`처럼 `number`의 안전 범위 밖인 ID도 그대로 유지 |
| `db.transaction(함수)` | 함수가 성공하면 commit, 오류면 rollback | 두 번째 INSERT가 중복되자 그보다 앞선 `임시 줄`도 남지 않음 |
| `db.serialize()` | 데이터베이스 전체를 `Buffer`로 만듦 | 파일 백업·암호화·네트워크 전송 전에 사용할 수 있는 바이트를 얻음 |
| `Database.deserialize()` | 바이트에서 새 DB를 복원 | 복원본에 `readonly: true`를 주어 원본 백업을 실수로 바꾸지 않음 |
| `using db` | 블록이 끝날 때 DB 연결을 닫음 | `close()`를 빼먹기 쉬운 짧은 도구와 테스트에서 특히 유용 |

`safeIntegers`를 켜면 모든 정수 결과가 `bigint`가 됩니다. 따라서 JSON 응답에 그대로 넣을 수 없습니다. API로 보낼 때는 문자열로 바꾸거나, 안전 범위 안인지 검사해 `number`로 바꾸세요.

```ts
const row = { id: 9_007_199_254_740_993n, won: 1_165_000n };
const json = JSON.stringify(row, (_key, value) => (typeof value === "bigint" ? value.toString() : value));
// {"id":"9007199254740993","won":"1165000"}
```

`serialize()` 결과는 SQLite 데이터베이스 바이트 자체입니다. 민감한 고객 데이터라면 백업 파일의 접근 권한을 제한하고, 전송하거나 보관할 때는 별도의 암호화를 적용해야 합니다. 운영 DB의 정기 백업에는 데이터베이스가 지원하는 백업 절차와 복구 훈련도 함께 준비하세요.
