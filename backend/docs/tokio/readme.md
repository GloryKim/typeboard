# Tokio 학습 로드맵

> [grammer/11.md](../grammer/11.md)가 **언어로서의 async / Future / Pin**이라면, 이 폴더는 **Tokio 런타임과 API**를 코드로 체화하는 교재입니다. `source.md` 대화에서 나온 개념을 "개념 → 코드 예제 → 설명 → 체크포인트"로 정리했습니다.

### 상세 교재 바로가기

| 단계 | 문서 |
|---|---|
| 1단계 입문 & 런타임 기동 | [01.md](./01.md) |
| 2단계 Future · Task · spawn | [02.md](./02.md) |
| 3단계 poll · Waker · 이벤트 루프 | [03.md](./03.md) |
| 4단계 스레드 모델 & 스레드 풀 | [04.md](./04.md) |
| 5단계 실전 I/O (timer / TCP / HTTP) | [05.md](./05.md) |
| 6단계 에러 처리 & 동시성 제어 | [06.md](./06.md) |

---

## 1단계. 입문 & 런타임 기동

- [ ] Tokio가 무엇인지 (async 런타임, 이벤트 루프)
- [ ] `Cargo.toml`에 `tokio` 의존성 추가 (`features = ["full"]` 등)
- [ ] `#[tokio::main]` / `#[tokio::test]`
- [ ] `async fn` + `.await`의 기본 흐름
- **실습**: `sleep` 후 인사 출력하는 최소 바이너리

---

## 2단계. Future · Task · spawn

- [ ] Future = 아직 끝나지 않은 작업의 결과
- [ ] Task = 런타임이 스케줄링하는 실행 단위
- [ ] `tokio::spawn`으로 동시 실행
- [ ] `JoinHandle`로 결과 기다리기 / `tokio::join!`
- **실습**: 두 태스크를 spawn하고 둘 다 끝날 때까지 join

---

## 3단계. poll · Waker · 이벤트 루프

- [ ] `poll`이 `Pending` / `Ready`를 반환하는 의미
- [ ] I/O 준비 시 Waker가 태스크를 깨움
- [ ] OS 비동기 I/O와 블로킹 없는 대기
- **실습**: 동기 `thread::sleep` vs `tokio::time::sleep` 차이 한 문단으로 정리

---

## 4단계. 스레드 모델 & 스레드 풀

- [ ] multi-thread vs current-thread 런타임
- [ ] 워커 스레드 / 작업 큐 감 잡기
- [ ] CPU-bound vs I/O-bound와 `spawn_blocking`
- **실습**: `Builder`로 워커 수 지정해 보기

---

## 5단계. 실전 I/O

- [ ] `tokio::time::sleep` / `interval`
- [ ] `TcpListener` 에코 서버 스케치
- [ ] `reqwest` + `#[tokio::main]` HTTP GET
- **실습**: example.com에 GET 후 status / body 일부 출력

---

## 6단계. 에러 처리 & 동시성 제어

- [ ] async에서 `Result` + `?`
- [ ] `tokio::sync::Mutex` vs `std::sync::Mutex`
- [ ] 락을 await 경계 밖으로 오래 붙잡지 않기
- **실습**: 공유 카운터를 여러 태스크가 안전하게 증가

---

## 참고 자료

- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) — 공식 튜토리얼
- [Tokio docs.rs](https://docs.rs/tokio) — API 레퍼런스
- [grammer 11단계 Async 문법](../grammer/11.md) — Future / Pin / async move
- [grammer 4단계 에러 처리](../grammer/04.md) — `Result` / `?` (6단계에서 재사용)

---

## 체크리스트 요약

```
[ ] 1단계: #[tokio::main] + sleep
[ ] 2단계: spawn / JoinHandle
[ ] 3단계: poll / Waker 개념
[ ] 4단계: multi vs current-thread
[ ] 5단계: timer / TCP / HTTP
[ ] 6단계: Result + sync::Mutex
```

1~2단계만 해도 “Tokio로 async 코드 돌리기”는 됩니다. 3~4단계는 “왜 빠른지 / 어떻게 스케줄하는지”를 머릿속에 남기는 단계이고, 5~6단계는 실무 패턴입니다.
