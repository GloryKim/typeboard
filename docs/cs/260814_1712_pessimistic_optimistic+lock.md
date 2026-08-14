# 비관적 락과 낙관적 락 — 개념과 Bun 내장 API 예제

| 항목 | 내용 |
|------|------|
| 주제 | Pessimistic Lock(비관적 락) vs Optimistic Lock(낙관적 락) |
| 예제 범위 | **npm 패키지 없음** — `bun:sqlite`, `Bun.sleep`, `Bun.file` 등 Bun 런타임 내장만 사용 |
| 전제 | Bun 설치 환경에서 `bun run` 으로 바로 실행 가능한 TypeScript 예제 |
| 작성일 | 2026-08-10 |

---

## 왜 이 문서가 필요한가

웹 서버, API, 배치 작업 어디서든 **같은 데이터를 동시에 여러 요청이 건드리면** 값이 깨진다. 대표적인 장면은 재고 차감, 계좌 잔액 이체, 좌석 예매, 쿠폰 사용 횟수 증가다. 두 명이 동시에 “마지막 1개”를 사려 할 때, 둘 다 “재고 1개”를 읽고 각각 1을 빼서 저장하면 결과 재고는 0이어야 하는데 1이 남거나, 반대로 음수가 되기도 한다. 이걸 **lost update(갱신 유실)** 라고 부른다.

이 문제를 막는 대표 전략이 **비관적 락**과 **낙관적 락**이다. 둘 다 “동시에 쓰지 못하게 하거나, 쓰려다 실패하게 만든다”는 목표는 같지만, **언제 막을지**, **충돌이 날 때 무엇을 희생하는지(대기 vs 재시도)** 가 다르다.

아래 예제는 DB 드라이버·ORM·Redis 같은 외부 라이브러리 없이, Bun이 기본으로 넣어 준 **`bun:sqlite`** 만으로 비관적·낙관적 패턴을 그대로 구현한다. SQLite는 단일 파일·단일 프로세스에서도 트랜잭션과 잠금으로 동시성 제어를 연습하기 좋고, Bun 문서에서도 `db.transaction()` 과 `.immediate` / `.exclusive` 같은 **잠금 수준을 골라 쓰는 API**를 공식 지원한다.

---

## 동시성 문제를 먼저 느끼기 — 락 없이 재고를 깎으면

비관적·낙관적 락을 이해하려면, **보호 장치 없이 읽고-계산하고-쓰기**를 반복했을 때 무엇이 깨지는지부터 봐야 한다.

두 번의 주문이 같은 상품 재고를 동시에 처리한다고 상상해 보자.

```text
초기 재고: 10

[요청 A] stock=10 을 읽음
[요청 B] stock=10 을 읽음   ← A가 아직 저장 전이라 B도 10을 본다
[요청 A] 10 - 1 = 9 를 저장
[요청 B] 10 - 1 = 9 를 저장   ← 실제로는 두 번 팔렸으니 8이어야 함
```

결과 재고 9. **한 번의 판매가 증발**했다.

Bun + `bun:sqlite` 로 “락 없이” 이 **패턴**을 코드로 옮기면 아래와 같다. 트랜잭션도 없고, 읽기와 쓰기 사이에 다른 작업이 끼어들 여지를 일부러 남긴다.

> **주의:** Bun 프로세스 **하나** 안에서 위 함수를 **순차 호출**하면, 두 번째 호출은 첫 UPDATE 이후 값을 읽기 때문에 재고 8처럼 “맞아 보일” 수 있다. **lost update는 진짜 동시성**(여러 프로세스·Worker·동시 HTTP 요청)에서 터진다. 아래 코드는 **위험한 read-modify-write 패턴** 자체를 보여 주는 것이고, 동시에 두 요청이 같은 snapshot을 읽으면 타임라인처럼 깨진다.

```typescript
import { Database } from "bun:sqlite";

const db = new Database(":memory:");
db.exec(`
  CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    stock INTEGER NOT NULL
  );
  INSERT INTO products (id, name, stock) VALUES (1, '키보드', 10);
`);

const readStock = db.query<{ stock: number }, [number]>(
  "SELECT stock FROM products WHERE id = ?"
);
const writeStock = db.query("UPDATE products SET stock = ? WHERE id = ?");

function sellWithoutLock(productId: number) {
  const row = readStock.get(productId);
  if (!row || row.stock <= 0) return false;

  // 실무: 다른 요청/프로세스가 이 틈에 같은 stock=10 을 읽을 수 있음
  Bun.sleepSync(1);

  writeStock.run(row.stock - 1, productId);
  return true;
}

// 순차 실행만으로는 lost update가 안 보일 수 있음 — 패턴의 위험성이 핵심
sellWithoutLock(1);
sellWithoutLock(1);
console.log(readStock.get(1)?.stock); // 순차면 8, 동시면 9 가능
```

`Bun.sleepSync` 는 Bun 내장으로, “읽은 뒤 쓰기 전에 잠깐 멈춤”을 흉내 낸다. **여러 Bun 워커·여러 API 요청·여러 서버**가 동시에 같은 row를 read-modify-write 하면 위 타임라인대로 **9** 같은 잘못된 재고가 저장된다. **락·버전 검사·원자적 UPDATE** 중 하나가 필요한 이유가 여기 있다.

---

## 비관적 락(Pessimistic Lock)이란

**“충돌이 날 것이다”** 고 가정하고, **데이터를 읽거나 수정하기 전에 미리 잠근다.** 다른 트랜잭션·요청은 그 자원을 쓰려면 **대기**하거나 **접근이 거부**된다.

비유하면 도서관에서 **인기 책 한 권**을 빌리려 할 때, “일단 내가 책장 앞에 서서 그 책을 손에 쥔 다음에 대출 처리한다”에 가깝다. 뒤 사람은 내가 처리 끝낼 때까지 기다린다.

SQL 세계에서는 `SELECT ... FOR UPDATE` 가 비관적 락의 대표다. PostgreSQL·MySQL 등에서 읽은 row에 **배타적 잠금**을 걸어, 같은 row를 다른 트랜잭션이 `FOR UPDATE` 하거나 `UPDATE`/`DELETE` 하려면 대기하게 만든다.

SQLite + Bun에서는 **`bun:sqlite`의 트랜잭션**과 **`BEGIN IMMEDIATE` / `BEGIN EXCLUSIVE`** 로 같은 취지를 구현한다.

| 구분 | 비관적 락 |
|------|-----------|
| 가정 | 충돌이 자주 난다 / 충돌 비용이 크다 |
| 타이밍 | **읽기 전·읽은 직후** 잠금 |
| 충돌 시 | 대기(큐) 또는 타임아웃 |
| 장점 | 정합성 확보가 직관적, “한 번에 한 명” 보장 |
| 단점 | 잠금 유지 시간만큼 **처리량(TPS) 하락**, 데드락·핫스팟 병목 |

`SELECT FOR UPDATE` 를 핫 row 전체에 걸면, 영상·블로그에서 말하듯 **처리량이 1/10 수준으로 떨어질 수 있다.** 숫자는 환경마다 다르지만 방향은 같다. **같은 row를 많은 요청이 동시에 건드리면** 결국 **직렬화**되기 때문이다.

---

## Bun `bun:sqlite` 로 비관적 락 구현하기

Bun 공식 SQLite 모듈은 `Database.transaction()` 으로 트랜잭션 함수를 만든다. 기본은 `BEGIN DEFERRED` 에 가깝고, **쓰기 잠금을 빨리 잡고 싶을 때** `.immediate()` 또는 `.exclusive()` 를 쓴다.

문서 요약:

- `transaction(fn)` — 성공 시 commit, 예외 시 rollback  
- `transaction(fn).deferred(...)` — `BEGIN DEFERRED`  
- `transaction(fn).immediate(...)` — `BEGIN IMMEDIATE` (reserved lock, 쓰기 잠금 선점에 유리)  
- `transaction(fn).exclusive(...)` — `BEGIN EXCLUSIVE` (가장 강한 배타)

재고 차감처럼 **읽고 → 검사하고 → 쓰기**를 한 덩어리로 묶을 때, 비관적 패턴은 **트랜잭션 안에서 row를 읽고 바로 UPDATE** 하는 것이다. SQLite는 PostgreSQL의 `FOR UPDATE` 문법은 없지만, **쓰기 트랜잭션 자체가 다른 쓰기를 직렬화**하므로 같은 파일 DB에서는 충분히 “비관적” 동작을 연습할 수 있다.

### 예제 — `transaction().immediate` 로 재고 1개씩 안전하게 차감

```typescript
import { Database } from "bun:sqlite";

const db = new Database(":memory:");
db.exec(`
  CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    stock INTEGER NOT NULL
  );
  INSERT INTO products (id, name, stock) VALUES (1, '키보드', 100);
`);

const selectForUpdate = db.query<{ stock: number }, [number]>(
  "SELECT stock FROM products WHERE id = ?"
);
const decreaseStock = db.query(
  "UPDATE products SET stock = stock - 1 WHERE id = ? AND stock >= 1"
);

const sellPessimistic = db.transaction((productId: number) => {
  const row = selectForUpdate.get(productId);
  if (!row) throw new Error("상품 없음");
  if (row.stock <= 0) return { ok: false as const, reason: "품절" };

  // 비즈니스 로직(할인 계산, 로그 등)을 트랜잭션 안에서 처리
  // 이 구간 동안 다른 immediate/exclusive 쓰기 트랜잭션은 대기
  Bun.sleepSync(2);

  const result = decreaseStock.run(productId);
  if (result.changes !== 1) return { ok: false as const, reason: "동시 차감 실패" };

  return { ok: true as const, remaining: row.stock - 1 };
}).immediate;

for (let i = 0; i < 5; i++) {
  console.log(sellPessimistic(1));
}

console.log("최종 재고:", db.query("SELECT stock FROM products WHERE id = 1").get());
// stock: 95
```

여기서 쓰는 Bun/SQLite 내장 포인트:

- `db.transaction(fn).immediate` — 쓰기 잠금을 **일찍** 잡는 비관적 성격  
- `decreaseStock.run()` 의 **`changes`** — 실제로 몇 row가 바뀌었는지 (0이면 조건 불일치)  
- `Bun.sleepSync` — 트랜잭션이 길어질수록 다른 요청 대기 시간도 길어짐을 체감

### 비관적 락이 “느려지는” 이유를 코드로 읽기

비관적 락의 비용은 **잠금을 쥔 채로 있는 시간**에 비례한다. 위 예제에서 `Bun.sleepSync(2)` 를 트랜잭션 **안**에 넣었다. 실무에서는 이 자리에 **외부 API 호출**, **무거운 계산**, **사용자 입력 대기** 같은 것이 들어가면 안 된다. FOR UPDATE 걸고 3초 동안 다른 서비스 기다리면, 그 3초 동안 같은 row를 건드리는 모든 요청이 줄을 선다.

정리하면 비관적 락은:

- **충돌이 잦고**, 잘못되면 **돈·재고·좌석**처럼 되돌리기 어려울 때  
- **트랜잭션을 짧게** 유지할 수 있을 때  
- **처리량보다 정합성**이 우선일 때  

잘 맞는다.

---

## 낙관적 락(Optimistic Lock)이란

**“대부분 충돌 안 날 것이다”** 고 가정한다. **읽을 때는 잠그지 않는다.** 대신 **쓸 때** “내가 읽었을 때와 데이터가 같은가?”를 검사하고, 같으면 commit, 다르면 **실패 처리 후 재시도**한다.

비유하면 회의실 예약 앱에서 **미리 방을 잠그지 않고**, “저장” 버튼을 누를 때 “방금까지도 이 시간대가 비어 있었니?”를 확인하는 방식이다. 그 사이 다른 사람이 예약했으면 **저장 실패 → 다시 조회 → 재시도**한다.

구현 패턴은 보통 **version(또는 updated_at) 컬럼**을 둔다.

```text
읽기:  id=1, stock=10, version=7
계산:  stock=9
쓰기:  UPDATE ... SET stock=9, version=8 WHERE id=1 AND version=7
       → changes=1 이면 성공 (내가 본 7에서 8로 내가 바꿨다)
       → changes=0 이면 누군가 먼저 version을 8로 올림 → 재시도
```

| 구분 | 낙관적 락 |
|------|-----------|
| 가정 | 충돌이 드물다 |
| 타이밍 | **쓸 때** 버전·조건 검사 |
| 충돌 시 | 실패 → 재시도 또는 사용자에게 “다시 시도” |
| 장점 | 읽기·대기 부담 적음, **충돌 적을 때 처리량 유리** |
| 단점 | 충돌 많으면 **재시도 폭주**, UX·서버 부하 |

---

## Bun `bun:sqlite` 로 낙관적 락 구현하기

외부 ORM 없이 **version 컬럼 + 조건부 UPDATE + `changes` 확인 + 재시도** 만으로 낙관적 락을 완성할 수 있다. 재시도 간격에는 Bun 내장 **`Bun.sleep(ms)`** (비동기) 또는 **`Bun.sleepSync(ms)`** (동기)를 쓴다.

### 예제 — version 컬럼과 재시도 루프

```typescript
import { Database } from "bun:sqlite";

const db = new Database(":memory:");
db.exec(`
  CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    stock INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 0
  );
  INSERT INTO products (id, name, stock, version) VALUES (1, '키보드', 100, 0);
`);

const readProduct = db.query<
  { id: number; stock: number; version: number },
  [number]
>("SELECT id, stock, version FROM products WHERE id = ?");

const updateOptimistic = db.query(`
  UPDATE products
  SET stock = ?, version = version + 1
  WHERE id = ? AND version = ? AND stock >= 1
`);

async function sellOptimistic(
  productId: number,
  maxRetries = 5
): Promise<{ ok: boolean; remaining?: number; reason?: string }> {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    const row = readProduct.get(productId);
    if (!row) return { ok: false, reason: "상품 없음" };
    if (row.stock <= 0) return { ok: false, reason: "품절" };

    const nextStock = row.stock - 1;

    // 읽은 뒤 쓰기 전에 다른 판매가 끼어들 수 있음 — 낙관적 락은 쓸 때 검증
    await Bun.sleep(1);

    const result = updateOptimistic.run(nextStock, productId, row.version);

    if (result.changes === 1) {
      return { ok: true, remaining: nextStock };
    }

    // version 불일치 = 다른 트랜잭션이 먼저 commit
    await Bun.sleep(attempt * 2);
  }

  return { ok: false, reason: "재시도 초과(충돌 과다)" };
}

async function main() {
  const results = await Promise.all([
    sellOptimistic(1),
    sellOptimistic(1),
    sellOptimistic(1),
  ]);
  console.log(results);
  console.log(readProduct.get(1));
}

main();
```

낙관적 락 예제에서 쓰는 Bun 내장만 정리:

- `bun:sqlite` 의 `query`, `run`, **`changes`**  
- `Bun.sleep` — 재시도 백오프(backoff)  
- npm 패키지·Redis 분산 락·Prisma 없음

`UPDATE ... WHERE id = ? AND version = ?` 가 **compare-and-swap** 역할을 한다. “내가 읽은 version일 때만 고친다”는 한 줄이 낙관적 락의 핵심이다.

### `updated_at` 대신 `version`을 쓰는 이유

타임스탬프로도 낙관적 검증을 할 수 있지만, **시계 오차**, **동일 밀리초 충돌**, **DB DEFAULT CURRENT_TIMESTAMP** 동작 차이 때문에 **정수 version** 이 더 흔하고 안전하다. Bun SQLite 예제에서도 `version INTEGER` 를 1씩 올리는 방식이 가장 직관적이다.

---

## 같은 문제, 다른 해법 — 원자적 UPDATE 한 방

락 “패턴”은 아니지만, 재고처럼 **연산 자체가 단순**하면 아래처럼 **한 문장 UPDATE** 로 lost update를 막을 수 있다. 이것도 `bun:sqlite` prepare/run 만으로 가능하다.

```typescript
import { Database } from "bun:sqlite";

const db = new Database(":memory:");
db.exec(`
  CREATE TABLE products (id INTEGER PRIMARY KEY, stock INTEGER NOT NULL);
  INSERT INTO products VALUES (1, 100);
`);

const atomicDecrease = db.query(`
  UPDATE products
  SET stock = stock - 1
  WHERE id = ? AND stock >= 1
`);

console.log(atomicDecrease.run(1).changes); // 1
console.log(atomicDecrease.run(1).changes); // 1
console.log(db.query("SELECT stock FROM products WHERE id = 1").get());
// { stock: 98 }
```

**읽기 → 계산 → 쓰기** 세 단계를 애플리케이션에서 하지 않고, DB가 `stock - 1` 을 **원자적으로** 처리한다.  
다만 “읽은 값으로 복잡한 비즈니스 규칙을 적용한 뒤 쓰기”가 필요하면 비관적·낙관적·트랜잭션 설계가 다시 필요해진다.

---

## 비관적 vs 낙관적 — 한눈에 비교

```text
                    비관적 락                    낙관적 락
─────────────────────────────────────────────────────────────────
마음가짐            충돌 날 거야                 충돌 잘 없을 거야
잠금 시점           읽기/처리 시작 전·중           쓰기 직전(조건 검사)
충돌 시             대기                         실패 → 재시도
Bun 구현            transaction().immediate      version + WHERE + changes
처리량(충돌 적음)   상대적으로 불리              유리
처리량(충돌 많음)   직렬화로 상한 낮음           재시도 경쟁으로 불리
트랜잭션 길이       짧아야 함(치명)              읽기 구간은 길어도 됨(쓰기만 짧으면)
대표 SQL            SELECT ... FOR UPDATE        UPDATE ... WHERE version = ?
```

---

## 실무에서 고르는 감각

**비관적 락을 먼저 검토할 때**

- 같은 row·같은 계좌·같은 좌석에 **초당 많은 요청**이 몰린다  
- 한 번 잘못되면 **금전·재고 불일치**처럼 복구 비용이 크다  
- 트랜잭션 안에서 **외부 I/O 없이** 밀리초 단위로 끝낼 수 있다  
- “한 명씩만 처리”가 비즈니스적으로 받아들여진다  

**낙관적 락을 먼저 검토할 때**

- 대부분의 요청이 **서로 다른 row** 를 건드린다  
- 충돌률이 **낮다**(예: 프로필 수정, 게시글 편집)  
- 읽기 비중이 크고, **락 대기**를 UX에 노출하고 싶지 않다  
- 실패 시 **재시도** 또는 “다른 사용자가 수정했습니다” 안내가 가능하다  

**둘 다 애매할 때**

- 재고 `- 1` 처럼 단순하면 **원자적 UPDATE**  
- Bun 단일 프로세스·단일 SQLite 파일이면 `transaction().immediate`  
- 여러 Bun 인스턴스·여러 서버면 SQLite 파일 공유만으로는 한계 → Postgres 등 + DB 락 또는 분산 조율(이 문서 범위 밖)

---

## Bun 예제만으로 재현하는 “동시에 두 번 팔기” 실험

`bun:sqlite` 는 **동기 API**다. 한 프로세스 안에서 `Promise.all` 로 여러 async 함수를 돌려도, SQLite 호출 자체는 순차적으로 실행될 수 있다. **진짜 OS 수준 동시성**을 보려면 같은 DB 파일을 **여러 Bun 프로세스**가 열거나 Worker를 쓰면 된다. 다만 개념 학습용으로는 **낙관적 UPDATE의 `changes === 0`** 과 **비관적 `immediate` 트랜잭션 직렬화**만으로도 충분하다.

아래는 **낙관적** 쪽에서 의도적으로 충돌을 유발하는 축소 실험이다. 같은 version을 읽은 두 로직이 동시에 쓰기를 시도하면 **하나만 성공**한다.

```typescript
import { Database } from "bun:sqlite";

const db = new Database(":memory:");
db.exec(`
  CREATE TABLE wallet (
    id INTEGER PRIMARY KEY,
    balance INTEGER NOT NULL,
    version INTEGER NOT NULL
  );
  INSERT INTO wallet VALUES (1, 1000, 0);
`);

const read = db.query<{ balance: number; version: number }, [number]>(
  "SELECT balance, version FROM wallet WHERE id = ?"
);

const write = db.query(`
  UPDATE wallet
  SET balance = ?, version = version + 1
  WHERE id = ? AND version = ?
`);

function tryDebit(amount: number, snapshot: { balance: number; version: number }) {
  const next = snapshot.balance - amount;
  return write.run(next, 1, snapshot.version).changes === 1;
}

const snap = read.get(1)!;

const first = tryDebit(100, snap);
const second = tryDebit(100, snap); // 같은 version으로 두 번째 시도 → 실패해야 정상

console.log({ first, second, wallet: read.get(1) });
// first: true, second: false
// balance: 900, version: 1
```

두 번째 `tryDebit` 는 **같은 snapshot(version=0)** 을 쓰므로 낙관적 충돌로 거절된다. 실제 서비스에서는 두 번째 요청이 **다시 read → retry** 해야 한다.

---

## 비관적 예제 — `exclusive` 트랜잭션으로 배치 작업 보호

재고 말고 **여러 row를 한꺼번에** 맞춰야 할 때도 비관적 트랜잭션이 쓰인다. Bun에서는 `.exclusive()` 로 더 강하게 잠글 수 있다.

```typescript
import { Database } from "bun:sqlite";

const db = new Database(":memory:");
db.exec(`
  CREATE TABLE ledger (
    id INTEGER PRIMARY KEY,
    account TEXT NOT NULL,
    amount INTEGER NOT NULL
  );
  INSERT INTO ledger (account, amount) VALUES ('A', 1000), ('B', 500);
`);

const sumByAccount = db.query<{ account: string; total: number }, []>(`
  SELECT account, SUM(amount) AS total FROM ledger GROUP BY account
`);

const insertEntry = db.query(
  "INSERT INTO ledger (account, amount) VALUES (?, ?)"
);

const rebalance = db.transaction(() => {
  const rows = sumByAccount.all();
  const diff =
    (rows.find((r) => r.account === "A")?.total ?? 0) -
    (rows.find((r) => r.account === "B")?.total ?? 0);

  if (diff > 0) {
    insertEntry.run("B", diff);
    insertEntry.run("A", -diff);
  }
}).exclusive;

rebalance();
console.log(sumByAccount.all());
```

`exclusive` 는 읽기·쓰기 모두에 가깝게 **배타**에 가깝게 동작한다. **짧은 배치·정산·마이그레이션**처럼 “중간 상태를 절대 보이면 안 되는” 작업에 맞춘다. 일반 API 요청마다 `exclusive` 를 쓰면 처리량이 급락할 수 있으니 **배치 전용**으로 생각하는 편이 낫다.

---

## 자주 하는 실수 — Bun/SQLite 기준

**FOR UPDATE만 붙였는데 트랜잭션이 길다**  
Postgres에서 흔한 패턴이지만, 트랜잭션 안에서 HTTP 호출·파일 처리·`Bun.sleep` 을 길게 하면 비관적 락이 그 시간만큼 유지된다. Bun 예제에서도 `sleep` 은 **데모용**이지 패턴이 아니다.

**낙관적 락인데 재시도 없음**  
`changes === 0` 일 때 그냥 500 에러만 내면 충돌률이 조금만 올라가도 실패율이 폭증한다. **지수 백오프 + 최대 재시도 횟수**(`Bun.sleep` 활용)가 필요하다.

**version을 UI에 노출하지 않음**  
클라이언트가 수정 폼을 열어 둔 동안 서버 version이 바뀌면 저장은 실패해야 한다. REST라면 `ETag` / `version` 필드를 내려주고, 저장 시 다시 보내게 한다.

**SQLite 파일 하나를 여러 서버가 NFS로 공유**  
Bun `bun:sqlite` 는 **로컬·엣지·단일 인스턴스**에 강하다. 다중 writer 서버는 Postgres + DB 수준 락/낙관적 컬럼을 검토한다.

**원자적 UPDATE로 될 문제에 비관적 락 남용**  
`stock = stock - 1` 만 필요한데 read-modify-write + `immediate` 를 쓰면 설계가 무거워진다.

---

## HTTP API 형태로 붙일 때의 골격 (Bun.serve + bun:sqlite)

외부 프레임워크 없이 Bun만 쓴다면 대략 이런 형태다. (라우팅은 `Bun.serve` 내장)

```typescript
import { Database } from "bun:sqlite";

const db = new Database("shop.db");
db.exec(`
  CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY,
    stock INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 0
  );
`);

const sellPessimistic = db.transaction((id: number) => {
  const row = db.query("SELECT stock FROM products WHERE id = ?").get(id) as
    | { stock: number }
    | null;
  if (!row || row.stock < 1) return Response.json({ ok: false }, { status: 409 });
  db.query("UPDATE products SET stock = stock - 1 WHERE id = ?").run(id);
  return Response.json({ ok: true, stock: row.stock - 1 });
}).immediate;

Bun.serve({
  port: 3000,
  routes: {
    "/products/:id/sell-pessimistic": (req) => {
      const id = Number(req.params.id);
      return sellPessimistic(id);
    },
  },
});
```

낙관적 API는 요청 body에 **`version`** 을 받아 `UPDATE ... WHERE id = ? AND version = ?` 후 `changes` 로 409/200을 나누면 된다.

---

## 마무리 정리

**비관적 락**은 Bun에서 `bun:sqlite` 의 **`db.transaction(fn).immediate()`** 또는 **`.exclusive()`** 로 “읽기부터 쓰기까지 한 트랜잭션 + 쓰기 잠금 선점”을 구현한다. 충돌을 **미리 막고 대기**시키므로 정합성은 강하지만, **잠금 시간 = 처리량 병목**이 된다.

**낙관적 락**은 **`version` 컬럼**과 **`UPDATE ... WHERE version = ?`**, 그리고 **`run().changes`** 로 “내가 본 상태일 때만 반영”을 구현한다. 잠금 대기는 없지만 **`changes === 0` 일 때 `Bun.sleep` 과 함께 재시도**하는 로직이 필수다.

둘 중 우열이 있는 게 아니라 **충돌 빈도·트랜잭션 길이·비즈니스 비용**에 따라 고른다. Bun만으로 연습할 때는 `:memory:` 또는 로컬 `*.db` 파일 + `bun:sqlite` 만으로 두 패턴을 모두 손에 익힐 수 있고, 프로덕션으로 갈수록 같은 개념을 Postgres의 `FOR UPDATE`·`version` 컬럼·원자적 SQL로 그대로 옮기면 된다.

---

## Bun 내장 API 빠른 참조 (본 문서 예제에서 사용한 것)

| API | 용도 |
|-----|------|
| `import { Database } from "bun:sqlite"` | SQLite DB 열기 |
| `Database.open(path)` / `new Database(path)` | 파일 또는 `:memory:` |
| `db.exec(sql)` | DDL·다중 문 |
| `db.query(sql)` | prepared statement |
| `.get()` / `.all()` / `.run()` | 조회·실행 |
| `.run()` 반환값 `.changes` | 낙관적 성공 여부 판정 |
| `db.transaction(fn)` | 트랜잭션 래핑 |
| `.immediate()` / `.exclusive()` / `.deferred()` | 잠금 수준 (비관적) |
| `Bun.sleep(ms)` | async 재시도 대기 |
| `Bun.sleepSync(ms)` | sync 지연 (데모용) |
| `Bun.serve({ routes })` | HTTP API 골격 |

npm install 없이 위 API만으로 비관적·낙관적 락의 **개념 → 실패 사례 → 구현 → 선택 기준**까지 연결해 두었다.
