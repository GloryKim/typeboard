# Tokio Runtime Roadmap

## Part 1 — Runtime Basics

### Tokio 로드맵 — 1단계: 입문 & 런타임 기동 (상세 교재)

> 이 문서는 Tokio 로드맵 1단계를 코드 예제와 함께 풀어놓은 교재입니다. 각 절은 "개념 → 코드 예제 → 설명 → 체크포인트" 순서입니다. `source.md`에서 말한 “Tokio는 러스트에서 가장 널리 쓰이는 비동기 런타임”을 여기서부터 손으로 확인합니다.

---

#### 1.1 Tokio란 무엇인가

##### 개념

Rust의 `async` / `await`는 **문법**이고, 실제로 퓨처를 `poll`해서 돌리는 쪽이 **런타임(executor)** 입니다. Tokio는 Rust 생태계에서 가장 널리 쓰이는 비동기 런타임으로, 네트워크 요청·파일 I/O·타이머 같은 작업을 **스레드를 블로킹하지 않고** 효율적으로 처리합니다.

핵심 그림:

1. 작업을 **태스크** 단위로 나눈다.
2. **이벤트 루프**가 I/O 준비를 감시한다.
3. 준비된 태스크만 이어서 실행한다.
4. 개발자는 `async`/`await`로 동기 코드처럼 읽히는 흐름을 유지한다.

그래서 대규모 네트워크 서버나, 동시에 많은 I/O가 필요한 프로그램에 자주 쓰입니다. “스레드를 요청마다 하나씩 늘리는” 모델과 비교하면, 대기 중인 연결이 많아도 워커 스레드 수를 작게 유지할 수 있습니다.

##### 체크포인트

- [ ] “async 문법”과 “Tokio 런타임”을 한 문장씩으로 구분해 적어보기
- [ ] [grammer/11.md](../grammer/11.md)의 “`async fn`은 `impl Future` 설탕”과 연결해보기

---

#### 1.2 의존성 추가 — `Cargo.toml`

##### 개념

Tokio를 쓰려면 먼저 의존성을 추가합니다.

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

학습·프로토타입에서는 `features = ["full"]`이 편합니다. 배포나 바이너리 크기가 민감하면 필요한 feature만 고릅니다.

| feature | 대략 |
|---|---|
| `rt` / `rt-multi-thread` | 런타임 (멀티스레드) |
| `macros` | `#[tokio::main]`, `#[tokio::test]` |
| `time` | `sleep`, `interval`, `timeout` |
| `net` | TCP / UDP / Unix 소켓 |
| `sync` | `Mutex`, `RwLock`, `Semaphore`, 채널 등 |
| `io-util` | `AsyncReadExt` / `AsyncWriteExt` |
| `fs` | 비동기 파일 API |
| `signal` | OS 시그널 |

최소 “hello + sleep”만 돌리려면 대략 이렇게도 됩니다:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
```

##### 체크포인트

- [ ] 새 바이너리 크레이트를 만들고 `tokio`를 추가한 뒤 `cargo check` 통과시키기
- [ ] (선택) `full` 대신 `macros` + `rt-multi-thread` + `time`만 켠 최소 구성 확인

---

#### 1.3 `#[tokio::main]` — 비동기 진입점

##### 개념

일반 `fn main`은 `.await`를 쓸 수 없습니다. `#[tokio::main]`을 붙이면 매크로가 **런타임을 띄운 뒤** async `main`을 그 위에서 실행합니다.

```rust
###[tokio::main]
async fn main() {
    println!("hello from tokio");
}
```

대략적인 설탕(개념):

```rust
fn main() {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async {
            println!("hello from tokio");
        });
}
```

즉 `#[tokio::main]`은 “진입점에서 런타임을 만들고 `block_on`한다”는 보일러플레이트를 줄여 줍니다. 테스트도 같은 계열입니다.

```rust
###[tokio::test]
async fn it_works() {
    assert_eq!(2 + 2, 4);
}
```

런타임 flavor를 명시할 수도 있습니다 (4단계에서 깊게):

```rust
###[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("single-threaded runtime");
}
```

##### 체크포인트

- [ ] `#[tokio::main]` 없는 `async fn main`을 컴파일해 보고 에러를 읽어보기
- [ ] `#[tokio::test]`로 async 테스트 하나 작성
- [ ] (선택) `Runtime::new().block_on(...)`를 직접 써서 매크로 없이 동일 동작 재현

---

#### 1.4 첫 `.await` — 타이머

##### 개념

```rust
use tokio::time::{sleep, Duration};

###[tokio::main]
async fn main() {
    println!("start");
    sleep(Duration::from_millis(200)).await;
    println!("after 200ms");
}
```

`sleep(...).await` 동안 이 태스크는 **OS 스레드를 `thread::sleep`으로 붙잡는 것**이 아닙니다. “200ms 뒤에 깨워 달라”고 타이머에 등록한 뒤, 런타임이 다른 태스크를 돌릴 수 있게 양보합니다. (지금은 태스크가 하나뿐이라 체감은 “그냥 기다림”에 가깝습니다. 2단계에서 spawn을 쓰면 차이가 드러납니다.)

비교용 — **하면 안 되는** 패턴:

```rust
###[tokio::main]
async fn main() {
    // 워커 스레드 전체를 블로킹 — async 런타임에서 피해야 함
    std::thread::sleep(std::time::Duration::from_millis(200));
}
```

짧은 실험·학습에서는 티가 안 날 수 있지만, 서버에 이런 코드가 있으면 동시 처리량이 급격히 떨어집니다. CPU를 오래 쓰는 작업은 4단계의 `spawn_blocking`을 씁니다.

##### 체크포인트

- [ ] sleep 전후 시각을 찍어 대략 200ms가 지나는지 확인
- [ ] `tokio::time::sleep`과 `std::thread::sleep`을 각각 쓰고, “왜 async에서는 tokio 쪽인가”를 한 문장으로 적기

---

#### 1.5 `async fn` 호출과 `.await`

##### 개념

```rust
async fn add(a: i32, b: i32) -> i32 {
    a + b
}

###[tokio::main]
async fn main() {
    let fut = add(1, 2); // 아직 본문이 실행되지 않을 수 있음 — Future를 만든 것
    let sum = fut.await; // 여기서 진행
    println!("sum = {sum}");
}
```

[grammer/11.md](../grammer/11.md)에서 본 그대로입니다.

- `async fn`을 **호출** → `impl Future`를 얻음
- `.await` (또는 런타임이 `poll`) → 실제로 진행

Tokio의 역할은 그 Future들을 **스케줄링하고 I/O와 연결**하는 쪽입니다.

##### 체크포인트

- [ ] `add(1, 2)`만 하고 `.await`를 빼 본 뒤, 경고/동작이 어떻게 다른지 관찰
- [ ] `async { 1 + 2 }` 블록을 `.await`해서 같은 결과 확인

---

#### 1.6 종합 실습 — 최소 “비동기 인사”

```rust
use tokio::time::{sleep, Duration};

async fn greet(name: &str) {
    sleep(Duration::from_millis(50)).await;
    println!("hello, {name}");
}

###[tokio::main]
async fn main() {
    greet("tokio").await;
}
```

##### 이 예제에서 짚을 포인트

1. `Cargo.toml`에 tokio가 있어야 하고, `#[tokio::main]`이 executor를 제공합니다.
2. `async fn greet` 호출만으로는 본문이 안 돌고, `.await`해야 진행됩니다.
3. `sleep`은 블로킹이 아니라 타이머 등록 + 양보입니다.
4. 다음 단계에서 같은 `greet`을 `spawn`으로 여러 개 띄우면 “동시성”이 체감됩니다.

##### 확장 과제 (선택)

- [ ] `greet`을 세 번 **순차** `.await`
- [ ] 인사 사이에 `Duration`을 인자로 받게 리팩터링
- [ ] (미리보기) `tokio::spawn`으로 세 개를 동시에 돌리고 출력 순서가 섞이는지 보기 → [02.md](./02.md)

---

#### 이 단계 한 장 요약

```
async/await = 언어 문법
Tokio       = Future를 돌리는 런타임 + I/O/타이머/동기화 도구

Cargo.toml  → tokio = { version = "1", features = [...] }
###[tokio::main] → Runtime 생성 + block_on
.await      → 이 지점에서 양보 가능 (블로킹 스레드 sleep과 다름)
```

---

#### 다음 단계 예고

[2단계 Future · Task · spawn](./02.md)에서는 “퓨처를 만들고, 태스크로 올려, 런타임이 스케줄링한다”는 그림을 코드로 고정합니다. `source.md`에서 말한 task / future 모델의 입구입니다.

---

## Part 2 — Future, Task, spawn

### Tokio 로드맵 — 2단계: Future · Task · spawn (상세 교재)

> 이 문서는 Tokio 로드맵 2단계를 코드 예제와 함께 풀어놓은 교재입니다. `source.md`의 “Tokio는 future와 task를 기반으로 동작한다”를 API 레벨에서 체화합니다.

> 선수: [1단계](./01.md), [grammer/11.md](../grammer/11.md)의 Future / `async move`

---

#### 2.1 Future — 아직 끝나지 않은 작업의 결과

##### 개념

`Future`는 “나중에 값이 생길 수 있는 계산”을 나타내는 타입입니다. Tokio 런타임은 이 Future들을 조합·스케줄링합니다.

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// 표준 라이브러리 개념 (단순화)
trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```

- `Poll::Pending` — 아직 아님. 준비되면 `Waker`로 다시 깨워 달라.
- `Poll::Ready(val)` — 완료.

평소에는 `Future`를 직접 `poll`하지 않습니다. `.await`가 그 일을 대신하고, Tokio가 executor 역할을 합니다.

```rust
async fn fetch_number() -> i32 {
    42
}

###[tokio::main]
async fn main() {
    let n = fetch_number().await;
    println!("{n}");
}
```

##### 체크포인트

- [ ] `async fn`이 `impl Future<Output = ...>`를 반환한다는 문장을 코드 주석으로 다시 적어보기
- [ ] [grammer/11.md](../grammer/11.md) 11.1절과 이 절을 나란히 읽기

---

#### 2.2 Task — 런타임이 스케줄링하는 실행 단위

##### 개념

**Task**는 Tokio가 독립적으로 스케줄링하는 단위입니다. 하나의 태스크 안에는 여러 `.await`가 이어질 수 있고, await 지점에서 다른 태스크에게 CPU를 양보합니다.

비유:

| 개념 | 대략 |
|---|---|
| Future | “할 일 명세서” (아직 안 돌 수도 있음) |
| Task | 런타임 큐에 올라간 “실행 중인/대기 중인 일감” |
| Runtime | 큐에서 꺼내 `poll`하고, I/O/타이머와 연결하는 엔진 |

`#[tokio::main]`의 `async fn main` 자체도 **하나의 태스크**로 돌아갑니다. 그 안에서 `tokio::spawn`을 호출하면 추가 태스크가 생깁니다.

##### 체크포인트

- [ ] “Future를 만들었다고 바로 실행되는가?”에 No라고 답할 수 있는지 확인
- [ ] main 태스크와 spawn된 태스크가 동시에 진행될 수 있다는 점을 한 문장으로

---

#### 2.3 `tokio::spawn` — 동시 실행

##### 개념

```rust
use tokio::time::{sleep, Duration};

###[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        sleep(Duration::from_millis(100)).await;
        println!("task done");
        7
    });

    println!("main continues");
    let result = handle.await.unwrap();
    println!("result = {result}");
}
```

동작 요약:

1. `spawn`에 넘긴 Future가 **새 태스크**로 스케줄됩니다.
2. `JoinHandle`을 `.await`하면 그 태스크의 종료(와 반환값)를 기다립니다.
3. 태스크가 panic하면 `handle.await`는 `Err(JoinError)`가 됩니다.

여러 개를 동시에:

```rust
use tokio::time::{sleep, Duration};

###[tokio::main]
async fn main() {
    let h1 = tokio::spawn(async {
        sleep(Duration::from_millis(80)).await;
        println!("A");
    });
    let h2 = tokio::spawn(async {
        sleep(Duration::from_millis(40)).await;
        println!("B");
    });

    h1.await.unwrap();
    h2.await.unwrap();
    // 보통 B가 먼저 출력 — 동시에 대기했기 때문
}
```

##### `'static` + `Send` + `move`

`spawn`된 태스크는 호출 스택보다 오래 살 수 있으므로, 캡처하는 데이터는 보통 **`'static` + `Send`** 제약을 만족해야 합니다.

```rust
###[tokio::main]
async fn main() {
    let name = String::from("tokio");

    // move로 소유권을 태스크 안으로
    let handle = tokio::spawn(async move {
        println!("hello, {name}");
    });

    // println!("{name}"); // 이미 이동
    handle.await.unwrap();
}
```

외부 참조만 빌려 쓰면:

```rust
###[tokio::main]
async fn main() {
    let name = String::from("tokio");
    // tokio::spawn(async { println!("{name}"); }); // 수명/`'static` 문제로 실패하기 쉬움
    let _ = name;
}
```

공유가 필요하면 `Arc` (grammer 7단계)를 clone해서 넘깁니다. 6단계에서 Mutex와 함께 다시 나옵니다.

##### 체크포인트

- [ ] spawn 두 개 + sleep 시간을 다르게 해서 출력 순서가 바뀌는지 확인
- [ ] `async`에서 외부 `String`을 쓰려다 에러를 내고 `async move`로 고치기
- [ ] panic하는 태스크의 `JoinHandle.await`가 `Err`인지 확인

---

#### 2.4 `join!` / `try_join!` — spawn 없이 동시에 기다리기

##### 개념

같은 태스크 안에서 여러 Future를 **동시에** 진행시키려면 `tokio::join!`이 편리합니다. (내부적으로 협력적으로 poll)

```rust
use tokio::time::{sleep, Duration};

async fn work(id: &str, ms: u64) -> String {
    sleep(Duration::from_millis(ms)).await;
    format!("{id} done")
}

###[tokio::main]
async fn main() {
    let (a, b) = tokio::join!(
        work("A", 100),
        work("B", 50),
    );
    println!("{a}, {b}");
}
```

`spawn`과의 차이 (감각):

| | `join!` | `spawn` |
|---|---|---|
| 새 태스크 | 아님 (같은 태스크에서 협력) | 새 태스크 |
| 스케줄러 경계 | 가벼움 | 약간 더 무겁고, 멀티스레드에서 다른 워커로 갈 수 있음 |
| 반환값 | 튜플로 한 번에 | `JoinHandle` 각각 |
| 취소/독립성 | 묶여 있음 | 더 독립적 |

`Result`면 `try_join!`:

```rust
async fn ok() -> Result<&'static str, &'static str> {
    Ok("ok")
}

async fn boom() -> Result<&'static str, &'static str> {
    Err("boom")
}

###[tokio::main]
async fn main() {
    let result = tokio::try_join!(ok(), boom());
    println!("{result:?}"); // Err("boom")
}
```

##### 체크포인트

- [ ] 순차 `.await` 두 번 vs `join!`의 총 소요 시간 비교
- [ ] `try_join!`에서 한쪽만 실패하는 케이스 작성

---

#### 2.5 종합 실습 — 두 작업을 동시에

```rust
use std::time::Instant;
use tokio::time::{sleep, Duration};

async fn job(name: &'static str, ms: u64) -> u64 {
    println!("{name} start");
    sleep(Duration::from_millis(ms)).await;
    println!("{name} end");
    ms
}

###[tokio::main]
async fn main() {
    let start = Instant::now();

    let h1 = tokio::spawn(job("left", 120));
    let h2 = tokio::spawn(job("right", 80));

    let (r1, r2) = tokio::join!(h1, h2);
    let total = r1.unwrap() + r2.unwrap();

    println!("sum of durations = {total}");
    println!("wall clock ≈ {:?}", start.elapsed());
    // wall clock은 ~120ms 근처 (순차 200ms보다 짧음)
}
```

##### 이 예제에서 짚을 포인트

1. Future를 `spawn`해 **Task**로 올렸다.
2. 두 태스크가 타이머를 **동시에** 기다린다.
3. `JoinHandle`을 모아 결과를 합친다.
4. 벽시계 시간이 합이 아니라 **max에 가깝다**는 게 동시성의 증거다.

##### 확장 과제

- [ ] 같은 작업을 `join!(job(...), job(...))`만으로 (spawn 없이) 다시 작성
- [ ] 태스크 3개로 늘리고 출력 순서 관찰
- [ ] 한 태스크에서 `panic!`을 내고 나머지 결과는 어떻게 되는지 확인

---

#### 이 단계 한 장 요약

```
Future  = 나중에 끝날 수 있는 계산 (poll → Pending | Ready)
Task    = 런타임 큐에 올라간 실행 단위
spawn   = Future → Task (+ JoinHandle)
join!   = 같은 태스크에서 여러 Future를 동시에 진행
move/'static/Send = spawn 캡처의 기본 제약
```

---

#### 다음 단계 예고

[3단계 poll · Waker · 이벤트 루프](./03.md)에서는 태스크가 **어떻게 잠들었다가 다시 깨어나는지**를 `poll`과 `Waker`로 설명합니다. `source.md`의 “준비가 되면 Waker가 호출되고 태스크가 다시 폴링된다”가 본편입니다.

---

## Part 3 — poll, Waker, Event Loop

### Tokio 로드맵 — 3단계: poll · Waker · 이벤트 루프 (상세 교재)

> 이 문서는 Tokio 로드맵 3단계를 풀어놓은 교재입니다. API 사용법보다 **“왜 await 중에 다른 일이 가능한가”**에 초점입니다. `source.md`의 이벤트 루프 · poll · Waker 설명을 코드/그림으로 고정합니다.

> 선수: [2단계](./02.md). 언어 쪽 배경은 [grammer/11.md](../grammer/11.md)의 `Future::poll`.

---

#### 3.1 이벤트 루프 — OS 비동기 I/O를 감시한다

##### 개념

Tokio의 이벤트 루프는 OS가 제공하는 비동기 I/O 메커니즘(Linux `epoll`, macOS `kqueue`, Windows `IOCP` 등) 위에서 돌아갑니다.

TCP 소켓을 비동기로 읽을 때의 감각:

1. 데이터가 아직 없으면 태스크는 **스레드를 붙잡고 기다리지 않는다**.
2. “이 소켓이 읽기 가능해지면 알려줘”라고 OS/런타임에 등록한다.
3. 그동안 런타임은 **준비된 다른 태스크**를 실행한다.
4. 소켓이 준비되면 해당 태스크를 다시 스케줄한다.

그래서 “수많은 연결을 기다리면서도 워커 스레드 수는 작게” 유지할 수 있습니다. `source.md`에서 말한 “시스템 콜을 직접 블로킹하지 않고, 준비가 될 때까지 태스크를 대기시킨다”가 이 그림입니다.

```text
[ OS: epoll/kqueue/IOCP ]
           ▲
           │  readiness
           ▼
[ Tokio reactor / 이벤트 루프 ]
           │
           │  wake / schedule
           ▼
[ 워커: Task A  poll → Pending ]
[ 워커: Task B  poll → Ready  ]
```

##### 체크포인트

- [ ] “블로킹 I/O”와 “비동기 I/O + 태스크 대기”의 차이를 자기 말로 한 문단
- [ ] 연결 1만 개를 스레드 1만 개로 처리하는 모델의 문제를 한 줄로

---

#### 3.2 `poll` — Pending과 Ready

##### 개념

모든 Future는 (개념적으로) `poll`로 진행합니다.

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

enum Poll<T> {
    Ready(T),
    Pending,
}
```

흐름:

1. 런타임이 태스크의 Future를 `poll`한다.
2. 아직 I/O·타이머가 안 끝났으면 `Pending`을 반환한다.
3. `Pending`을 줄 때 **나중에 깨울 `Waker`**를 등록해 둔다.
4. 준비가 되면 Waker가 호출되고, 런타임이 다시 `poll`한다.
5. 끝나면 `Ready(value)`.

개발자가 평소에 `poll`을 직접 호출할 일은 거의 없습니다. `.await`가 상태 머신 안에서 이 과정을 반복합니다. 다만 “await = 마법의 일시정지”가 아니라 **`poll` 협력 스케줄링**이라는 걸 알면, 블로킹 호출이 왜 치명적인지 이해됩니다.

의도적으로 단순한 Future 스케치 (교육용):

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct Immediate(i32);

impl Future for Immediate {
    type Output = i32;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<i32> {
        Poll::Ready(self.0)
    }
}

###[tokio::main]
async fn main() {
    let n = Immediate(7).await;
    println!("{n}");
}
```

`Pending`을 반환하는 Future는 `cx.waker().clone().wake()`를 언젠가 호출해 줘야 다시 poll됩니다. 안 하면 **영구히 잠든 태스크**가 됩니다.

##### 체크포인트

- [ ] `Poll::Pending` / `Poll::Ready`의 의미를 카드 한 장으로 정리
- [ ] “`.await`는 내부적으로 poll 루프와 같다”를 [grammer/11.md](../grammer/11.md) 상태 머신 절과 연결

---

#### 3.3 Waker — 태스크를 다시 깨우는 손잡이

##### 개념

`Waker`는 “이 태스크를 다시 스케줄해 달라”는 핸들입니다. `source.md`의 표현을 빌리면:

> I/O가 준비되면 Waker가 호출되고, 그 태스크가 다시 실행되어 다음 작업을 진행한다.

```text
Task A: read().await
   │
   ├─ poll → Pending, Waker 등록
   │
   │   (다른 태스크들 실행…)
   │
   │   OS: "소켓 readable"
   │         │
   │         ▼
   │      Waker.wake()
   │         │
   └─────────┴─→ 런타임이 Task A를 큐에 다시 넣음
                 poll → Ready(data) → 다음 줄 진행
```

타이머도 같은 패턴입니다. `sleep(200ms).await`는 “200ms 뒤 이 Waker를 wake”라고 타이머 휠/힙에 등록하는 일에 가깝습니다.

실무에서 Waker를 직접 만질 일은 드뭅니다. 커스텀 Future·저수준 I/O를 짤 때, 또는 “왜 내 태스크가 영원히 Pending인가?”를 디버깅할 때 개념이 필요합니다.

##### 체크포인트

- [ ] Waker가 없으면 `Pending` 이후 태스크가 어떻게 되는지 설명
- [ ] 타이머와 소켓이 **같은 wake 패턴**을 쓴다는 점을 그림 한 장으로

---

#### 3.4 블로킹이 이벤트 루프를 망가뜨리는 이유

##### 개념

```rust
###[tokio::main]
async fn main() {
    // ❌ 워커 스레드를 200ms 동안 점유
    std::thread::sleep(std::time::Duration::from_millis(200));

    // ✅ 태스크만 대기, 워커는 다른 일 가능
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}
```

동기 블로킹(`thread::sleep`, 동기 파일/DB 클라이언트, 무거운 CPU 루프)을 async 함수 한가운데서 하면:

1. 그 워커는 `poll` 루프로 돌아오지 못한다.
2. 같은 워커에 스케줄된 다른 태스크도 굶는다.
3. multi-thread라도 워커 수만큼만 버티다 전체가 느려진다.

그래서:

- I/O 대기는 **Tokio의 async API** (또는 async 호환 라이브러리)
- 어쩔 수 없는 블로킹/CPU 부하는 **`spawn_blocking`** (4단계)

##### 체크포인트

- [ ] async 컨텍스트에서 동기 sleep을 쓰면 안 되는 이유를 면접용 30초 답변으로 적어보기
- [ ] “Pending = CPU를 spin하며 기다림”이 **아님**을 분명히 하기

---

#### 3.5 종합 정리 — 한 요청의 생명주기

HTTP 클라이언트 GET을 예로 들면 (`source.md` / 5단계와 연결):

```text
1. #[tokio::main] → 런타임 기동, main 태스크 실행
2. reqwest::get(url) → Future 생성 (아직 네트워크 완료 전)
3. .await
     → poll
     → 소켓 연결/읽기 미완료면 Pending + Waker 등록
     → 이벤트 루프가 다른 태스크 처리
4. OS가 readable/writable 통지 → Waker → 다시 poll
5. Ready(Response) → text().await도 같은 패턴 반복
6. 출력 / 종료
```

개발자 코드는 동기처럼 보이지만, 각 `.await`가 **중단점**이고 그 사이에서 스케줄러가 끼어듭니다.

##### 확장 과제

- [ ] 위 생명주기를 자기 노트에 손으로 다시 그리기
- [ ] (선택) `futures` 크레이트나 직접 `poll_fn`으로 “두 번 poll해야 Ready”인 Future를 만들어 보기
- [ ] Tokio tutorial의 “Async in depth” 섹션을 이 문서와 대조하며 읽기

---

#### 이 단계 한 장 요약

```
이벤트 루프  → OS readiness 감시 + 태스크 스케줄
poll         → Pending | Ready
Waker        → Pending 이후 다시 poll할 신호
.await       → poll 협력의 문법 설탕
블로킹 금지  → 워커를 점유하면 루프 전체가 멈춘다
```

---

#### 다음 단계 예고

[4단계 스레드 모델 & 스레드 풀](./04.md)에서는 그 poll/wake가 **어떤 스레드들 위**에서 도는지를 봅니다. multi-thread / current-thread / `spawn_blocking`이 `source.md`의 스레드 모델 설명과 맞닿습니다.

---

## Part 4 — Thread Model and Thread Pool

### Tokio 로드맵 — 4단계: 스레드 모델 & 스레드 풀 (상세 교재)

> 이 문서는 Tokio 로드맵 4단계를 풀어놓은 교재입니다. `source.md`의 “멀티스레드 런타임과 단일 스레드 런타임”, “디폴트 스레드 풀”을 설정과 코드로 확인합니다.

> 선수: [2단계 spawn](./02.md), [3단계 이벤트 루프](./03.md)

---

#### 4.1 두 가지 flavor — multi-thread vs current-thread

##### 개념

Tokio는 기본적으로 두 가지 런타임 모드를 제공합니다.

| | multi-thread | current-thread |
|---|---|---|
| 워커 | CPU 코어에 맞춰 여러 개 | 보통 1개 (호출 스레드) |
| 태스크 분산 | 워커 간 스케줄링 / work-stealing | 한 스레드에서 협력적으로 |
| 기본 `#[tokio::main]` | 이쪽 (feature에 따라) | `flavor = "current_thread"` |
| 언제 | 서버, 처리량 | 임베디드·테스트·단순 CLI, `!Send` 제약 완화에 유리한 경우 |

```rust
// 멀티스레드 (흔함)
###[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    println!("multi-thread runtime");
}

// 단일 스레드
###[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("current-thread runtime");
}
```

multi-thread에서는 `spawn`된 태스크가 **다른 워커**에서 실행될 수 있습니다. 그래서 Future가 `Send`여야 하는 제약이 더 두드러집니다. current-thread는 같은 스레드에서만 돌리므로 일부 `!Send` 케이스가 편해질 수 있지만, CPU 병렬성은 기대하기 어렵습니다.

##### 체크포인트

- [ ] 같은 프로그램을 두 flavor로 바꿔 `cargo run`해 보기
- [ ] “서버 기본값은 왜 multi-thread인가”를 한 문장으로

---

#### 4.2 스레드 풀과 작업 큐 — 감각

##### 개념

`source.md` 표현을 다듬으면:

- 태스크는 큐에 쌓인다.
- 워커 스레드들이 큐에서 꺼내 `poll`한다.
- I/O-bound 태스크는 Pending이 많아 워커를 오래 점유하지 않는다.
- 필요에 따라 런타임이 워커를 활용해 여러 작업을 동시에 진행한다.

정확한 내부 구조(LIFO 슬롯, remote queue, work-stealing 등)는 버전·설정에 따라 디테일이 달라지므로, 학습 단계에서는 이 정도면 충분합니다:

```text
[ Task queue(s) ]
       │
       ├─ Worker 0 ── poll / run until Pending or budget
       ├─ Worker 1
       └─ Worker N
              ▲
              │ wake
       [ reactor: I/O · timers ]
```

개발자는 보통 **스레드를 직접 만들지 않습니다.** `spawn`만 하면 풀이 알아서 가져갑니다. 직접 `std::thread::spawn`으로 무거운 일을 섞으면 Tokio 스케줄러 밖에 나가므로, 결과를 다시 async 세계로 가져오는 경로를 따로 설계해야 합니다. 그 정석 중 하나가 `spawn_blocking`입니다.

##### 체크포인트

- [ ] “워커 스레드”와 “요청마다 OS 스레드” 모델의 차이를 표로 그리기
- [ ] Tokio tutorial의 runtime 절을 훑으며 용어 맞춰보기

---

#### 4.3 `Runtime::Builder`로 직접 구성

##### 개념

매크로 대신 빌더를 쓰면 테스트·라이브러리·커스텀 바이너리에서 제어하기 좋습니다.

```rust
use tokio::runtime::Builder;
use tokio::time::{sleep, Duration};

fn main() {
    let rt = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        sleep(Duration::from_millis(50)).await;
        println!("ran on custom runtime");
    });
}
```

current-thread 버전:

```rust
use tokio::runtime::Builder;

fn main() {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        println!("current_thread builder");
    });
}
```

`enable_all()`은 time/net/io 등 드라이버를 켭니다. feature를 쪼개 쓰는 프로젝트에서는 `enable_time()` 등만 고르기도 합니다.

##### 체크포인트

- [ ] `worker_threads(2)`로 빌드해 간단한 spawn 여러 개 실행
- [ ] `#[tokio::main]`을 `Builder` + `block_on`으로 치환해 보기

---

#### 4.4 CPU-bound vs I/O-bound — `spawn_blocking`

##### 개념

| 종류 | 예 | Tokio에서 |
|---|---|---|
| I/O-bound | 소켓, 타이머, 대부분의 HTTP | `async` + `.await` |
| CPU-bound / 동기 블로킹 | 이미지 인코딩, 동기 DB 드라이버 | `spawn_blocking` |

```rust
###[tokio::main]
async fn main() {
    let handle = tokio::task::spawn_blocking(|| {
        // 여기는 일반 동기 코드 — 별도의 블로킹 스레드 풀에서 실행
        let mut x = 0u64;
        for i in 0..10_000_000 {
            x = x.wrapping_add(i);
        }
        x
    });

    let result = handle.await.unwrap();
    println!("result = {result}");
}
```

`spawn_blocking`은 async 워커를 막지 않기 위해 **별도 블로킹 풀**에 일을 보냅니다. 끝난 결과는 다시 Future로 받아 `.await`할 수 있습니다.

남용 주의: 블로킹 풀도 무한히 커지지 않습니다. 동기 라이브러리를 async 경로의 기본값처럼 쓰면 풀이 고갈될 수 있습니다. 가능하면 async 네이티브 크레이트를 쓰는 편이 낫습니다.

##### 체크포인트

- [ ] 일부러 async 함수 안에서 긴 CPU 루프를 돌린 뒤 / `spawn_blocking`으로 옮긴 뒤 체감 비교 (다른 태스크의 타이머 정확도 등)
- [ ] `spawn` vs `spawn_blocking` 사용 기준을 자기 규칙 두 줄로 적기

---

#### 4.5 종합 실습 — 워커 수 고정 + 혼합 작업

```rust
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::time::{sleep, Duration};

fn main() {
    let rt = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let start = Instant::now();

        let io = tokio::spawn(async {
            sleep(Duration::from_millis(100)).await;
            "io"
        });

        let cpu = tokio::task::spawn_blocking(|| {
            std::thread::sleep(Duration::from_millis(100)); // 의도적 블로킹 예제
            "cpu"
        });

        let (a, b) = tokio::join!(io, cpu);
        println!("{}, {}", a.unwrap(), b.unwrap());
        println!("elapsed ≈ {:?}", start.elapsed());
    });
}
```

##### 이 예제에서 짚을 포인트

1. 런타임 flavor·워커 수를 **명시**했다.
2. I/O 대기는 async 태스크, 블로킹은 `spawn_blocking`.
3. 둘을 `join!`으로 기다려 벽시계가 합이 아니라 겹침에 가깝게 나오는지 본다.
4. `thread::sleep`을 async 워커에 직접 넣지 않은 이유를 3단계와 연결한다.

##### 확장 과제

- [ ] `worker_threads(1)`로 바꿔 동작·시간 관찰
- [ ] `current_thread`에서 `spawn_blocking`이 어떻게 동작하는지 문서/실험으로 확인
- [ ] axum 서버를 떠올리며: “핸들러 안에서 동기 HTTP 클라이언트를 쓰면?”에 대한 답을 적어보기

---

#### 이 단계 한 장 요약

```
multi_thread   → 여러 워커, 처리량, Send 제약 뚜렷
current_thread → 한 스레드 협력, 단순/테스트
Builder        → worker_threads, enable_*, block_on
spawn          → async 태스크 (I/O-bound)
spawn_blocking → 블로킹/CPU를 별도 풀로
```

---

#### 다음 단계 예고

[5단계 실전 I/O](./05.md)에서는 타이머·TCP·`reqwest` HTTP로 “런타임 위에서 실제로 무엇을 쓰는지”를 코드로 고정합니다. `source.md`의 HTTP 예제가 여기서 완성됩니다.

---

## Part 5 — Practical I/O (timer / TCP / HTTP)

### Tokio 로드맵 — 5단계: 실전 I/O (timer / TCP / HTTP) (상세 교재)

> 이 문서는 Tokio 로드맵 5단계를 코드 예제와 함께 풀어놓은 교재입니다. `source.md`에서 반복해서 나온 “비동기 HTTP / 타이머 / 네트워크”를 실행 가능한 패턴으로 정리합니다.

> 선수: [1단계](./01.md)~[4단계](./04.md). HTTP 예제는 `reqwest` 크레이트가 추가로 필요합니다.

---

#### 5.1 타이머 — `sleep` / `interval` / `timeout`

##### 개념

```rust
use tokio::time::{sleep, interval, timeout, Duration};

###[tokio::main]
async fn main() {
    sleep(Duration::from_millis(100)).await;

    let mut tick = interval(Duration::from_millis(50));
    for i in 0..3 {
        tick.tick().await;
        println!("tick {i}");
    }

    let result = timeout(Duration::from_millis(30), sleep(Duration::from_millis(100))).await;
    println!("timeout? {}", result.is_err()); // true — 안쪽 sleep이 더 김
}
```

- `sleep` — 한 번 대기
- `interval` — 주기적 깨움 (서버의 heartbeat, 배치 틱에 흔함)
- `timeout` — “이 Future가 N 안에 안 끝나면 실패”

모두 3단계의 **타이머 등록 + Waker** 패턴입니다. `std::thread::sleep`으로 대체하지 마세요.

##### 체크포인트

- [ ] `interval`로 5번 찍고 총 소요 시간 대략 확인
- [ ] `timeout`으로 성공/실패 케이스 각각 만들기

---

#### 5.2 TCP — 최소 에코 서버 스케치

##### 개념

```rust
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

###[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:3000").await?;
    println!("listening on 3000");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        println!("connect from {addr}");

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(0) => return, // EOF
                    Ok(n) => n,
                    Err(_) => return,
                };
                if socket.write_all(&buf[..n]).await.is_err() {
                    return;
                }
            }
        });
    }
}
```

포인트:

1. `accept().await` — 새 연결이 올 때까지 Pending.
2. 연결마다 `spawn` — 한 클라이언트가 느려도 리스너 루프는 계속.
3. `AsyncReadExt` / `AsyncWriteExt` — `read`/`write_all`이 async.

다른 터미널에서:

```bash
nc 127.0.0.1 3000
```

타이핑하면 그대로 돌아옵니다.

실무 서버는 보통 axum / hyper 위에서 돌아가지만, “연결당 태스크” 모델은 동일합니다.

##### 체크포인트

- [ ] 에코 서버를 띄우고 `nc`로 문자열 주고받기
- [ ] `spawn`을 빼면 (연결을 순차로만 처리하면) 두 번째 클라이언트가 어떻게 되는지 생각해 보기

---

#### 5.3 HTTP 클라이언트 — `reqwest` + Tokio

##### 개념

`source.md`의 대표 예제입니다. Cargo.toml:

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
```

(환경에 따라 `features = ["json"]` 등 추가. TLS feature는 플랫폼/정책에 맞게 고르면 됩니다.)

```rust
###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = reqwest::get("https://example.com").await?;
    let status = response.status();
    let body = response.text().await?;

    println!("status = {status}");
    println!("body len = {}", body.len());
    println!("{}", &body[..body.len().min(120)]);
    Ok(())
}
```

동작 방식 (`source.md` 복습):

1. `#[tokio::main]` → 런타임 안에서 main 실행
2. `reqwest::get` → 비동기 요청 Future 등록
3. `.await` → I/O 완료까지 이벤트 루프가 다른 일 가능
4. 준비되면 다시 poll → `Response`
5. `text().await` → 본문도 같은 비동기 패턴
6. 출력

여러 URL을 동시에:

```rust
###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let u1 = reqwest::get("https://example.com");
    let u2 = reqwest::get("https://www.rust-lang.org");

    let (r1, r2) = tokio::try_join!(u1, u2)?;
    println!("{} / {}", r1.status(), r2.status());
    Ok(())
}
```

##### 체크포인트

- [ ] example.com GET이 성공하는지 확인
- [ ] `try_join!`으로 두 요청 동시 실행 후 벽시계 비교
- [ ] 잘못된 URL로 에러가 `?`로 전파되는지 확인 (6단계 예고)

---

#### 5.4 파일이 아니라 “동시 I/O”가 핵심인 이유

##### 개념

Tokio의 강점은 파일 한 개를 빠르게 읽는 것만이 아니라, **대기 시간이 긴 작업 다수를 겹쳐 처리**하는 데 있습니다.

```rust
use tokio::time::{sleep, Duration, Instant};

async fn fake_io(name: &str, ms: u64) {
    println!("{name} wait {ms}ms");
    sleep(Duration::from_millis(ms)).await;
    println!("{name} done");
}

###[tokio::main]
async fn main() {
    let start = Instant::now();
    tokio::join!(
        fake_io("A", 100),
        fake_io("B", 100),
        fake_io("C", 100),
    );
    println!("elapsed ≈ {:?}", start.elapsed()); // ~100ms, not ~300ms
}
```

HTTP·TCP·타이머가 모두 이 “겹침” 위에 있습니다. CPU만 쥐어짜는 작업이면 4단계 `spawn_blocking` / 병렬 크레이트를 검토하세요.

##### 체크포인트

- [ ] 위 `fake_io` 세 개를 순차 await로 바꿔 시간이 ~300ms가 되는지 확인
- [ ] “동시성(concurrency) vs 병렬성(parallelism)”을 Tokio 맥락에서 한 문장씩

---

#### 5.5 종합 실습 — HTTP GET 유틸

```rust
async fn fetch_status(url: &str) -> Result<u16, reqwest::Error> {
    let resp = reqwest::get(url).await?;
    Ok(resp.status().as_u16())
}

###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let urls = [
        "https://example.com",
        "https://www.rust-lang.org",
    ];

    let mut handles = Vec::new();
    for url in urls {
        let url = url.to_string();
        handles.push(tokio::spawn(async move { fetch_status(&url).await }));
    }

    for h in handles {
        match h.await? {
            Ok(code) => println!("status = {code}"),
            Err(e) => eprintln!("request error: {e}"),
        }
    }
    Ok(())
}
```

##### 이 예제에서 짚을 포인트

1. `reqwest` + Tokio 조합의 기본 패턴
2. URL마다 `spawn`으로 동시 요청
3. `JoinHandle`의 `Result`와 요청 `Result`가 **두 겹**일 수 있음 (`h.await?` 후 안쪽 `match`)
4. 에러 처리를 더 깔끔히 묶는 법은 [06.md](./06.md)

##### 확장 과제

- [ ] `timeout`으로 요청당 2초 제한
- [ ] 응답 body 대신 status만 모으는 버전 / body 길이를 모으는 버전
- [ ] (선택) 같은 아이디어로 아주 작은 axum 핸들러에서 `reqwest` 호출해 보기

---

#### 이 단계 한 장 요약

```
time:   sleep / interval / timeout
net:    TcpListener + accept + spawn + AsyncRead/Write
http:   reqwest::get(...).await → text().await
핵심:   대기 작업을 겹쳐 벽시계를 줄인다
```

---

#### 다음 단계 예고

[6단계 에러 처리 & 동시성 제어](./06.md)에서는 `Result`/`?`와 `tokio::sync::Mutex`로, 실패와 공유 상태를 비동기 코드에서 안전하게 다루는 법을 정리합니다.

---

## Part 6 — Errors and Concurrency Control

### Tokio 로드맵 — 6단계: 에러 처리 & 동시성 제어 (상세 교재)

> 이 문서는 Tokio 로드맵 6단계를 코드 예제와 함께 풀어놓은 교재입니다. `source.md`의 “Result로 오류 처리”, “tokio::sync::Mutex로 공유 자원 보호”를 grammer 스타일로 고정합니다.

> 선수: [5단계](./05.md), [grammer/04.md](../grammer/04.md)의 `Result`/`?`, [grammer/07.md](../grammer/07.md)의 `Arc` 감각.

---

#### 6.1 async에서 `Result`와 `?`

##### 개념

Tokio 작업도 실패는 예외가 아니라 **`Result`** 로 표현하는 경우가 많습니다. `?`는 async 함수 안에서도 동일하게 동작합니다.

```rust
async fn read_title(url: &str) -> Result<String, reqwest::Error> {
    let body = reqwest::get(url).await?.text().await?;
    let title = body
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    Ok(title)
}

###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let title = read_title("https://example.com").await?;
    println!("{title}");
    Ok(())
}
```

`expr?`의 의미는 grammer 4단계와 같습니다.

- `Ok(v)` → `v`를 꺼냄
- `Err(e)` → `From::from(e)` 후 **즉시 return**

`main`을 `Result<(), E>`로 두면 최상위에서도 `?`를 쓸 수 있습니다.

여러 종류 에러를 한 타입으로 모을 때:

```rust
async fn load() -> Result<String, Box<dyn std::error::Error>> {
    let s = tokio::fs::read_to_string("config.txt").await?;
    let _num: i32 = s.trim().parse()?;
    Ok(s)
}
```

실무에서는 `thiserror` / `anyhow`로 커스텀·빠른 프로토타입을 많이 합니다. 문법 핵심은 여전히 `Result` + `?`입니다.

##### `JoinError`와 안쪽 `Result` — 두 겹

```rust
###[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        Result::<i32, &'static str>::Err("nope")
    });

    match handle.await {
        Ok(Ok(v)) => println!("value = {v}"),
        Ok(Err(e)) => println!("task returned err: {e}"),
        Err(join) => println!("task panicked: {join}"),
    }
}
```

- 바깥: 태스크가 panic 없이 끝났는지 (`JoinError`)
- 안쪽: 비즈니스 `Result`

##### 체크포인트

- [ ] `async fn`에서 `?` 두 번 체이닝하는 함수 작성
- [ ] spawn된 태스크의 `Result`를 `Ok(Ok)` / `Ok(Err)` / `Err`로 구분해 처리
- [ ] grammer 4단계 `?` = `From` 변환 문장을 다시 읽기

---

#### 6.2 `tokio::sync::Mutex` — await 친화적 락

##### 개념

여러 태스크가 공유 상태를 건드릴 때, 동기 `std::sync::Mutex`를 async에서 그대로 쓰면 위험합니다. 락을 잡은 채 `.await`하면 **워커 스레드를 붙잡은 채로** 다른 태스크를 기다리게 되어 데드락·기아에 가깝게 갈 수 있습니다.

`tokio::sync::Mutex`는 락을 못 받으면 스레드를 블로킹하지 않고, 태스크가 **양보했다가** 락이 풀리면 Waker로 다시 깨어납니다. (`source.md`의 Mutex 설명과 동일)

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

###[tokio::main]
async fn main() {
    let counter = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::new();

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        handles.push(tokio::spawn(async move {
            let mut guard = counter.lock().await;
            *guard += 1;
            // guard는 여기서 drop — 락 해제
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    println!("count = {}", *counter.lock().await);
}
```

패턴:

1. 공유 상태는 `Arc<Mutex<T>>` (또는 `Arc<RwLock<T>>`)
2. 태스크마다 `Arc::clone`
3. `lock().await`로 가드 획득
4. **가드 수명을 짧게** — 특히 `.await` 호출 전에 drop

나쁜 예:

```rust
async fn bad(m: Arc<Mutex<Vec<u8>>>) {
    let mut g = m.lock().await;
    // ❌ 락을 든 채 await — 다른 태스크가 같은 락을 못 얻고, 스케줄에 따라 데드락 가능
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    g.push(1);
}
```

나은 예:

```rust
async fn good(m: Arc<Mutex<Vec<u8>>>) {
    {
        let mut g = m.lock().await;
        g.push(1);
    } // drop
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
}
```

##### `std::sync::Mutex`는 언제?

짧은 크리티컬 섹션에서 **절대 `.await`하지 않는다면** `std::sync::Mutex`도 쓰입니다. 다만 실수하기 쉬우므로, async 코드 경로의 기본값은 `tokio::sync::*`를 권장하는 편이 안전합니다.

| | `std::sync::Mutex` | `tokio::sync::Mutex` |
|---|---|---|
| 락 대기 | OS 스레드 블로킹 | 태스크 Pending + wake |
| `.await` 중 보유 | 위험 | 가능은 하나 **비권장**(수명 길게 잡지 말 것) |
| 오버헤드 | 상대적으로 낮을 수 있음 | async용 |

##### 체크포인트

- [ ] `Arc<Mutex<u64>>` 카운터를 태스크 20개로 증가시켜 최종값 20 확인
- [ ] 락 가드를 든 채 `sleep().await`하는 버전을 만들고, 왜 나쁜지 설명
- [ ] `RwLock`으로 읽기 여러 / 쓰기 하나 패턴 스케치

---

#### 6.3 그 외 자주 쓰는 `tokio::sync`

##### 개념

전부 외울 필요는 없고, “이런 게 있다” 수준이면 6단계 목표에 충분합니다.

| API | 용도 |
|---|---|
| `Mutex` / `RwLock` | 공유 상태 |
| `Semaphore` | 동시 실행 수 제한 (예: 동시에 HTTP 10개만) |
| `Notify` | 단순한 신호 |
| `mpsc` | 다수 생산자 → 하나 소비자 채널 |
| `oneshot` | 일회성 응답 |
| `watch` | 최신 값 방송 (설정 핫리로드 등) |
| `broadcast` | 여러 구독자에게 메시지 |

Semaphore 예:

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

###[tokio::main]
async fn main() {
    let sem = Arc::new(Semaphore::new(2)); // 동시에 2개만
    let mut handles = Vec::new();

    for i in 0..5 {
        let sem = Arc::clone(&sem);
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            println!("run {i}");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}
```

##### 체크포인트

- [ ] Semaphore(2)로 5개 작업을 돌려 출력이 겹치는 정도 관찰
- [ ] (선택) `mpsc`로 숫자 생성 태스크 → 합산 태스크 파이프라인

---

#### 6.4 종합 실습 — 동시 요청 + 공유 집계 + 에러

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

async fn fetch_len(url: &str) -> Result<usize, reqwest::Error> {
    let body = reqwest::get(url).await?.text().await?;
    Ok(body.len())
}

###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let total = Arc::new(Mutex::new(0usize));
    let urls = [
        "https://example.com",
        "https://www.rust-lang.org",
        "https://httpbin.org/get",
    ];

    let mut handles = Vec::new();
    for url in urls {
        let total = Arc::clone(&total);
        let url = url.to_string();
        handles.push(tokio::spawn(async move {
            match fetch_len(&url).await {
                Ok(len) => {
                    let mut g = total.lock().await;
                    *g += len;
                    Ok::<_, String>(len)
                }
                Err(e) => Err(format!("{url}: {e}")),
            }
        }));
    }

    for h in handles {
        match h.await? {
            Ok(len) => println!("ok len={len}"),
            Err(e) => eprintln!("fail {e}"),
        }
    }

    println!("total bytes = {}", *total.lock().await);
    Ok(())
}
```

의존성: `tokio` + `reqwest` (5단계와 동일).

##### 이 예제에서 짚을 포인트

1. 네트워크 실패는 `Result`로 남기고 프로세스 전체를 무조건 죽이지 않을 수 있다.
2. 성공한 길이만 `Mutex`로 합산 — 짧은 크리티컬 섹션.
3. `JoinError`(`h.await?`)와 요청 에러(`Err(e)`)를 분리.
4. 3단계 Waker / 4단계 스레드 풀 위에서 이 모든 태스크가 스케줄된다.

##### 확장 과제

- [ ] `Semaphore`로 동시 요청 수 2로 제한
- [ ] `timeout`으로 요청당 상한
- [ ] `anyhow::Result`로 main 시그니처 단순화
- [ ] 합계를 Mutex 대신 `mpsc`로 보내 한 태스크가 fold 하게 리팩터링

---

#### 이 단계 한 장 요약

```
async + Result + ?     → 실패를 값으로 전파
JoinHandle 두 겹 Result → panic vs 비즈니스 에러
tokio::sync::Mutex     → 락 대기 중에도 워커 양보
락 + await             → 가드 수명 짧게
Semaphore / mpsc 등    → 동시성 패턴 도구 상자
```

---

#### 로드맵을 마친 뒤

```
1 입문/런타임 → 2 Task/spawn → 3 poll/Waker → 4 스레드 모델 → 5 I/O → 6 에러/동기화
```

이제 [Tokio Tutorial](https://tokio.rs/tokio/tutorial)을 처음부터 다시 보면, 문서의 문장이 코드 감각과 맞물릴 것입니다. 언어 문법 구멍은 [grammer/readme.md](../grammer/readme.md)로, 런타임 구멍은 이 폴더로 메우면 됩니다.

원본 대화 메모: [source.md](./source.md)

---
