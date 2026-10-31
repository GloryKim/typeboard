# Reqwest HTTP Client — Sender/Receiver, Cookbook, and Roadmap

## Part 1 — Axum Sender / Receiver

### Axum 으로 만드는 송신/수신 서버 예제 (axios 스타일)

Rust 는 axios 같은 "내장 HTTP 클라이언트"가 없기 때문에, 서버(Axum) 안에서 다른 서버로
요청을 보낼 때는 `reqwest` 크레이트를 axios 처럼 사용합니다.

이 예제는 웹서버 2개로 구성됩니다.

- **Receiver 서버** (`:3000`) — 요청을 받아서 처리 (수신 전용)
- **Sender 서버** (`:4000`) — 요청을 받으면, 내부에서 `reqwest` 로 Receiver 서버에 다시 요청을 쏨 (발송 역할, axios.post 와 동일한 패턴)

```
클라이언트(curl 등)
      │  POST /send
      ▼
┌───────────────┐        reqwest.post()        ┌──────────────────┐
│ Sender 서버    │ ───────────────────────────▶ │ Receiver 서버      │
│ :4000         │        (axios 처럼 호출)        │ :3000             │
└───────────────┘ ◀─────────────────────────── └──────────────────┘
                     응답 받아서 다시 클라이언트에 반환
```

---

#### 1. 프로젝트 구조

```
axum-demo/
├── receiver/
│   ├── Cargo.toml
│   └── src/main.rs
└── sender/
    ├── Cargo.toml
    └── src/main.rs
```

두 서버는 완전히 독립된 바이너리(별도 프로세스)로 각자 실행됩니다.

---

#### 2. Receiver 서버 (수신)

##### `receiver/Cargo.toml`

```toml
[package]
name = "receiver-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

##### `receiver/src/main.rs`

```rust
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};

###[derive(Debug, Deserialize, Serialize)]
struct Message {
    from: String,
    content: String,
}

###[derive(Debug, Serialize)]
struct Ack {
    status: String,
    received: Message,
}

###[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/receive", post(receive_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();

    println!("✅ Receiver server listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

// 다른 서버(Sender)로부터 POST 요청을 수신하는 핸들러
async fn receive_handler(Json(payload): Json<Message>) -> Json<Ack> {
    println!("📥 수신: {:?}", payload);

    Json(Ack {
        status: "ok".to_string(),
        received: payload,
    })
}
```

---

#### 3. Sender 서버 (발송)

Sender 서버는 외부(예: curl)로부터 `/send` 요청을 받으면, 그 내용을 그대로
**Receiver 서버로 다시 전송**합니다. 이때 `reqwest::Client` 가 axios 역할을 합니다.

##### `sender/Cargo.toml`

```toml
[package]
name = "sender-server"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

##### `sender/src/main.rs`

```rust
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};

###[derive(Debug, Deserialize, Serialize, Clone)]
struct Message {
    from: String,
    content: String,
}

###[derive(Debug, Serialize)]
struct SendResult {
    forwarded_to: String,
    response: serde_json::Value,
}

###[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new().route("/send", post(send_handler));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:4000")
        .await
        .unwrap();

    println!("✅ Sender server listening on http://localhost:4000");
    axum::serve(listener, app).await.unwrap();
}

// axios.post(url, data) 와 동일한 역할을 하는 핸들러
async fn send_handler(Json(payload): Json<Message>) -> Json<SendResult> {
    let client = reqwest::Client::new();

    let res = client
        .post("http://localhost:3000/receive") // Receiver 서버 주소
        .json(&payload)
        .send()
        .await
        .expect("Receiver 서버로 요청 실패");

    let body: serde_json::Value = res.json().await.expect("응답 파싱 실패");

    Json(SendResult {
        forwarded_to: "http://localhost:3000/receive".to_string(),
        response: body,
    })
}
```

---

#### 4. axios ↔ reqwest 비교

| axios (JS)                                   | reqwest (Rust)                                                          |
|-----------------------------------------------|----------------------------------------------------------------------|
| `axios.post(url, data)`                       | `client.post(url).json(&data).send().await`                          |
| `axios.get(url)`                              | `client.get(url).send().await`                                       |
| `const client = axios.create({ baseURL })`    | `let client = reqwest::Client::new();` (baseURL은 직접 문자열로 조합)      |
| `res.data`                                    | `res.json::<T>().await` 또는 `res.text().await`                        |
| Promise 기반 async                             | `async/await` + Tokio 런타임                                           |

---

#### 5. 실행 방법

```bash
### 1) Receiver 서버 실행 (터미널 1)
cd receiver
cargo run

### 2) Sender 서버 실행 (터미널 2)
cd sender
cargo run
```

---

#### 6. 테스트

Sender 서버에 요청을 보내면, Sender → Receiver 로 다시 요청이 전달됩니다.

```bash
curl -X POST http://localhost:4000/send \
  -H "Content-Type: application/json" \
  -d '{"from": "hair", "content": "hello axum"}'
```

**예상 응답 (Sender 로부터):**

```json
{
  "forwarded_to": "http://localhost:3000/receive",
  "response": {
    "status": "ok",
    "received": {
      "from": "hair",
      "content": "hello axum"
    }
  }
}
```

**Receiver 서버 콘솔 로그:**

```
📥 수신: Message { from: "hair", content: "hello axum" }
```

---

#### 7. 확장 아이디어

- `reqwest::Client` 를 `AppState` 로 만들어 매 요청마다 새로 생성하지 않고 재사용 (Connection pooling)
- Sender 쪽에 재시도/타임아웃 로직 추가 (`reqwest::ClientBuilder::timeout`)
- 두 서버 사이 통신을 gRPC(tonic) 나 메시지 큐(Kafka, Redis Streams)로 교체해 비교해보기

---

## Part 2 — Reqwest Feature Cookbook

### reqwest 기능별 예제 코드 모음

`reqwest` 는 Rust 에서 가장 널리 쓰이는 HTTP 클라이언트 크레이트입니다.
axios 처럼 GET/POST, 헤더, 쿼리, JSON, 파일 업로드, 타임아웃, 인증 등을 지원합니다.

#### 공통 `Cargo.toml`

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "multipart", "cookies", "gzip"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

#### 1. 기본 GET 요청

```rust
###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let res = reqwest::get("https://httpbin.org/get").await?;

    println!("Status: {}", res.status());

    let body = res.text().await?;
    println!("Body: {}", body);

    Ok(())
}
```

---

#### 2. GET + 쿼리 파라미터

axios 의 `axios.get(url, { params })` 와 동일한 패턴입니다.

```rust
use reqwest::Client;

###[derive(serde::Serialize)]
struct Query {
    page: u32,
    limit: u32,
}

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();

    let res = client
        .get("https://httpbin.org/get")
        .query(&Query { page: 1, limit: 20 })
        // 또는 튜플로도 가능: .query(&[("page", "1"), ("limit", "20")])
        .send()
        .await?;

    println!("{}", res.text().await?);
    Ok(())
}
```

---

#### 3. POST + JSON Body

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

###[derive(Debug, Serialize, Deserialize)]
struct User {
    name: String,
    age: u32,
}

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();

    let new_user = User {
        name: "hair".to_string(),
        age: 30,
    };

    let res = client
        .post("https://httpbin.org/post")
        .json(&new_user) // Content-Type: application/json 자동 설정
        .send()
        .await?;

    let parsed: serde_json::Value = res.json().await?;
    println!("{:#?}", parsed);

    Ok(())
}
```

---

#### 4. 커스텀 헤더 설정

```rust
use reqwest::{Client, header};

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();

    let res = client
        .get("https://httpbin.org/headers")
        .header("X-Custom-Header", "hello")
        .header(header::USER_AGENT, "my-rust-app/1.0")
        .send()
        .await?;

    println!("{}", res.text().await?);
    Ok(())
}
```

---

#### 5. Bearer 토큰 인증

axios 의 `headers: { Authorization: 'Bearer ' + token }` 패턴입니다.

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...";

    let res = client
        .get("https://httpbin.org/bearer")
        .bearer_auth(token)
        .send()
        .await?;

    println!("{}", res.text().await?);
    Ok(())
}
```

Basic 인증은 `.basic_auth(username, Some(password))` 를 사용합니다.

---

#### 6. 클라이언트 재사용 (Connection Pooling)

axios 의 `axios.create({ baseURL, timeout })` 와 유사합니다.
**요청마다 `Client::new()` 를 새로 만들지 말고, 하나를 만들어 재사용**하는 것이 중요합니다
(내부적으로 커넥션 풀을 유지하기 때문).

```rust
use reqwest::Client;
use std::time::Duration;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(10)
        .build()?;

    // 이 client 를 AppState 등에 넣어서 여러 핸들러에서 공유
    let res1 = client.get("https://httpbin.org/get").send().await?;
    let res2 = client.get("https://httpbin.org/ip").send().await?;

    println!("{} / {}", res1.status(), res2.status());
    Ok(())
}
```

---

#### 7. 타임아웃 & 에러 처리

```rust
use reqwest::Client;
use std::time::Duration;

###[tokio::main]
async fn main() {
    let client = Client::builder()
        .timeout(Duration::from_millis(500)) // 0.5초 안에 안 오면 타임아웃
        .build()
        .unwrap();

    match client.get("https://httpbin.org/delay/3").send().await {
        Ok(res) if res.status().is_success() => {
            println!("성공: {}", res.status());
        }
        Ok(res) => {
            println!("서버 에러 응답: {}", res.status());
        }
        Err(e) if e.is_timeout() => {
            println!("⏱️ 타임아웃 발생: {}", e);
        }
        Err(e) => {
            println!("기타 에러: {}", e);
        }
    }
}
```

---

#### 8. 상태 코드 검사 + `error_for_status`

```rust
use reqwest::Client;

###[tokio::main]
async fn main() {
    let client = Client::new();

    let result = client
        .get("https://httpbin.org/status/404")
        .send()
        .await
        .and_then(|res| res.error_for_status()); // 4xx/5xx 면 Err 로 변환

    match result {
        Ok(res) => println!("정상 응답: {}", res.status()),
        Err(e) => println!("❌ 요청 실패: {}", e),
    }
}
```

---

#### 9. Form 데이터 전송 (`application/x-www-form-urlencoded`)

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();

    let params = [("username", "hair"), ("password", "1234")];

    let res = client
        .post("https://httpbin.org/post")
        .form(&params)
        .send()
        .await?;

    println!("{}", res.text().await?);
    Ok(())
}
```

---

#### 10. 파일 업로드 (Multipart)

`multipart` feature 가 필요합니다.

```rust
use reqwest::{Client, multipart};

###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let file_bytes = tokio::fs::read("photo.png").await?;

    let part = multipart::Part::bytes(file_bytes)
        .file_name("photo.png")
        .mime_str("image/png")?;

    let form = multipart::Form::new()
        .text("description", "프로필 사진")
        .part("file", part);

    let res = client
        .post("https://httpbin.org/post")
        .multipart(form)
        .send()
        .await?;

    println!("{}", res.text().await?);
    Ok(())
}
```

---

#### 11. 응답 스트리밍 (대용량 파일 다운로드)

전체를 메모리에 올리지 않고 chunk 단위로 처리합니다.

```rust
use futures_util::StreamExt;
use reqwest::Client;
use tokio::io::AsyncWriteExt;

###[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let res = client
        .get("https://httpbin.org/bytes/1000000")
        .send()
        .await?;

    let mut file = tokio::fs::File::create("downloaded.bin").await?;
    let mut stream = res.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }

    println!("다운로드 완료");
    Ok(())
}
```

> `Cargo.toml` 에 `futures-util = "0.3"` 추가 필요

---

#### 12. 여러 요청 동시 실행 (병렬 처리)

`Promise.all()` 과 동일한 패턴입니다.

```rust
use reqwest::Client;
use futures_util::future::join_all;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();

    let urls = vec![
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/1",
    ];

    let requests = urls.iter().map(|url| client.get(*url).send());

    let results = join_all(requests).await;

    for r in results {
        match r {
            Ok(res) => println!("완료: {}", res.status()),
            Err(e) => println!("실패: {}", e),
        }
    }

    Ok(())
}
```

---

#### 13. 쿠키 유지 (세션처럼 사용)

`cookies` feature 필요.

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::builder()
        .cookie_store(true) // 쿠키 자동 저장/전송
        .build()?;

    // 로그인 요청 -> 서버가 Set-Cookie 응답
    client.get("https://httpbin.org/cookies/set?session=abc123").send().await?;

    // 이후 요청에 쿠키가 자동으로 실림
    let res = client.get("https://httpbin.org/cookies").send().await?;
    println!("{}", res.text().await?);

    Ok(())
}
```

---

#### 14. 리다이렉트 정책 제어

```rust
use reqwest::{Client, redirect::Policy};

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::builder()
        .redirect(Policy::limited(5)) // 최대 5번까지만 리다이렉트 허용
        // .redirect(Policy::none()) // 리다이렉트 전혀 따라가지 않음
        .build()?;

    let res = client.get("https://httpbin.org/redirect/2").send().await?;
    println!("최종 URL: {}", res.url());

    Ok(())
}
```

---

#### 15. 재시도 로직 (수동 구현)

reqwest 자체엔 내장 재시도가 없어서 직접 루프를 돕니다.
(자동화하려면 `reqwest-retry` + `reqwest-middleware` 크레이트 사용 가능)

```rust
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

async fn get_with_retry(client: &Client, url: &str, max_retries: u32) -> Result<String, reqwest::Error> {
    let mut attempt = 0;

    loop {
        match client.get(url).send().await {
            Ok(res) if res.status().is_success() => {
                return res.text().await;
            }
            Ok(_) | Err(_) if attempt < max_retries => {
                attempt += 1;
                let backoff = Duration::from_millis(200 * 2u64.pow(attempt));
                println!("재시도 {}/{} ({}ms 대기)", attempt, max_retries, backoff.as_millis());
                sleep(backoff).await;
            }
            Ok(res) => return res.text().await,
            Err(e) => return Err(e),
        }
    }
}

###[tokio::main]
async fn main() {
    let client = Client::new();
    match get_with_retry(&client, "https://httpbin.org/status/500", 3).await {
        Ok(body) => println!("성공: {}", body),
        Err(e) => println!("최종 실패: {}", e),
    }
}
```

---

#### 16. Proxy 사용

```rust
use reqwest::{Client, Proxy};

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::builder()
        .proxy(Proxy::all("http://127.0.0.1:8080")?)
        .build()?;

    let res = client.get("https://httpbin.org/ip").send().await?;
    println!("{}", res.text().await?);

    Ok(())
}
```

---

#### 17. Blocking (동기) 클라이언트

async 런타임 없이 쓰고 싶을 때 (`features = ["blocking", "json"]` 필요).

```rust
fn main() -> Result<(), reqwest::Error> {
    let client = reqwest::blocking::Client::new();

    let res = client
        .get("https://httpbin.org/get")
        .send()?;

    println!("{}", res.text()?);
    Ok(())
}
```

---

#### axios ↔ reqwest 빠른 대응표

| 기능             | axios (JS)                                      | reqwest (Rust)                                              |
|------------------|--------------------------------------------------|---------------------------------------------------------------|
| GET              | `axios.get(url)`                                  | `client.get(url).send().await`                                |
| POST + JSON      | `axios.post(url, data)`                           | `client.post(url).json(&data).send().await`                   |
| 쿼리 파라미터    | `axios.get(url, { params })`                      | `.query(&params)`                                              |
| 헤더             | `axios.get(url, { headers })`                     | `.header(key, value)`                                          |
| Bearer 토큰      | `headers: { Authorization: 'Bearer ' + t }`       | `.bearer_auth(token)`                                          |
| 타임아웃         | `axios.create({ timeout })`                        | `Client::builder().timeout(Duration)`                          |
| 인스턴스 재사용  | `axios.create({ baseURL })`                        | `Client::new()` 를 한 번 만들어 공유                          |
| 병렬 요청        | `Promise.all([...])`                              | `futures::future::join_all(...)`                                |
| 인터셉터/재시도  | axios-retry 등 미들웨어                            | `reqwest-middleware` + `reqwest-retry` 크레이트                |

---

#### 18. PUT / PATCH / DELETE

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();

    // PUT: 리소스 전체 교체
    let res = client
        .put("https://httpbin.org/put")
        .json(&serde_json::json!({ "name": "hair", "age": 31 }))
        .send()
        .await?;
    println!("PUT: {}", res.status());

    // PATCH: 리소스 부분 수정
    let res = client
        .patch("https://httpbin.org/patch")
        .json(&serde_json::json!({ "age": 32 }))
        .send()
        .await?;
    println!("PATCH: {}", res.status());

    // DELETE
    let res = client
        .delete("https://httpbin.org/delete")
        .send()
        .await?;
    println!("DELETE: {}", res.status());

    Ok(())
}
```

---

#### 19. 응답 헤더 확인

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();
    let res = client.get("https://httpbin.org/get").send().await?;

    println!("Status: {}", res.status());

    for (key, value) in res.headers() {
        println!("{}: {:?}", key, value);
    }

    if let Some(content_type) = res.headers().get("content-type") {
        println!("Content-Type: {:?}", content_type);
    }

    Ok(())
}
```

---

#### 20. `tokio::select!` 로 타임아웃/취소 직접 제어

`Client` 자체 타임아웃 대신, 요청을 다른 조건(취소 신호 등)과 경쟁시키고 싶을 때 사용합니다.

```rust
use reqwest::Client;
use tokio::time::{sleep, Duration};

###[tokio::main]
async fn main() {
    let client = Client::new();

    let request = client.get("https://httpbin.org/delay/5").send();
    let timeout = sleep(Duration::from_secs(2));

    tokio::select! {
        res = request => {
            match res {
                Ok(r) => println!("요청 완료: {}", r.status()),
                Err(e) => println!("요청 에러: {}", e),
            }
        }
        _ = timeout => {
            println!("⏱️ 2초 초과, 직접 취소 처리");
        }
    }
}
```

---

#### 21. `tokio::spawn` 으로 백그라운드 요청 (fire-and-forget)

핸들러 응답은 먼저 반환하고, 로깅/웹훅 같은 요청은 백그라운드로 보낼 때 씁니다.

```rust
use reqwest::Client;

###[tokio::main]
async fn main() {
    let client = Client::new();

    tokio::spawn(async move {
        let res = client
            .post("https://httpbin.org/post")
            .json(&serde_json::json!({ "event": "user_signup" }))
            .send()
            .await;

        match res {
            Ok(r) => println!("백그라운드 요청 완료: {}", r.status()),
            Err(e) => println!("백그라운드 요청 실패: {}", e),
        }
    });

    println!("메인 로직은 기다리지 않고 계속 진행");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await; // 데모용 대기
}
```

---

#### 22. 동시 요청 개수 제한 (Semaphore 로 Rate Limiting)

`join_all` 은 전부 한꺼번에 쏘지만, 실제로는 동시 요청 수를 제한해야 할 때가 많습니다.

```rust
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;
use futures_util::future::join_all;

###[tokio::main]
async fn main() {
    let client = Client::new();
    let semaphore = Arc::new(Semaphore::new(3)); // 동시에 3개까지만

    let urls = vec![
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/1",
    ];

    let tasks = urls.into_iter().map(|url| {
        let client = client.clone();
        let permit = semaphore.clone();
        async move {
            let _permit = permit.acquire().await.unwrap(); // 슬롯 확보
            client.get(url).send().await.map(|r| r.status())
        }
    });

    let results = join_all(tasks).await;
    for r in results {
        println!("{:?}", r);
    }
}
```

---

#### 23. 자체 서명 인증서 허용 (개발/테스트 환경 전용)

내부망이나 사내 서버가 self-signed cert 를 쓸 때 사용합니다. **운영 환경에서는 지양**해야 합니다.

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::builder()
        .danger_accept_invalid_certs(true) // ⚠️ 인증서 검증 비활성화
        .build()?;

    let res = client.get("https://self-signed.badssl.com/").send().await?;
    println!("{}", res.status());

    Ok(())
}
```

---

#### 24. Axum 서버의 `AppState` 로 `Client` 공유하기

앞선 예제(Sender 서버)처럼 매 요청마다 `Client::new()` 를 만들지 않고,
Axum 의 상태로 등록해서 전역 재사용합니다.

```rust
use axum::{extract::State, routing::get, Json, Router};
use reqwest::Client;
use std::sync::Arc;

###[derive(Clone)]
struct AppState {
    http_client: Arc<Client>,
}

###[tokio::main]
async fn main() {
    let state = AppState {
        http_client: Arc::new(Client::new()),
    };

    let app = Router::new()
        .route("/proxy", get(proxy_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn proxy_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let res = state
        .http_client
        .get("https://httpbin.org/get")
        .send()
        .await
        .unwrap();

    Json(res.json().await.unwrap())
}
```

---

#### 25. 조건부 요청 (ETag / If-None-Match)

캐시 검증에 쓰이는 패턴입니다. 서버가 304 Not Modified 를 주면 재다운로드를 생략합니다.

```rust
use reqwest::Client;

###[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let client = Client::new();
    let etag = "\"abc123\""; // 이전 응답에서 저장해둔 ETag

    let res = client
        .get("https://httpbin.org/etag/abc123")
        .header("If-None-Match", etag)
        .send()
        .await?;

    if res.status() == reqwest::StatusCode::NOT_MODIFIED {
        println!("변경 없음, 캐시 사용");
    } else {
        println!("새 데이터 수신: {}", res.status());
    }

    Ok(())
}
```

---

#### 참고

- 공식 문서: https://docs.rs/reqwest
- 미들웨어(재시도, 로깅 등) 확장: `reqwest-middleware` 크레이트

---

## Part 3 — Backend Learning Roadmap

### Rust 백엔드 학습 로드맵: Tokio → Axum → Reqwest → 통합 실습

> 대상: Rust 기초 문법은 알고 있고, Tokio 비동기 개념(async/await, executor, thread pool)에 대한 이해는 어느 정도 있는 상태에서 시작.
> 목표: axum으로 서버를 만들고, reqwest로 외부 API를 호출하며, tokio 동시성을 실전에서 다루는 감각을 익히는 것.

---

#### 0단계. 사전 점검 (이미 알고 있다면 스킵)

- [ ] `async fn`, `.await`, `Future` trait의 기본 동작 이해
- [ ] `tokio::main` 매크로가 하는 일 (런타임 부트스트랩)
- [ ] `Send` / `Sync` 트레잇이 async 코드에서 왜 중요한지
- [ ] `Arc<Mutex<T>>` / `Arc<RwLock<T>>` 패턴

여기까지는 이미 Tokio 개념 학습을 하셨으니 빠르게 리마인드만 하고 넘어가면 됩니다.

---

#### 1단계. Tokio 심화 — "동시성 제어" 감각 잡기

axum 서버를 만들기 전에 Tokio의 동시성 프리미티브를 실습으로 확실히 잡고 가는 게 좋습니다. 서버 코드에서 이 지식이 그대로 재사용됩니다.

##### 1-1. Task Spawning
- `tokio::spawn`으로 여러 작업 동시 실행
- `JoinHandle`, `JoinSet`으로 결과 수집
- **실습**: 10개의 "가짜 작업"(sleep으로 시뮬레이션)을 동시에 실행하고 전체 완료 시간이 직렬 실행보다 짧아지는 걸 확인

##### 1-2. 채널 (Channel)
- `mpsc::channel` — 다대일 메시지 전달
- `oneshot::channel` — 요청-응답 패턴 (나중에 axum 핸들러에서 워커 스레드와 통신할 때 유용)
- `broadcast::channel` — 다대다 브로드캐스트
- `watch::channel` — 상태 변경 알림
- **실습**: producer-consumer 패턴 (작업 큐 시뮬레이션), 이후 웹소켓 브로드캐스트에 재사용

##### 1-3. 동기화 프리미티브
- `tokio::sync::Mutex` vs `std::sync::Mutex` 차이 (언제 async Mutex를 써야 하는지)
- `RwLock`으로 읽기 다수/쓰기 소수 상황 최적화
- `Semaphore`로 동시 실행 개수 제한 (예: 외부 API 호출 rate limiting)
- **실습**: 세마포어로 "동시에 최대 3개까지만 요청 처리" 만들어보기

##### 1-4. 취소 & 타임아웃
- `tokio::time::timeout`
- `tokio::select!` 매크로로 여러 future 경쟁시키기
- `CancellationToken` (tokio-util)
- **실습**: 느린 작업에 타임아웃 걸고, `select!`로 "먼저 끝나는 것" 처리하는 예제

##### 1-5. 에러 처리 패턴
- `anyhow` vs `thiserror` 언제 뭘 쓸지
- `?` 연산자와 async 함수 조합
- **실습**: 커스텀 에러 타입 정의 후 여러 async 함수 체이닝

---

#### 2단계. Axum 기초 — 서버의 뼈대

##### 2-1. 최소 서버
```rust
// GET / 하나만 있는 서버부터 시작
```
- `Router`, `Route`, 핸들러 함수 시그니처 이해
- `#[tokio::main]`과 `axum::serve` 연결

##### 2-2. 라우팅
- 경로 파라미터: `/users/:id`
- 쿼리 파라미터: `Query<T>` extractor
- 여러 HTTP 메서드 (`get`, `post`, `put`, `delete`) 라우팅
- 중첩 라우터 (`nest`)로 API 버저닝 (`/api/v1/...`)
- **실습**: CRUD 스켈레톤 라우터 구성 (아직 실제 저장소 없이 더미 응답만)

##### 2-3. Extractor 이해
- `Json<T>` — 요청 바디 파싱
- `Path<T>` — 경로 변수 추출
- `State<T>` — 공유 상태 주입
- `Extension<T>` — 미들웨어에서 넘긴 데이터 받기
- `Header` 관련 extractor
- **실습**: JSON 바디로 사용자 생성 요청 받아서 파싱 후 그대로 echo

##### 2-4. 응답 만들기
- `impl IntoResponse` 이해
- `(StatusCode, Json<T>)` 튜플 응답 패턴
- 커스텀 에러 타입에 `IntoResponse` 구현하기 (에러를 HTTP 상태코드로 매핑)
- **실습**: `AppError` enum 만들고 `NotFound`, `BadRequest`, `Internal` 매핑

##### 2-5. 상태 공유 (State)
- `Arc<AppState>` 패턴
- `AppState` 안에 DB 커넥션 풀, 설정값, reqwest 클라이언트 등을 넣는 구조
- **실습**: 요청 카운터를 `Arc<Mutex<u64>>`로 State에 넣고 미들웨어에서 증가시키기

##### 2-6. 미들웨어 & Layer
- `tower::ServiceBuilder`로 레이어 쌓기
- 로깅 미들웨어 (`tower-http::trace`)
- CORS (`tower-http::cors`)
- 커스텀 미들웨어 (`axum::middleware::from_fn`)로 요청 로깅, 인증 체크
- **실습**: 요청 헤더에서 토큰 검사하는 간단한 인증 미들웨어

---

#### 3단계. Reqwest — 외부 세계와 통신하기

##### 3-1. 기본 GET/POST
- `reqwest::Client` 생성 (매 요청마다 새로 만들지 말고 재사용할 것 — 이 부분 중요)
- `.get()`, `.post()`, `.json()`, `.send()`, `.await`
- **실습**: 공개 API(예: JSONPlaceholder) 호출해서 데이터 가져오기

##### 3-2. 요청 커스터마이징
- 헤더 설정, 쿼리 파라미터, 타임아웃 설정
- `reqwest::Client::builder()`로 커넥션 풀 크기, 기본 타임아웃 등 설정
- **실습**: 외부 API 클라이언트를 struct로 감싸서 재사용 가능한 모듈로 만들기

##### 3-3. 에러 처리 & 재시도
- `reqwest::Error` 종류 (타임아웃, 커넥션 실패, 상태코드 에러 구분)
- 재시도 로직 직접 구현 (exponential backoff)
- 또는 `reqwest-retry` / `backoff` 크레이트 활용
- **실습**: 일부러 실패하는 mock 서버 만들어 재시도 로직 테스트

##### 3-4. 동시 요청
- `futures::future::join_all` 또는 `tokio::join!`로 여러 API 동시 호출
- **실습**: 여러 외부 API를 동시에 호출해서 결과를 하나로 합치는 aggregator 만들기 (BFF 패턴 연습)

---

#### 4단계. Axum + Reqwest 통합 — "프록시/BFF 서버" 실습

여기서부터가 진짜 실전 연습입니다. 서버가 클라이언트 요청을 받아서, 내부적으로 다른 API를 호출하고, 가공해서 응답하는 패턴.

##### 4-1. 프록시 서버
- 클라이언트 → axum → reqwest로 외부 API 호출 → 응답 가공 → 클라이언트에 반환
- **실습**: 날씨 API나 공공 API를 감싸는 간단한 프록시 서버

##### 4-2. Fan-out / Fan-in 패턴
- 하나의 요청이 들어오면 여러 외부 서비스를 동시에 호출하고 결과를 합쳐서 응답
- `AppState`에 `reqwest::Client`를 넣고 여러 핸들러에서 공유
- **실습**: "상품 상세 정보"를 여러 마이크로서비스(가격 서비스, 재고 서비스, 리뷰 서비스 — 각각 mock)에서 동시에 가져와 합치는 API

##### 4-3. 타임아웃 + 부분 실패 처리
- 외부 API 중 하나가 느리거나 실패해도 나머지는 정상 응답하는 graceful degradation
- `tokio::select!` + `timeout` 조합
- **실습**: 3개 외부 호출 중 1개가 타임아웃 나도 나머지 2개 데이터로 응답 구성

##### 4-4. 백그라운드 작업
- 요청에 즉시 응답하고, 실제 처리는 `tokio::spawn`으로 백그라운드에서 진행 (fire-and-forget, 혹은 job queue)
- **실습**: "요청 접수 → 202 Accepted 응답 → 백그라운드에서 외부 API 호출 후 결과 저장" 패턴

---

#### 5단계. 실전 기능 확장

##### 5-1. 데이터베이스 연동
- `sqlx` (컴파일 타임 쿼리 체크) 또는 `sea-orm`
- 커넥션 풀을 `AppState`에 포함
- **실습**: 기존 CRUD 스켈레톤에 실제 PostgreSQL/SQLite 연결

##### 5-2. WebSocket
- axum의 `ws::WebSocketUpgrade` extractor
- `broadcast` 채널과 결합해서 실시간 알림 서버
- **실습**: 여러 클라이언트에게 실시간으로 메시지 브로드캐스트하는 채팅/알림 서버 (이 부분은 로봇/센서 데이터 스트리밍에도 응용 가능한 패턴이라 특히 관심 있으실 수 있습니다)

##### 5-3. 파일 업로드/스트리밍
- `Multipart` extractor
- `axum::body::Body`로 스트리밍 응답 (대용량 데이터 chunk 단위 전송)
- **실습**: 파일 업로드 받아서 로컬 저장, 또는 스트리밍으로 대용량 JSON 응답 내려주기

##### 5-4. 인증/인가
- JWT 발급/검증 (`jsonwebtoken` 크레이트)
- 미들웨어에서 토큰 검증 후 `Extension`으로 사용자 정보 전달
- **실습**: 로그인 → JWT 발급 → 인증이 필요한 라우트 보호

##### 5-5. 설정 관리
- `config` 크레이트 또는 직접 `envy`/`.env` 파싱
- 환경별 설정 분리 (dev/prod)

##### 5-6. 관측성 (Observability)
- `tracing` + `tracing-subscriber`로 구조화 로깅
- `tower-http::trace`로 요청/응답 자동 로깅
- 필요시 OpenTelemetry 연동

##### 5-7. 테스트
- `axum::body::Body`와 `tower::ServiceExt::oneshot`으로 핸들러 단위 테스트
- `wiremock` 크레이트로 reqwest 호출 대상 mock 서버 구성
- **실습**: 4단계에서 만든 프록시 서버에 대한 통합 테스트 작성

---

#### 6단계. 마무리 프로젝트 (통합 실습)

지금까지 배운 걸 전부 합친 미니 프로젝트를 하나 만들어보는 걸 추천합니다. 예시 스펙:

**"외부 API 애그리게이터 + 실시간 알림 서버"**
- axum 라우터 (REST API + WebSocket)
- reqwest로 외부 API 2~3개 동시 호출, 부분 실패 처리
- Tokio 세마포어로 동시 호출 개수 제한 (rate limiting)
- 백그라운드 작업 큐 (`tokio::spawn` + `mpsc` 채널)
- SQLite/Postgres에 결과 캐싱 (`sqlx`)
- JWT 인증
- `tracing`으로 로깅
- `wiremock`으로 외부 API 테스트

이 정도 규모면 실무에서 마주치는 대부분의 패턴(동시성, 에러 처리, 상태 공유, 관측성)을 한 번씩 다 건드리게 됩니다.

---

#### 참고 자료

- [[Tokio 공식 튜토리얼](https://tokio.rs/tokio/tutorial)](https://tokio.rs/tokio/tutorial)
- [[Axum 공식 예제 모음](https://github.com/tokio-rs/axum/tree/main/examples)](https://github.com/tokio-rs/axum/tree/main/examples) — 각 단계별로 대응하는 예제가 거의 다 있음
- [[Reqwest 문서](https://docs.rs/reqwest)](https://docs.rs/reqwest)
- Zero To Production in Rust (책) — axum은 아니지만 actix 기반으로 백엔드 아키텍처 전반을 다룸, 패턴 참고용으로 유용
- `tower` / `tower-http` 문서 — axum이 tower 위에 얹혀 있으므로 미들웨어 이해에 필수

---

#### 학습 순서 요약 (체크리스트용)

```
[ ] 1단계: Tokio 심화 (spawn, channel, sync, timeout/select, error handling)
[ ] 2단계: Axum 기초 (routing, extractor, response, state, middleware)
[ ] 3단계: Reqwest 기초 (client, 커스터마이징, 에러/재시도, 동시 요청)
[ ] 4단계: Axum + Reqwest 통합 (프록시, fan-out/in, 부분실패, 백그라운드)
[ ] 5단계: 실전 확장 (DB, WebSocket, 파일, 인증, 설정, 로깅, 테스트)
[ ] 6단계: 통합 프로젝트
```

각 단계는 순서대로 갈 필요는 없고, 2~3단계는 병행해도 괜찮습니다 (axum 서버 안에서 reqwest 호출하는 걸 바로 실습하면서 배우는 것도 자연스러움).

. gRPC — tonic
MSA 하시는 중이면 REST보다 gRPC가 더 자연스러운 경우가 많습니다.

프로젝트: 지금 만드신 axum REST 서버를 gRPC 서비스로 하나 더 만들어서, axum 서버가 내부적으로 gRPC로 다른 마이크로서비스와 통신하게 구성 (REST는 외부, gRPC는 내부 서비스 간 — 실무에서 흔한 패턴)
protobuf 스키마 설계, streaming RPC (양방향 스트림)까지 해보면 LiDAR 같은 연속 데이터 스트리밍 감각과도 연결됩니다

메시지 큐 연동 — Redis Streams / Kafka / MQTT
메모리에 이미 Redis Streams, MQTT, Kafka 스킬셋이 있으신 걸 보니 관심 있으실 것 같습니다.
프로젝트: axum 서버가 요청을 받으면 Kafka(또는 Redis Streams)에 이벤트를 발행하고, 별도 Rust 워커(consumer)가 비동기로 소비해서 처리하는 이벤트 기반 아키텍처
rdkafka 또는 redis 크레이트 + tokio 조합. 4단계에서 만든 "백그라운드 작업" 패턴을 실제 큐 기반으로 업그레이드하는 셈

WASM — 브라우저/엣지에서 Rust 돌리기
프로젝트: 계산 집약적인 로직(예: 눈사람 문제 같은 알고리즘)을 Rust로 짜서 wasm-bindgen으로 WASM 빌드 후 브라우저에서 실행. axum 서버는 정적 파일만 서빙
프론트엔드 쪽 Three.js 관심도 있으셨으니, WASM + Three.js로 시각화까지 엮으면 재밌는 조합이 될 것 같습니다

임베디드/실시간 — embassy
C++ 임베디드/LiDAR 드라이버 배경이 있으시니 이 방향이 특히 잘 맞을 수 있습니다.
프로젝트: embassy (async 임베디드 프레임워크)로 MCU에서 센서 값을 읽고, UART/시리얼로 호스트에 전송 → 호스트의 axum 서버가 수신해서 처리하는 엔드투엔드 파이프라인
"Rust가 하드웨어 레벨 실패 모드에서 어떤 보장을 주는지"에 대해 예전에 관심 가지셨던 것과 바로 연결됩니다

gRPC/WebSocket 대신 — 실시간 협업 서버
프로젝트: CRDT(yrs — Rust용 Yjs 포트) + WebSocket으로 여러 클라이언트가 동시에 문서를 편집하는 서버. 동시성 제어를 네트워크 레벨에서 다루는 색다른 연습이 됩니다

Tauri 데스크톱 앱 + 백엔드 연동
Tauri/MinGW 작업 이력이 있으시니, 지금 만드는 axum 서버를 Tauri 앱의 백엔드로 붙여보는 것도 실전 연습이 됩니다.
프로젝트: Tauri 프론트엔드 ↔ 로컬에서 도는 axum 서버(sidecar) ↔ reqwest로 외부 API. 데스크톱 앱 배포 시 axum 서버를 바이너리에 임베드하는 패턴 연습

CLI 도구 — clap + async
프로젝트: reqwest로 여러 API를 병렬 호출해서 결과를 정리해주는 CLI 도구 (예: 여러 마이크로서비스 헬스체크 도구). 서버가 아니라 클라이언트 관점에서 tokio 동시성을 연습하는 것도 의미 있습니다

우선순위를 매긴다면, 지금 MSA/backend 작업 흐름상 **1번(gRPC)**과 **2번(메시지 큐)**이 가장 실무 연결성이 높고, **4번(embassy)**은 배경상 가장 흥미로우실 것 같은데 — 어느 방향이 지금 하시는 일과 더 맞닿아 있나요?

---
