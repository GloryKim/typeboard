# SELECT FOR UPDATE와 처리량 — 비관적 락 vs 낙관적 락

> 보고서 출처 영상: [SELECT FOR UPDATE를 걸었더니 처리량이 10분의 1로 떨어졌습니다 (비관적 락 vs 낙관적 락)](https://www.youtube.com/shorts/tm-qpZMP8uk)  
> 채널: [2분코딩](https://www.youtube.com/@2%EB%B6%84%EC%BD%94%EB%94%A9)  
> 이 문서는 영상 주제(비관적 락으로 처리량이 급감한 현상, 낙관적 락과의 대비)를 CS 관점에서 정리하고, **예제는 전부 Rust**로 작성합니다.  
> 양식: **개념 → 코드 → 설명 → 체크포인트**

---

## 목차

1. [영상에서 던진 문제](#1-영상에서-던진-문제)
2. [동시성 제어가 필요한 이유](#2-동시성-제어가-필요한-이유)
3. [비관적 락 (Pessimistic Lock)](#3-비관적-락-pessimistic-lock)
4. [왜 SELECT FOR UPDATE 후 처리량이 급감하나](#4-왜-select-for-update-후-처리량이-급감하나)
5. [낙관적 락 (Optimistic Lock)](#5-낙관적-락-optimistic-lock)
6. [Rust로 비교 — 인메모리 모델](#6-rust로-비교--인메모리-모델)
7. [Rust + SQLx — SELECT FOR UPDATE](#7-rust--sqlx--select-for-update)
8. [Rust + SQLx — version 컬럼 낙관적 락](#8-rust--sqlx--version-컬럼-낙관적-락)
9. [원자적 UPDATE — 긴 읽기-수정-쓰기 윈도우를 없애기](#9-원자적-update--긴-읽기-수정-쓰기-윈도우를-없애기)
10. [선택 기준](#10-선택-기준)
11. [자주 하는 실수](#11-자주-하는-실수)
12. [체크리스트](#12-체크리스트)

---

## 1. 영상에서 던진 문제

### 개념

Shorts 제목의 핵심 주장:

> `SELECT ... FOR UPDATE`를 걸었더니 **처리량(throughput)이 약 1/10**으로 떨어졌다.  
> 원인 축은 **비관적 락 vs 낙관적 락** 선택이다.

해석:

| 관찰 | 의미 |
|---|---|
| FOR UPDATE 적용 | 읽은 row를 트랜잭션 끝날 때까지 **배타적으로 잠금** |
| 처리량 급감 | 같은 핫 row에 요청이 몰리면 **대기 큐**가 생김 → TPS 직렬화 |
| “10분의 1” | 특정 벤치 수치. 환경·트랜잭션 길이·충돌률에 따라 달라짐. **방향성**이 중요 |

영상이 경고하는 지점:

- “동시성 문제 있으니 일단 FOR UPDATE”는 쉬워 보이지만,
- **충돌이 드문 경로**에까지 걸면 락 오버헤드만 사고,
- **충돌이 많은 핫스팟**에서는 안정적일 수 있지만 TPS 상한이 락 보유 시간에 묶인다.

### 체크포인트

- [ ] FOR UPDATE = 비관적 락의 대표 SQL임을 말할 수 있는지
- [ ] 처리량 하락이 “버그”가 아니라 **직렬화 비용**일 수 있음을 이해했는지

---

## 2. 동시성 제어가 필요한 이유

### 개념

두 요청이 같은 재고/잔액을 **읽고 → 계산 → 쓰기** 하면 lost update가 난다.

```text
T1: stock=10 읽음
T2: stock=10 읽음
T1: 10-1=9 쓰기
T2: 10-1=9 쓰기   ← 둘 다 팔렸는데 재고는 9 (실제로는 8이어야 함)
```

해결 축:

1. **비관적**: 읽는 순간 잠가서, 같은 row를 **쓰려는(또는 FOR UPDATE로 읽으려는)** 다른 트랜잭션을 대기시킴  
   (PostgreSQL 등 MVCC에서는 일반 `SELECT`는 보통 막히지 않고 스냅샷을 본다)  
2. **낙관적**: 잠그지 않고 쓸 때 “내가 읽은 버전인가?” 검사  
3. **원자적 SQL**: `UPDATE ... SET stock = stock - 1 WHERE stock >= 1` 한 방에

### Rust로 재현 (보호 없음)

```rust
/// 논리만 보여주는 lost update (싱글스레드)
fn lost_update_logic() {
    let mut stock = 10;
    let a = stock; // 10
    let b = stock; // 10
    stock = a - 1; // 9
    stock = b - 1; // 9  ← 두 번 팔렸는데 재고는 9 (8이어야 함)
    assert_eq!(stock, 9);
}
```

동시 스레드/프로세스에서는 같은 “읽고 → 계산 → 쓰기”를 `Mutex` / DB 락 / CAS / 원자적 UPDATE 없이 하면 깨진다.  
참고: `AtomicI32`의 `load` 후 계산 후 `store`도 CAS(`compare_exchange` / `fetch_sub`)가 아니면 lost update가 난다.

### 체크포인트

- [ ] lost update 시나리오를 타임라인으로 그릴 수 있는지
- [ ] FOR UPDATE가 “모든 읽기를 막는다”가 아님(MVCC)을 구분

---

## 3. 비관적 락 (Pessimistic Lock)

### 개념

**“충돌할 것이다”** 가정. 자원에 손대기 **전에** 잠근다.

SQL 대표:

```sql
BEGIN;
SELECT id, stock FROM products WHERE id = $1 FOR UPDATE;
-- 여기서부터 같은 row 에 대한 다른 FOR UPDATE / UPDATE / DELETE 는 대기
-- (일반 SELECT 는 MVCC 스냅샷으로 대개 대기하지 않음)
UPDATE products SET stock = stock - 1 WHERE id = $1;
COMMIT;  -- 락 해제
```

특징:

| | |
|---|---|
| 충돌 처리 | 쓰기를 **대기**시켜 직렬화 (충돌을 “실패”가 아니라 “줄 세우기”) |
| 구현 부담 | 앱 재시도가 상대적으로 단순 |
| 비용 | 락 대기, 커넥션 점유, 데드락 가능 |
| 잘 맞는 곳 | 재고·잔액 등 **같은 row 경쟁이 잦고** 트랜잭션이 **짧은** 경우 |

PostgreSQL에서 `FOR UPDATE`는 선택한 row에 row-level exclusive lock을 건다.  
트랜잭션이 길수록(외부 API 호출을 트랜잭션 안에 넣으면) 락 보유 시간이 늘어 **처리량이 더 크게** 깎인다.

### 체크포인트

- [ ] FOR UPDATE 락이 **커밋/롤백까지** 유지됨을 알기
- [ ] 트랜잭션 안에 느린 I/O를 넣으면 안 되는 이유

---

## 4. 왜 SELECT FOR UPDATE 후 처리량이 급감하나

### 개념

같은 `product_id`에 초당 N개 요청이 오면:

```text
요청1 ──[락 보유 t초]──▶ commit
요청2 ........ 대기 ......──[락]──▶
요청3 .............. 대기 ........──[락]──▶
```

이상적으로 TPS 상한 ≈ `1 / (평균 락 보유 시간)`.

- 락 보유 10ms → 이론상 같은 row당 ~100 TPS  
- 락 보유 100ms → ~10 TPS  

“원래 락 없이(또는 낙관적으로) 돌리던 경로”에 FOR UPDATE를 끼우면:

1. 병렬이던 작업이 **한 줄로 섬**  
2. 커넥션 풀이 대기 트랜잭션으로 가득 참  
3. 전체 API 처리량이 체감으로 크게 떨어짐 → 영상처럼 “1/10”로 보일 수 있음

**중요:** 처리량 하락 ≠ 틀린 구현. **핫 row를 직렬화한 대가**다.  
다만 **충돌이 거의 없는 데이터**에까지 걸면 대가에 비해 이득이 없다.

### Rust로 “직렬화 병목” 감각

```rust
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::thread;

fn pessimistic_throughput_feel() {
    let stock = Arc::new(Mutex::new(10_000i32));
    let start = Instant::now();
    let mut handles = vec![];

    for _ in 0..8 {
        let stock = Arc::clone(&stock);
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                let mut g = stock.lock().unwrap(); // 비관적: 임계구역 진입 = 락
                // 짧은 임계구역이라도, 경쟁이 심하면 대기 누적
                *g -= 1;
                // 만약 여기서 sleep 하면(=느린 작업) 처리량은 더 무너짐
                // thread::sleep(Duration::from_millis(1));
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    eprintln!("elapsed = {:?}", start.elapsed());
    eprintln!("stock = {}", *stock.lock().unwrap());
}
```

`sleep`을 임계구역에 넣는 순간이 DB에서 “FOR UPDATE 후 외부 HTTP”와 같은 실수다.

### 체크포인트

- [ ] 핫 row TPS ≈ 1/락보유시간 관계를 설명
- [ ] 락 안에 외부 호출 금지

---

## 5. 낙관적 락 (Optimistic Lock)

### 개념

**“충돌은 드물다”** 가정. 읽을 때는 안 잠그고, **쓸 때** 충돌을 검사한다.

대표 구현 — `version` 컬럼:

```sql
SELECT id, stock, version FROM products WHERE id = $1;
-- 앱에서 stock' = stock - 1 계산

UPDATE products
SET stock = $2, version = version + 1
WHERE id = $1 AND version = $3;
-- affected rows == 0 이면 다른 트랜잭션이 먼저 커밋한 것 → 재시도 또는 실패
```

특징:

| | |
|---|---|
| 충돌 감지 시점 | **쓰기 시점** |
| 장점 | 락 대기 없음 → 충돌 적을 때 처리량·확장성 유리 |
| 단점 | 재시도 로직 필요. 충돌 많으면 **재시도 폭풍**으로 오히려 느려짐 |
| 잘 맞는 곳 | 읽기 많음, 같은 row 동시 쓰기 드묾 |

영상 맥락에서 “FOR UPDATE 때문에 느려졌다”면, 그 경로의 실제 충돌률이 낮아 **낙관적(또는 원자적 UPDATE)** 이 맞았을 가능성이 있다.  
반대로 선착순 재고처럼 충돌이 폭발하면 낙관적 재시도가 더 아플 수 있다.

### 체크포인트

- [ ] version 불일치 = `rows_affected == 0`
- [ ] 낙관적은 “락 없음”이지 “동시성 문제 없음”이 아님

---

## 6. Rust로 비교 — 인메모리 모델

DB 없이 두 전략의 차이를 코드로 고정한다.

### 비관적 — `Mutex`

```rust
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct Account {
    balance: i64,
}

fn withdraw_pessimistic(account: &Mutex<Account>, amount: i64) -> Result<(), &'static str> {
    let mut guard = account.lock().unwrap(); // SELECT FOR UPDATE 에 대응
    if guard.balance < amount {
        return Err("insufficient");
    }
    guard.balance -= amount;
    Ok(()) // drop(guard) == COMMIT 시 락 해제에 대응
}

fn demo_pessimistic() {
    let account = Arc::new(Mutex::new(Account { balance: 100 }));
    withdraw_pessimistic(&account, 30).unwrap();
    assert_eq!(account.lock().unwrap().balance, 70);
}
```

### 낙관적 — version + CAS 루프

```rust
#[derive(Clone, Debug)]
struct VersionedBalance {
    balance: i64,
    version: u64,
}

struct OptimisticStore {
    /// DB 낙관적 락 비유용.
    /// 저장소 자체는 Mutex 로 두었지만, "읽은 version 이 그대로일 때만 커밋"이 핵심이다.
    /// (진짜 락-프리 읽기는 ArcSwap 등으로 스냅샷을 바꾸는 쪽에 가깝다.)
    inner: std::sync::Mutex<VersionedBalance>,
}

impl OptimisticStore {
    fn new(balance: i64) -> Self {
        Self {
            inner: std::sync::Mutex::new(VersionedBalance { balance, version: 0 }),
        }
    }

    fn withdraw(&self, amount: i64) -> Result<(), &'static str> {
        loop {
            let snapshot = self.inner.lock().unwrap().clone();
            if snapshot.balance < amount {
                return Err("insufficient");
            }
            let next = VersionedBalance {
                balance: snapshot.balance - amount,
                version: snapshot.version + 1,
            };

            let mut guard = self.inner.lock().unwrap();
            if guard.version != snapshot.version {
                // 다른 갱신이 먼저 들어옴 → 재시도 (낙관적 충돌)
                continue;
            }
            *guard = next;
            return Ok(());
        }
    }
}
```

차이는 DB에서도 같다: 비관적은 **대기**, 낙관적은 **실패 후 재시도**.  
위 인메모리 예는 version 검사 로직 학습용이며, DB처럼 “읽기 구간 row 락이 아예 없다”와 동일하지는 않다.

### 체크포인트

- [ ] Mutex 임계구역 = FOR UPDATE 트랜잭션 구간으로 대응시켜 보기
- [ ] version 불일치 시 `continue`가 SQL 재시도에 대응함을 알기

---

## 7. Rust + SQLx — SELECT FOR UPDATE

PostgreSQL + `sqlx` 예시. (실행에는 DB 필요)

```toml
[dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

```rust
use sqlx::{PgPool, Postgres, Transaction};

pub async fn reserve_stock_pessimistic(
    pool: &PgPool,
    product_id: i64,
    qty: i32,
) -> anyhow::Result<()> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    // 비관적 락: 이 row 를 트랜잭션 끝까지 잠금
    let row = sqlx::query_as::<_, (i32,)>(
        "SELECT stock FROM products WHERE id = $1 FOR UPDATE",
    )
    .bind(product_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow::anyhow!("product not found"))?;

    let stock = row.0;
    if stock < qty {
        // 함수가 Err 로 return 되면 tx 가 drop → 롤백 → 락 해제
        anyhow::bail!("out of stock");
    }

    sqlx::query("UPDATE products SET stock = stock - $1 WHERE id = $2")
        .bind(qty)
        .bind(product_id)
        .execute(&mut *tx)
        .await?;

    // 느린 작업은 절대 여기(커밋 전)에서 하지 말 것
    // let _ = reqwest::get("https://example.com").await;

    tx.commit().await?;
    Ok(())
}
```

### 설명

- `FOR UPDATE` 이후 ~ `commit` 사이가 병목 구간이다.  
- 영상에서 처리량이 무너진 전형적 원인: 이 구간이 길거나, 한 상품에 트래픽이 몰림.  
- `FOR UPDATE NOWAIT` / `SKIP LOCKED` 변형으로 대기 대신 즉시 실패·다른 row 선택도 가능하다 (큐 워커 패턴).

```rust
// 락을 못 받으면 즉시 에러 (대기하지 않음)
// SELECT stock FROM products WHERE id = $1 FOR UPDATE NOWAIT;
```

### 체크포인트

- [ ] `begin` → `FOR UPDATE` → `UPDATE` → `commit` 순서 암기
- [ ] 커밋 전 외부 HTTP 금지

---

## 8. Rust + SQLx — version 컬럼 낙관적 락

스키마:

```sql
CREATE TABLE products (
  id BIGSERIAL PRIMARY KEY,
  stock INT NOT NULL,
  version BIGINT NOT NULL DEFAULT 0
);
```

```rust
use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow)]
struct Product {
    id: i64,
    stock: i32,
    version: i64,
}

pub async fn reserve_stock_optimistic(
    pool: &PgPool,
    product_id: i64,
    qty: i32,
) -> anyhow::Result<()> {
    const MAX_RETRY: usize = 8;

    for attempt in 0..MAX_RETRY {
        let product = sqlx::query_as::<_, Product>(
            "SELECT id, stock, version FROM products WHERE id = $1",
        )
        .bind(product_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("product not found"))?;

        if product.stock < qty {
            anyhow::bail!("out of stock");
        }

        let result = sqlx::query(
            r#"
            UPDATE products
            SET stock = $1, version = version + 1
            WHERE id = $2 AND version = $3
            "#,
        )
        .bind(product.stock - qty)
        .bind(product_id)
        .bind(product.version)
        .execute(pool)
        .await?;

        if result.rows_affected() == 1 {
            return Ok(());
        }

        // 충돌 — 짧게 쉬고 재시도 (실제로는 jitter backoff)
        tokio::time::sleep(std::time::Duration::from_millis(2 + attempt as u64)).await;
    }

    anyhow::bail!("optimistic lock failed after retries");
}
```

### 설명

- 읽기 구간에는 row 락이 없다 → 충돌이 드물면 TPS가 FOR UPDATE보다 잘 나온다.  
- `rows_affected() == 0`을 **반드시** 처리해야 한다. 무시하면 갱신이 조용히 사라진다.  
- 충돌이 많으면 재시도가 CPU·DB를 잡아먹고, 영상 반대 상황(낙관적이 더 느림)이 된다.

### 체크포인트

- [ ] 성공 조건이 `rows_affected == 1` 임을 코드에서 확인
- [ ] 재시도 상한·backoff가 있는 이유

---

## 9. 원자적 UPDATE — 긴 읽기-수정-쓰기 윈도우를 없애기

재고 차감처럼 **앱에서 읽어서 계산할 필요가 거의 없으면** 한 문장이 가장 단순하고 빠른 경우가 많다.

```rust
pub async fn reserve_stock_atomic(
    pool: &PgPool,
    product_id: i64,
    qty: i32,
) -> anyhow::Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE products
        SET stock = stock - $1
        WHERE id = $2 AND stock >= $1
        "#,
    )
    .bind(qty)
    .bind(product_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 1 {
        Ok(())
    } else {
        anyhow::bail!("out of stock or missing product");
    }
}
```

정확한 말:

- “락이 전혀 없다”가 아니다. PostgreSQL은 **그 UPDATE 문 실행 동안** row 락을 잡는다.
- 없애는 것은 `SELECT`와 `UPDATE` **사이**의 긴 윈도우(그리고 `SELECT FOR UPDATE`로 트랜잭션 내내 잡는 락)다.
- 한 문장으로 조건·증감이 끝나 lost update 창이 사라진다.
- 복잡한 도메인 규칙이 앱에 있으면 비관/낙관을 다시 검토한다.

### 체크포인트

- [ ] 원자적 UPDATE ≠ 락 제로, **긴 FOR UPDATE 구간을 안 쓰는 것**임을 구분
- [ ] 단순 증감은 원자적 UPDATE를 먼저 떠올리기

---

## 10. 선택 기준

영상 메시지를 CS 표로 정리하면:

| 상황 | 선호 |
|---|---|
| 같은 row 동시 쓰기 **드묾**, 처리량 중요 | **낙관적** (version) 또는 원자적 UPDATE |
| 같은 row 경쟁 **심함**, 반드시 순차 처리 | **비관적** `FOR UPDATE` (트랜잭션은 짧게) |
| 읽기만 하고 거의 안 씀 | 락 불필요 / 낙관적 |
| FOR UPDATE 후 TPS가 급락 | 충돌률 측정. 낮으면 낙관·원자적으로 전환 검토 |
| 낙관적 재시도 폭풍 | 핫패스만 비관적으로 승격, 또는 큐·샤딩 |

한 줄 결론 (영상이 겨냥한 실수):

> 동시성 두려워서 전부 `SELECT FOR UPDATE`로 잠그면,  
> **잠글 필요가 적었던 경로의 병렬성까지 죽여** 처리량이 크게 떨어질 수 있다.

### 체크포인트

- [ ] “기본 낙관, 측정된 핫 row만 비관” 전략을 말할 수 있는지

---

## 11. 자주 하는 실수

1. **모든 UPDATE 경로에 FOR UPDATE**  
   → 불필요한 직렬화, 풀 고갈.

2. **FOR UPDATE 트랜잭션 안에 HTTP/슬립**  
   → 락 보유 시간 폭발 → 처리량 붕괴 (영상 증상과 동일한 계열).

3. **낙관적 락에서 `rows_affected` 무시**  
   → 갱신 유실.

4. **낙관적 무한 재시도**  
   → 재시도 폭풍. 상한·backoff·실패 응답 필요.

5. **version을 어떤 UPDATE에서 안 올림**  
   → 보호 구멍.

6. **비관적 = 항상 느리다 / 낙관적 = 항상 빠르다**  
   → 충돌률에 따라 뒤집힌다. 측정이 기준.

7. **애플리케이션 Mutex만 걸고 멀티 인스턴스 배포**  
   → 프로세스 밖 동시성은 막지 못함. DB/분산 락 필요.

---

## 12. 체크리스트

- [ ] lost update 시나리오 설명
- [ ] `SELECT FOR UPDATE` 동작과 락 해제 시점
- [ ] 처리량 ≈ 1/락보유시간 직관
- [ ] Rust `Mutex` 비관 / version 루프 낙관 예제 이해
- [ ] sqlx 비관·낙관·원자적 UPDATE 세 패턴 구분
- [ ] 영상처럼 TPS가 무너지면 “락 범위·보유 시간·충돌률”부터 의심

---

## 부록 A. 용어

| 용어 | 의미 |
|---|---|
| Throughput (처리량) | 단위 시간당 완료 요청 수 (TPS 등) |
| Contention (경쟁) | 같은 자원에 동시 접근이 몰리는 정도 |
| Row-level lock | 행 단위 락 |
| CAS | Compare-And-Swap. `version` 조건 UPDATE와 같은 계열의 “기대값일 때만 교체” |
| Retry storm | 충돌 후 다수가 동시에 재시도해 부하가 증폭되는 현상 |
| MVCC | Multi-Version Concurrency Control. 일반 SELECT가 FOR UPDATE 락에 안 막히고 스냅샷을 볼 수 있는 기반 |

---

## 부록 B. 소스

- 영상: https://www.youtube.com/shorts/tm-qpZMP8uk  
- 제목: SELECT FOR UPDATE를 걸었더니 처리량이 10분의 1로 떨어졌습니다 (비관적 락 vs 낙관적 락)  
- 이 보고서는 Shorts 주제·제목에서 드러난 주장을 CS 개념과 Rust 코드로 재구성한 학습 자료다. 벤치의 “정확히 1/10”은 환경 의존이므로, 숫자는 **방향성**으로 읽는다.

---

끝. `FOR UPDATE`는 강력한 도구이지만, **잠그는 시간과 잠그는 범위**가 곧 처리량 상한이다. Rust에서는 그 감각을 `Mutex` 임계구역과 sqlx 트랜잭션으로 그대로 옮기면 된다.
