# Rust 문법 학습 로드맵

> 이미 Tokio/Axum 등 실전 코드를 다뤄보셨기 때문에, 완전 입문자용은 아니고 "구멍 메우기 + 심화" 관점으로 구성했습니다. 아는 부분은 빠르게 훑고 넘어가시면 됩니다.

### 상세 교재 바로가기

| 단계 | 문서 |
|---|---|
| 1단계 기본 문법 | [01.md](./01.md) |
| 2단계 소유권 & 빌림 | [02.md](./02.md) |
| 3단계 타입 시스템 | [03.md](./03.md) |
| 4단계 에러 처리 | [04.md](./04.md) |
| 5단계 컬렉션 & 이터레이터 | [05.md](./05.md) |
| 6단계 클로저 | [06.md](./06.md) |
| 7단계 스마트 포인터 | [07.md](./07.md) |
| 8단계 모듈 & 프로젝트 구조 | [08.md](./08.md) |
| 9단계 매크로 | [09.md](./09.md) |
| 10단계 Unsafe & FFI (선택) | [10.md](./10.md) |
| 11단계 Async 문법 | [11.md](./11.md) |

---

## 1단계. 기본 문법 (빠르게 리마인드)

- [ ] 변수 바인딩, `let` / `let mut` / `const` / `static`
- [ ] 주석: `//`, `/* */`, 문서 주석 `///` / `//!`
- [ ] 스칼라 타입 (정수 오버플로우 동작, `f32`/`f64`, `bool`, `char`), 튜플, 배열, 슬라이스, `as` 타입 캐스팅
- [ ] `println!`/`format!` 포맷팅 문법 (`{}` vs `{:?}`, 정렬·폭·정밀도 지정자)
- [ ] 함수, 표현식 vs 문장 (`;`의 의미 — 블록이 값이 되는 방식)
- [ ] 제어 흐름: `if let`, `while let`, `for`(Range `..`/`..=`), `loop`의 반환값(`break value`)
- [ ] `match`와 패턴 매칭 기초 (`_`, `|`, range 패턴)
- **실습**: FizzBuzz 대신 — 간단한 상태 머신 (신호등 등)을 `enum` + `match`로 구현

---

## 2단계. 소유권 & 빌림 — Rust의 핵심

이미 `unique_ptr` 비교 등으로 개념은 잡혀 있으실 텐데, 세부 규칙을 문법 레벨에서 확실히 짚는 게 중요합니다.

- [ ] 소유권 이동(move) vs 복사(`Copy` trait)
- [ ] 빌림 규칙: `&T` (불변 참조 다수) vs `&mut T` (가변 참조 1개), NLL(Non-Lexical Lifetime)
- [ ] 슬라이스(`&[T]`, `&str`)가 빌림의 한 형태라는 점
- [ ] 라이프타임 문법: `'a` 표기, 함수 시그니처에서 라이프타임 생략 규칙(elision rules) 3가지
- [ ] 구조체에 참조를 담을 때 라이프타임 명시 (`struct Foo<'a> { x: &'a str }`)
- **실습**: 라이프타임 명시가 필요한 함수(예: 두 문자열 슬라이스 중 더 긴 것 반환)를 직접 작성 → 컴파일러 에러 메시지 읽는 연습

---

## 3단계. 타입 시스템 심화

### 3-1. 구조체와 열거형
- [ ] `struct` (일반/튜플/유닛 구조체), `impl` 블록
- [ ] `enum`에 데이터 넣기 (Rust enum이 다른 언어의 tagged union인 것)
- [ ] `Option<T>`, `Result<T, E>`를 라이브러리 코드가 아니라 "언어 문법의 일부"로 체화하기

### 3-2. 트레잇 (Trait)
- [ ] 트레잇 정의, 구현, 기본 메서드(default method)
- [ ] 트레잇을 매개변수로 받기: `impl Trait` vs `dyn Trait` vs 제네릭 `<T: Trait>`
- [ ] 트레잇 바운드 문법: `where` 절
- [ ] 연산자 오버로딩 (`std::ops::Add` 등 구현)
- [ ] `Deref`, `Drop` — 스마트 포인터처럼 동작하게 만들기
- **실습**: 커스텀 `Shape` 트레잇 만들고 여러 도형 구조체에 구현, `Vec<Box<dyn Shape>>`로 다형성 다루기

### 3-3. 제네릭
- [ ] 제네릭 함수/구조체/열거형 문법
- [ ] 제네릭 + 트레잇 바운드 조합 (`fn largest<T: PartialOrd>(list: &[T]) -> &T`)
- [ ] `impl<T> Foo<T>` 형태로 제네릭 구조체에 메서드 구현
- [ ] 모노모피제이션(monomorphization) 개념 — 컴파일 타임에 코드가 어떻게 생성되는지 (rustc 파이프라인 배경 지식과 연결됨)

---

## 4단계. 에러 처리 문법

- [ ] `Result<T, E>`와 `?` 연산자의 실제 동작 (`From` 트레잇을 통한 에러 타입 변환)
- [ ] `panic!` vs `Result` — 언제 뭘 쓸지의 문법적/관용적 기준
- [ ] `unwrap`, `expect`, `unwrap_or`, `unwrap_or_else`, `ok_or` 등 콤비네이터 메서드
- [ ] 커스텀 에러 타입에 `std::error::Error` 구현
- **실습**: 여러 종류의 에러(파싱 에러, IO 에러)를 하나의 커스텀 enum 에러로 통합하고 `?`로 체이닝

---

## 5단계. 컬렉션 & 이터레이터 — 실무에서 가장 많이 씀

- [ ] `Vec`, `HashMap`, `HashSet`, `BTreeMap` 기본 API
- [ ] 이터레이터 문법: `iter()` vs `into_iter()` vs `iter_mut()`의 차이 (소유권 관점)
- [ ] 어댑터 체이닝: `map`, `filter`, `fold`, `collect`, `zip`, `enumerate`, `flat_map`
- [ ] `collect::<Vec<_>>()` 처럼 타입 명시가 필요한 상황 이해
- [ ] 커스텀 이터레이터 만들기: `Iterator` 트레잇 직접 구현
- **실습**: 파일에서 읽은 텍스트를 이터레이터 체인만으로 단어 빈도수 세기 (for 루프 없이)

---

## 6단계. 클로저 & 함수형 문법

- [ ] 클로저 문법 (`|x| x + 1`), 타입 추론
- [ ] `Fn`, `FnMut`, `FnOnce` 트레잇의 차이 — 클로저가 환경을 어떻게 캡처하는지
- [ ] `move` 클로저 (스레드/async에서 자주 씀)
- [ ] 함수 포인터 vs 클로저 (`fn` 타입과 클로저 타입의 차이)
- **실습**: 콜백을 받는 함수를 제네릭 + `Fn` 트레잇 바운드로 작성

---

## 7단계. 스마트 포인터 & 메모리 모델

- [ ] `Box<T>` — 힙 할당, 재귀 타입 정의에 필요한 이유
- [ ] `Rc<T>` / `Arc<T>` — 참조 카운팅, 공유 소유권
- [ ] `RefCell<T>` / `Cell<T>` — 내부 가변성(interior mutability), 런타임 빌림 체크
- [ ] `Rc<RefCell<T>>` 패턴과 순환 참조 문제 (`Weak<T>`)
- **실습**: 트리/그래프 구조를 `Rc<RefCell<Node>>`로 구현하고 순환 참조를 `Weak`로 해결

---

## 8단계. 모듈 시스템 & 프로젝트 구조

- [ ] `mod`, `pub`, `pub(crate)`, `use` 경로 규칙
- [ ] 파일 시스템과 모듈 트리의 대응 관계 (`mod.rs` vs 2018 스타일)
- [ ] 워크스페이스(workspace)로 여러 크레이트 관리
- [ ] `Cargo.toml`의 `[features]`, 조건부 컴파일(`#[cfg(...)]`)
- **실습**: 지금까지 만든 axum 프로젝트를 여러 모듈(`handlers`, `models`, `errors`)로 분리

---

## 9단계. 매크로

- [ ] 선언적 매크로 (`macro_rules!`) 기초 문법
- [ ] 자주 쓰는 derive 매크로 이해: `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`
- [ ] 절차적 매크로(proc macro) 개념만 — 직접 만들기보다 `serde`, `axum` 등이 어떻게 매크로를 활용하는지 읽는 수준으로
- **실습**: 간단한 `macro_rules!`로 반복 코드 줄이기 (예: 여러 테스트 케이스를 매크로로 생성)

---

## 10단계. Unsafe & 저수준 문법 (선택, 임베디드/드라이버 작업과 연결됨)

- [ ] `unsafe` 블록이 실제로 무엇을 허용하는지 (5가지: raw pointer 역참조, unsafe fn 호출, mutable static 접근, unsafe trait 구현, union 필드 접근)
- [ ] Raw pointer (`*const T`, `*mut T`) 문법
- [ ] FFI: `extern "C"` 블록으로 C 라이브러리 바인딩
- [ ] `#[repr(C)]`로 메모리 레이아웃 제어
- **실습**: LiDAR 드라이버 배경을 살려서, 기존 C++ 드라이버의 간단한 함수 하나를 FFI로 Rust에서 호출해보기

---

## 11단계. Async 문법 자체 (Tokio 런타임 말고 "언어 문법"으로서)

이미 Tokio는 다뤄보셨으니, 여기서는 언어 차원의 async 문법만 짚습니다.

- [ ] `async fn`이 실제로 `impl Future`를 반환하는 설탕(sugar)이라는 점
- [ ] `.await`가 상태 머신을 어떻게 생성하는지 개념적으로
- [ ] `async move` 블록과 클로저의 관계
- [ ] `Pin<T>`이 왜 필요한지 (self-referential struct 문제)
- **실습**: 간단한 `Future` 트레잇을 직접 구현해보기 (executor 없이 `poll` 수동 호출)

---

## 참고 자료

- [The Rust Book (공식)](https://doc.rust-lang.org/book/) — 2~11단계 대부분이 이 순서를 따름
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) — 문법 항목별 짧은 실행 가능 예제
- [Rustlings](https://github.com/rust-lang/rustlings) — 컴파일 에러를 직접 고치며 배우는 연습 문제 세트, 문법 훈련에 특히 좋음
- [Rust Reference](https://doc.rust-lang.org/reference/) — 문법의 정확한 스펙이 필요할 때 (사전 용도)
- Jon Gjengset의 "Crust of Rust" 유튜브 시리즈 — 라이프타임, 트레잇 객체, 스마트 포인터 등 심화 주제를 코드 레벨로 파고듦 (7, 9, 11단계와 특히 잘 맞음)

---

## 체크리스트 요약

```
[ ] 1단계: 기본 문법 리마인드
[ ] 2단계: 소유권/빌림/라이프타임
[ ] 3단계: 구조체/열거형/트레잇/제네릭
[ ] 4단계: 에러 처리 (Result, ?, 커스텀 에러)
[ ] 5단계: 컬렉션 & 이터레이터
[ ] 6단계: 클로저 & Fn/FnMut/FnOnce
[ ] 7단계: 스마트 포인터 (Box/Rc/Arc/RefCell)
[ ] 8단계: 모듈 시스템 & 프로젝트 구조
[ ] 9단계: 매크로
[ ] 10단계: Unsafe & FFI (선택)
[ ] 11단계: Async 문법 (Future/Pin)
```

2~7단계가 Rust 문법의 핵심이자 가장 낯선 부분이고, 8단계부터는 실전 코드를 짜면서 자연스럽게 익혀도 충분합니다. 10, 11단계는 필요할 때 찾아봐도 되는 심화 주제라 우선순위는 낮게 잡아도 됩니다.
