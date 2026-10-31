# Rust Grammar Roadmap

### 문법 구멍 메우기부터 async·unsafe 심화까지, 한 권으로 이어 읽기

이미 Tokio/Axum 등 실전 코드를 다뤄 본 사람을 기준으로 한 **구멍 메우기 + 심화** 교재다. 아는 절은 빠르게 건너뛰어도 된다. 원래 `etc/01.md`–`11.md`로 나뉘어 있던 상세 교재를 단계 순서대로 한데 모았다.

## 목차

| 단계 | 내용 |
|---|---|
| 1단계 | [기본 문법](#1단계-기본-문법) |
| 2단계 | [소유권 & 빌림](#2단계-소유권-빌림) |
| 3단계 | [타입 시스템](#3단계-타입-시스템) |
| 4단계 | [에러 처리](#4단계-에러-처리) |
| 5단계 | [컬렉션 & 이터레이터](#5단계-컬렉션-이터레이터) |
| 6단계 | [클로저](#6단계-클로저) |
| 7단계 | [스마트 포인터](#7단계-스마트-포인터) |
| 8단계 | [모듈 & 프로젝트 구조](#8단계-모듈-프로젝트-구조) |
| 9단계 | [매크로](#9단계-매크로) |
| 10단계 | [Unsafe & FFI (선택)](#10단계-unsafe-ffi-선택) |
| 11단계 | [Async 문법](#11단계-async-문법) |

### 이 문서를 읽는 법

- 각 단계는 **개념 → 코드 → 설명 → 체크포인트** 순이다. 가능하면 예제를 직접 컴파일해 에러 메시지를 읽는다.
- 단계 끝의 “다음” 링크로 순서만 따라가면 된다. 10단계는 선택 심화다.
- 로드맵 체크리스트(할 일 목록)는 아래에 요약해 두었다. 본문은 그 상세 해설이다.

## 로드맵 체크리스트 요약

> 이미 Tokio/Axum 등 실전 코드를 다뤄보셨기 때문에, 완전 입문자용은 아니고 "구멍 메우기 + 심화" 관점으로 구성했습니다. 아는 부분은 빠르게 훑고 넘어가시면 됩니다.

### 1단계. 기본 문법 (빠르게 리마인드)

- [ ] 변수 바인딩, `let` / `let mut` / `const` / `static`
- [ ] 주석: `//`, `/* */`, 문서 주석 `///` / `//!`
- [ ] 스칼라 타입 (정수 오버플로우 동작, `f32`/`f64`, `bool`, `char`), 튜플, 배열, 슬라이스, `as` 타입 캐스팅
- [ ] `println!`/`format!` 포맷팅 문법 (`{}` vs `{:?}`, 정렬·폭·정밀도 지정자)
- [ ] 함수, 표현식 vs 문장 (`;`의 의미 — 블록이 값이 되는 방식)
- [ ] 제어 흐름: `if let`, `while let`, `for`(Range `..`/`..=`), `loop`의 반환값(`break value`)
- [ ] `match`와 패턴 매칭 기초 (`_`, `|`, range 패턴)
- **실습**: FizzBuzz 대신 — 간단한 상태 머신 (신호등 등)을 `enum` + `match`로 구현

---

### 2단계. 소유권 & 빌림 — Rust의 핵심

이미 `unique_ptr` 비교 등으로 개념은 잡혀 있으실 텐데, 세부 규칙을 문법 레벨에서 확실히 짚는 게 중요합니다.

- [ ] 소유권 이동(move) vs 복사(`Copy` trait)
- [ ] 빌림 규칙: `&T` (불변 참조 다수) vs `&mut T` (가변 참조 1개), NLL(Non-Lexical Lifetime)
- [ ] 슬라이스(`&[T]`, `&str`)가 빌림의 한 형태라는 점
- [ ] 라이프타임 문법: `'a` 표기, 함수 시그니처에서 라이프타임 생략 규칙(elision rules) 3가지
- [ ] 구조체에 참조를 담을 때 라이프타임 명시 (`struct Foo<'a> { x: &'a str }`)
- **실습**: 라이프타임 명시가 필요한 함수(예: 두 문자열 슬라이스 중 더 긴 것 반환)를 직접 작성 → 컴파일러 에러 메시지 읽는 연습

---

### 3단계. 타입 시스템 심화

#### 3-1. 구조체와 열거형
- [ ] `struct` (일반/튜플/유닛 구조체), `impl` 블록
- [ ] `enum`에 데이터 넣기 (Rust enum이 다른 언어의 tagged union인 것)
- [ ] `Option<T>`, `Result<T, E>`를 라이브러리 코드가 아니라 "언어 문법의 일부"로 체화하기

#### 3-2. 트레잇 (Trait)
- [ ] 트레잇 정의, 구현, 기본 메서드(default method)
- [ ] 트레잇을 매개변수로 받기: `impl Trait` vs `dyn Trait` vs 제네릭 `<T: Trait>`
- [ ] 트레잇 바운드 문법: `where` 절
- [ ] 연산자 오버로딩 (`std::ops::Add` 등 구현)
- [ ] `Deref`, `Drop` — 스마트 포인터처럼 동작하게 만들기
- **실습**: 커스텀 `Shape` 트레잇 만들고 여러 도형 구조체에 구현, `Vec<Box<dyn Shape>>`로 다형성 다루기

#### 3-3. 제네릭
- [ ] 제네릭 함수/구조체/열거형 문법
- [ ] 제네릭 + 트레잇 바운드 조합 (`fn largest<T: PartialOrd>(list: &[T]) -> &T`)
- [ ] `impl<T> Foo<T>` 형태로 제네릭 구조체에 메서드 구현
- [ ] 모노모피제이션(monomorphization) 개념 — 컴파일 타임에 코드가 어떻게 생성되는지 (rustc 파이프라인 배경 지식과 연결됨)

---

### 4단계. 에러 처리 문법

- [ ] `Result<T, E>`와 `?` 연산자의 실제 동작 (`From` 트레잇을 통한 에러 타입 변환)
- [ ] `panic!` vs `Result` — 언제 뭘 쓸지의 문법적/관용적 기준
- [ ] `unwrap`, `expect`, `unwrap_or`, `unwrap_or_else`, `ok_or` 등 콤비네이터 메서드
- [ ] 커스텀 에러 타입에 `std::error::Error` 구현
- **실습**: 여러 종류의 에러(파싱 에러, IO 에러)를 하나의 커스텀 enum 에러로 통합하고 `?`로 체이닝

---

### 5단계. 컬렉션 & 이터레이터 — 실무에서 가장 많이 씀

- [ ] `Vec`, `HashMap`, `HashSet`, `BTreeMap` 기본 API
- [ ] 이터레이터 문법: `iter()` vs `into_iter()` vs `iter_mut()`의 차이 (소유권 관점)
- [ ] 어댑터 체이닝: `map`, `filter`, `fold`, `collect`, `zip`, `enumerate`, `flat_map`
- [ ] `collect::<Vec<_>>()` 처럼 타입 명시가 필요한 상황 이해
- [ ] 커스텀 이터레이터 만들기: `Iterator` 트레잇 직접 구현
- **실습**: 파일에서 읽은 텍스트를 이터레이터 체인만으로 단어 빈도수 세기 (for 루프 없이)

---

### 6단계. 클로저 & 함수형 문법

- [ ] 클로저 문법 (`|x| x + 1`), 타입 추론
- [ ] `Fn`, `FnMut`, `FnOnce` 트레잇의 차이 — 클로저가 환경을 어떻게 캡처하는지
- [ ] `move` 클로저 (스레드/async에서 자주 씀)
- [ ] 함수 포인터 vs 클로저 (`fn` 타입과 클로저 타입의 차이)
- **실습**: 콜백을 받는 함수를 제네릭 + `Fn` 트레잇 바운드로 작성

---

### 7단계. 스마트 포인터 & 메모리 모델

- [ ] `Box<T>` — 힙 할당, 재귀 타입 정의에 필요한 이유
- [ ] `Rc<T>` / `Arc<T>` — 참조 카운팅, 공유 소유권
- [ ] `RefCell<T>` / `Cell<T>` — 내부 가변성(interior mutability), 런타임 빌림 체크
- [ ] `Rc<RefCell<T>>` 패턴과 순환 참조 문제 (`Weak<T>`)
- **실습**: 트리/그래프 구조를 `Rc<RefCell<Node>>`로 구현하고 순환 참조를 `Weak`로 해결

---

### 8단계. 모듈 시스템 & 프로젝트 구조

- [ ] `mod`, `pub`, `pub(crate)`, `use` 경로 규칙
- [ ] 파일 시스템과 모듈 트리의 대응 관계 (`mod.rs` vs 2018 스타일)
- [ ] 워크스페이스(workspace)로 여러 크레이트 관리
- [ ] `Cargo.toml`의 `[features]`, 조건부 컴파일(`#[cfg(...)]`)
- **실습**: 지금까지 만든 axum 프로젝트를 여러 모듈(`handlers`, `models`, `errors`)로 분리

---

### 9단계. 매크로

- [ ] 선언적 매크로 (`macro_rules!`) 기초 문법
- [ ] 자주 쓰는 derive 매크로 이해: `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`
- [ ] 절차적 매크로(proc macro) 개념만 — 직접 만들기보다 `serde`, `axum` 등이 어떻게 매크로를 활용하는지 읽는 수준으로
- **실습**: 간단한 `macro_rules!`로 반복 코드 줄이기 (예: 여러 테스트 케이스를 매크로로 생성)

---

### 10단계. Unsafe & 저수준 문법 (선택, 임베디드/드라이버 작업과 연결됨)

- [ ] `unsafe` 블록이 실제로 무엇을 허용하는지 (5가지: raw pointer 역참조, unsafe fn 호출, mutable static 접근, unsafe trait 구현, union 필드 접근)
- [ ] Raw pointer (`*const T`, `*mut T`) 문법
- [ ] FFI: `extern "C"` 블록으로 C 라이브러리 바인딩
- [ ] `#[repr(C)]`로 메모리 레이아웃 제어
- **실습**: LiDAR 드라이버 배경을 살려서, 기존 C++ 드라이버의 간단한 함수 하나를 FFI로 Rust에서 호출해보기

---

### 11단계. Async 문법 자체 (Tokio 런타임 말고 "언어 문법"으로서)

이미 Tokio는 다뤄보셨으니, 여기서는 언어 차원의 async 문법만 짚습니다.

- [ ] `async fn`이 실제로 `impl Future`를 반환하는 설탕(sugar)이라는 점
- [ ] `.await`가 상태 머신을 어떻게 생성하는지 개념적으로
- [ ] `async move` 블록과 클로저의 관계
- [ ] `Pin<T>`이 왜 필요한지 (self-referential struct 문제)
- **실습**: 간단한 `Future` 트레잇을 직접 구현해보기 (executor 없이 `poll` 수동 호출)

---

### 참고 자료

- [The Rust Book (공식)](https://doc.rust-lang.org/book/) — 2~11단계 대부분이 이 순서를 따름
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) — 문법 항목별 짧은 실행 가능 예제
- [Rustlings](https://github.com/rust-lang/rustlings) — 컴파일 에러를 직접 고치며 배우는 연습 문제 세트, 문법 훈련에 특히 좋음
- [Rust Reference](https://doc.rust-lang.org/reference/) — 문법의 정확한 스펙이 필요할 때 (사전 용도)
- Jon Gjengset의 "Crust of Rust" 유튜브 시리즈 — 라이프타임, 트레잇 객체, 스마트 포인터 등 심화 주제를 코드 레벨로 파고듦 (7, 9, 11단계와 특히 잘 맞음)

---

### 체크리스트 요약

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

---

## 1단계. 기본 문법

 
---
 
### 1.1 변수 바인딩 — `let` / `let mut` / `const` / `static`
 
#### 개념
 
Rust에서 `let`으로 만든 변수는 **기본적으로 불변(immutable)**입니다. 다른 언어와 반대 방향의 기본값이라는 게 핵심입니다.
 
```rust
fn main() {
    let x = 5;
    println!("x = {x}");
 
    x = 6; // 컴파일 에러! x는 불변
}
```
 
컴파일러가 이렇게 알려줍니다:
 
```
error[E0384]: cannot assign twice to immutable variable `x`
```
 
가변으로 만들려면 `mut` 키워드를 붙입니다.
 
```rust
fn main() {
    let mut x = 5;
    println!("x = {x}");
    x = 6; // OK
    println!("x = {x}");
}
```
 
#### `let`의 재바인딩(shadowing) — 다른 언어에는 없는 개념
 
```rust
fn main() {
    let x = 5;
    let x = x + 1;      // 새로운 x (타입도 바뀔 수 있음)
    let x = x * 2;
    println!("x = {x}"); // 12
 
    let spaces = "   ";       // &str 3개
    let spaces = spaces.len(); // usize — 완전히 다른 타입으로 재바인딩 가능
    println!("spaces = {spaces}"); //3
}
```
 
`shadowing`은 `mut`과 다릅니다. `mut`은 같은 메모리 위치의 값을 바꾸는 것이고, shadowing은 **완전히 새로운 변수를 같은 이름으로 다시 만드는 것**입니다. 그래서 타입까지 바뀔 수 있습니다.
 
#### `const` — 컴파일 타임 상수
 
```rust
const MAX_CONNECTIONS: u32 = 100_000; //u32 형식은 최대 10자리(4,294,967,295)까지 설정 가능
const PI: f64 = 3.14159265358979; //최대 저장 가능 값: 파이의 실제 값인 3.141592653589793238...에서 보듯, 소수점 15번째 자리부터는 반올림되거나 잘려서 저장
 
fn main() {
    println!("MAX_CONNECTIONS = {MAX_CONNECTIONS}");
}
```
 
- `const`는 **항상 타입을 명시**해야 합니다 (`let`은 추론 가능하지만 `const`는 불가).
- 값은 반드시 컴파일 타임에 계산 가능해야 합니다 (함수 호출 결과 등은 안 됨, 단 `const fn`은 예외).
- `mut`을 붙일 수 없습니다. 절대 불변.
- 관례상 이름은 `SCREAMING_SNAKE_CASE`.

#### `static` — 프로그램 전역에 고정된 메모리 주소를 갖는 값
 
```rust
static GREETING: &str = "Hello, Rust!";
static mut COUNTER: u32 = 0; // unsafe 없이는 접근 불가 (2024 에디션 이후 더 엄격해짐)
 
fn main() {
    println!("{GREETING}");
}
```
 
`const`와 `static`의 차이:
 
| | `const` | `static` |
|---|---|---|
| 메모리 주소 | 매 사용처마다 인라인됨 (주소 없음) | 프로그램 전체에서 고정된 하나의 주소 |
| 가변성 | 항상 불변 | `mut` 가능하지만 `unsafe` 필요 (스레드 안전성 책임을 프로그래머가 짐) |
| 용도 | 매직 넘버, 설정값 | 전역 카운터, 전역 캐시 등 (실무에서는 `static`보다 `once_cell`/`std::sync::OnceLock` + `Arc` 조합을 훨씬 많이 씀) |
 
> **실무 팁**: axum 서버에서 "전역 상태"가 필요하면 `static mut`을 쓰지 말고, `AppState`를 `Arc`로 감싸서 핸들러에 주입하는 패턴(2단계에서 다룰 내용)을 쓰는 게 정석입니다. `static mut`은 데이터 레이스 위험 때문에 최근 Rust 에디션에서 점점 더 강하게 제한되고 있습니다.
 
#### 체크포인트
 
- [ ] `let mut x = 0;`로 시작해서 반복문 안에서 값을 누적하는 코드를 작성해보기
- [ ] shadowing을 이용해 문자열을 파싱한 숫자로 재바인딩하는 코드 작성 (`let s = "42"; let s: i32 = s.parse().unwrap();`)
- [ ] `const`로 상수 하나 선언 후 타입을 생략했을 때 나오는 에러 메시지 직접 확인
---
 
### 1.2 주석 — `//`, `/* */`, 문서 주석

#### 개념

```rust
// 한 줄 주석 — 줄 끝까지 무시됨

/*
   여러 줄 블록 주석
   /* 이렇게 중첩도 가능 — C와 다른 점 */
*/

/// 문서 주석(doc comment) — 바로 아래 아이템(함수/구조체 등)을 문서화
/// `cargo doc --open`으로 HTML 문서를 뽑아낼 수 있음
fn add(a: i32, b: i32) -> i32 {
    a + b // 코드 뒤에 붙이는 인라인 주석
}

//! 모듈/크레이트 전체를 설명하는 내부 문서 주석 (보통 파일 맨 위에 씀)

fn main() {
    println!("{}", add(1, 2));
}
```

`///`와 `//!`는 그냥 주석이 아니라 **문서화 도구가 읽는 문법**입니다. `///`는 바로 다음에 오는 아이템을, `//!`는 자신이 속한 모듈/크레이트 전체를 설명합니다. `serde`, `tokio` 같은 크레이트의 docs.rs 페이지가 바로 이 주석들로 만들어집니다.

#### 체크포인트

- [ ] `cargo doc --open`을 실행해서 `///` 주석이 실제 HTML 문서로 렌더링되는 걸 직접 확인
- [ ] 블록 주석(`/* */`) 안에 블록 주석을 중첩해보고 정상 동작하는지 확인 (C/C++에서는 안 되는 경우가 많음)
---
 
### 1.3 스칼라 타입 & 복합 타입
 
#### 정수 타입과 오버플로우
 
Rust의 정수 타입은 크기와 부호가 이름에 그대로 드러납니다: `i8, i16, i32, i64, i128, isize` / `u8, u16, u32, u64, u128, usize`.
 
```rust
fn main() {
    let a: u8 = 255;
    // let b = a + 1; // debug 빌드에서는 panic! ("attempt to add with overflow")
 
    let b = a.wrapping_add(1); // 명시적으로 "감싸기(wrap)" 연산
    println!("b = {b}"); // 0
 
    let c = a.checked_add(1); // Option<u8> 반환, 오버플로우면 None
    println!("c = {c:?}"); // None
 
    let (d, overflowed) = a.overflowing_add(1);
    println!("d = {d}, overflowed = {overflowed}"); // d = 0, overflowed = true
 
    let e = a.saturating_add(1); // 최댓값에서 멈춤
    println!("e = {e}"); // 255
}
```
 
**중요한 포인트**: debug 빌드에서는 오버플로우 시 `panic!`이 발생하지만, `--release` 빌드에서는 **기본적으로 wrapping** 동작을 합니다 (성능을 위해 체크를 생략). 이 차이 때문에 "로컬에서는 잘 됐는데 배포하니 이상한 값이 나온다"는 버그가 종종 발생합니다. 오버플로우 가능성이 있는 연산은 반드시 `checked_*`, `wrapping_*`, `saturating_*` 중 의도에 맞는 걸 명시적으로 쓰는 게 좋습니다.
 
#### 부동소수점 타입
 
```rust
fn main() {
    let x: f64 = 2.0;      // 기본값은 f64
    let y: f32 = 3.0;      // 32비트가 필요하면 명시
 
    println!("{}", 0.1 + 0.2); // 0.30000000000000004 — 다른 언어와 동일한 부동소수점 오차
 
    // NaN 비교 함정
    let nan = f64::NAN;
    println!("{}", nan == nan); // false! NaN은 자기 자신과도 같지 않음
}
```
 
#### 불리언과 문자 타입 — `bool`, `char`
 
```rust
fn main() {
    let is_active: bool = true;
    let is_admin = false;
 
    if is_active && !is_admin {
        println!("일반 활성 사용자");
    }
 
    let c: char = 'A';   // 작은따옴표! char는 문자열(&str)과 다른 타입
    let heart = '❤';     // 이모지 하나도 char 하나
    let ch_kr = '가';
 
    println!("{c}, {heart}, {ch_kr}");
    println!("char 크기: {} 바이트", std::mem::size_of::<char>()); // 4
}
```
 
> **주의**: Rust의 `char`는 4바이트 **유니코드 스칼라 값**입니다. C/C++의 1바이트 `char`와 이름은 같지만 완전히 다른 개념입니다. 문자열 안에서는 UTF-8로 인코딩되므로, "가"처럼 `char` 1개가 UTF-8로는 3바이트를 차지하는 경우가 흔합니다 (문자열 인덱싱이 바이트 인덱스인 이유와 연결됨).
 
#### 타입 캐스팅 — `as`
 
```rust
fn main() {
    let f = 3.9_f64;
    let i = f as i32; // 3 — 소수점 버림(truncation), 반올림 아님
    println!("{i}");
 
    let big: i64 = 300;
    let small = big as u8; // 300 % 256 = 44 — 잘림, 오버플로우 체크 없음!
    println!("{small}");
 
    let code = 'A' as u32; // 65 — char → 정수 코드포인트
    println!("{code}");
 
    let back = 65u8 as char; // 'A' — u8 → char는 항상 안전해서 직접 캐스팅 가능
    println!("{back}");
}
```
 
> `as`는 "값이 잘릴 수도 있다"는 걸 코드에 명시적으로 드러내는 캐스팅입니다. Rust는 암묵적 타입 변환을 허용하지 않기 때문에(다른 언어처럼 `int`가 자동으로 `long`이 되는 일이 없음) 타입을 바꾸려면 항상 `as`나 `From`/`Into`(4단계)를 명시해야 합니다. 값 손실이 없다는 걸 보장받고 싶다면 `as` 대신 `TryFrom`/`try_into()`를 쓰는 게 더 안전합니다.
 
#### 튜플
 
```rust
fn main() {
    let point: (f64, f64, &str) = (3.0, 4.0, "origin-offset");
 
    // 구조 분해(destructuring)
    let (x, y, label) = point;
    println!("{x}, {y}, {label}");
 
    // 인덱스로 접근
    println!("x = {}", point.0);
 
    // 단위 튜플(unit) — 사실상 "값 없음"을 표현
    let unit: () = ();
}
```
 
#### 배열 — 고정 크기, 스택에 할당
 
```rust
fn main() {
    let arr: [i32; 5] = [1, 2, 3, 4, 5];
    let zeros = [0; 10]; // [0, 0, ..., 0] 길이 10짜리 배열
 
    println!("첫 원소: {}", arr[0]);
    println!("길이: {}", arr.len());
 
    // 범위를 벗어나면?
    let idx = 10;
    // println!("{}", arr[idx]); // 컴파일은 되지만 런타임에 panic! (인덱스 아웃 오브 바운드)
}
```
 
#### 슬라이스 — 배열/벡터의 "일부를 참조"하는 뷰
 
```rust
fn main() {
    let arr = [10, 20, 30, 40, 50];
 
    let slice: &[i32] = &arr[1..4]; // [20, 30, 40] — arr을 소유하지 않고 "빌려서" 봄
    println!("{slice:?}");
 
    // 문자열 슬라이스 &str도 원리는 동일
    let s = String::from("hello world");
    let hello: &str = &s[0..5];
    let world: &str = &s[6..11];
    println!("{hello} / {world}");
}
```
 
> 슬라이스는 "포인터 + 길이" 두 개로 이루어진 fat pointer입니다. `arr`을 복사하지 않고 그 일부를 가리키기만 하기 때문에, 2단계(소유권/빌림)에서 다룰 "빌림(borrow)"의 가장 흔한 실제 사례가 바로 슬라이스입니다.
 
#### 체크포인트
 
- [ ] `wrapping_add`, `checked_add`, `saturating_add`를 각각 실행해보고 차이를 출력으로 확인
- [ ] `f64::NAN`, `f64::INFINITY`를 가지고 비교 연산 몇 가지 실험
- [ ] 길이 5인 배열을 슬라이스로 잘라서 합계를 구하는 함수 작성 (`fn sum(slice: &[i32]) -> i32`)
- [ ] `'가'`처럼 한글 한 글자를 `char`에 담고 `size_of::<char>()`와 UTF-8 바이트 길이를 비교해보기
- [ ] `300i64 as u8`처럼 값이 잘리는 캐스팅을 직접 해보고 결과값이 왜 그렇게 나오는지 계산으로 검증
---
 
### 1.4 출력 매크로와 포맷팅 문법 — `println!` / `format!`
 
#### 개념
 
```rust
fn main() {
    let name = "Rust";
    let version = 1.75;
 
    println!("Hello, {name}! (v{version})");         // 변수 이름을 {} 안에 직접 캡처
    println!("{} is {}", name, "awesome");            // 위치 기반 {}
    println!("{0} - {1} - {0}", "a", "b");            // 인덱스로 인자 재사용 → a - b - a
    println!("{lang} v{v}", lang = "Rust", v = 1.75); // 이름 있는 인자
 
    #[derive(Debug)]
    struct Point { x: i32, y: i32 }
    let p = Point { x: 1, y: 2 };
    println!("{p:?}");  // Debug 포맷 — Point { x: 1, y: 2 }
    println!("{p:#?}"); // pretty-print Debug — 여러 줄로 들여쓰기
 
    println!("{:>8}|", "hi");   // 오른쪽 정렬, 폭 8
    println!("{:<8}|", "hi");   // 왼쪽 정렬, 폭 8
    println!("{:^8}|", "hi");   // 가운데 정렬
    println!("{:05}", 42);      // 0으로 채워서 폭 5 → 00042
    println!("{:.2}", 3.14159); // 소수점 둘째 자리까지 → 3.14
    println!("{:x}", 255);      // 16진수 → ff
    println!("{:b}", 10);       // 2진수 → 1010
 
    let s: String = format!("{name}-{version}"); // 출력하지 않고 String으로 조립
    eprintln!("에러 로그는 stderr로: {s}");       // println!과 동일 문법, stderr에 출력
}
```
 
`{}`는 `Display` 트레잇을, `{:?}`는 `Debug` 트레잇을 요구합니다. 커스텀 타입을 `println!("{}", x)`로 그대로 출력하려면 `Display`를 직접 구현해야 하고(3단계에서 다룸), `#[derive(Debug)]`만 붙이면 `{:?}`는 바로 쓸 수 있습니다. 실무에서는 로그·디버깅용으로 새 타입에 일단 `#[derive(Debug)]`부터 붙여두는 습관이 유용합니다.
 
#### 체크포인트
 
- [ ] 이름 있는 인자(`{lang}` 형태)와 위치 인자(`{0}`)를 섞어서 문자열 하나 조립해보기
- [ ] `#[derive(Debug)]`가 없는 구조체를 `{:?}`로 출력했을 때 나오는 컴파일 에러 메시지 확인
- [ ] `{:.2}`, `{:>10}`, `{:05}`를 조합해서 숫자 목록을 표처럼 정렬 출력해보기
---
 
### 1.5 함수, 표현식 vs 문장
 
#### 핵심 규칙: Rust는 "표현식 지향(expression-oriented)" 언어
 
```rust
fn main() {
    let y = {
        let a = 3;
        let b = 4;
        a * a + b * b // 세미콜론 없음 → 이 블록의 "값"이 됨
    };
    println!("y = {y}"); // 25
}
```
 
블록 `{ ... }`의 마지막 줄에 세미콜론이 없으면 그게 블록 전체의 반환값이 됩니다. 세미콜론을 붙이면 그 줄은 "문장(statement)"이 되어 `()` (unit 타입)을 반환합니다.
 
```rust
fn main() {
    let y = {
        let a = 3;
        a * a; // 세미콜론 있음 → 이 블록은 () 반환
    };
    println!("{y:?}"); // ()
}
```
 
#### 함수도 결국 같은 규칙
 
```rust
fn square(x: i32) -> i32 {
    x * x // return 키워드 없이, 세미콜론 없이 → 함수의 반환값
}
 
fn square_explicit(x: i32) -> i32 {
    return x * x; // 명시적 return도 가능 (주로 조기 반환에 사용)
}
 
fn early_return(x: i32) -> i32 {
    if x < 0 {
        return 0; // 조건 만족 시 즉시 반환
    }
    x * x // 나머지 경우 여기로 흐름
}
```
 
#### `if`도 표현식이다
 
다른 언어와 다르게, Rust의 `if`는 값을 만들어낼 수 있습니다.
 
```rust
fn main() {
    let condition = true;
    let number = if condition { 5 } else { 6 }; // 삼항 연산자 대신 이렇게 씀
    println!("number = {number}");
 
    // 주의: 두 분기의 타입이 반드시 같아야 함
    // let number = if condition { 5 } else { "six" }; // 컴파일 에러!
}
```
 
#### 이 규칙이 왜 중요한가 (실전 연결)
 
axum 핸들러를 짜다 보면 이런 패턴을 아주 많이 씁니다:
 
```rust
fn classify(score: i32) -> &'static str {
    if score >= 90 {
        "A"
    } else if score >= 80 {
        "B"
    } else {
        "F"
    }
}
```
 
`return` 없이 마지막 표현식이 곧 반환값이 되는 이 문법을 체화해두면, 이후 `match` 기반 에러 처리, `Result` 체이닝 코드를 읽을 때 훨씬 자연스럽게 읽힙니다.
 
#### 체크포인트
 
- [ ] 세미콜론 하나를 일부러 빼먹거나 추가해서 컴파일 에러/타입 변화를 직접 확인
- [ ] `if` 표현식으로 두 숫자 중 큰 값을 반환하는 함수 작성 (`std::cmp::max` 쓰지 말고 직접)
- [ ] 함수 안에서 블록 표현식을 이용해 중간 계산을 캡슐화하는 코드 작성
---
 
### 1.6 제어 흐름 — `if let`, `while let`, `for`, `loop`의 반환값
 
#### `if let` — `match`의 축약형
 
`Option`이나 `Result` 중 "한 가지 패턴만" 관심 있을 때 `match`는 장황합니다.
 
```rust
fn main() {
    let config_value: Option<i32> = Some(3);
 
    // match로 쓰면
    match config_value {
        Some(v) => println!("설정값: {v}"),
        None => {}
    }
 
    // if let으로 쓰면 훨씬 간결
    if let Some(v) = config_value {
        println!("설정값: {v}");
    }
 
    // else와 결합 가능
    if let Some(v) = config_value {
        println!("설정값: {v}");
    } else {
        println!("설정 없음, 기본값 사용");
    }
}
```
 
#### `while let` — 패턴이 맞는 동안 반복
 
```rust
fn main() {
    let mut stack = vec![1, 2, 3, 4, 5];
 
    while let Some(top) = stack.pop() {
        println!("pop: {top}");
    }
    // 5, 4, 3, 2, 1 순서로 출력, stack이 비면(None) 자동 종료
}
```
 
`while let`은 이후 3단계에서 채널을 다룰 때(`while let Some(msg) = rx.recv().await`) 그대로 다시 나오는 아주 중요한 패턴입니다.
 
#### `for` 루프와 Range(`..`, `..=`)
 
```rust
fn main() {
    for i in 0..5 {          // 0,1,2,3,4 — 5는 포함 안 됨(반개방 구간)
        print!("{i} ");
    }
    println!();
 
    for i in 0..=5 {         // 0,1,2,3,4,5 — 5까지 포함
        print!("{i} ");
    }
    println!();
 
    for i in (0..10).step_by(2) { // 0,2,4,6,8
        print!("{i} ");
    }
    println!();
 
    for i in (0..5).rev() {  // 4,3,2,1,0 — 역순
        print!("{i} ");
    }
    println!();
 
    let arr = [10, 20, 30];
    for x in arr {            // 배열/컬렉션을 직접 순회
        print!("{x} ");
    }
    println!();
 
    for (i, x) in arr.iter().enumerate() { // 인덱스와 값을 동시에
        println!("{i}: {x}");
    }
}
```
 
> C/C++의 `for (int i = 0; i < n; i++)` 같은 3-부분 for문은 Rust에 **없습니다**. Rust의 `for`는 항상 "패턴을 이터레이터에 대해 순회"하는 형태(`for 패턴 in 이터레이터`)뿐입니다. 인덱스 기반 반복이 필요할 때도 `0..n` 같은 Range를 순회하는 식으로 씁니다. Range + `enumerate()` 조합이 사실상 다른 언어의 전통적 for문 자리를 대신한다고 보면 됩니다. (이터레이터 자체는 5단계에서 훨씬 깊게 다룹니다.)
 
#### `loop`의 반환값 — `break value`
 
Rust의 `loop`는 무한 루프이면서 동시에 **값을 반환할 수 있는 표현식**입니다.
 
```rust
fn main() {
    let mut counter = 0;
 
    let result = loop {
        counter += 1;
        if counter == 10 {
            break counter * 2; // break에 값을 실어서 loop 표현식의 결과로 만듦
        }
    };
 
    println!("result = {result}"); // 20
}
```
 
#### 라벨(label)이 붙은 루프 — 중첩 루프 제어
 
```rust
fn main() {
    let mut count = 0;
    'outer: loop {
        let mut remaining = 10;
        loop {
            if remaining == 9 {
                break; // 안쪽 loop만 탈출
            }
            if count == 2 {
                break 'outer; // 바깥 loop까지 탈출
            }
            remaining -= 1;
        }
        count += 1;
    }
    println!("count = {count}");
}
```
 
#### 체크포인트
 
- [ ] `Result<i32, String>` 값을 `if let Ok(v) = ...` 패턴으로 처리해보기
- [ ] `Vec`을 만들고 `while let Some(x) = iter.next()`로 순회 (이터레이터는 5단계에서 더 깊게 다룸)
- [ ] `for i in (1..=100).rev()`로 100부터 1까지 역순 출력, `step_by`로 짝수만 걸러 출력해보기
- [ ] `loop { ... break value; }`로 "1부터 시작해서 처음으로 1000을 넘는 3의 배수"를 찾는 코드 작성
---
 
### 1.7 `match`와 패턴 매칭 기초
 
#### 기본 형태 — 반드시 모든 경우를 다뤄야 함(exhaustiveness)
 
```rust
fn describe(n: i32) -> &'static str {
    match n {
        0 => "zero",
        1 => "one",
        _ => "many", // 나머지 모든 경우 — 이게 없으면 컴파일 에러
    }
}
```
 
Rust 컴파일러는 `match`의 모든 경우가 다뤄졌는지 **컴파일 타임에 검사**합니다. 이게 `switch`문과의 결정적 차이이며, "런타임에 처리 안 된 케이스가 튀어나오는" 버그를 원천 차단합니다.
 
#### 여러 값을 한 번에 — `|` 패턴
 
```rust
fn is_weekend(day: u32) -> bool {
    match day {
        6 | 7 => true,
        _ => false,
    }
}
```
 
#### 범위 패턴 — `..=`
 
```rust
fn grade(score: u32) -> char {
    match score {
        90..=100 => 'A',
        80..=89 => 'B',
        70..=79 => 'C',
        0..=69 => 'F',
        _ => panic!("잘못된 점수"), // 100 초과값 방어
    }
}
```
 
#### 구조 분해 패턴 — 튜플, 구조체, enum 속을 바로 들여다보기
 
```rust
fn main() {
    let point = (3, -4);
 
    match point {
        (0, 0) => println!("원점"),
        (x, 0) => println!("x축 위, x = {x}"),
        (0, y) => println!("y축 위, y = {y}"),
        (x, y) if x == y => println!("y = x 위, ({x}, {y})"), // match guard
        (x, y) => println!("일반 점, ({x}, {y})"),
    }
}
```
 
`if x == y` 부분처럼 패턴 뒤에 조건을 추가로 붙이는 걸 **match guard**라고 합니다.
 
#### `enum`과 결합했을 때가 진짜 진가를 발휘하는 지점
 
```rust
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Triangle { base: f64, height: f64 },
}
 
fn area(shape: &Shape) -> f64 {
    match shape {
        Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
        Shape::Rectangle { width, height } => width * height,
        Shape::Triangle { base, height } => 0.5 * base * height,
    }
}
 
fn main() {
    let shapes = vec![
        Shape::Circle { radius: 2.0 },
        Shape::Rectangle { width: 3.0, height: 4.0 },
        Shape::Triangle { base: 5.0, height: 6.0 },
    ];
 
    for s in &shapes {
        println!("area = {:.2}", area(s));
    }
}
```
 
나중에 새로운 `Shape` variant(예: `Pentagon`)를 추가하면, `area` 함수의 `match`가 컴파일 에러를 내서 "새 케이스 처리 안 했다"고 바로 알려줍니다. 이게 Rust enum + match 조합이 상태 관리에 강력한 이유입니다.
 
#### `@` 바인딩 — 패턴에 매치하면서 값도 변수로 갖고 싶을 때
 
```rust
fn main() {
    let msg_id = 5;
 
    match msg_id {
        id @ 3..=7 => println!("범위 안의 id: {id}"),
        id => println!("범위 밖의 id: {id}"),
    }
}
```
 
#### 체크포인트
 
- [ ] `Option<i32>`를 `match`로 처리하되 `Some(n) if n > 0` 형태의 match guard 사용해보기
- [ ] 3개 이상의 variant를 가진 `enum`을 만들고 `match`에서 하나를 일부러 빼먹었을 때 나오는 컴파일 에러 확인
- [ ] 튜플 구조 분해로 좌표 4개(원점, x축, y축, 일반)를 분류하는 함수 작성
---
 
### 1.8 종합 실습 — 신호등 상태 머신
 
지금까지 배운 걸 전부 사용하는 미니 프로젝트입니다: `enum` + `match` + `if let` + `loop` + 표현식 지향 스타일.
 
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum TrafficLight {
    Red,
    Yellow,
    Green,
}
 
impl TrafficLight {
    /// 다음 상태로 전이 — match가 표현식이므로 return 없이 바로 반환
    fn next(self) -> TrafficLight {
        match self {
            TrafficLight::Red => TrafficLight::Green,
            TrafficLight::Green => TrafficLight::Yellow,
            TrafficLight::Yellow => TrafficLight::Red,
        }
    }
 
    /// 각 상태가 지속되는 시간(초) — 실무에서 설정값을 이렇게 상태에 붙여두는 패턴 자주 씀
    fn duration_secs(self) -> u32 {
        match self {
            TrafficLight::Red => 5,
            TrafficLight::Yellow => 2,
            TrafficLight::Green => 4,
        }
    }
 
    fn label(self) -> &'static str {
        match self {
            TrafficLight::Red => "🔴 RED",
            TrafficLight::Yellow => "🟡 YELLOW",
            TrafficLight::Green => "🟢 GREEN",
        }
    }
}
 
fn main() {
    let mut light = TrafficLight::Red;
    let mut total_elapsed = 0;
    let mut cycle_count = 0;
 
    // loop + break value 활용: "몇 초 뒤에 3번째 사이클이 끝나는지" 계산
    let time_for_three_cycles = loop {
        println!(
            "[{total_elapsed:>3}s] {} (지속 {}초)",
            light.label(),
            light.duration_secs()
        );
 
        total_elapsed += light.duration_secs();
 
        // if let으로 "빨간불에서 초록불로 넘어가는 순간"만 감지해서 사이클 카운트
        let prev = light;
        light = light.next();
        if let (TrafficLight::Yellow, TrafficLight::Red) = (prev, light) {
            cycle_count += 1;
            if cycle_count == 3 {
                break total_elapsed;
            }
        }
    };
 
    println!("---");
    println!("3번째 사이클 종료까지 걸린 시간: {time_for_three_cycles}초");
}
```
 
#### 이 예제에서 짚어야 할 포인트
 
1. **`#[derive(Debug, Clone, Copy, PartialEq)]`**: `TrafficLight`가 크기가 작고 단순하니 `Copy`를 붙여서 `self`를 값으로 받아도(`fn next(self)`) 소유권 이동 걱정 없이 쓸 수 있게 했습니다. (이 부분은 2단계 소유권에서 "왜 이게 가능한지" 다시 다룹니다.)
2. **`match`가 표현식**이라 `next()`, `duration_secs()`, `label()` 모두 `return` 없이 마지막 값이 그대로 반환됩니다.
3. **`if let (A, B) = (x, y)`** 처럼 튜플로 묶어서 "이전 상태와 다음 상태의 조합"을 한 번에 패턴 매칭하는 실용적인 트릭을 보여줍니다.
4. **`loop { ... break value; }`**로 "특정 조건이 될 때까지 반복하고 그 시점의 계산 결과를 변수에 담는" 패턴을 실전처럼 사용했습니다.
#### 확장 과제 (선택)
 
- [ ] `TrafficLight`에 `Blinking` (점멸) 상태를 추가하고 `match`가 컴파일 에러를 내는 지점들을 모두 고쳐보기 (exhaustiveness 체감용)
- [ ] `duration_secs()`를 외부 설정(하드코딩 대신 함수 인자)으로 받도록 리팩터링
- [ ] 신호등 상태 변화를 `Vec<TrafficLight>`에 기록했다가 마지막에 전체 이력을 출력
---

---

> **다음:** [2단계. 소유권 & 빌림](#2단계-소유권-빌림)

## 2단계. 소유권 & 빌림

> 이어서: 앞 장의 [1단계. 기본 문법](#1단계-기본-문법)에서 이어집니다.

---

### 2.1 소유권 이동(move) vs 복사(`Copy`)

#### 개념 — Rust의 기본 규칙 세 가지

1. **모든 값은 소유자(owner)가 하나**다.
2. **소유자가 스코프를 벗어나면 값이 drop**된다 (메모리 해제).
3. **소유권을 넘기면(move) 이전 변수는 더 이상 쓸 수 없다.**

이 세 규칙이 GC도 없고 `free`도 수동으로 안 해도 메모리 안전성이 보장되는 이유입니다.

#### 이동(move) — `String`으로 보기

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1; // s1의 소유권이 s2로 이동(move)

    // println!("{s1}"); // 컴파일 에러!
    println!("{s2}");    // OK
}
```

에러 메시지:

```
error[E0382]: borrow of moved value: `s1`
  |
  |     let s1 = String::from("hello");
  |         -- move occurs because `s1` has type `String`, which does not implement the `Copy` trait
  |     let s2 = s1;
  |              -- value moved here
  |     println!("{s1}");
  |                ^^ value borrowed here after move
```

`String`은 힙에 데이터를 들고 있어서, 단순 대입이 "포인터 복사 + 두 변수가 같은 힙을 가리킴"이 되면 이중 해제(double free)가 됩니다. Rust는 그걸 막기 위해 **대입 시 소유권을 옮기고, 이전 이름은 무효화**합니다.

내부적으로 `s1`은 대략 이런 구조입니다:

```
s1: { ptr ──► [h][e][l][l][o]  (힙)
      len: 5
      cap: 5 }
```

`let s2 = s1;`은 스택의 `{ptr, len, cap}`만 `s2`로 옮기고, `s1`은 무효화합니다. 힙 데이터는 복사하지 않습니다 (얕은 이동).

#### 깊은 복사가 필요하면 — `.clone()`

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1.clone(); // 힙 데이터까지 복사 — 비용 있음

    println!("s1 = {s1}, s2 = {s2}"); // 둘 다 OK
}
```

`.clone()`은 "의도적으로 비싸다"는 신호를 코드에 남깁니다. 실무에서 무분별한 `clone()`은 성능 냄새이지만, **소유권 문제를 일단 통과시키기 위해 clone을 남발하는 단계**는 학습 초기에 흔합니다. 나중에 빌림으로 고치면 됩니다.

#### `Copy` 타입 — 이동 대신 비트 복사

스택에만 있고 크기가 작으며, drop 시 특별한 정리가 필요 없는 타입은 `Copy` trait을 구현합니다. 이런 타입은 대입해도 이전 변수가 무효화되지 않습니다.

```rust
fn main() {
    let x = 5;      // i32는 Copy
    let y = x;      // 비트 복사 — x는 여전히 유효
    println!("x = {x}, y = {y}");

    let t = (1, 2.0, true); // 모든 필드가 Copy면 튜플도 Copy
    let t2 = t;
    println!("{t:?}, {t2:?}");
}
```

대표적인 `Copy` 타입:

| 타입 | Copy? | 이유 |
|---|---|---|
| 정수/부동소수/bool/char | ✅ | 스택만, drop 불필요 |
| 튜플/배열 (원소가 전부 Copy) | ✅ | 위와 동일 |
| `&T` (불변 참조) | ✅ | 참조 자체는 복사 가능 (빌림 규칙으로 안전성 보장) |
| `&mut T` | ❌ | 가변 참조는 유일해야 하므로 Copy 불가 |
| `String`, `Vec<T>`, `Box<T>` | ❌ | 힙 소유, drop 시 해제 필요 |
| 직접 만든 struct (기본) | ❌ | `#[derive(Copy, Clone)]`를 붙여야 함 |

```rust
#[derive(Debug, Clone, Copy)] // Copy를 쓰려면 Clone도 함께 필요
struct Point {
    x: i32,
    y: i32,
}

// String 필드는 Copy 불가 — 컴파일 에러
// #[derive(Copy, Clone)]
// struct Person { name: String }

fn main() {
    let p1 = Point { x: 1, y: 2 };
    let p2 = p1; // Copy
    println!("{p1:?}, {p2:?}");
}
```

1단계 신호등 예제에서 `TrafficLight`에 `Copy`를 붙인 이유가 바로 이것입니다. enum variant에 힙 데이터가 없으니 `Copy`가 가능했고, `fn next(self)`처럼 값으로 받아도 호출 측 변수가 죽지 않았습니다.

#### 함수 인자로 넘길 때도 이동한다

```rust
fn take_ownership(s: String) {
    println!("받음: {s}");
} // 여기서 s가 drop

fn make_copy(n: i32) {
    println!("복사본: {n}");
}

fn main() {
    let s = String::from("hello");
    take_ownership(s);
    // println!("{s}"); // 에러 — 이미 이동됨

    let n = 42;
    make_copy(n);
    println!("{n}"); // OK — i32는 Copy
}
```

소유권을 함수에 넘긴 뒤 **다시 돌려받고 싶으면** 반환하면 됩니다:

```rust
fn take_and_give_back(s: String) -> String {
    s // 소유권을 호출자에게 반환
}

fn main() {
    let s1 = String::from("hello");
    let s2 = take_and_give_back(s1);
    println!("{s2}");
}
```

하지만 이런 "받았다가 다시 돌려주기"는 장황합니다. **그래서 빌림(borrow)이 있습니다.**

#### 체크포인트

- [ ] `String`을 대입한 뒤 이전 변수를 출력해서 에러 메시지를 직접 읽어보기
- [ ] `Vec<i32>`도 `String`과 같은지 확인 (move vs Copy)
- [ ] `#[derive(Copy, Clone)]`를 `String` 필드가 있는 struct에 붙여보고 에러 확인
- [ ] 함수에 `String`을 넘긴 뒤 반환으로 소유권을 되찾는 코드 작성

---

### 2.2 빌림 규칙 — `&T` vs `&mut T`, NLL

#### 개념

값을 **소유하지 않고 참조만** 하는 것이 빌림입니다.

- `&T` — 불변 참조 (공유 빌림). **동시에 여러 개** 가능.
- `&mut T` — 가변 참조 (배타적 빌림). **동시에 딱 하나**만 가능.
- 불변 참조와 가변 참조는 **동시에 존재할 수 없다.**

이 규칙이 데이터 레이스와 use-after-free를 **컴파일 타임에** 막는 핵심입니다.

#### 불변 참조 — 읽기만, 여러 개 OK

```rust
fn calculate_length(s: &String) -> usize {
    s.len()
    // s.push_str("!"); // 에러 — &String으로는 수정 불가
} // s는 참조일 뿐이라 drop하지 않음. 소유권은 호출자에게 그대로.

fn main() {
    let s1 = String::from("hello");
    let len = calculate_length(&s1);
    println!("'{s1}' 의 길이: {len}"); // s1 여전히 유효!
}
```

불변 참조는 여러 개를 동시에 가져도 됩니다:

```rust
fn main() {
    let s = String::from("hello");
    let r1 = &s;
    let r2 = &s;
    println!("{r1}, {r2}"); // OK
}
```

#### 가변 참조 — 수정 가능, 단 하나

```rust
fn append_world(s: &mut String) {
    s.push_str(", world");
}

fn main() {
    let mut s = String::from("hello"); // 원본도 mut이어야 &mut을 빌릴 수 있음
    append_world(&mut s);
    println!("{s}"); // "hello, world"
}
```

가변 참조를 두 개 동시에 만들면:

```rust
fn main() {
    let mut s = String::from("hello");
    let r1 = &mut s;
    let r2 = &mut s; // 컴파일 에러!
    println!("{r1}, {r2}");
}
```

```
error[E0499]: cannot borrow `s` as mutable more than once at a time
```

불변 + 가변을 섞어도:

```rust
fn main() {
    let mut s = String::from("hello");
    let r1 = &s;
    let r2 = &mut s; // 에러 — 불변 참조가 살아있는 동안 가변 참조 불가
    println!("{r1}, {r2}");
}
```

```
error[E0502]: cannot borrow `s` as mutable because it is also borrowed as immutable
```

#### NLL (Non-Lexical Lifetime) — "마지막 사용 지점"까지가 수명

예전 Rust는 참조의 수명이 **중괄호 스코프 전체**였습니다. 지금은 **실제로 마지막으로 쓰인 지점**까지입니다.

```rust
fn main() {
    let mut s = String::from("hello");

    let r1 = &s;
    let r2 = &s;
    println!("{r1}, {r2}");
    // 여기서 r1, r2의 "수명"이 끝남 (이후 사용 없음) — NLL

    let r3 = &mut s; // OK! 불변 참조는 이미 끝난 상태
    println!("{r3}");
}
```

NLL 덕분에 "참조를 쓴 뒤에는 다시 가변으로 빌릴 수 있는" 자연스러운 코드가 컴파일됩니다. 다만 **같은 줄에서 불변·가변을 동시에 쓰면** 여전히 막힙니다.

#### dangling reference는 컴파일러가 막는다

```rust
fn dangle() -> &String { // 수명 문제로 컴파일 불가
    let s = String::from("hello");
    &s // s는 함수 끝에서 drop → 반환된 참조는 허공
}

// 올바른 방법: 소유권을 반환
fn no_dangle() -> String {
    let s = String::from("hello");
    s
}
```

```
error[E0106]: missing lifetime specifier
```

또는 (시그니처를 억지로 맞추려 하면):

```
error[E0515]: cannot return reference to local variable `s`
```

#### 실무에서 자주 보는 패턴 — 함수는 보통 빌림으로 받는다

```rust
// ❌ 소유권을 뺏어감 — 호출자가 이후 s를 못 씀
fn bad_print(s: String) {
    println!("{s}");
}

// ✅ 읽기만 하면 됨 → &str (더 일반적, 아래 2.3 참고)
fn good_print(s: &str) {
    println!("{s}");
}

// ✅ 수정이 필요 → &mut
fn good_append(s: &mut String) {
    s.push('!');
}
```

#### 체크포인트

- [ ] 불변 참조 2개 + 가변 참조 1개를 같은 스코프에서 만들어 에러를 유도한 뒤, NLL로 통과되게 고쳐보기
- [ ] `fn push_exclamation(s: &mut String)` 작성 후 호출
- [ ] 지역 변수의 참조를 반환하는 함수를 작성해 dangling 에러 메시지 읽어보기

---

### 2.3 슬라이스는 빌림의 한 형태

#### 개념

슬라이스(`&[T]`, `&str`)는 **소유하지 않는 뷰**입니다. "어딘가에 있는 연속 데이터의 일부(또는 전체)를 가리키는 참조 + 길이"입니다.

```
&str / &[T] = { data_ptr, length }  ← fat pointer (보통 2워드)
```

#### `&String`보다 `&str`을 받는 이유

```rust
// String만 받음 — &String, &str, &"리터럴" 모두 받고 싶을 때 불편
fn first_word_narrow(s: &String) -> &str {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b' ' {
            return &s[0..i];
        }
    }
    &s[..]
}

// &str을 받으면 String, &str, 리터럴 전부 수용 (Deref coercion)
fn first_word(s: &str) -> &str {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b' ' {
            return &s[0..i];
        }
    }
    &s[..]
}

fn main() {
    let owned = String::from("hello world");
    let w1 = first_word(&owned);     // &String → &str (자동)
    let w2 = first_word(&owned[..]); // 명시적 슬라이스
    let w3 = first_word("hello world"); // 리터럴도 OK

    println!("{w1}, {w2}, {w3}");
}
```

**관용구**: 함수 인자로 문자열을 "읽기만" 할 거면 `String`/`&String` 말고 **`&str`**을 받으세요. 컬렉션도 비슷하게 `Vec<T>` 대신 **`&[T]`**를 받는 게 일반적입니다.

#### 슬라이스와 빌림 규칙이 충돌하는 순간

```rust
fn main() {
    let mut s = String::from("hello world");
    let word = first_word(&s); // word는 s를 빌림 중 (&str)

    // s.clear(); // 에러! word가 가리키는 데이터가 날아가면 안 됨
    println!("첫 단어: {word}");
}
```

`word`가 `s`의 일부를 빌리고 있는 동안 `s.clear()`로 내용을 지울 수 없습니다. **슬라이스도 빌림**이므로 `&` / `&mut` 규칙이 그대로 적용됩니다.

#### 배열/벡터 슬라이스

```rust
fn sum(slice: &[i32]) -> i32 {
    slice.iter().sum()
}

fn main() {
    let arr = [1, 2, 3, 4, 5];
    let v = vec![10, 20, 30];

    println!("{}", sum(&arr));      // 배열 → &[i32]
    println!("{}", sum(&v));        // Vec → &[i32]
    println!("{}", sum(&arr[1..4])); // 부분 슬라이스
}
```

#### UTF-8 함정 — 바이트 인덱스로 자르면 panic

```rust
fn main() {
    let s = String::from("안녕하세요");
    // let bad = &s[0..2]; // panic! — '안'은 3바이트, 2에서 자르면 문자 중간
    let ok = &s[0..3]; // "안" — 한 글자 단위로 맞춰야 함
    println!("{ok}");

    // 안전하게 문자 단위로:
    let first_char: String = s.chars().take(1).collect();
    println!("{first_char}");
}
```

#### 체크포인트

- [ ] `fn average(nums: &[f64]) -> f64`를 작성하고 배열/벡터/부분 슬라이스에 모두 호출
- [ ] `first_word` 결과를 들고 있는 동안 `clear`를 시도해 빌림 에러 확인
- [ ] 한글 문자열을 바이트 인덱스로 잘못 잘랐을 때 panic을 한 번 경험해보기

---

### 2.4 라이프타임 문법 — `'a`와 elision rules

#### 왜 라이프타임이 필요한가

참조는 "얼마나 오래 유효한가"라는 수명을 가집니다. 대부분의 경우 컴파일러가 추론하지만, **함수가 참조를 반환하거나 여러 참조 인자를 받을 때** "반환값이 어느 입력의 수명을 따라가는지"를 사람이 알려줘야 할 때가 있습니다.

```rust
// 컴파일 에러 — 반환된 &str이 x를 가리키는지 y를 가리키는지 모름
fn longest(x: &str, y: &str) -> &str {
    if x.len() > y.len() { x } else { y }
}
```

```
error[E0106]: missing lifetime specifier
  |
  | fn longest(x: &str, y: &str) -> &str {
  |             ----     ----     ^ expected named lifetime parameter
  |
  = help: this function's return type contains a borrowed value,
           but the signature does not say whether it is borrowed from `x` or `y`
```

#### 라이프타임 명시

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

의미: "`x`와 `y`, 그리고 반환값 모두 **최소 `'a`만큼은 살아있는** 영역에서만 유효하다."  
실제로는 **둘 중 더 짧은 쪽**의 수명에 반환값이 묶입니다.

```rust
fn main() {
    let s1 = String::from("long string");
    {
        let s2 = String::from("xyz");
        let result = longest(s1.as_str(), s2.as_str());
        println!("더 긴 쪽: {result}"); // s2가 아직 살아 있으므로 OK
    }
    // 여기서 result를 밖으로 빼내려면? s2가 이미 죽었을 수 있어서 불가한 경우가 생김
}
```

바깥 스코프로 빼내려다 실패하는 예:

```rust
fn main() {
    let s1 = String::from("long string");
    let result;
    {
        let s2 = String::from("xyz");
        result = longest(s1.as_str(), s2.as_str());
        // result의 수명은 s1과 s2 중 짧은 쪽(= s2)에 묶임
    } // s2 drop
    // println!("{result}"); // 에러 — s2가 이미 죽음
}
```

#### 라이프타임은 "값이 얼마나 사는지"가 아니라 "참조가 유효한 구간 제약"

`'a`는 타입이 아닙니다. **참조들의 관계에 대한 계약**입니다. 컴파일러가 그 계약을 위반하는 코드를 거부합니다.

#### 라이프타임 생략 규칙 (elision rules) — 3가지

매번 `'a`를 쓸 필요는 없습니다. 아래 규칙으로 컴파일러가 자동으로 채웁니다.

**규칙 1.** 각 참조 입력 인자는 각자의 라이프타임을 갖습니다.

```rust
// 작성한 것
fn foo(x: &str, y: &i32);

// 컴파일러가 보는 것
fn foo<'a, 'b>(x: &'a str, y: &'b i32);
```

**규칙 2.** 입력 라이프타임이 **하나뿐**이면, 그 라이프타임이 모든 출력 참조에 부여됩니다.

```rust
// 작성한 것
fn first_word(s: &str) -> &str;

// 컴파일러가 보는 것
fn first_word<'a>(s: &'a str) -> &'a str;
```

**규칙 3.** 메서드에서 입력이 여러 개여도, 그 중 하나가 `&self` / `&mut self`이면 **`self`의 라이프타임**이 모든 출력에 부여됩니다.

```rust
impl MyStruct {
    // 작성한 것
    fn get_name(&self, _hint: &str) -> &str;

    // 컴파일러가 보는 것
    fn get_name<'a, 'b>(&'a self, _hint: &'b str) -> &'a str;
}
```

이 세 규칙으로 해결되지 않으면 **직접 명시**해야 합니다. `longest`가 그 대표 사례입니다 (입력 참조가 둘, self도 없음 → 규칙 2·3 해당 없음).

#### `'static` — 프로그램 전체 수명

```rust
fn main() {
    let s: &'static str = "I live forever"; // 문자열 리터럴은 바이너리에 박혀 있음
    let leak: &'static str = Box::leak(String::from("leaked").into_boxed_str());
    // Box::leak은 의도적 누수 — 정말 필요할 때만
}
```

`'static`은 "절대 죽지 않는다"는 뜻이지만, 실무에서 모든 참조에 `'static`을 요구하는 건 (특히 async/스레드에서) **설계가 잘못된 신호**인 경우가 많습니다. 진짜 전역/리터럴이 아니면 일반 라이프타임이나 owned 타입(`String`, `Arc<str>` 등)을 쓰세요.

#### 체크포인트

- [ ] `longest`에서 라이프타임 어노테이션을 지워 에러를 재현하고, 도움말 문구를 읽어보기
- [ ] 입력 참조가 하나인 함수에서는 `'a` 없이도 되는지 확인 (elision 규칙 2)
- [ ] `longest` 결과를 짧은 쪽 스코프 밖으로 빼내려다 실패하는 코드 작성

---

### 2.5 구조체에 참조 담기 — `struct Foo<'a>`

#### 개념

구조체 필드가 참조이면, **그 참조가 가리키는 데이터가 구조체보다 오래 살아야** 합니다. 그래서 구조체 정의에 라이프타임 매개변수가 필요합니다.

```rust
struct Novel<'a> {
    title: &'a str,
    body: &'a str,
}

fn main() {
    let title = String::from("The Rust Book");
    let body = String::from("Once upon a time...");

    let novel = Novel {
        title: title.as_str(),
        body: body.as_str(),
    };

    println!("{} — {}", novel.title, &novel.body[..20.min(novel.body.len())]);
} // novel, body, title 순으로 drop — 참조 대상이 novel보다 오래 삶 ✓
```

필드마다 다른 수명을 줄 수도 있습니다:

```rust
struct Excerpt<'a, 'b> {
    part: &'a str,
    note: &'b str,
}
```

보통은 하나로 통일(`'a`)하는 편이 읽기 쉽고, 정말 수명이 갈라질 때만 여러 개를 씁니다.

#### `impl`에도 라이프타임을 달아야 한다

```rust
struct ImportantExcerpt<'a> {
    part: &'a str,
}

impl<'a> ImportantExcerpt<'a> {
    fn level(&self) -> i32 {
        3
    }

    // elision 규칙 3: 반환 &str은 self의 수명을 따름
    fn announce_and_return_part(&self, announcement: &str) -> &str {
        println!("주목! {announcement}");
        self.part
    }
}

fn main() {
    let novel = String::from("Call me Ishmael. Some years ago...");
    let first_sentence = novel.split('.').next().unwrap();
    let excerpt = ImportantExcerpt {
        part: first_sentence,
    };
    println!("{}", excerpt.announce_and_return_part("발췌문"));
}
```

#### 참조를 담기 vs 소유하기 — 언제 뭘 쓸까

| 선택 | 타입 예 | 언제 |
|---|---|---|
| 소유 | `String`, `Vec<T>`, `PathBuf` | 구조체가 데이터를 책임져야 할 때, API 경계를 넘길 때 |
| 빌림 | `&str`, `&[T]`, `&Path` | 짧은 스코프의 뷰, zero-copy 파싱, 수명이 명확할 때 |
| 공유 소유 | `Arc<str>`, `Rc<String>` | 여러 소유자가 필요하지만 라이프타임 연결은 싫을 때 (7단계) |

실무(axum 핸들러, 설정 구조체 등)에서는 **소유하는 타입(`String`)을 필드에 두는 편이 훨씬 흔합니다.** 라이프타임을 구조체에 달기 시작하면 시그니처가 전염병처럼 퍼지기 때문입니다. "zero-copy로 파싱 결과를 들고 있어야 한다"는 요구가 명확할 때만 `struct Foo<'a>`를 쓰세요.

#### 체크포인트

- [ ] `struct Config<'a> { name: &'a str }`를 만들고, name이 가리키는 `String`을 Config보다 먼저 drop되게 배치해 에러 유도
- [ ] 같은 구조를 `name: String`으로 바꿔 라이프타임이 사라지는 것 확인
- [ ] `impl<'a>` 블록에 메서드 하나를 직접 추가

---

### 2.6 종합 실습 — 더 긴 문자열 슬라이스 반환 + 에러 읽기

#### 실습 A: `longest` 완성

```rust
/// 두 문자열 슬라이스 중 더 긴 쪽을 반환한다.
/// 반환된 참조는 `a`와 `b` 중 **더 짧은 수명**에 묶인다.
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() { a } else { b }
}

fn main() {
    let a = "short";
    let b = "much longer";
    println!("{}", longest(a, b));
}
```

#### 실습 B: 컴파일러와 대화하기 (의도적 실패)

아래 코드를 **그대로** 컴파일해보고, 메시지를 읽어 고치세요.

```rust
// ① 라이프타임 누락
fn pick(x: &str, y: &str) -> &str {
    if x.len() > 3 { x } else { y }
}

// ② dangling
fn make_ref() -> &str {
    let s = String::from("temp");
    &s
}

// ③ 빌림 규칙 위반
fn main() {
    let mut data = String::from("hello");
    let r = &data;
    data.push_str("!");
    println!("{r}");
}
```

예상되는 에러 코드:

| 번호 | 대략적인 에러 | 고치는 방향 |
|---|---|---|
| ① | `E0106` missing lifetime specifier | `fn pick<'a>(x: &'a str, y: &'a str) -> &'a str` |
| ② | `E0515` / `E0106` | `&str` 대신 `String`을 반환 |
| ③ | `E0502` cannot borrow as mutable | `println`을 `push_str` **앞**으로 옮기거나, `r` 사용을 끝낸 뒤 수정 |

#### 실습 C: 미니 파서 — 참조를 담은 구조체

```rust
#[derive(Debug)]
struct Token<'a> {
    kind: TokenKind,
    lexeme: &'a str, // 원본 소스의 일부 — zero-copy
}

#[derive(Debug, PartialEq)]
enum TokenKind {
    Ident,
    Number,
    Plus,
}

fn tokenize(src: &str) -> Vec<Token<'_>> {
    // Token<'_> 의 '_ 는 "src에서 추론된 라이프타임"을 의미
    let mut tokens = Vec::new();
    for part in src.split_whitespace() {
        let kind = match part {
            "+" => TokenKind::Plus,
            p if p.chars().all(|c| c.is_ascii_digit()) => TokenKind::Number,
            _ => TokenKind::Ident,
        };
        tokens.push(Token { kind, lexeme: part });
    }
    tokens
}

fn main() {
    let source = String::from("x 42 + y");
    let tokens = tokenize(&source);
    for t in &tokens {
        println!("{t:?}");
    }
    // source를 여기서 clear하면? tokens가 source를 빌리므로 불가
}
```

포인트:

1. `Token<'a>`가 `src`의 슬라이스를 들고 있어 **복사 없이** 토큰화합니다.
2. `Vec<Token<'_>>`의 `'_`는 "명시적으로 이름 붙이기 싫고 추론에 맡긴다"는 뜻입니다.
3. `tokens`가 살아있는 동안 `source`를 변경·drop할 수 없습니다 — 2.2·2.3 규칙의 연장입니다.

#### 확장 과제 (선택)

- [ ] `longest`를 세 문자열 버전 `longest3`으로 확장
- [ ] `Token`에 `span: std::ops::Range<usize>`(바이트 오프셋)를 추가해, 슬라이스 대신 인덱스로도 원본을 다시 잘라보기
- [ ] axum에서 흔히 쓰는 `State(Arc<AppState>)` 패턴을 떠올리며: "공유 상태가 왜 `'a` 참조가 아니라 `Arc`인가?"를 한 문단으로 정리

---

### 이 단계에서 머릿속에 남겨둘 한 장 요약

```
값 대입/함수 전달
  ├─ Copy 타입     → 비트 복사, 양쪽 유효
  └─ non-Copy      → move, 이전 이름 무효
        └─ 필요하면 .clone() (힙 복사)

빌림
  ├─ &T            → 다수 가능, 읽기만
  ├─ &mut T        → 하나만, 읽기+쓰기
  └─ &와 &mut 동시 → 불가 (NLL: 마지막 사용 이후면 OK)

슬라이스 &[T], &str = 빌림(뷰) + 길이

라이프타임 'a
  ├─ 참조 사이의 "이 관계가 유지되는 구간" 계약
  ├─ elision 3규칙으로 대부분 생략
  └─ struct에 참조 필드 → struct Foo<'a>
```

---

---

> **다음:** [3단계. 타입 시스템](#3단계-타입-시스템)

## 3단계. 타입 시스템

> 이어서: 앞 장의 [2단계. 소유권 & 빌림](#2단계-소유권-빌림)에서 이어집니다.

---

### 3-1. 구조체와 열거형

### 3.1.1 `struct` — 일반 / 튜플 / 유닛

#### 일반(named-field) 구조체

```rust
struct User {
    active: bool,
    username: String,
    email: String,
    sign_in_count: u64,
}

fn main() {
    let mut user = User {
        active: true,
        username: String::from("async"),
        email: String::from("async@example.com"),
        sign_in_count: 1,
    };

    user.email = String::from("new@example.com"); // 필드 변경은 struct 전체가 mut일 때

    // 필드 초기화 축약: 변수명 == 필드명이면 한 번만
    let username = String::from("glory");
    let email = String::from("glory@example.com");
    let user2 = User {
        username,
        email,
        active: true,
        sign_in_count: 1,
    };

    // 구조체 업데이트 문법 (..남은 필드는 다른 인스턴스에서)
    let user3 = User {
        email: String::from("another@example.com"),
        ..user2 // user2의 나머지 필드 이동/복사 — user2.username 등은 이후 사용 불가(비 Copy)
    };
    println!("{} / {}", user3.username, user.active);
}
```

#### 튜플 구조체 — 필드 이름은 없고 타입만

```rust
struct Color(i32, i32, i32);
struct Point(i32, i32, i32);

fn main() {
    let black = Color(0, 0, 0);
    let origin = Point(0, 0, 0);
    // black과 origin은 필드 타입이 같아도 **다른 타입** — 서로 대입 불가
    println!("R = {}", black.0);
}
```

뉴타입 패턴에 자주 씁니다 (`struct UserId(u64);`처럼 의미만 다른 래퍼).

#### 유닛 구조체 — 필드 없음

```rust
struct AlwaysEqual;

fn main() {
    let _marker = AlwaysEqual;
    // 트레잇만 구현하고 데이터는 없을 때, 또는 타입 레벨 마커로 사용
}
```

#### `impl` 블록 — 메서드와 연관 함수

```rust
#[derive(Debug)]
struct Rectangle {
    width: u32,
    height: u32,
}

impl Rectangle {
    /// 연관 함수(associated function) — self 없음. `::`로 호출 (생성자에 흔함)
    fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    fn square(size: u32) -> Self {
        Self {
            width: size,
            height: size,
        }
    }

    /// 메서드 — 첫 인자 self
    fn area(&self) -> u32 {
        self.width * self.height
    }

    fn can_hold(&self, other: &Rectangle) -> bool {
        self.width > other.width && self.height > other.height
    }

    fn set_width(&mut self, width: u32) {
        self.width = width;
    }

    /// self를 소비(move) — 변환/빌더 끝에서 가끔 씀
    fn into_square(self) -> Self {
        let side = self.width.max(self.height);
        Self::square(side)
    }
}

fn main() {
    let mut rect = Rectangle::new(30, 50);
    println!("area = {}", rect.area());
    rect.set_width(40);
    let sq = Rectangle::square(20);
    println!("can_hold? {}", rect.can_hold(&sq));
}
```

`self` 수신 방식 정리:

| 시그니처 | 의미 | 언제 |
|---|---|---|
| `&self` | 불변 빌림 | 읽기 |
| `&mut self` | 가변 빌림 | 수정 |
| `self` | 소유권 이동 | 소비/변환 |
| `mut self` | 소유 + 내부에서 mut | 소비하면서 중간 수정 |

`impl` 블록은 **여러 개로 쪼개도** 됩니다. 나중에 제네릭/`cfg`별로 나누는 데 유용합니다.

#### 체크포인트

- [ ] `User`에 `display_name(&self) -> &str` 메서드 추가
- [ ] 뉴타입 `struct Meters(f64);`와 `struct Feet(f64);`를 만들고 서로 섞어 쓰는 실수를 타입으로 방지하는 느낌 확인
- [ ] `into_square(self)` 호출 후 기존 `rect`를 쓰면 move 에러가 나는지 확인

---

### 3.1.2 `enum` — tagged union으로서의 열거형

#### 데이터가 들어간 variant

Rust의 enum은 C의 enum(정수 별칭)이 아니라 **tagged union**입니다. variant마다 다른 형태의 데이터를 가질 수 있습니다.

```rust
enum IpAddr {
    V4(u8, u8, u8, u8),
    V6(String),
}

enum Message {
    Quit,                       // 유닛
    Move { x: i32, y: i32 },    // 구조체형
    Write(String),              // 튜플형
    ChangeColor(i32, i32, i32),
}

impl Message {
    fn call(&self) {
        match self {
            Message::Quit => println!("quit"),
            Message::Move { x, y } => println!("move to ({x}, {y})"),
            Message::Write(text) => println!("write: {text}"),
            Message::ChangeColor(r, g, b) => println!("color: {r},{g},{b}"),
        }
    }
}

fn main() {
    let home = IpAddr::V4(127, 0, 0, 1);
    let loopback = IpAddr::V6(String::from("::1"));
    Message::Write(String::from("hello")).call();
    let _ = (home, loopback);
}
```

메모리상으로는 대략 "태그(어떤 variant인지) + 가장 큰 variant만큼의 페이로드"입니다. (`niche optimization`으로 `Option<&T>`가 포인터 크기만 차지하는 등 최적화도 있습니다.)

#### `Option<T>` — null 없는 언어의 핵심

```rust
// 표준 라이브러리 정의 (개념)
enum Option<T> {
    None,
    Some(T),
}

fn divide(a: f64, b: f64) -> Option<f64> {
    if b == 0.0 { None } else { Some(a / b) }
}

fn main() {
    match divide(10.0, 2.0) {
        Some(v) => println!("결과: {v}"),
        None => println!("0으로 나눔"),
    }

    // 콤비네이터 (5·6단계에서 더 깊게)
    let v = divide(10.0, 0.0).unwrap_or(0.0);
    let doubled = divide(10.0, 2.0).map(|x| x * 2.0);
    println!("{v}, {doubled:?}");
}
```

`null` 대신 `Option`을 쓰면 "없을 수 있음"이 **타입에 드러나고**, `match`/`if let`/`?`로 강제 처리됩니다.

#### `Result<T, E>` — 실패 가능한 연산

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}

fn parse_port(s: &str) -> Result<u16, std::num::ParseIntError> {
    s.parse()
}

fn main() {
    match parse_port("8080") {
        Ok(port) => println!("port = {port}"),
        Err(e) => println!("파싱 실패: {e}"),
    }
}
```

`Option`은 "값 없음", `Result`는 "실패 원인"까지 담습니다. 4단계에서 `?`와 커스텀 에러로 깊게 다룹니다. 여기서는 **문법의 일부처럼** `Ok`/`Err`/`Some`/`None`을 쓰는 데 익숙해지면 됩니다.

#### `match`가 exhaustiveness를 강제한다

```rust
enum TrafficLight {
    Red,
    Yellow,
    Green,
}

fn action(light: TrafficLight) -> &'static str {
    match light {
        TrafficLight::Red => "멈추세요",
        TrafficLight::Yellow => "주의",
        TrafficLight::Green => "지나가세요",
        // variant를 하나 빼먹으면 컴파일 에러 — 리팩터링 안전망
    }
}
```

#### 체크포인트

- [ ] HTTP 메서드 `enum Method { Get, Post, Put, Delete }` + `fn is_safe(&self) -> bool` 작성
- [ ] `Option<i32>`를 `if let` / `match` / `map` 세 방식으로 처리
- [ ] `Result`를 반환하는 작은 함수 하나 작성 (`parse` 활용)

---

### 3-2. 트레잇 (Trait)

### 3.2.1 정의, 구현, 기본 메서드

#### 트레잇 = 공통 동작의 인터페이스

```rust
trait Summary {
    fn summarize_author(&self) -> String;

    /// 기본 구현 — 구현체가 오버라이드하지 않으면 이게 쓰임
    fn summarize(&self) -> String {
        format!("(Read more from {}...)", self.summarize_author())
    }
}

struct NewsArticle {
    headline: String,
    location: String,
    author: String,
    content: String,
}

impl Summary for NewsArticle {
    fn summarize_author(&self) -> String {
        self.author.clone()
    }
    // summarize는 기본 구현 사용
}

struct Tweet {
    username: String,
    content: String,
    reply: bool,
    retweet: bool,
}

impl Summary for Tweet {
    fn summarize_author(&self) -> String {
        format!("@{}", self.username)
    }

    fn summarize(&self) -> String {
        format!("{}: {}", self.username, self.content)
    }
}

fn main() {
    let tweet = Tweet {
        username: String::from("async"),
        content: String::from("Rust traits are great"),
        reply: false,
        retweet: false,
    };
    println!("{}", tweet.summarize());
}
```

#### 트레잇을 쓰려면 스코프에 있어야 한다

```rust
use std::io::Write; // Write 트레잇을 가져와야 .write_all 등이 보임

fn main() {
    let mut buf = Vec::new();
    buf.write_all(b"hello").unwrap();
}
```

다른 크레이트의 타입에 **우리 트레잇**을 구현하거나, **우리 타입**에 외부 트레잇을 구현하는 것은 가능합니다. 하지만 **외부 타입 + 외부 트레잇** 조합은 orphan rule 때문에 불가합니다 (뉴타입으로 우회).

#### 체크포인트

- [ ] `trait Greet { fn greet(&self); }`를 `User`에 구현
- [ ] 기본 메서드 하나를 두고, 구현체 하나에서는 오버라이드해보기

---

### 3.2.2 `impl Trait` vs `dyn Trait` vs 제네릭 `<T: Trait>`

같은 "Summary를 받는 함수"를 세 가지로 쓸 수 있습니다.

#### ① 제네릭 — 정적 디스패치 (monomorphization)

```rust
fn notify_generic<T: Summary>(item: &T) {
    println!("Breaking news! {}", item.summarize());
}
```

호출 타입마다 함수 복사본이 컴파일 타임에 생성됩니다. **빠르고**, 타입별 최적화 가능. 바이너리는 커질 수 있음.

#### ② `impl Trait` (인자) — 제네릭의 문법 설탕에 가까움

```rust
fn notify_impl(item: &impl Summary) {
    println!("Breaking news! {}", item.summarize());
}
```

단순할 때 읽기 좋습니다. 다만 **두 인자가 같은 타입이어야 할 때**는 제네릭이 명확합니다:

```rust
// 둘 다 Summary면 되지만, 서로 다른 타입일 수 있음
fn wrong(a: &impl Summary, b: &impl Summary) {}

// 같은 T로 묶고 싶을 때
fn right<T: Summary>(a: &T, b: &T) {}
```

반환 위치의 `impl Trait`:

```rust
fn returns_summarizable() -> impl Summary {
    Tweet {
        username: String::from("horse_ebooks"),
        content: String::from("of course"),
        reply: false,
        retweet: false,
    }
}
```

**한 가지 구체 타입만** 반환할 수 있습니다. 분기마다 다른 타입을 반환하려면 `Box<dyn Summary>`가 필요합니다.

#### ③ `dyn Trait` — 동적 디스패치 (트레잇 객체)

```rust
fn notify_dyn(item: &dyn Summary) {
    println!("Breaking news! {}", item.summarize());
}

fn main() {
    let mut feed: Vec<Box<dyn Summary>> = Vec::new();
    feed.push(Box::new(NewsArticle {
        headline: String::from("Penguins win"),
        location: String::from("Pittsburgh"),
        author: String::from("Iceburgh"),
        content: String::from("..."),
    }));
    feed.push(Box::new(Tweet {
        username: String::from("horse_ebooks"),
        content: String::from("of course"),
        reply: false,
        retweet: false,
    }));

    for item in &feed {
        println!("{}", item.summarize());
    }
}
```

`dyn Summary`는 **fat pointer**(데이터 포인터 + vtable 포인터)입니다. 크기가 타입마다 다르므로 보통 `&dyn Trait` 또는 `Box<dyn Trait>`로 다룹니다.

| 방식 | 디스패치 | 이종 컬렉션 | 바이너리 크기 | 런타임 비용 |
|---|---|---|---|---|
| `<T: Trait>` / `impl Trait` | 정적 | 불가 (한 타입) | 커질 수 있음 | 없음(인라인 가능) |
| `dyn Trait` | 동적(vtable) | 가능 | 작음 | 간접 호출 |

**실무 감각**: 라이브러리 API·핫 패스는 제네릭, 플러그인/핸들러 목록·이종 컬렉션은 `Box<dyn Trait>`.

#### object safety

모든 트레잇이 `dyn Trait`가 될 수 있는 건 아닙니다. 예를 들어 `Self: Sized`가 필요하거나, 제네릭 메서드가 있으면 트레잇 객체가 될 수 없습니다.

```rust
trait NotObjectSafe {
    fn generic<T>(&self, x: T); // dyn 불가
}
```

#### 체크포인트

- [ ] 같은 `notify`를 제네릭 / `impl Trait` / `&dyn Trait` 세 버전으로 작성
- [ ] `Vec<Box<dyn Summary>>`에 서로 다른 타입 두 개를 push

---

### 3.2.3 트레잇 바운드와 `where` 절

```rust
use std::fmt::Display;

// 짧은 형태
fn print_pair<T: Display + Clone, U: Display>(a: T, b: U) {
    println!("{a} / {b}");
}

// where 절 — 시그니처가 길어질 때 가독성 ↑
fn print_pair_where<T, U>(a: T, b: U)
where
    T: Display + Clone,
    U: Display,
{
    println!("{a} / {b}");
}

// 반환 타입에도 바운드
fn pair_string<T, U>(a: T, b: U) -> String
where
    T: Display,
    U: Display,
{
    format!("{a}-{b}")
}
```

조건부 메서드 구현:

```rust
struct Pair<T> {
    x: T,
    y: T,
}

impl<T> Pair<T> {
    fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl<T: Display + PartialOrd> Pair<T> {
    fn cmp_display(&self) {
        if self.x >= self.y {
            println!("가장 큰 멤버: {}", self.x);
        } else {
            println!("가장 큰 멤버: {}", self.y);
        }
    }
}
```

`T`가 `Display + PartialOrd`일 때만 `cmp_display`가 존재합니다.

---

### 3.2.4 연산자 오버로딩 — `std::ops`

```rust
use std::ops::Add;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

impl Add for Point {
    type Output = Point;

    fn add(self, other: Point) -> Point {
        Point {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

fn main() {
    assert_eq!(
        Point { x: 1, y: 0 } + Point { x: 2, y: 3 },
        Point { x: 3, y: 3 }
    );
}
```

`Add`, `Sub`, `Mul`, `Div`, `Index`, `IndexMut`, `Not` 등 연산자는 전부 트레잇입니다. `Output` 같은 **연관 타입(associated type)**도 여기서 처음 자주 만납니다.

다른 타입과의 덧셈:

```rust
impl Add<i32> for Point {
    type Output = Point;
    fn add(self, rhs: i32) -> Point {
        Point {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}
```

---

### 3.2.5 `Deref` / `Drop` — 스마트 포인터처럼 동작시키기

#### `Drop` — 스코프 끝에서 자동 호출

```rust
struct Connection {
    name: String,
}

impl Drop for Connection {
    fn drop(&mut self) {
        println!("Closing connection: {}", self.name);
    }
}

fn main() {
    let _c = Connection {
        name: String::from("db"),
    };
    println!("main 끝 직전");
} // 여기서 Drop::drop 호출
```

`Drop`은 조기 호출이 필요하면 `std::mem::drop(value)`로 소유권을 넘겨 즉시 drop합니다. (`Drop::drop`을 직접 호출하는 건 불가.)

#### `Deref` / `DerefMut` — `*`와 자동 역참조 강제(coercion)

```rust
use std::ops::Deref;

struct MyBox<T>(T);

impl<T> MyBox<T> {
    fn new(x: T) -> MyBox<T> {
        MyBox(x)
    }
}

impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn hello(name: &str) {
    println!("Hello, {name}!");
}

fn main() {
    let x = MyBox::new(String::from("Rust"));
    // &MyBox<String> → Deref → &String → Deref → &str
    hello(&x);
}
```

`String` → `&str`, `Vec<T>` → `&[T]`도 같은 `Deref` 메커니즘입니다. 2단계에서 `&String` 대신 `&str`을 받으라고 한 이유와 연결됩니다.

#### 체크포인트

- [ ] `Point`에 `Add`를 구현하고 `p1 + p2` 테스트
- [ ] `Drop`으로 "파괴 로그"가 찍히는 타입 하나 만들기
- [ ] (선택) `MyBox`에 `DerefMut`도 구현해 `*x = ...` 되게 하기

---

### 3-3. 제네릭

### 3.3.1 함수 / 구조체 / 열거형

```rust
fn identity<T>(x: T) -> T {
    x
}

struct Point<T> {
    x: T,
    y: T,
}

struct Point2<T, U> {
    x: T,
    y: U,
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

fn main() {
    let a = identity(5);
    let b = identity("hi");
    let integer = Point { x: 5, y: 10 };
    let float = Point { x: 1.0, y: 4.0 };
    let mixed = Point2 { x: 5, y: 4.0 };
    let _ = (a, b, integer, float, mixed, Either::Left::<i32, &str>(1));
}
```

### 3.3.2 제네릭 + 트레잇 바운드

```rust
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    let mut largest = &list[0];
    for item in list {
        if item > largest {
            largest = item;
        }
    }
    largest
}

fn main() {
    let numbers = vec![34, 50, 25, 100, 65];
    println!("가장 큰 수: {}", largest(&numbers));

    let chars = vec!['y', 'm', 'a', 'q'];
    println!("가장 큰 문자: {}", largest(&chars));
}
```

`T: PartialOrd`가 없으면 `>`를 쓸 수 없습니다. **제네릭은 "아무 타입"이 아니라 "이 동작을 할 수 있는 타입"**입니다.

### 3.3.3 `impl<T> Foo<T>` — 제네릭 구조체의 메서드

```rust
impl<T> Point<T> {
    fn x(&self) -> &T {
        &self.x
    }
}

// 특정 타입에만 메서드
impl Point<f32> {
    fn distance_from_origin(&self) -> f32 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }
}

// 메서드가 다른 제네릭 파라미터를 도입
impl<T, U> Point2<T, U> {
    fn mixup<V, W>(self, other: Point2<V, W>) -> Point2<T, W> {
        Point2 {
            x: self.x,
            y: other.y,
        }
    }
}
```

### 3.3.4 모노모피제이션 (monomorphization)

```rust
fn foo<T: std::fmt::Display>(x: T) {
    println!("{x}");
}

fn main() {
    foo(1i32);
    foo(1.0f64);
}
```

컴파일러는 대략 이런 식으로 **타입별 구체 함수**를 생성합니다:

```rust
fn foo_i32(x: i32) { println!("{x}"); }
fn foo_f64(x: f64) { println!("{x}"); }
```

그래서 제네릭 호출은 C++ 템플릿처럼 **런타임 오버헤드가 없고**, `dyn`과 달리 vtable도 없습니다. 대신 코드 중복으로 바이너리가 커질 수 있습니다 (트레이드오프).

`rustc` 파이프라인 관점: HIR → ... → monomorphized MIR → LLVM IR. 제네릭은 이 과정에서 "타입 인자별로 복사"됩니다. (임베디드에서 바이너리 크기가 민감하면 `dyn`을 선택하는 이유이기도 합니다.)

#### 체크포인트

- [ ] `fn smallest<T: PartialOrd>(list: &[T]) -> &T` 작성
- [ ] `Point<T>`에 `swap(self) -> Point<T>` (x/y 교환) 구현
- [ ] 제네릭 함수를 두 타입으로 호출한 뒤, (선택) `cargo bloat` 또는 Godbolt로 모노모피제이션 흔적 구경

---

### 3.4 종합 실습 — `Shape` 트레잇과 다형성

```rust
use std::f64::consts::PI;

trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> &'static str;

    fn describe(&self) -> String {
        format!("{} (area = {:.2})", self.name(), self.area())
    }
}

struct Circle {
    radius: f64,
}

struct Rectangle {
    width: f64,
    height: f64,
}

struct Triangle {
    base: f64,
    height: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        PI * self.radius * self.radius
    }
    fn name(&self) -> &'static str {
        "Circle"
    }
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }
    fn name(&self) -> &'static str {
        "Rectangle"
    }
}

impl Shape for Triangle {
    fn area(&self) -> f64 {
        0.5 * self.base * self.height
    }
    fn name(&self) -> &'static str {
        "Triangle"
    }
}

/// 정적 디스패치 버전
fn total_area_generic(shapes: &[impl Shape]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

/// 동적 디스패치 버전 — 이종 컬렉션
fn total_area_dyn(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

fn main() {
    let mono = [Circle { radius: 2.0 }, Circle { radius: 3.0 }];
    // 참고: [impl Shape]는 위 시그니처 예시용. 실제로는 제네릭 슬라이스가 더 흔함.
    // 아래는 dyn 컬렉션에 집중:

    let shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(Circle { radius: 2.0 }),
        Box::new(Rectangle {
            width: 3.0,
            height: 4.0,
        }),
        Box::new(Triangle {
            base: 5.0,
            height: 6.0,
        }),
    ];

    for s in &shapes {
        println!("{}", s.describe());
    }
    println!("합계 면적 = {:.2}", total_area_dyn(&shapes));

    let _ = mono;
    let _ = total_area_generic(&[
        Rectangle {
            width: 1.0,
            height: 2.0,
        },
        Rectangle {
            width: 3.0,
            height: 4.0,
        },
    ]);
}
```

`total_area_generic`의 `&[impl Shape]`는 교육용입니다. 실무에서는 보통:

```rust
fn total_area_generic<S: Shape>(shapes: &[S]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}
```

#### 이 예제에서 짚을 포인트

1. **기본 메서드 `describe`** — 구현체마다 반복하지 않음.
2. **`Vec<Box<dyn Shape>>`** — 서로 다른 크기의 타입을 하나의 리스트에.
3. **제네릭 vs dyn** — 동종 슬라이스는 제네릭, 이종이면 dyn.
4. (연결) `Box`가 필요한 이유 중 하나 = **크기를 알 수 없는 타입을 힙에 두고 포인터로 다루기** (7단계에서 재등장).

#### 확장 과제

- [ ] `Shape`에 `perimeter(&self) -> f64` 추가 후 모든 구현 수정 (트레잇 변경의 파급 체감)
- [ ] `impl Add for Point` 스타일로 `Circle` 반지름 스케일 (`Mul<f64>`) 구현
- [ ] `fn shape_from_name(name: &str) -> Box<dyn Shape>`처럼 팩토리 함수 작성 (반환 `impl Shape`는 타입 하나로 고정되어 어려움 → dyn이 자연스러움)

---

### 이 단계 한 장 요약

```
struct / enum / impl     → 데이터 + 동작의 기본 단위
Option / Result          → null·예외 대신 타입으로 표현
Trait                    → 공통 동작
  ├─ <T: Trait> / impl Trait  → 정적 디스패치
  ├─ dyn Trait                → 동적 디스패치, 이종 컬렉션
  ├─ where / 바운드
  └─ Add, Deref, Drop         → 연산자·스마트포인터 훅
Generic + monomorphization → 컴파일 타임 코드 생성, 런타임 비용 0
```

---

---

> **다음:** [4단계. 에러 처리](#4단계-에러-처리)

## 4단계. 에러 처리

> 이어서: 앞 장의 [3단계. 타입 시스템](#3단계-타입-시스템)에서 이어집니다.

---

### 4.1 `Result`와 `?` 연산자 — 실제 동작

#### `Result` 복습

```rust
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

성공은 `Ok(value)`, 실패는 `Err(error)`. 예외 스택 언와인딩 대신 **반환값으로** 실패를 표현합니다.

#### `?`가 하는 일 — 설탕을 풀면

```rust
fn read_username_from_file() -> Result<String, std::io::Error> {
    let mut f = std::fs::File::open("username.txt")?;
    let mut s = String::new();
    std::io::Read::read_to_string(&mut f, &mut s)?;
    Ok(s)
}
```

`expr?`는 대략 다음와 같습니다:

```rust
match expr {
    Ok(v) => v,
    Err(e) => return Err(From::from(e)),
}
```

핵심 두 가지:

1. **`Ok`면 안의 값을 꺼내** 이후 코드에 넘긴다.
2. **`Err`면 즉시 함수에서 return** 하되, `From::from(e)`로 **에러 타입을 변환**할 수 있다.

그래서 `?`를 쓰려면:

- 함수 반환 타입이 `Result<_, E>` (또는 `Option`, 또는 `Try`를 구현한 타입)이어야 하고
- `E`가 `From<내부에러>`를 구현해야 (타입이 이미 같으면 identity)

#### `Option`에도 `?`가 된다

```rust
fn last_char_of_first_line(text: &str) -> Option<char> {
    text.lines().next()?.chars().last()
}
```

`None`이면 그 자리에서 `return None`.

#### `main`에서도 `?` — `main`이 `Result`를 반환할 때

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let s = std::fs::read_to_string("Cargo.toml")?;
    println!("{} bytes", s.len());
    Ok(())
}
```

프로토타입·예제에서 자주 보는 패턴입니다. 라이브러리 코드에서는 보통 구체 에러 타입을 씁니다.

#### `From`이 `?`와 맞물리는 예

```rust
use std::fs;
use std::io;
use std::num::ParseIntError;

enum MyError {
    Io(io::Error),
    Parse(ParseIntError),
}

impl From<io::Error> for MyError {
    fn from(e: io::Error) -> Self {
        MyError::Io(e)
    }
}

impl From<ParseIntError> for MyError {
    fn from(e: ParseIntError) -> Self {
        MyError::Parse(e)
    }
}

fn sum_file(path: &str) -> Result<i32, MyError> {
    let content = fs::read_to_string(path)?; // io::Error → MyError
    let mut sum = 0;
    for line in content.lines() {
        sum += line.parse::<i32>()?; // ParseIntError → MyError
    }
    Ok(sum)
}
```

`?` 한 글자가 `From` 구현을 찾아 에러를 감싸 줍니다. **커스텀 에러 enum + `From` 구현**이 Rust 에러 처리의 기본 골격입니다.

#### 체크포인트

- [ ] `File::open` + `read_to_string`을 `match`로 쓴 버전과 `?` 버전을 나란히 작성
- [ ] `Option`에 `?`를 쓰는 함수 하나 작성
- [ ] 서로 다른 두 에러를 `From`으로 하나의 enum에 합치기

---

### 4.2 `panic!` vs `Result` — 언제 뭘 쓰나

#### `panic!`이 하는 일

```rust
fn main() {
    panic!("crash and burn");
}
```

기본적으로 스택 언와인딩 후 프로세스 종료(또는 `abort`). 호출자가 **회복할 기회가 없습니다.**

#### 관용적 기준

| 상황 | 선택 | 예 |
|---|---|---|
| 복구 가능 / 호출자가 결정해야 함 | `Result` | 파일 없음, 파싱 실패, 네트워크 타임아웃, 잘못된 사용자 입력 |
| 프로그래머 버그 / 불변조건 깨짐 | `panic!` / `assert!` | 인덱스 로직이 틀렸음, "여긴 오면 안 됨", 계약 위반 |
| 프로토타입·예제·테스트 | `unwrap`/`expect` 허용 | 나중에 `?`로 교체할 전제 |
| 라이브러리 공개 API | 웬만하면 `Result` | 호출자에게 선택권 |

```rust
fn get(slice: &[i32], index: usize) -> Option<&i32> {
    slice.get(index) // 복구 가능 → Option/Result
}

fn first(slice: &[i32]) -> &i32 {
    // 빈 슬라이스를 넘기는 건 API 계약 위반으로 보는 설계도 가능
    assert!(!slice.is_empty(), "slice must not be empty");
    &slice[0]
}
```

axum 핸들러에서는 보통 **도메인 에러 → HTTP 상태 코드**로 매핑하고, 정말 예상 밖일 때만 panic(그리고 미들웨어/타워가 500 처리)입니다.

#### `assert!` / `debug_assert!` / `unreachable!`

```rust
fn divide(a: i32, b: i32) -> i32 {
    assert!(b != 0); // release에도 남음
    a / b
}

fn only_in_debug(x: i32) {
    debug_assert!(x > 0); // release에서는 제거
}

fn exhaustive_demo(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v,
        None => unreachable!("호출 전에 이미 걸렀음"),
    }
}
```

#### 체크포인트

- [ ] "사용자 입력 파싱 실패"와 "내부 배열 길이 가정 깨짐"을 각각 `Result` / `panic`으로 나누어 함수 두 개 작성
- [ ] `assert!`가 실패할 때 메시지를 커스터마이즈해보기

---

### 4.3 콤비네이터 메서드 — `unwrap` 가족

`match` 대신 메서드 체인으로 `Option`/`Result`를 다루는 API입니다.

#### `Option` 쪽

```rust
fn main() {
    let maybe = Some(5);

    // 위험 — None이면 panic
    let _v = maybe.unwrap();
    let _v = maybe.expect("값이 있어야 함"); // panic 메시지 지정 — unwrap보다 나음

    // 안전 — 기본값
    let v = None::<i32>.unwrap_or(0);
    let v = None::<i32>.unwrap_or_else(|| expensive_default());

    // Option ↔ Result
    let r: Result<i32, &str> = Some(1).ok_or("없음");
    let r: Result<i32, String> = None.ok_or_else(|| String::from("없음"));

    // 변환
    let doubled = Some(2).map(|x| x * 2);           // Option<i32>
    let flat = Some(2).and_then(|x| Some(x * 2));   // 클로저가 Option 반환할 때

    // 필터
    let pos = Some(5).filter(|&x| x > 0);

    let _ = (v, r, doubled, flat, pos);
}

fn expensive_default() -> i32 {
    42
}
```

#### `Result` 쪽

```rust
fn parse_id(s: &str) -> Result<u32, std::num::ParseIntError> {
    s.parse()
}

fn main() {
    let id = parse_id("42").unwrap_or(0);
    let id = parse_id("x").unwrap_or_else(|_| 0);

    // map: Ok만 변환, Err는 그대로
    let n = parse_id("10").map(|x| x * 2);

    // map_err: Err만 변환
    let n = parse_id("x").map_err(|e| format!("파싱 실패: {e}"));

    // and_then: Ok일 때 또 다른 Result 반환 (flatMap)
    let n = parse_id("10").and_then(|x| {
        if x > 0 {
            Ok(x)
        } else {
            "0".parse() // 타입만 맞추는 예시
        }
    });

    // is_ok / is_err / ok() / err()
    if parse_id("1").is_ok() {
        println!("ok");
    }

    let _ = (id, n);
}
```

#### 언제 `unwrap`/`expect`를 써도 되나

- 테스트 코드
- 리터럴·상수처럼 **실패가 논리적으로 불가능한** 경우  
  (`"42".parse::<i32>().unwrap()`)
- 프로토타입 (머지 전에 제거 권장)

그 외 프로덕션 경로에서는 `?`, `unwrap_or*`, `map_err`를 선호합니다.

#### `unwrap` vs `expect`

둘 다 panic이지만, `expect("포트는 숫자여야 한다")`처럼 **왜 여기선 성공해야 하는지**를 남기면 디버깅이 훨씬 쉽습니다.

#### 체크포인트

- [ ] `Option`을 `ok_or`로 `Result`로 바꾼 뒤 `?`와 연결
- [ ] `map`과 `and_then` 차이를 짧은 예제로 설명·실행
- [ ] 코드베이스에서 `unwrap`을 `expect` 또는 `?`로 바꿀 후보 하나 찾기

---

### 4.4 커스텀 에러와 `std::error::Error`

#### 최소 구현 — `Display` + `Error`

```rust
use std::fmt;
use std::error::Error;

#[derive(Debug)]
struct AppError {
    message: String,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for AppError {} // 기본 메서드만으로도 충분할 때가 많음
```

#### enum으로 여러 원인을 통합 (실무 기본형)

```rust
use std::fmt;
use std::error::Error;
use std::fs;
use std::io;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(ParseIntError),
    InvalidConfig(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO 에러: {e}"),
            AppError::Parse(e) => write!(f, "파싱 에러: {e}"),
            AppError::InvalidConfig(msg) => write!(f, "설정 에러: {msg}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AppError::Io(e) => Some(e),
            AppError::Parse(e) => Some(e),
            AppError::InvalidConfig(_) => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self {
        AppError::Parse(e)
    }
}
```

`source()`를 구현하면 에러 체인(`e.source()`)으로 원인을 추적할 수 있습니다.

#### `thiserror` / `anyhow` (알아두기)

실무에서는 보일러플레이트를 줄이기 위해:

- **라이브러리**: `thiserror`로 enum 에러 정의
- **애플리케이션**: `anyhow::Result` / `anyhow::Error`로 빠르게 전파

```rust
// thiserror 스타일 (개념 — 의존성 추가 필요)
// #[derive(Debug, thiserror::Error)]
// enum AppError {
//     #[error("IO 에러")]
//     Io(#[from] std::io::Error),
//     #[error("파싱 에러")]
//     Parse(#[from] std::num::ParseIntError),
//     #[error("설정 에러: {0}")]
//     InvalidConfig(String),
// }
```

이 단계 실습에서는 **표준 라이브러리만**으로 같은 구조를 손으로 구현해 원리를 익힙니다.

#### axum과의 연결 (미리보기)

```rust
// 개념 스케치 — axum feature에 따라 다름
// impl IntoResponse for AppError {
//     fn into_response(self) -> Response {
//         let status = match &self {
//             AppError::InvalidConfig(_) => StatusCode::BAD_REQUEST,
//             AppError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
//             ...
//         };
//         (status, self.to_string()).into_response()
//     }
// }
```

핸들러가 `Result<T, AppError>`를 반환하면 `?`와 HTTP 매핑이 한 줄로 이어집니다.

#### 체크포인트

- [ ] `AppError` enum에 `Display`, `Error`, `From` 두 개 구현
- [ ] `source()`가 `Some`을 반환하는 경로를 `e.source()`로 출력해보기
- [ ] (선택) 같은 enum을 `thiserror`로 다시 적어 비교

---

### 4.5 종합 실습 — 파싱 + IO 에러를 `?`로 체이닝

설정 파일에서 포트와 워커 수를 읽는 미니 로더입니다.

```rust
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::num::ParseIntError;
use std::path::Path;

#[derive(Debug)]
enum ConfigError {
    Io(io::Error),
    Parse(ParseIntError),
    MissingKey(String),
    InvalidValue(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "파일을 읽을 수 없음: {e}"),
            ConfigError::Parse(e) => write!(f, "숫자 파싱 실패: {e}"),
            ConfigError::MissingKey(k) => write!(f, "필수 키 없음: {k}"),
            ConfigError::InvalidValue(msg) => write!(f, "잘못된 값: {msg}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(value: io::Error) -> Self {
        ConfigError::Io(value)
    }
}

impl From<ParseIntError> for ConfigError {
    fn from(value: ParseIntError) -> Self {
        ConfigError::Parse(value)
    }
}

#[derive(Debug)]
struct Config {
    port: u16,
    workers: usize,
}

/// 매우 단순한 KEY=VALUE 줄 단위 포맷
fn parse_config(text: &str) -> Result<Config, ConfigError> {
    let mut port: Option<u16> = None;
    let mut workers: Option<usize> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| ConfigError::InvalidValue(format!("'=' 없음: {line}")))?;

        match key.trim() {
            "port" => {
                let p: u16 = value.trim().parse()?; // ParseIntError → ConfigError
                if p == 0 {
                    return Err(ConfigError::InvalidValue("port는 0일 수 없음".into()));
                }
                port = Some(p);
            }
            "workers" => {
                let w: usize = value.trim().parse()?;
                if w == 0 {
                    return Err(ConfigError::InvalidValue("workers는 1 이상".into()));
                }
                workers = Some(w);
            }
            other => {
                return Err(ConfigError::InvalidValue(format!("알 수 없는 키: {other}")));
            }
        }
    }

    Ok(Config {
        port: port.ok_or_else(|| ConfigError::MissingKey("port".into()))?,
        workers: workers.ok_or_else(|| ConfigError::MissingKey("workers".into()))?,
    })
}

fn load_config(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path)?; // io::Error → ConfigError
    parse_config(&text)
}

fn main() {
    // 데모용: 파일 대신 문자열로도 테스트 가능
    let sample = "\
## server config
port=8080
workers=4
";
    match parse_config(sample) {
        Ok(cfg) => println!("로드 성공: {cfg:?}"),
        Err(e) => {
            eprintln!("실패: {e}");
            if let Some(src) = e.source() {
                eprintln!("  caused by: {src}");
            }
        }
    }

    // 실제 파일이 있으면:
    // match load_config("app.conf") { ... }
    let _ = load_config;
}
```

#### 이 예제에서 짚을 포인트

1. **`?` + `From`** — IO/파싱이 하나의 `ConfigError`로 합류.
2. **`ok_or_else` + `?`** — `Option`을 `Result`로 바꿔 같은 흐름에 태움.
3. **도메인 에러** (`MissingKey`, `InvalidValue`)는 변환이 아니라 **직접 `Err(...)` 생성**.
4. **`source()`** — 래핑된 원인 출력.

#### 확장 과제

- [ ] `host=127.0.0.1` 필드를 추가하고, 없으면 기본값 `"0.0.0.0"` (`unwrap_or_else` / `unwrap_or`)
- [ ] `parse_config` 단위 테스트를 `#[cfg(test)]` 모듈에 작성 (성공 / 키 없음 / 잘못된 숫자)
- [ ] `map_err`로 `ParseIntError`에 키 이름을 붙여 더 친절한 메시지로 바꾸기
- [ ] (선택) 같은 구조를 `thiserror`로 재작성

---

### 이 단계 한 장 요약

```
실패 표현
  ├─ Result<T, E>     → 복구 가능
  └─ panic!/assert!   → 버그·불변조건

전파
  └─ ?  = match + early return + From::from

콤비네이터
  unwrap/expect/unwrap_or(_else)/map/map_err/and_then/ok_or

커스텀 에러
  enum + Display + Error + From  →  ? 체이닝의 중심
```

---

---

> **다음:** [5단계. 컬렉션 & 이터레이터](#5단계-컬렉션-이터레이터)

## 5단계. 컬렉션 & 이터레이터

> 이어서: 앞 장의 [4단계. 에러 처리](#4단계-에러-처리)에서 이어집니다.

---

### 5.1 컬렉션 기본 API — `Vec` / `HashMap` / `HashSet` / `BTreeMap`

#### `Vec<T>` — 가변 길이 배열

```rust
fn main() {
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);

    let mut v = vec![1, 2, 3]; // 매크로 — 타입·용량 추론
    v.extend([4, 5]);

    println!("len={}, cap={}", v.len(), v.capacity());
    println!("첫 원소 = {}", v[0]);          // 없으면 panic
    println!("안전한 접근 = {:?}", v.get(10)); // Option<&T>

    // 슬라이스로 함수에 넘기기 (2단계 복습)
    print_slice(&v);
    print_slice(&v[1..3]);
}

fn print_slice(s: &[i32]) {
    println!("{s:?}");
}
```

자주 쓰는 API:

| 메서드 | 역할 |
|---|---|
| `push` / `pop` | 끝에서 추가·제거 |
| `insert` / `remove` | 중간 — O(n), 자주 쓰면 다른 구조 검토 |
| `extend` / `append` | 여러 원소 합치기 |
| `retain` | 조건에 맞는 것만 남기기 |
| `drain(..)` | 구간을 빼내며 이터레이터로 |
| `split_at_mut` | 겹치지 않는 가변 슬라이스 둘 |

```rust
fn main() {
    let mut v = vec![1, 2, 3, 4, 5];
    v.retain(|&x| x % 2 == 1);
    println!("{v:?}"); // [1, 3, 5]

    let mut v = vec![10, 20, 30, 40];
    let drained: Vec<_> = v.drain(1..3).collect();
    println!("drained={drained:?}, left={v:?}"); // [20,30], [10,40]
}
```

#### `HashMap<K, V>` — 해시 맵

```rust
use std::collections::HashMap;

fn main() {
    let mut scores = HashMap::new();
    scores.insert(String::from("Blue"), 10);
    scores.insert(String::from("Red"), 50);

    // 조회
    let s = scores.get("Blue"); // Option<&i32>
    let s = scores.get("Blue").copied().unwrap_or(0);

    // entry API — 없으면 넣고, 있으면 수정 (실무 단골)
    scores.entry(String::from("Yellow")).or_insert(30);
    let blue = scores.entry(String::from("Blue")).or_insert(0);
    *blue += 5;

    for (k, v) in &scores {
        println!("{k}: {v}");
    }
}
```

`entry` 패턴으로 단어 빈도:

```rust
use std::collections::HashMap;

fn word_count(text: &str) -> HashMap<&str, usize> {
    let mut map = HashMap::new();
    for w in text.split_whitespace() {
        *map.entry(w).or_insert(0) += 1;
    }
    map
}
```

키는 `Eq + Hash`가 필요합니다. `String`/`i32`/`&str`(수명 주의) 등이 흔합니다.

#### `HashSet<T>` — 중복 없는 집합

```rust
use std::collections::HashSet;

fn main() {
    let mut set = HashSet::from([1, 2, 3]);
    set.insert(2); // false — 이미 있음
    set.insert(4);

    println!("{}", set.contains(&3));

    let a = HashSet::from([1, 2, 3]);
    let b = HashSet::from([2, 3, 4]);
    println!("{:?}", a.intersection(&b).collect::<Vec<_>>()); // 2, 3
    println!("{:?}", a.union(&b).collect::<Vec<_>>());
    println!("{:?}", a.difference(&b).collect::<Vec<_>>());
}
```

#### `BTreeMap` / `BTreeSet` — 정렬된 맵·집합

```rust
use std::collections::BTreeMap;

fn main() {
    let mut map = BTreeMap::new();
    map.insert(3, "c");
    map.insert(1, "a");
    map.insert(2, "b");

    // 키 순서 보장 — 순회·range에 유리
    for (k, v) in &map {
        println!("{k}: {v}");
    }

    for (k, v) in map.range(2..) {
        println!("range {k}: {v}");
    }
}
```

| | `HashMap` | `BTreeMap` |
|---|---|---|
| 순서 | 없음(랜덤화) | 키 정렬 |
| 평균 연산 | O(1) | O(log n) |
| 키 요구 | `Eq + Hash` | `Ord` |
| 언제 | 기본값, 빈도수 | 범위 쿼리, 안정적 순서 |

#### 체크포인트

- [ ] `Vec`에서 `get` vs 인덱싱 차이를 panic으로 체감
- [ ] `entry().or_insert`로 문자 빈도수 맵 만들기
- [ ] 같은 데이터를 `HashMap`과 `BTreeMap`에 넣고 순회 순서 비교

---

### 5.2 `iter` vs `into_iter` vs `iter_mut` — 소유권

컬렉션을 순회하는 세 가지 길이 **무엇을 빌리거나 소비하느냐**입니다.

```rust
fn main() {
    let mut v = vec![String::from("a"), String::from("b")];

    // 1) iter() → Iterator<Item = &T>  (불변 빌림)
    for s in v.iter() {
        println!("len={}", s.len());
    }
    // v 여전히 사용 가능

    // 2) iter_mut() → Iterator<Item = &mut T>
    for s in v.iter_mut() {
        s.push('!');
    }

    // 3) into_iter() → Iterator<Item = T>  (소유권 이동, v 소비)
    for s in v.into_iter() {
        println!("{s}");
    }
    // println!("{v:?}"); // 에러 — moved
}
```

`for x in collection`은 **`IntoIterator`**를 호출합니다.

| 표현 | 대략 동일 | Item |
|---|---|---|
| `for x in &v` | `v.iter()` | `&T` |
| `for x in &mut v` | `v.iter_mut()` | `&mut T` |
| `for x in v` | `v.into_iter()` | `T` |

`HashMap`도 같습니다:

```rust
use std::collections::HashMap;

fn main() {
    let map = HashMap::from([("a", 1), ("b", 2)]);

    for (k, v) in &map {          // (&K, &V)
        println!("{k}: {v}");
    }
    for (k, v) in map {           // (K, V) — map 소비
        println!("{k}: {v}");
    }
}
```

#### 체크포인트

- [ ] 같은 `Vec<String>`에 세 가지 순회를 쓰고, 어떤 게 소비인지 확인
- [ ] `for x in v` 후 `v`를 써서 move 에러를 읽어보기

---

### 5.3 어댑터 체이닝

이터레이터는 **lazy**입니다. `collect` / `sum` / `for` / `count` 같은 **consuming adapter**를 만나야 실제로 돌아갑니다.

```rust
fn main() {
    let v = vec![1, 2, 3, 4, 5, 6];

    let result: Vec<i32> = v
        .iter()
        .filter(|x| *x % 2 == 0)
        .map(|x| x * 10)
        .collect();

    println!("{result:?}"); // [20, 40, 60]
}
```

#### 자주 쓰는 어댑터

```rust
fn main() {
    let v = vec!["10", "x", "20", "30"];

    // map — 각 원소 변환
    let lens: Vec<_> = v.iter().map(|s| s.len()).collect();

    // filter — 조건
    let even: Vec<_> = (0..10).filter(|x| x % 2 == 0).collect();

    // filter_map — filter + map + Option 펼치기
    let nums: Vec<i32> = v
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    // enumerate — (index, item)
    for (i, x) in nums.iter().enumerate() {
        println!("{i}: {x}");
    }

    // zip — 두 이터레이터를 쌍으로 (짧은 쪽에 맞춤)
    let a = [1, 2, 3];
    let b = ["a", "b"];
    let pairs: Vec<_> = a.iter().zip(b.iter()).collect();
    println!("{pairs:?}");

    // flat_map — 하나 → 여러 개로 펼침
    let words = ["hello world", "rust lang"];
    let tokens: Vec<&str> = words.iter().flat_map(|s| s.split_whitespace()).collect();

    // fold / reduce — 누적
    let sum = (1..=10).fold(0, |acc, x| acc + x);
    let sum2: i32 = (1..=10).sum(); // 전용 메서드

    // take / skip / chain
    let first3: Vec<_> = (0..100).take(3).collect();
    let skipped: Vec<_> = (0..5).skip(2).collect();
    let chained: Vec<_> = (0..2).chain(10..12).collect();

    let _ = (lens, even, tokens, sum, sum2, first3, skipped, chained);
}
```

#### `collect`와 타입 명시

`collect`의 반환 타입은 여러 가능성이 있어 **종종 명시**가 필요합니다.

```rust
fn main() {
    let v = vec![1, 2, 3];

    let a: Vec<_> = v.iter().map(|x| x * 2).collect();
    let b = v.iter().map(|x| x * 2).collect::<Vec<_>>();
    let c: std::collections::HashSet<_> = v.iter().copied().collect();

    // Result를 collect하면 하나라도 Err면 전체가 Err — 실무 보석
    let parsed: Result<Vec<i32>, _> = ["1", "2", "x"]
        .iter()
        .map(|s| s.parse::<i32>())
        .collect();
    println!("{parsed:?}"); // Err(...)

    let _ = (a, b, c);
}
```

`collect::<Result<Vec<_>, _>>()` 패턴은 4단계 `?`와 자주 짝을 이룹니다.

#### 체크포인트

- [ ] `filter` + `map` + `collect`로 짝수만 제곱한 `Vec` 만들기
- [ ] `filter_map`으로 파싱 가능한 줄만 모으기
- [ ] `map(parse).collect::<Result<Vec<_>, _>>()` 성공/실패 케이스 확인

---

### 5.4 커스텀 이터레이터 — `Iterator` 직접 구현

필수 구현은 `type Item`과 `fn next(&mut self) -> Option<Self::Item>` 하나면 됩니다. 나머지 어댑터(`map` 등)는 자동으로 따라옵니다.

```rust
struct Counter {
    current: usize,
    max: usize,
}

impl Counter {
    fn new(max: usize) -> Self {
        Self { current: 0, max }
    }
}

impl Iterator for Counter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.max {
            let v = self.current;
            self.current += 1;
            Some(v)
        } else {
            None
        }
    }
}

fn main() {
    let sum: usize = Counter::new(5).sum(); // 0+1+2+3+4 = 10
    let evens: Vec<_> = Counter::new(10).filter(|x| x % 2 == 0).collect();
    println!("sum={sum}, evens={evens:?}");
}
```

슬라이스/범위를 감싼 이터레이터:

```rust
struct Windows<'a, T> {
    slice: &'a [T],
    size: usize,
    index: usize,
}

impl<'a, T> Windows<'a, T> {
    fn new(slice: &'a [T], size: usize) -> Self {
        Self {
            slice,
            size,
            index: 0,
        }
    }
}

impl<'a, T> Iterator for Windows<'a, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.index + self.size > self.slice.len() {
            return None;
        }
        let window = &self.slice[self.index..self.index + self.size];
        self.index += 1;
        Some(window)
    }
}

fn main() {
    let data = [1, 2, 3, 4, 5];
    for w in Windows::new(&data, 3) {
        println!("{w:?}");
    }
    // 표준: data.windows(3) 과 같은 아이디어
}
```

#### 체크포인트

- [ ] `Countdown(n)`이터레이터: n, n-1, …, 1 후 `None`
- [ ] 그 위에 `.map(|x| x * x).collect::<Vec<_>>()` 체이닝

---

### 5.5 종합 실습 — for 없이 단어 빈도수

로드맵 실습: **파일(또는 문자열)에서 읽은 텍스트를 이터레이터 체인만으로** 단어 빈도 집계.

```rust
use std::collections::HashMap;
use std::fs;

fn word_frequencies(text: &str) -> HashMap<String, usize> {
    text
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .fold(HashMap::new(), |mut acc, word| {
            *acc.entry(word).or_insert(0) += 1;
            acc
        })
}

/// 빈도 내림차순 Top N
fn top_n(freq: &HashMap<String, usize>, n: usize) -> Vec<(&String, &usize)> {
    let mut items: Vec<_> = freq.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    items.into_iter().take(n).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 파일이 있으면 읽고, 없으면 샘플 사용
    let text = fs::read_to_string("input.txt").unwrap_or_else(|_| {
        String::from(
            "Rust rust RUST is fun. Iterators in Rust are lazy and powerful. Fun fun fun!",
        )
    });

    let freq = word_frequencies(&text);

    // for 루프 없이 출력도 체인으로
    top_n(&freq, 5)
        .iter()
        .enumerate()
        .for_each(|(i, (word, count))| {
            println!("{}. {word} — {count}", i + 1);
        });

    Ok(())
}
```

포인트:

1. **`split` → `map` 정규화 → `filter` → `fold`/`entry`** — 루프 없이 집계.
2. 정규화에서 `flat_map(to_lowercase)` — 유니코드 소문자가 여러 char일 수 있음.
3. `sort_by` + `take`로 Top N.
4. 파일 IO는 4단계 `Result`/`unwrap_or_else`와 연결.

`for_each`는 사이드이펙트용 consuming adapter입니다. 값을 모을 땐 `collect`가 맞습니다.

#### 확장 과제

- [ ] `BTreeMap`으로 바꿔 알파벳 순 출력
- [ ] 최소 길이 N 미만 단어 제외 (`filter`)
- [ ] `lines().flat_map(split_whitespace)`로 줄 단위 파일 스트리밍 느낌 내기
- [ ] 파싱 실패 줄을 `filter_map`으로 건너뛰는 로그 카운터 버전

---

### 이 단계 한 장 요약

```
Vec / HashMap / HashSet / BTreeMap
iter(&T)  iter_mut(&mut T)  into_iter(T)

Iterator = lazy
  adapters: map filter filter_map flat_map zip enumerate take fold …
  consume:  collect sum count for_each any all …

collect::<Vec<_>>() / collect::<Result<Vec<_>, _>>()
custom: impl Iterator { type Item; fn next }
```

---

---

> **다음:** [6단계. 클로저](#6단계-클로저)

## 6단계. 클로저

> 이어서: 앞 장의 [5단계. 컬렉션 & 이터레이터](#5단계-컬렉션-이터레이터)에서 이어집니다.

---

### 6.1 클로저 문법과 타입 추론

#### 기본 형태

```rust
fn main() {
    let add_one = |x| x + 1;
    let add_one = |x: i32| -> i32 { x + 1 }; // 타입 명시

    println!("{}", add_one(5));

    // 인자가 없으면
    let answer = || 42;
    println!("{}", answer());

    // 여러 인자
    let sum = |a, b| a + b;
    println!("{}", sum(2, 3));
}
```

함수와 비교:

```rust
fn add_one_fn(x: i32) -> i32 {
    x + 1
}

fn main() {
    let add_one_closure = |x: i32| x + 1;
    // 문법은 짧지만, 클로저는 **주변 변수를 캡처**할 수 있음
}
```

#### 타입 추론 — 첫 사용에서 고정

```rust
fn main() {
    let example = |x| x;

    let s = example(String::from("hi"));
    // let n = example(5); // 에러 — 이미 String으로 고정됨
    println!("{s}");
}
```

함수는 시그니처에 타입이 필수이고, 클로저는 **문맥에서 추론**됩니다. 그래서 `map(|x| x + 1)`처럼 짧게 쓸 수 있습니다.

#### 체크포인트

- [ ] `|x| x * 2`를 `map`에 넘겨 `Vec` 만들기
- [ ] 같은 클로저에 다른 타입을 두 번 넣어 추론 고정 에러 확인

---

### 6.2 `Fn` / `FnMut` / `FnOnce` — 환경을 어떻게 캡처하는가

클로저는 주변 변수를 세 가지 방식으로 캡처합니다. 그에 대응하는 트레잇:

| 트레잇 | 호출 시 self | 캡처 | 여러 번 호출 |
|---|---|---|---|
| `FnOnce` | `self` (소비) | 환경을 move로 가져감 | 한 번만 보장 |
| `FnMut` | `&mut self` | 환경을 가변 빌림 | 여러 번, 수정 가능 |
| `Fn` | `&self` | 환경을 불변 빌림 | 여러 번, 읽기만 |

계층: **`Fn` ⊂ `FnMut` ⊂ `FnOnce`**  
(`Fn`이면 `FnMut`/`FnOnce`로도 쓸 수 있음)

#### 불변 캡처 → `Fn`

```rust
fn main() {
    let x = 10;
    let print_x = || println!("x = {x}"); // &x 캡처
    print_x();
    print_x(); // 여러 번 OK
    println!("여전히 x = {x}");
}
```

#### 가변 캡처 → `FnMut`

```rust
fn main() {
    let mut count = 0;
    let mut inc = || {
        count += 1; // &mut count
        println!("count = {count}");
    };
    inc();
    inc();
    // println!("{count}"); // inc가 아직 살아 있으면 빌림 충돌 가능
}
```

#### 소비 캡처 → `FnOnce`

```rust
fn main() {
    let name = String::from("Rust");
    let consume = || {
        let n = name; // name을 move
        println!("bye {n}");
    };
    consume();
    // consume(); // 에러 — FnOnce는 두 번 호출 불가
    // println!("{name}"); // 이미 이동됨
}
```

`drop(name)`처럼 환경의 값을 소비하면 `FnOnce`만 구현됩니다.

#### 트레잇 바운드로 콜백 받기

```rust
fn call_twice<F>(mut f: F)
where
    F: FnMut(),
{
    f();
    f();
}

fn consume_with<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    f()
}

fn main() {
    let mut n = 0;
    call_twice(|| n += 1);
    println!("n = {n}");

    let s = String::from("hi");
    let s = consume_with(|| s); // s 이동 후 반환
    println!("{s}");
}
```

#### 체크포인트

- [ ] 같은 변수를 읽기만 / 수정 / move 하는 클로저 셋을 만들고 각각 몇 번 호출 가능한지 확인
- [ ] `FnMut` 바운드 함수에 `Fn` 클로저를 넘겨보기 (동작해야 함)

---

### 6.3 `move` 클로저

기본 캡처는 "필요 최소" (가능하면 참조). **`move`를 붙이면 캡처를 전부 소유권 이동**으로 강제합니다.

```rust
fn main() {
    let name = String::from("async");

    let f = move || {
        println!("Hello, {name}");
    };
    // println!("{name}"); // 에러 — 이미 클로저 안으로 move
    f();
}
```

#### 왜 필요한가 — 스레드

```rust
use std::thread;

fn main() {
    let data = vec![1, 2, 3];

    // move 없으면: 스레드가 data를 빌리려 하는데,
    // main의 data가 먼저 끝날 수 있어 컴파일 에러
    let handle = thread::spawn(move || {
        println!("data = {data:?}");
    });

    handle.join().unwrap();
}
```

async에서도 동일합니다. `tokio::spawn(async move { ... })`의 `move`가 같은 이유입니다 — **퓨처가 현재 스택 프레임보다 오래 살 수 있으므로** 참조 대신 소유가 안전합니다.

#### `move` + `Fn`이 되는 경우

`Copy` 타입을 move로 캡처하면 원본은 그대로이고, 클로저 안의 복사본을 불변으로만 쓰면 `Fn`일 수 있습니다.

```rust
fn main() {
    let x = 5; // Copy
    let f = move || println!("{x}");
    f();
    f();
    println!("{x}"); // OK
}
```

#### 체크포인트

- [ ] `thread::spawn`에 `move` 없는 클로저로 에러를 재현한 뒤 `move`로 고치기
- [ ] `String`을 `move`로 캡처한 뒤 원본 사용 에러 확인

---

### 6.4 함수 포인터 `fn` vs 클로저

```rust
fn add_one(x: i32) -> i32 {
    x + 1
}

fn do_twice(f: fn(i32) -> i32, arg: i32) -> i32 {
    f(arg) + f(arg)
}

fn main() {
    let answer = do_twice(add_one, 5);
    println!("{answer}"); // 12

    // 상태를 캡처하지 않는 클로저는 fn으로 coerce 가능
    let answer = do_twice(|x| x + 1, 5);

    // 환경을 캡처하면 fn이 아님
    let y = 2;
    // let answer = do_twice(|x| x + y, 5); // 에러
    // → 제네릭 F: Fn(i32) -> i32 로 받아야 함
}
```

| | `fn` | 클로저 |
|---|---|---|
| 타입 | 함수 포인터 (하나의 타입) | **클로저마다 고유한 익명 타입** |
| 캡처 | 불가 | 가능 |
| 크기 | 포인터 크기 | 캡처한 환경만큼 |
| 어디에 넘기나 | `fn(...)` 파라미터, C FFI | `impl Fn*` / `F: Fn*` / `Box<dyn Fn*>` |

실무: Rust 콜백은 거의 항상 **제네릭 + `Fn`/`FnMut`/`FnOnce`**. `fn`은 FFI·함수 테이블·캡처 없는 훅에 씁니다.

#### `Box<dyn Fn>` — 이종·저장

```rust
fn main() {
    let handlers: Vec<Box<dyn Fn(i32) -> i32>> = vec![
        Box::new(|x| x + 1),
        Box::new(|x| x * 2),
    ];
    for h in handlers {
        println!("{}", h(10));
    }
}
```

#### 체크포인트

- [ ] `fn(i32)->i32`를 받는 함수와 `F: Fn(i32)->i32`를 받는 함수를 나란히 작성
- [ ] 캡처 있는 클로저를 `fn` 파라미터에 넣어 에러 확인

---

### 6.5 종합 실습 — 제네릭 + `Fn` 콜백

```rust
/// 각 원소에 콜백을 적용한 새 Vec
fn map_vec<T, U, F>(input: Vec<T>, mut f: F) -> Vec<U>
where
    F: FnMut(T) -> U,
{
    let mut out = Vec::with_capacity(input.len());
    for item in input {
        out.push(f(item));
    }
    out
}

/// 조건을 만족하는 동안 누적 (단순 reduce)
fn reduce<T, F>(items: &[T], init: T, mut f: F) -> T
where
    T: Clone,
    F: FnMut(T, &T) -> T,
{
    let mut acc = init;
    for item in items {
        acc = f(acc, item);
    }
    acc
}

/// 이벤트 버스 흉내 — FnMut 콜백 등록
struct Bus {
    listeners: Vec<Box<dyn FnMut(&str)>>,
}

impl Bus {
    fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    fn on<F>(&mut self, listener: F)
    where
        F: FnMut(&str) + 'static,
    {
        self.listeners.push(Box::new(listener));
    }

    fn emit(&mut self, event: &str) {
        for listener in &mut self.listeners {
            listener(event);
        }
    }
}

fn main() {
    let nums = vec![1, 2, 3];
    let doubled = map_vec(nums, |x| x * 2);
    println!("{doubled:?}");

    let sum = reduce(&[1, 2, 3, 4], 0, |acc, x| acc + *x);
    println!("sum = {sum}");

    let mut bus = Bus::new();
    let mut log_count = 0;
    bus.on(move |e| {
        log_count += 1;
        println!("[{log_count}] event: {e}");
    });
    // 주의: 위 move 클로저는 log_count를 소유. Bus가 'static 요구.
    // log_count를 바깥에서 다시 쓰려면 다른 패턴(Cell/RefCell/채널) 필요 — 7단계.

    bus.emit("start");
    bus.emit("stop");
}
```

`Bus` 예제의 `move` + `'static`은 “콜백을 struct에 저장하려면 수명이 어떻게 되나”를 미리 보여 줍니다. 더 나은 공유는 7단계 `Rc`/`RefCell`에서 다룹니다.

#### 확장 과제

- [ ] `filter_vec<T, F>(input: Vec<T>, f: F) -> Vec<T> where F: FnMut(&T) -> bool` 구현
- [ ] `map_vec`를 이터레이터 `.map().collect()`로 다시 쓰고 동작 비교
- [ ] `FnOnce`만 받는 `once(f)` 헬퍼 작성 — 두 번 호출 시 컴파일 에러 유도

---

### 이 단계 한 장 요약

```
|x| expr          클로저, 타입 추론
캡처
  읽기   → Fn
  수정   → FnMut
  소비   → FnOnce
move     → 캡처를 소유권으로 (thread/async)
fn(...)  → 캡처 없는 함수 포인터
콜백 API → F: Fn/FnMut/FnOnce 제네릭 (또는 Box<dyn Fn*>)
```

---

---

> **다음:** [7단계. 스마트 포인터](#7단계-스마트-포인터)

## 7단계. 스마트 포인터

> 이어서: 앞 장의 [6단계. 클로저](#6단계-클로저)에서 이어집니다.

---

### 7.1 `Box<T>` — 힙 할당

#### 왜 쓰는가

1. **큰 데이터를 스택 대신 힙에** 두고, 스택에는 포인터만.
2. **재귀 타입** — 크기를 알 수 없는 타입을 컴파일하려면 간접이 필요.
3. **소유권을 옮기되** 복사 비용은 포인터만 (큰 struct 이동).
4. **`dyn Trait`** — `Box<dyn Shape>` (3단계).

```rust
fn main() {
    let b = Box::new(5);
    println!("{b}"); // Deref로 T처럼 사용
}
```

#### 재귀 타입 — 리스트

```rust
// enum List { Cons(i32, List), Nil }  // 크기 무한 → 에러

enum List {
    Cons(i32, Box<List>),
    Nil,
}

use List::{Cons, Nil};

fn main() {
    let list = Cons(1, Box::new(Cons(2, Box::new(Cons(3, Box::new(Nil))))));
    let _ = list;
}
```

`Box`는 **유일한 소유자**입니다. 공유가 필요하면 `Rc`/`Arc`로 갑니다.

#### 체크포인트

- [ ] `Box::new`로 큰 배열(예: `[0u8; 1024]`)을 힙에 두기
- [ ] 재귀 `List`를 `Cons`/`Nil`로 만들어 보기

---

### 7.2 `Rc<T>` / `Arc<T>` — 참조 카운팅

#### `Rc<T>` — 단일 스레드 공유 소유권

```rust
use std::rc::Rc;

fn main() {
    let a = Rc::new(String::from("shared"));
    let b = Rc::clone(&a); // 깊은 복사 아님 — 카운트 +1
    let c = Rc::clone(&a);

    println!("count = {}", Rc::strong_count(&a)); // 3
    println!("{a}, {b}, {c}");
} // 스코프 끝에서 카운트 감소, 0이 되면 해제
```

`Rc::clone`은 관례상 `.clone()`보다 `Rc::clone(&x)`로 써서 “비싸지 않은 클론”임을 드러냅니다.

#### `Arc<T>` — 스레드 안전 버전

```rust
use std::sync::Arc;
use std::thread;

fn main() {
    let data = Arc::new(vec![1, 2, 3]);
    let mut handles = vec![];

    for i in 0..3 {
        let data = Arc::clone(&data);
        handles.push(thread::spawn(move || {
            println!("thread {i}: {:?}", data);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}
```

| | `Rc` | `Arc` |
|---|---|---|
| 스레드 | 단일 | 다중 (atomic refcount) |
| 비용 | 더 저렴 | 약간 더 비쌈 |
| axum AppState | — | **`Arc<AppState>` 표준** |

`Rc`/`Arc`가 주는 건 **불변 공유**입니다. 공유하면서 내용을 바꾸려면 아래 `RefCell`/`Mutex`가 필요합니다.

#### 체크포인트

- [ ] `Rc`로 두 리스트가 같은 꼬리를 공유하는 그래프 흉내
- [ ] `Rc`를 `thread::spawn`에 넣어 에러를 보고 `Arc`로 고치기

---

### 7.3 `RefCell<T>` / `Cell<T>` — 내부 가변성

#### 빌림 규칙을 **런타임**에 검사

보통 `&T`로는 수정 불가. `RefCell`은 **불변 참조만 있어도** 내부 값을 빌림 체크 후 수정하게 합니다. 위반 시 **컴파일 에러가 아니라 panic**.

```rust
use std::cell::RefCell;

fn main() {
    let data = RefCell::new(5);

    *data.borrow_mut() += 1;
    println!("{}", data.borrow());

    // 런타임 panic 예 (주석 해제 금지 권장):
    // let r1 = data.borrow();
    // let r2 = data.borrow_mut(); // already borrowed
}
```

#### `Cell<T>` — `Copy` 타입에 단순 set/get

```rust
use std::cell::Cell;

fn main() {
    let c = Cell::new(1);
    c.set(2);
    println!("{}", c.get());
}
```

`Cell`은 참조를 빌려주지 않고 값 복사로 出入합니다. `Copy`가 아닌 큰 타입에는 `RefCell`이 맞습니다.

#### `Rc<RefCell<T>>` — 공유 + 내부 가변

```rust
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let shared = Rc::new(RefCell::new(vec![1]));
    let a = Rc::clone(&shared);
    let b = Rc::clone(&shared);

    a.borrow_mut().push(2);
    b.borrow_mut().push(3);
    println!("{:?}", shared.borrow()); // [1, 2, 3]
}
```

단일 스레드 UI·트리·그래프에서 매우 흔합니다.  
멀티스레드면 `Arc<Mutex<T>>` / `Arc<RwLock<T>>`가 대응 패턴입니다.

#### 체크포인트

- [ ] `RefCell`에서 불변 borrow 중 mut borrow로 panic 재현 (짧게)
- [ ] `Rc<RefCell<i32>>`로 두 핸들이 같은 카운터를 올리기

---

### 7.4 순환 참조와 `Weak<T>`

`Rc`끼리 서로를 강한 참조로 가리키면 카운트가 0이 되지 않아 **누수**가 납니다.

```rust
use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Debug)]
struct Node {
    value: i32,
    parent: RefCell<Weak<Node>>,          // 부모는 Weak
    children: RefCell<Vec<Rc<Node>>>,     // 자식은 Rc
}

fn main() {
    let leaf = Rc::new(Node {
        value: 3,
        parent: RefCell::new(Weak::new()),
        children: RefCell::new(vec![]),
    });

    println!("leaf strong={}, weak={}", Rc::strong_count(&leaf), Rc::weak_count(&leaf));

    {
        let branch = Rc::new(Node {
            value: 5,
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(vec![Rc::clone(&leaf)]),
        });

        *leaf.parent.borrow_mut() = Rc::downgrade(&branch);

        println!(
            "branch strong={}, weak={}",
            Rc::strong_count(&branch),
            Rc::weak_count(&branch)
        );
        println!("leaf parent = {:?}", leaf.parent.borrow().upgrade());
    } // branch drop → leaf의 parent Weak는 upgrade 시 None

    println!("leaf parent = {:?}", leaf.parent.borrow().upgrade());
    println!("leaf strong={}", Rc::strong_count(&leaf));
}
```

규칙 감각:

- **소유 관계의 방향**에는 `Rc` (부모 → 자식).
- **역참조**(자식 → 부모, 캐시, 옵저버)에는 `Weak`.
- `Weak`는 `upgrade() -> Option<Rc<T>>`로만 접근 — 이미 죽었으면 `None`.

#### 체크포인트

- [ ] 부모·자식을 둘 다 `Rc`로만 연결해 `strong_count`가 안 내려가는 상황 관찰 후 `Weak`로 수정
- [ ] `upgrade` 성공/실패를 분기하는 코드 작성

---

### 7.5 종합 실습 — 트리 + `Weak`로 순환 해결

```rust
use std::cell::RefCell;
use std::rc::{Rc, Weak};

#[derive(Debug)]
struct TreeNode {
    name: String,
    parent: RefCell<Weak<TreeNode>>,
    children: RefCell<Vec<Rc<TreeNode>>>,
}

impl TreeNode {
    fn new(name: impl Into<String>) -> Rc<Self> {
        Rc::new(Self {
            name: name.into(),
            parent: RefCell::new(Weak::new()),
            children: RefCell::new(vec![]),
        })
    }

    fn add_child(self: &Rc<Self>, child: &Rc<TreeNode>) {
        child.parent.borrow_mut().replace(Rc::downgrade(self));
        self.children.borrow_mut().push(Rc::clone(child));
    }

    fn path(self: &Rc<Self>) -> String {
        let mut parts = vec![self.name.clone()];
        let mut current = Rc::clone(self);
        while let Some(parent) = current.parent.borrow().upgrade() {
            parts.push(parent.name.clone());
            current = parent;
        }
        parts.reverse();
        parts.join("/")
    }
}

fn main() {
    let root = TreeNode::new("root");
    let home = TreeNode::new("home");
    let async_user = TreeNode::new("async");

    root.add_child(&home);
    home.add_child(&async_user);

    println!("{}", async_user.path()); // root/home/async
    println!("root children = {}", root.children.borrow().len());

    // root만 잡고 있으면 서브트리 유지.
    // 순환이 없으므로 스코프 종료 시 정상 drop.
    drop(async_user);
    drop(home);
    println!("root still alive: {}", root.name);
}
```

#### 확장 과제

- [ ] `find(name: &str) -> Option<Rc<TreeNode>>` DFS 구현
- [ ] 같은 구조를 `Arc` + `Mutex`로 바꿔 스레드에서 자식 추가
- [ ] axum `State(Arc<AppState>)`를 떠올리며: 왜 `Rc`가 아니라 `Arc`인지 한 문단 정리

---

### 이 단계 한 장 요약

```
Box<T>           유일한 힙 소유, 재귀 타입, dyn Trait
Rc<T> / Arc<T>   공유 소유 (단일 / 멀티 스레드)
Cell / RefCell   내부 가변성 (Copy set / 런타임 빌림)
Mutex / RwLock   멀티스레드 내부 가변 (RefCell 대응)
Rc<RefCell<T>>   단일 스레드 공유+가변 단골
Weak<T>          순환 참조 끊기, upgrade → Option
```

---

---

> **다음:** [8단계. 모듈 & 프로젝트 구조](#8단계-모듈-프로젝트-구조)

## 8단계. 모듈 & 프로젝트 구조

> 이어서: 앞 장의 [7단계. 스마트 포인터](#7단계-스마트-포인터)에서 이어집니다.

---

### 8.1 `mod` / `pub` / `pub(crate)` / `use`

#### 모듈은 트리다

```rust
// lib.rs 또는 main.rs
mod front_of_house {
    pub mod hosting {
        pub fn add_to_waitlist() {}
        fn seat_at_table() {} // 모듈 안에서만
    }
    mod serving {
        fn take_order() {}
    }
}

pub fn eat_at_restaurant() {
    // 절대 경로
    crate::front_of_house::hosting::add_to_waitlist();
    // 상대 경로
    front_of_house::hosting::add_to_waitlist();
}
```

- `crate::` — 크레이트 루트부터
- `self::` — 현재 모듈
- `super::` — 부모 모듈

#### 가시성 키워드

| 키워드 | 의미 |
|---|---|
| (없음) | 부모 모듈까지 private |
| `pub` | 바깥 크레이트에도 공개 |
| `pub(crate)` | 이 크레이트 안에서만 공개 |
| `pub(super)` | 부모 모듈에만 |
| `pub(in path)` | 특정 경로 아래에서만 |

```rust
mod shop {
    pub struct Coffee {
        pub name: String,     // 필드는 각각 pub 필요
        price_cents: u32,     // private 필드
    }

    impl Coffee {
        pub fn new(name: impl Into<String>, price_cents: u32) -> Self {
            Self {
                name: name.into(),
                price_cents,
            }
        }

        pub(crate) fn price_cents(&self) -> u32 {
            self.price_cents
        }
    }
}
```

구조체를 `pub`으로 해도 **필드는 기본 private**입니다. 생성자·게터로 API를 통제하는 패턴이 흔합니다.

#### `use`로 경로 단축

```rust
use crate::front_of_house::hosting;

pub fn eat() {
    hosting::add_to_waitlist();
}

// 재수출 — 내부 구조를 숨기고 루트 API만 노출
pub use crate::front_of_house::hosting::add_to_waitlist;

// 이름 충돌 시
use std::fmt::Result as FmtResult;
use std::io::Result as IoResult;
```

관례:

- 구조체·enum·트레잇 → `use`로 타입 이름까지
- 함수 → 모듈까지만 `use`하고 `module::func()` (출처가 보임)
- 예외: `use std::collections::HashMap;`처럼 너무 흔한 것

#### 체크포인트

- [ ] nested `mod`에서 `pub`을 빼먹고 호출해 에러 확인
- [ ] `pub(crate)` 함수를 같은 크레이트 / (가상으로) 외부에서 쓰는 차이 설명

---

### 8.2 파일 시스템 ↔ 모듈 트리

#### 2018+ 스타일 (권장)

```
src/
  main.rs          // 또는 lib.rs
  models.rs        // mod models;
  models/
    user.rs        // models.rs 안에서 mod user;
    post.rs
  handlers.rs
  handlers/
    auth.rs
    posts.rs
  errors.rs
```

```rust
// main.rs
mod models;
mod handlers;
mod errors;

fn main() {
    // ...
}
```

```rust
// models.rs
pub mod user;
pub mod post;

pub use user::User; // 재수출로 crate::models::User 가능
```

```rust
// models/user.rs
pub struct User {
    pub id: u64,
    pub name: String,
}
```

#### 구식 `mod.rs` 스타일 (아직 많이 봄)

```
src/
  models/
    mod.rs      // 예전엔 폴더 = mod.rs
    user.rs
```

둘 다 유효합니다. **한 프로젝트에선 한 스타일로 통일**하세요. 새 코드는 `models.rs` + `models/` 조합이 읽기 쉽습니다.

#### `main.rs` vs `lib.rs`

- 바이너리만: `main.rs`
- 라이브러리: `lib.rs`가 루트
- 둘 다 있으면: `main.rs`가 `use my_crate::...`로 라이브러리를 호출 — **로직은 lib에, 진입점만 main**이 이상적

#### 체크포인트

- [ ] `mod foo;` + `foo.rs` 파일을 실제로 만들어 컴파일
- [ ] 서브모듈 `foo/bar.rs`를 `foo.rs`에서 `mod bar;`로 연결

---

### 8.3 워크스페이스

여러 크레이트를 한 저장소에서 공통 lockfile로 관리합니다.

```toml
## 루트 Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/api",
    "crates/core",
    "crates/macros",
]
```

```
my-project/
  Cargo.toml          # workspace
  Cargo.lock
  crates/
    api/Cargo.toml
    api/src/main.rs
    core/Cargo.toml
    core/src/lib.rs
```

```toml
## crates/api/Cargo.toml
[dependencies]
core = { path = "../core" }
tokio = { version = "1", features = ["full"] }
```

이점: 의존성 버전 통일, `cargo test --workspace`, 공통 프로파일.

#### 체크포인트

- [ ] (개념) axum 앱과 도메인 로직을 `api` / `core`로 나눈다고 가정하고 members 목록 스케치

---

### 8.4 `[features]`와 `#[cfg]`

#### Cargo features

```toml
## Cargo.toml
[features]
default = ["json"]
json = ["dep:serde_json"]
metrics = []

[dependencies]
serde_json = { version = "1", optional = true }
```

```rust
#[cfg(feature = "json")]
pub fn to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap()
}

#[cfg(not(feature = "json"))]
pub fn to_json<T>(_v: &T) -> String {
    String::from("{}")
}
```

빌드: `cargo build --features metrics` / `--no-default-features`

#### 자주 쓰는 `cfg`

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

#[cfg(target_os = "linux")]
fn os_specific() {}

#[cfg(debug_assertions)]
fn only_debug() {}
```

#### 체크포인트

- [ ] optional 의존성 하나를 feature 뒤에 두고 `cfg`로 감싸 보기
- [ ] `#[cfg(test)]` 모듈에 테스트 하나 작성

---

### 8.5 종합 실습 — axum 스타일 모듈 분리 (스케치)

실제 파일로 나누는 목표 구조:

```
src/
  main.rs
  lib.rs           // 재사용·테스트용 루트
  errors.rs
  models/
    mod.rs 또는 models.rs + …
    user.rs
  handlers/
    mod.rs
    health.rs
    users.rs
  state.rs
```

```rust
// errors.rs
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound,
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound => write!(f, "not found"),
            AppError::Internal(msg) => write!(f, "internal: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}
```

```rust
// models/user.rs
#[derive(Debug, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
}
```

```rust
// handlers/users.rs (axum 없이도 시그니처 감각)
use crate::errors::AppError;
use crate::models::User;

pub fn get_user(id: u64) -> Result<User, AppError> {
    if id == 0 {
        return Err(AppError::NotFound);
    }
    Ok(User {
        id,
        name: String::from("async"),
    })
}
```

```rust
// lib.rs
pub mod errors;
pub mod handlers;
pub mod models;
pub mod state;

pub use errors::AppError;
```

```rust
// main.rs
use my_crate::handlers;

fn main() {
    match handlers::users::get_user(1) {
        Ok(u) => println!("{u:?}"),
        Err(e) => eprintln!("{e}"),
    }
}
```

실습 순서:

1. 빈 바이너리/라이브러리에 위 모듈 파일 생성
2. `cargo check`로 경로·가시성 에러를 하나씩 해결
3. (선택) axum `Router`를 `handlers`에 모으고 `State(Arc<AppState>)`는 `state.rs`

#### 확장 과제

- [ ] `pub use`로 `crate::User`처럼 flat re-export API 만들기
- [ ] workspace에 `core`(순수 로직) / `api`(axum) 분리
- [ ] `metrics` feature로 로그 한 줄 조건부 컴파일

---

### 이 단계 한 장 요약

```
mod 트리 + pub / pub(crate)
use / pub use (재수출)
파일: foo.rs + foo/  또는  foo/mod.rs
workspace members
features + #[cfg(...)]
실전: handlers / models / errors / state
```

---

---

> **다음:** [9단계. 매크로](#9단계-매크로)

## 9단계. 매크로

> 이어서: 앞 장의 [8단계. 모듈 & 프로젝트 구조](#8단계-모듈-프로젝트-구조)에서 이어집니다.

---

### 9.1 선언적 매크로 — `macro_rules!`

#### 매크로는 함수가 아니다

함수는 값의 타입을 다루고, 매크로는 **토큰 트리**를 다른 코드로 바꿔 넣습니다. 가변 인자·문법 확장·컴파일 전에 코드 생성이 가능합니다.

```rust
macro_rules! say_hello {
    () => {
        println!("Hello!");
    };
}

fn main() {
    say_hello!();
}
```

호출은 `!`가 붙습니다: `println!`, `vec!`, `say_hello!`.

#### 매처와 조각(fragment) 지정자

```rust
macro_rules! create_function {
    ($func_name:ident) => {
        fn $func_name() {
            println!("함수 {:?} 호출됨", stringify!($func_name));
        }
    };
}

create_function!(foo);
create_function!(bar);

fn main() {
    foo();
    bar();
}
```

자주 쓰는 지정자:

| 지정자 | 의미 | 예 |
|---|---|---|
| `ident` | 식별자 | 함수·변수 이름 |
| `expr` | 표현식 | `1 + 2`, `foo()` |
| `ty` | 타입 | `i32`, `Vec<u8>` |
| `path` | 경로 | `std::vec::Vec` |
| `pat` | 패턴 | `Some(x)` |
| `stmt` | 문장 | |
| `block` | 블록 `{ ... }` |
| `tt` | 토큰 트리 | 거의 아무거나 |
| `literal` | 리터럴 | `"hi"`, `42` |

#### 반복 — `$()*` / `$()+` / `$()?`

```rust
macro_rules! vec_strs {
    ( $( $x:expr ),* ) => {
        {
            let mut v = Vec::new();
            $(
                v.push(format!("{}", $x));
            )*
            v
        }
    };
}

fn main() {
    let v = vec_strs![1, "two", 3.0];
    println!("{v:?}");
}
```

- `$( ... ),*` — 0회 이상, 쉼표 구분
- `$( ... ),+` — 1회 이상
- `$( ... )?` — 0 또는 1회
- 구분자 변경: `$( $x:expr );*` 등

#### `vec!`을 흉내 내기

```rust
macro_rules! my_vec {
    () => { Vec::new() };
    ( $( $elem:expr ),+ $(,)? ) => {
        {
            let mut v = Vec::new();
            $( v.push($elem); )*
            v
        }
    };
}

fn main() {
    let a: Vec<i32> = my_vec![];
    let b = my_vec![1, 2, 3];
    let c = my_vec![1, 2, 3,]; // trailing comma
    println!("{a:?} {b:?} {c:?}");
}
```

#### 위생(hygiene) — 변수 충돌

`macro_rules!`는 기본적으로 위생적입니다. 매크로 안의 `$x`와 호출 측 `x`가 의도치 않게 섞이지 않습니다. (완전 제어가 필요하면 proc macro 영역.)

#### 체크포인트

- [ ] `macro_rules! hash_map`으로 `hash_map!("a" => 1, "b" => 2)` 형태 만들기
- [ ] `$(,)?` trailing comma를 지원하는지 실험

---

### 9.2 Derive 매크로 — 읽는 법

```rust
#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}
```

컴파일러(정확히는 derive 매크로)가 대략 이런 `impl`을 **생성**합니다:

```rust
impl std::fmt::Debug for Point { /* ... */ }
impl Clone for Point { /* 필드별 clone */ }
impl PartialEq for Point { /* 필드별 == */ }
```

실무에서 자주 보는 조합:

| Derive | 용도 |
|---|---|
| `Debug` | `{:?}` 출력 |
| `Clone` / `Copy` | 복제 (Copy는 비트복사 가능 타입만) |
| `PartialEq` / `Eq` | 동등 비교 |
| `PartialOrd` / `Ord` | 정렬 |
| `Default` | `Default::default()` |
| `Hash` | `HashMap` 키 |
| `Serialize` / `Deserialize` | serde JSON 등 |
| thiserror의 `Error` | 에러 Display/From |

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct User {
    id: u64,
    name: String,
}
```

**직접 손으로 쓰는 `impl Debug`와 derive의 차이**를 한 번 비교해 보면, derive가 얼마나 보일러플레이트를 줄이는지 체감됩니다.

#### 체크포인트

- [ ] `PartialEq` 없는 struct를 `==`로 비교해 에러 확인 후 derive 추가
- [ ] `Copy`를 `String` 필드 struct에 붙여 orphan/제약 에러 읽기

---

### 9.3 절차적 매크로(proc macro) — 개념만

세 종류:

1. **Derive macros** — `#[derive(MyTrait)]`
2. **Attribute macros** — `#[route(GET, "/")]` (axum 계열 아이디어), `#[tokio::main]`
3. **Function-like macros** — `sqlx::query!()`, `println!`의 더 강한 버전

특징:

- 별도 크레이트 `proc-macro = true`
- 입력·출력은 `TokenStream`
- `syn`으로 파싱, `quote`로 코드 생성

```rust
// 개념 — 직접 구현은 이 단계 범위 밖
// #[proc_macro_derive(MyDebug)]
// pub fn my_debug(input: TokenStream) -> TokenStream { ... }
```

**읽는 연습**:

- `#[tokio::main]` → `async fn main`을 런타임 진입 코드로 래핑
- `#[derive(Deserialize)]` → 필드별 파서 생성
- axum extractor / `#[debug_handler]` → 시그니처 검사·에러 메시지 개선

직접 만들기보다 **expand**해서 보는 게 학습에 좋습니다.

```bash
cargo install cargo-expand
cargo expand
```

#### 체크포인트

- [ ] `#[tokio::main]` 또는 `#[derive(Debug)]` 하나를 `cargo expand`로 펼쳐 보기 (프로젝트 있을 때)
- [ ] serde `Deserialize`가 붙은 struct의 JSON 라운드트립 한 번 실행

---

### 9.4 종합 실습 — 테스트 케이스 생성 매크로

```rust
macro_rules! assert_parse_eq {
    ( $( $input:expr => $expected:expr ),+ $(,)? ) => {
        $(
            assert_eq!(
                $input.parse::<i32>().unwrap(),
                $expected,
                "input = {:?}",
                $input
            );
        )+
    };
}

macro_rules! test_cases {
    (
        $fn_name:ident,
        $( $name:ident : $input:expr => $expected:expr ),+ $(,)?
    ) => {
        $(
            #[test]
            fn $name() {
                let got = $fn_name($input);
                assert_eq!(got, $expected);
            }
        )+
    };
}

fn double(x: i32) -> i32 {
    x * 2
}

test_cases! {
    double,
    doubles_2: 2 => 4,
    doubles_5: 5 => 10,
    doubles_0: 0 => 0,
}

fn main() {
    assert_parse_eq! {
        "10" => 10,
        "42" => 42,
        "-1" => -1,
    };
    println!("assertions ok");
}
```

`test_cases!`는 테스트 이름을 매크로가 만들어 반복 `#[test] fn ...`을 줄입니다. 프로덕션에서도 비슷한 패턴으로 “표 형태 입력 → 코드”를 만듭니다.

#### 확장 과제

- [ ] `hash_map!(k => v, ...)`가 `HashMap`을 반환하도록 구현
- [ ] `ok_or_log!(expr)` 매크로: `Err`면 `eprintln` 후 early return (함수가 `Result`일 때) — 난이도 있음
- [ ] derive 가능한 작은 struct에 `Debug`를 손으로 구현 vs derive 비교

---

### 이 단계 한 장 요약

```
macro_rules!   토큰 매칭 → 코드 삽입  ($(...)* , ident/expr/ty…)
derive         흔한 impl 자동 생성
proc macro     TokenStream in/out — syn/quote, 읽기·expand 위주
실습           반복 테스트/맵 리터럴을 매크로로
```

---

---

> **다음:** [10단계. Unsafe & FFI (선택)](#10단계-unsafe-ffi-선택)

## 10단계. Unsafe & FFI (선택)

> 이어서: 앞 장의 [9단계. 매크로](#9단계-매크로)에서 이어집니다.

---

### 10.1 `unsafe`가 허용하는 다섯 가지

Safe Rust가 금지하고, `unsafe` 블록/`unsafe fn` 안에서만 가능한 것:

1. **raw pointer 역참조** (`*const T`, `*mut T`)
2. **`unsafe` 함수/메서드 호출**
3. **mutable static 접근**
4. **`unsafe` trait 구현**
5. **`union` 필드 접근**

```rust
fn main() {
    let mut num = 5;
    let r1 = &num as *const i32;
    let r2 = &mut num as *mut i32;

    unsafe {
        println!("r1 = {}", *r1);
        *r2 = 10;
        println!("r2 = {}", *r2);
    }
}
```

`unsafe` 밖에서는 raw pointer를 **만들고 넘기는 것**까지는 가능합니다. 위험한 연산(역참조 등)만 블록 안에 둡니다. **최소 범위**로 감싸는 게 관례입니다.

#### `unsafe fn` — 호출자가 계약을 지켜야 함

```rust
/// # Safety
/// `ptr`는 유효하고 정렬된 `i32`를 가리켜야 하며, 수명 동안 유효해야 한다.
unsafe fn read_i32(ptr: *const i32) -> i32 {
    unsafe { *ptr }
}

fn main() {
    let x = 42;
    let v = unsafe { read_i32(&x) };
    println!("{v}");
}
```

문서에 `# Safety` 섹션을 남기는 것이 사실상 필수 문화입니다.

#### mutable static

```rust
static mut COUNTER: u32 = 0;

fn main() {
    unsafe {
        COUNTER += 1;
        println!("{COUNTER}");
    }
}
```

데이터 레이스 가능 → 실무에서는 `AtomicU32`, `Mutex`, `OnceLock`을 쓰세요. (1단계 `static` 복습)

#### 체크포인트

- [ ] raw pointer를 `unsafe` 없이 역참조해 에러 확인
- [ ] `unsafe` 블록을 역참조 한 줄만 남기도록 좁히기

---

### 10.2 Raw pointer 문법

```rust
fn main() {
    let mut x = 5i32;

    let p: *const i32 = &x;
    let q: *mut i32 = &mut x;

    // 널 포인터
    let null: *const i32 = std::ptr::null();
    let null_mut: *mut i32 = std::ptr::null_mut();

    // 오프셋 (bounds는 프로그래머 책임)
    unsafe {
        let a = [10, 20, 30];
        let base = a.as_ptr();
        println!("{}", *base.add(1)); // 20
    }

    let _ = (p, q, null, null_mut);
}
```

참조(`&T`)와의 차이:

| | `&T` / `&mut T` | `*const T` / `*mut T` |
|---|---|---|
| 유효성 | 컴파일러 보장 | 프로그래머 보장 |
| null | 없음 | 가능 |
| 얼라이먼트 | 보장 | 깨질 수 있음 |
| 빌림 규칙 | 적용 | 적용 안 됨 |

`slice::from_raw_parts`, `Box::from_raw` / `into_raw` 등은 FFI·커스텀 할당자에서 자주 만납니다.

```rust
fn main() {
    let b = Box::new(123);
    let ptr = Box::into_raw(b); // 소유권 누수 형태로 넘김 — free 책임 이전
    unsafe {
        println!("{}", *ptr);
        let _ = Box::from_raw(ptr); // 다시 Rust가 drop
    }
}
```

#### 체크포인트

- [ ] `Box::into_raw` / `from_raw` 왕복
- [ ] `as_ptr()`로 배열 두 번째 원소 읽기

---

### 10.3 FFI — `extern "C"`

C ABI로 외부 함수를 선언합니다.

```rust
use std::os::raw::{c_char, c_int};
use std::ffi::CStr;

unsafe extern "C" {
    fn abs(input: c_int) -> c_int;
    // fn lidar_init(cfg: *const c_char) -> c_int; // 드라이버 예
}

fn main() {
    unsafe {
        println!("abs(-3) = {}", abs(-3));
    }
}
```

Rust → C 문자열:

```rust
use std::ffi::CString;

fn main() {
    let c = CString::new("hello").expect("NUL 불가");
    let ptr = c.as_ptr(); // *const c_char
    // C 함수에 ptr 전달 — c가 살아 있는 동안만
    let _ = ptr;
}
```

C → Rust 문자열:

```rust
use std::ffi::CStr;
use std::os::raw::c_char;

unsafe fn from_c(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
}
```

#### `#[no_mangle]` + `extern "C"` — Rust를 C에서 호출

```rust
#[unsafe(no_mangle)] // 에디션/버전에 따라 #[no_mangle]
pub unsafe extern "C" fn rust_add(a: i32, b: i32) -> i32 {
    a + b
}
```

(에디션·rustc 버전에 따라 속성 문법이 조금 다를 수 있습니다. 프로젝트 컴파일러에 맞추세요.)

#### 체크포인트

- [ ] `libc` 또는 `extern "C" { fn abs... }`로 시스템 함수 하나 호출
- [ ] `CString::new`에 내부 `\0`을 넣어 에러 확인

---

### 10.4 `#[repr(C)]` — 메모리 레이아웃

Rust 기본 레이아웃은 최적화로 필드 순서가 바뀔 수 있습니다. C와 맞춰야 하면:

```rust
#[repr(C)]
struct LidarPoint {
    x: f32,
    y: f32,
    z: f32,
    intensity: u16,
}

#[repr(C)]
enum CLike {
    A = 1,
    B = 2,
}
```

자주 쓰는 repr:

| 속성 | 의미 |
|---|---|
| `#[repr(C)]` | C 호환 레이아웃 |
| `#[repr(transparent)]` | 뉴타입 하나를 내부와 동일 ABI로 |
| `#[repr(u8)]` 등 | enum 판별자 크기 고정 |
| `#[repr(packed)]` | 패딩 제거 (얼라이먼트 주의, 위험) |

```rust
#[repr(transparent)]
struct UserId(u64); // FFI에서 u64처럼 전달 가능
```

#### 체크포인트

- [ ] `#[repr(C)]` struct의 `size_of` / `offset_of` (또는 수동 출력)로 필드 오프셋 확인
- [ ] `repr` 없는 같은 struct와 크기가 달라질 수 있음을 인지

---

### 10.5 종합 실습 — C 함수 하나를 Rust에서 호출 (스케치)

로드맵: LiDAR C++ 드라이버의 단순 함수 FFI. 최소 예는 C로도 충분합니다.

**c_api.h / c_api.c** (가상):

```c
// int lidar_version(void) { return 42; }
```

**build.rs** (개념):

```rust
// cc::Build::new().file("c_api.c").compile("lidar");
```

**Rust:**

```rust
unsafe extern "C" {
    fn lidar_version() -> i32;
}

pub fn version() -> i32 {
    unsafe { lidar_version() }
}

fn main() {
    // println!("lidar version = {}", version());
    println!("FFI 스케치 — 실제 .c / build.rs 를 프로젝트에 연결하세요");
}
```

실습 체크리스트:

- [ ] `cc` 크레이트로 C 파일 컴파일
- [ ] `#[repr(C)]`로 포인트 구조체 공유
- [ ] null 체크·에러 코드를 `Result`로 래핑해 **safe 공개 API**만 밖으로 노출

핵심 패턴: **`unsafe`는 모듈 내부에 가두고, 바깥에는 safe 함수만.**

```rust
mod ffi {
    unsafe extern "C" {
        pub(super) fn lidar_version() -> i32;
    }
}

pub fn lidar_version_safe() -> i32 {
    unsafe { ffi::lidar_version() }
}
```

#### 확장 과제

- [ ] C의 `int parse(const char*, int*)` 스타일을 Rust `Result`로 래핑
- [ ] `Box::into_raw`로 Rust 할당 버퍼를 C에 넘기고, 콜백에서 `from_raw`로 회수
- [ ] `static mut` 대신 `AtomicI32`로 카운터 작성

---

### 이 단계 한 장 요약

```
unsafe 5가지: *ptr, unsafe fn, mut static, unsafe trait, union
raw pointer ≠ 참조 (유효성·null·빌림)
extern "C" + CString/CStr
#[repr(C)] / transparent / u8
패턴: unsafe 내부 은닉 → safe API
```

---

---

> **다음:** [11단계. Async 문법](#11단계-async-문법)

## 11단계. Async 문법

> 이어서: 앞 장의 [10단계. Unsafe & FFI (선택)](#10단계-unsafe-ffi-선택)에서 이어집니다.

---

### 11.1 `async fn`은 `impl Future` 설탕이다

```rust
async fn hello() -> String {
    String::from("hello")
}

// 대략 동일:
fn hello_desugared() -> impl std::future::Future<Output = String> {
    async { String::from("hello") }
}
```

`async fn`을 **호출만** 하면 함수 본문은 실행되지 않습니다. **퓨처를 만들 뿐**이고, `.await`되거나 executor가 `poll`해야 진행합니다.

```rust
fn main() {
    let fut = hello(); // 아직 "hello" 문자열 생성 전일 수 있음(개념상)
    // fut.await; // main이 async가 아니면 불가
    let _ = fut;
}
```

Tokio에서는:

```rust
#[tokio::main]
async fn main() {
    let s = hello().await;
    println!("{s}");
}
```

`#[tokio::main]`이 `main`을 런타임 위에서 돌리는 **매크로 설탕**임은 9단계에서 본 내용과 같습니다.

#### `Future` 트레잇 (개념)

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```

- `Poll::Pending` — 아직 아님. 준비되면 `Waker`로 다시 poll 요청.
- `Poll::Ready(val)` — 완료.

#### 체크포인트

- [ ] `async fn`을 호출만 하고 await 없이 두면 사이드이펙트가 안 생기는지 (간단한 `println`으로) 실험
- [ ] `async { 1u32 }` 블록의 타입을 컴파일러에게 물어보기 (`let f = async { 1 };` 후 잘못된 타입 주석)

---

### 11.2 `.await`와 상태 머신

하나의 `async fn` 안에 `.await`가 여러 개면, 컴파일러는 **상태 머신 enum**을 생성합니다. 대략:

```rust
async fn fetch_and_process() -> u32 {
    let a = read_async().await; // 상태 0 → 1
    let b = read_async().await; // 상태 1 → 2
    a + b
}

async fn read_async() -> u32 {
    1
}
```

개념적 변환:

```text
enum FetchState {
    Start,
    WaitingFirst(FirstFut),
    WaitingSecond(u32, SecondFut),
    Done,
}
```

각 `poll`마다 현재 상태에서 내부 퓨처를 `poll`하고, `Ready`면 다음 상태로 전이합니다. **스택을 블로킹하지 않고** 중단·재개할 수 있는 이유입니다.

중요한 문법 포인트:

- `.await`는 `async` 함수/블록 안에서만.
- `await`는 메서드가 아니라 **키워드 연산** (`fut.await`).
- 에러 전파: `let x = fallible().await?` (4단계 `?`와 결합).

#### 체크포인트

- [ ] `async`가 아닌 함수에서 `.await`를 써서 에러 메시지 읽기
- [ ] `some_result()?.await` vs `some_fut.await?` 순서 차이를 한 번 헷갈려 보기

---

### 11.3 `async move`와 클로저

```rust
use std::future::Future;

fn spawn_like<F>(_f: F)
where
    F: Future + Send + 'static,
{
    // executor가 소유하려면 'static + 보통 Send
}

fn main() {
    let name = String::from("async");

    // async move 블록 — name을 퓨처 안으로 이동
    let fut = async move {
        println!("hello, {name}");
    };

    // println!("{name}"); // 이미 이동
    spawn_like(fut);
}
```

`async` 블록도 기본적으로 환경을 캡처합니다. 스레드/태스크보다 오래 살려면 **`async move`**로 소유권을 넘깁니다. 6단계 `move ||`와 같은 동기입니다.

```rust
#[tokio::main]
async fn main() {
    let data = vec![1, 2, 3];
    let handle = tokio::spawn(async move {
        println!("{data:?}");
    });
    handle.await.unwrap();
}
```

#### 체크포인트

- [ ] `tokio::spawn(async { ... })`에서 외부 `&str`을 쓰려다 `'static` 에러를 내고, `move` + owned `String`으로 해결
- [ ] `Arc`로 공유한 뒤 여러 태스크에 `clone` (7단계 연결)

---

### 11.4 `Pin`이 왜 필요한가

#### self-referential 구조 문제

`async` 상태 머신은 종종 **자기 자신을 가리키는 참조**를 품습니다.

```text
구조체 필드에 buffer가 있고,
다른 필드가 그 buffer를 참조
→ 구조체가 메모리에서 이동(move)되면 내부 참조가 무효
```

Rust는 보통 값을 마음대로 옮깁니다. 그걸 막으려고 **`Pin<&mut T>`** — “이 `T`는 메모리상에서 움직이지 않는다”는 약속 — 가 `Future::poll`의 수신자입니다.

```rust
use std::pin::Pin;
use std::task::{Context, Poll};

struct MyFuture;

impl std::future::Future for MyFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        Poll::Ready(())
    }
}
```

#### `Unpin`

대부분의 타입은 `Unpin`입니다 — 핀이 꽂혀 있어도 사실상 이동해도 안전. 그러면 `Pin<&mut T>`에서 `&mut T`를 얻기 쉽습니다.

self-referential async 상태 머신은 종종 `!Unpin`이라, `poll` 전에 `Pin`으로 고정해야 합니다.

```rust
use std::pin::pin; // std::pin::pin! 매크로 (버전 따라 pin!)

// let mut fut = pin!(async { 1 });
// 그 다음 executor가 fut.as_mut().poll(...)
```

실무에서 Tokio를 쓰면 `Pin`을 직접 만질 일은 많지 않습니다. 다만 에러 메시지에 `Pin` / `Unpin`이 뜨면 **“이 퓨처는 이동하면 안 된다”**고 읽으면 됩니다.

#### 체크포인트

- [ ] `Future`를 직접 impl한 스텁 타입을 만들고 `poll`이 `Ready`만 반환하게 하기
- [ ] (선택) `pin!`으로 로컬 퓨처를 고정하는 예제 컴파일

---

### 11.5 종합 실습 — `Future`를 직접 구현하고 수동 `poll`

executor 없이, 즉시 완료되는 퓨처와 “두 번 poll해야 끝나는” 퓨처를 만들어 봅니다.

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// 즉시 Ready
struct ReadyNow(u32);

impl Future for ReadyNow {
    type Output = u32;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u32> {
        Poll::Ready(self.0)
    }
}

/// 첫 poll → Pending, 둘째 → Ready
struct YieldOnce {
    yielded: bool,
    value: u32,
}

impl Future for YieldOnce {
    type Output = u32;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        let this = self.get_mut();
        if !this.yielded {
            this.yielded = true;
            cx.waker().wake_by_ref(); // 다시 poll 요청
            Poll::Pending
        } else {
            Poll::Ready(this.value)
        }
    }
}

fn dummy_waker() -> Waker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }
    fn dummy_raw_waker() -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable =
        RawWakerVTable::new(clone, no_op, no_op, no_op);
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}

fn block_on_manual<F: Future>(mut fut: F) -> F::Output {
    let waker = dummy_waker();
    let mut cx = Context::from_waker(&waker);
    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => continue, // 실전 executor는 여기서 대기
        }
    }
}

fn main() {
    let a = block_on_manual(ReadyNow(10));
    let b = block_on_manual(YieldOnce {
        yielded: false,
        value: 20,
    });
    println!("a={a}, b={b}");

    // async 문법과 연결
    let c = block_on_manual(async {
        let x = ReadyNow(1).await;
        let y = YieldOnce {
            yielded: false,
            value: 2,
        }
        .await;
        x + y
    });
    println!("c={c}");
}
```

포인트:

1. **`async` 블록도 `Future`** — 같은 `block_on_manual`로 구동.
2. **`Poll::Pending` + `wake`** — 재스케줄의 최소 모형.
3. **`Pin::new_unchecked`** — 로컬 변수가 이동하지 않는다는 전제 (실습용). 프로덕션은 `pin!` / `Box::pin`.

#### 확장 과제

- [ ] `map` 조합기: `Map<Fut, F>`가 내부 퓨처 `Ready`면 클로저 적용
- [ ] Tokio 없이 `async fn` 두 개를 순서로 await하는 블록을 `block_on_manual`로 실행
- [ ] `Box::pin(async { ... })`으로 힙에 고정한 뒤 poll
- [ ] (개념) “왜 `MutexGuard`를 await 걸쳐 들고 있으면 위험할 수 있나” 정리 — 홀드 중 yield

---

### 이 단계 한 장 요약

```
async fn / async {}  →  impl Future (호출 ≠ 실행)
.await                →  상태 머신 중단점
async move           →  캡처 소유권 (spawn/'static)
Future::poll         →  Pending | Ready + Waker
Pin                  →  self-ref 상태 머신 이동 방지
실습                 →  수동 poll 루프로 executor 감각
```

---

### 로드맵을 마친 뒤

- [2~7단계](#목차)가 문법 핵심 — 막히면 그 구간을 다시.
- 실전은 axum 핸들러에서 `Result` + `?`, `State(Arc<...>)`, `async`/`await`, 모듈 분리로 대부분 연결됩니다.
- 더 깊게: Jon Gjengset *Crust of Rust* (lifetime, trait objects, smart pointers, async).

이전: [10단계 Unsafe](#단계-10-unsafe-ffi-선택) · 목록: [readme.md](#목차)