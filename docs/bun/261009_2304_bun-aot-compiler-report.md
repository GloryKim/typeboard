# [종합 보고서] Bun AOT 컴파일러 실험 (`--compile --aot`) — 타입 기반 네이티브 선행 컴파일 분석

> **대상:** Bun 1.4 기반 실험 빌드 (`bun build --compile --aot`, 미출시)  
> **공개 시점:** 2026년 9월 말 ~ 10월 초 (Jarred Sumner 트윗, @bunjavascript 4부작 시리즈)  
> **핵심 키워드:** `AOT vs JIT`, `--compile / --bytecode / --aot`, `sound types`, `typecheck 바이트코드 인트린식`, `B3 백엔드`, `as 런타임 형변환`, `프로토타입 변경 차단`, `Claude Code 시작 -24%`  
> **원문:** [GeekNews — Bun, TypeScript를 네이티브 코드로 선행 컴파일하는 AOT 컴파일러 실험 공개](https://news.hada.io/topic?id=35124)

---

## 문서 안내

본 보고서는 Bun이 공개한 **AOT(Ahead-of-Time) 컴파일러 실험**을 기존 Bun 실행 모델과 비교해 정리한 문서입니다. 기존 `--compile`·`--bytecode`와 무엇이 다른지, 내부 구조가 어떻게 되는지, 왜 `as`·`any`·`delete` 같은 TypeScript 동작이 바뀌는지, 초기 성능 수치와 한계는 무엇인지를 한 파일에서 파악할 수 있도록 구성했습니다.

> **주의:** `--aot`는 아직 출시되지 않은 실험 기능입니다. 본문 예제의 AOT 동작은 공개된 요약(GeekNews)과 Jarred Sumner의 설명을 근거로 한 **예상 동작**이며, 실제로 실행해 검증한 결과가 아닙니다.

---

## 목차

1. [개요 및 핵심 요약](#1-개요-및-핵심-요약)
2. [배경: 현재 Bun의 실행 모델](#2-배경-현재-bun의-실행-모델)
3. [`--compile` / `--bytecode` / `--aot` 비교](#3---compile----bytecode----aot-비교)
4. [타임라인: 왜 지금 AOT인가](#4-타임라인-왜-지금-aot인가)
5. [AOT 내부 구조](#5-aot-내부-구조)
6. [동작 변경 심층 분석 — 4부작 시리즈 해설](#6-동작-변경-심층-분석--4부작-시리즈-해설)
7. [초기 성능 수치와 해석](#7-초기-성능-수치와-해석)
8. [유사 프로젝트 비교](#8-유사-프로젝트-비교)
9. [커뮤니티 반응](#9-커뮤니티-반응)
10. [예제: AOT 친화 코드 vs 깨질 수 있는 코드](#10-예제-aot-친화-코드-vs-깨질-수-있는-코드)
11. [실무 권고사항](#11-실무-권고사항)
12. [사실 검토 · 출처](#12-사실-검토--출처)

---

## 1. 개요 및 핵심 요약

Bun이 **TypeScript의 타입 정보를 "믿을 수 있게(sound)" 만든 다음, 그 타입을 근거로 코드를 실행 전에 기계어로 미리 컴파일**하는 실험을 공개했습니다. 타입을 믿을 수 있게 만들기 위해 지금까지 TypeScript에서 "타입을 속일 수 있던 구멍"(`as`, `any`, `delete`, 구조적 타이핑, 프로토타입 변경)을 막았고, 그 결과 일부 동작이 기존 JS/TS와 달라집니다.

| 항목 | 내용 |
| :--- | :--- |
| **무엇** | `bun build --compile --aot` — TS/JS를 빌드 시점에 네이티브 코드로 컴파일 |
| **어떻게** | Bun 파서·타입 체커 → 바이트코드에 타입 꼬리표 → JSC의 B3 백엔드로 기계어 생성 |
| **대가** | `as`가 실제 형변환, `any` 우회·`delete`·몽키패칭 차단 등 **의미 변경** |
| **초기 효과** | Claude Code CLI 기준 시작 **-24%**, 메모리 **-48%**, CPU **-46%**, 실행 파일 약 **2배** |
| **상태** | 실험 단계, 출시 일정 없음, 제3자 재현 없음 |

---

## 2. 배경: 현재 Bun의 실행 모델

### 2.1 JIT 파이프라인

```text
app.ts
  │  ① Bun 트랜스파일러: 타입을 "지우기만" 함 (검사 안 함)
  ▼
app.js
  │  ② JavaScriptCore(JSC): 파싱 → 바이트코드
  ▼
바이트코드
  │  ③ 인터프리터로 실행하며 프로파일 수집 ("이 변수엔 늘 숫자가 온다")
  ▼
  │  ④ 자주 도는(hot) 코드만 JIT 컴파일러가 기계어로 변환
  ▼
기계어 (실행 중에 생성)
```

### 2.2 타입은 실행에 쓰이지 않는다

Bun은 TypeScript를 **실행하지 않습니다**. 타입을 지우고 JS를 실행합니다. 그래서 아래 코드는 현재 Bun에서 그대로 실행됩니다. ([flaviocopes](https://flaviocopes.com/what-happens-when-bun-runs-typescript/))

```typescript
const port: number = "3000"; // tsc는 TS2322 에러, bun run은 "3000" 출력
console.log(port);
```

타입 검사는 별도 단계입니다. Bun 1.4부터는 `bun check` 또는 `bun build --check`로 실행 전에 검사할 수 있습니다. ([Bun Docs — Type checking](https://bun.com/docs/runtime/check.md))

### 2.3 JIT의 비용

JIT는 "실행해 보면서 추측"합니다. 추측이 틀리면(숫자만 오던 곳에 문자열이 들어오면) 최적화된 코드를 버리고 느린 경로로 돌아갑니다(**deopt**). 이 **추측·감시·되돌리기** 비용이 메모리, CPU, 시작 시간(웜업)을 잡아먹습니다. AOT는 이 비용을 빌드 시점으로 옮기려는 시도입니다.

---

## 3. `--compile` / `--bytecode` / `--aot` 비교

| 옵션 | 하는 일 | 기계어로 바뀌나 | 주 효과 |
| :--- | :--- | :---: | :--- |
| `bun build --compile` | JS 번들 + Bun 런타임 전체를 **실행 파일 하나로 포장** | ❌ | 배포 단순화 |
| `--compile --bytecode` | 파싱 결과(바이트코드)를 빌드 때 미리 생성 | ❌ | 시작 시 파싱 비용 제거 |
| `--compile --aot` (실험) | 코드를 **빌드 때 기계어로** 컴파일 | ✅ | 웜업·프로파일링 제거 |

- `--compile`은 "자동 압축 해제 ZIP처럼 런타임과 코드를 한 봉투에 넣은 것"이지 AOT가 아닙니다. Claude Code 네이티브 빌드가 100MB를 넘는 이유도 대부분이 JSC 런타임이기 때문입니다. ([frr.dev](https://www.frr.dev/posts/claude-code-native-build-bun/))
- `--bytecode`는 시작 시 파싱 비용만 줄입니다. `tsc` 시작이 2배 빨라진 사례가 있습니다. 바이트코드는 Bun 버전 간 호환되지 않습니다. ([Bun Docs — Bytecode Caching](https://bun.com/docs/bundler/bytecode))
- 2024년 "진짜 기계어 AOT를 해 달라"는 이슈에 Bun 팀은 **"AOT는 하지 않는다, JSC의 JIT로 충분히 빠르다"** 며 닫았습니다. ([oven-sh/bun#9375](https://github.com/oven-sh/bun/issues/9375)) 이번 실험은 그 입장을 뒤집은 것입니다.

---

## 4. 타임라인: 왜 지금 AOT인가

| 시점 | 사건 |
| :--- | :--- |
| 2025-12 | Bun이 Anthropic에 인수됨 ([Bun Blog — Rewriting Bun in Rust](https://bun.com/blog/bun-in-rust)) |
| 2026-07-08 | Zig → Rust 재작성 발표 (Zig 약 53만 줄 기계적 포팅) |
| 2026-08-20 | **Bun 1.4** 출시 — Rust로 작성된 첫 버전, `bun check` 타입 체커 추가 ([Bun 1.4](https://bun.com/blog/bun-v1.4)) |
| 2026-09-24 | Jarred Sumner: "JS는 서버용으로 설계되지 않았다. 고칠 곳은 엔진이다 — Sound types, AOT 컴파일, 객체를 공유하는 스레드" ([X 미러](https://twiscan.com/en/x/jarredsumner/2103092824363401586)) |
| 2026-09-28 | Claude Code CLI 대상 AOT 프로토타입 벤치마크 공개 |
| 2026-09-29 | AOT 내부 구조 설명 ([X 미러](https://twiscan.com/en/x/jarredsumner/2104754521121653039)) |
| 2026-10-04 | "첫 단계는 타입이다. TypeScript 호환 타입 체커를 Bun에 넣고 있다" ([X 미러](https://twiscan.com/en/x/jarredsumner/2106734042095972482)) |
| 2026-10 | @bunjavascript 4부작 비교 시리즈 → GeekNews 기사 |

동기는 9월 24일 트윗에 압축되어 있습니다. **"서버에서 Rust/Go와 경쟁하려면 JS 엔진 자체가 타입을 믿고 미리 컴파일할 수 있어야 한다."** Bun을 소유한 Anthropic의 대표 제품(Claude Code)이 TypeScript + Bun으로 동작하므로, 그 성능이 첫 실험 대상이 됐습니다.

---

## 5. AOT 내부 구조

```text
app.ts
  │  ① Bun의 번들러·파서가 TypeScript를 파싱 (이제 타입을 지우지 않음)
  ▼
  │  ② "typecheck 바이트코드 인트린식"으로 타입 정보를 JSC 바이트코드에 꼬리표처럼 부착
  ▼
타입 꼬리표가 붙은 바이트코드
  │  ③ B3(JSC 최상위 JIT가 쓰는 백엔드)로 빌드 시점에 기계어 생성
  ▼
기계어가 들어간 실행 파일 (실행 시 JIT 끔)
```

| 설계 포인트 | 설명 |
| :--- | :--- |
| **컴파일러 재사용** | 새 컴파일러를 만든 것이 아니라 JSC에 이미 있는 최적화 백엔드(B3)를 "실행 중"이 아니라 "빌드 때" 실행 |
| **JSC 수정 최소화** | 변경 범위를 작게 유지해 업스트림 WebKit 동기화가 어렵지 않도록 함 |
| **타입 공급원** | Bun 1.4의 `bun check` — TypeScript 7 `tsc`와 같은 에러를 내고 모든 CPU 코어 사용 |

---

## 6. 동작 변경 심층 분석 — 4부작 시리즈 해설

### 6.0 공통 원리: 왜 동작을 바꿔야 하나

컴파일러가 타입을 근거로 기계어를 만들려면 **타입이 거짓말을 하면 안 됩니다**. `x: number`를 믿고 "숫자 덧셈 명령어 하나"로 컴파일했는데 실행 중 문자열이 들어오면 프로그램이 망가집니다. 지금의 TypeScript는 의도적으로 **unsound**(타입이 실제 값을 보장하지 않음)합니다. 4개 시리즈는 모두 **타입을 sound하게 만들기 위해 탈출구를 막는 규칙**입니다.

### 6.1 타입을 활용한 컴파일과 실행 시점 검증

| 예시 | 기존 Bun / TS | AOT |
| :--- | :--- | :--- |
| `const port: number = "8080"` | 실행됨 (타입은 지워짐) | **빌드 실패**, 실행 파일 미생성 |
| `JSON.parse()` 결과를 `interface User`로 받는데 `age`에 문자열 | 통과 | **런타임 `TypeError`** |
| `(raw as number) + 1`, `raw = "5"` | `"51"` (문자열 연결) | **`6`** (`as`가 실제 형변환) |
| `x!` (non-null 단언), `x`가 `null` | 통과, 나중에 터짐 | **그 자리에서 에러** |

- 빌드 때 알 수 있는 것은 빌드 때 막고, `JSON.parse`처럼 외부에서 들어오는 값은 **경계에서 런타임 검사**합니다.
- 가장 논쟁적인 부분은 `as`입니다. 기존 TS의 `as`는 "컴파일러야, 내 말 믿어"라는 **주석**이라 JS로 바뀌면 사라집니다. AOT에서는 **실제 변환 연산**이 됩니다. 같은 코드가 실행 환경에 따라 `"51"`과 `6`으로 갈립니다.

### 6.2 타입 안정성을 유지하는 객체 모델

- `any`를 거쳐 들어온 객체도 타입 있는 함수에 전달되는 순간 필드 타입을 검사합니다.
- 이미 타입이 정해진 객체 필드를 `any`로 우회해 다른 타입 값으로 바꾸는 시도를 차단합니다.
- 필수 필드 `delete` 금지, `number[]`에 문자열이 섞이면 에러입니다.
- **클래스는 구조가 아니라 실제 인스턴스 여부로 검사**합니다. 모양이 같은 일반 객체를 클래스 대신 넘기면 거부합니다.

**이유:** 컴파일러가 "`User`의 `age`는 항상 고정 오프셋에 있는 숫자"라고 가정하고 기계어를 만들기 때문입니다. 필드가 사라지거나 타입이 바뀌면 가정이 깨집니다. 모양만 비슷한 객체는 메모리 배치가 다를 수 있습니다.

### 6.3 프로토타입 변경 제한과 정적 컴파일

- `Object.prototype` 속성 주입, `Array.prototype.map` 교체 같은 **몽키패칭 차단**
- 사용자 정의 클래스의 프로토타입 메서드 교체도 차단
- 단, 일반 객체에 동적 속성 추가나 `Math.tau` 같은 **새 속성 정의는 허용**

**이유:** `arr.map(...)`을 내장 `map` 직접 호출(또는 인라인)로 컴파일하려면 실행 중 바꿔치기가 없다는 보장이 필요합니다. JIT는 이를 감시하다가 바뀌면 deopt하지만, AOT는 미리 컴파일하므로 **바꿀 수 없게** 막는 쪽을 택했습니다.

### 6.4 네이티브 코드 생성과 JS 호환성

- AOT 함수에 `toString()`을 호출하면 소스 대신 `[native code]`가 표시됩니다.
- `Object.keys()`, 동적 속성 추가, 올바른 타입의 JSON 파싱 등은 그대로 동작합니다.
- **완전한 하위 호환은 아닙니다.** 6.1~6.3 때문에 기존 코드 일부는 에러가 나거나 결과가 달라집니다.

---

## 7. 초기 성능 수치와 해석

### 7.1 Claude Code CLI 측정값

macOS arm64, MacBook Pro 1대, 8회 중앙값, AOT는 JIT를 끈 상태입니다. ([AI Coding Weekly](https://aicodingweekly.news/stories/bun-aot-claude-code-24-percent-faster-startup-jarred-sumner), [X 미러](https://nitter.cf/jarredsumner/status/2106719577275302344))

| 항목 | Bun AOT | Bun JIT | 차이 |
| :--- | ---: | ---: | ---: |
| 시작 → 입력 가능 | 165 ms | 216 ms | **-24%** |
| 실행 + 5턴 전체 시간 | 322 ms | 529 ms | -39% |
| CPU 시간 | 395 ms | 738 ms | **-46%** |
| 입력 가능 시점 메모리 | 56 MB | 108 MB | **-48%** |
| 5턴 후 메모리 | 82 MB | 159 MB | -48% |
| 실행 파일 크기 | 430 MB | 229 MB | **+88%** |

### 7.2 해석

| 구분 | 내용 |
| :--- | :--- |
| **얻는 것** | 시작 시간, 메모리, CPU — JIT 웜업과 프로파일링이 없어지기 때문 |
| **잃는 것** | 실행 파일 크기 — 기계어가 바이트코드보다 훨씬 큼 |
| **남은 여지** | Jarred 본인 평가: "타입 활용은 아직 반쯤 구운 상태, 2~10배를 남겨 두고 있다" |
| **다른 워크로드** | TypeScript 6 컴파일러는 이미 AOT가 JIT보다 빠르다고 언급 |
| **불리한 경우** | 오래 떠 있으며 웜업이 끝난 HTTP 서버는 JIT가 여전히 유리할 수 있음 |
| **불일치** | 후속 트윗의 "Claude Code 430MB → 304MB"는 위 수치와 어떻게 맞는지 설명되지 않음 |

> 모든 수치는 **한 사람이 한 앱으로** 측정한 값입니다. 제3자 재현과 출시 일정은 없습니다.

---

## 8. 유사 프로젝트 비교

| 프로젝트 | 방식 | 일반 TS 호환 | 특징 |
| :--- | :--- | :---: | :--- |
| **Bun AOT** | JSC 바이트코드 + 타입 꼬리표 → B3 | 대부분 (일부 의미 변경) | 기존 Bun/npm 생태계 위에서 동작 |
| [Static Hermes](https://github.com/facebook/hermes/discussions/1137) (Meta) | 타입 정보 → LLVM → 기계어/Wasm | 대부분 | 동적 기능은 내장 인터프리터. "선택적 sound typing" 철학이 가장 유사 |
| [Porffor](https://canadahonk-porffor.mintlify.app/introduction) | JS → Wasm → C → 기계어 | 제한적 | 처음부터 새로 만든 실험적 컴파일러 |
| [AssemblyScript](https://www.heidemann.dev/posts/the-future-of-typescript-is-wasm) | TS 문법의 별도 언어 → Wasm | ❌ | npm 생태계 대부분 사용 불가 |
| [torajs](https://torajs.com/) | TS → 네이티브 바이너리 | 부분 | "Bun과 같은 출력"이 목표인 커뮤니티 프로젝트 |

Bun AOT의 차별점은 **이미 널리 쓰이는 런타임(Bun 1.4)과 엔진(JSC)에 붙인다**는 점입니다.

---

## 9. 커뮤니티 반응

| 입장 | 요지 |
| :--- | :--- |
| **우려** | "`as` 의미 변경은 너무 급진적, CoffeeScript처럼 독자 문법은 실패한다" — 같은 TS가 tsc/Node/Bun JIT/Bun AOT에서 다르게 동작할 위험 |
| **반론** | "TS를 제대로 썼다면 문제없을 최적화" — `as`를 거짓말 용도로 쓰지 않은 코드는 결과가 같음 |
| **관점** | "Claude CLI·Desktop이 전부 TS라 그 성능을 끌어올리려는 것, Bun이 점점 Claude 에뮬레이터 같다" |
| **스레드 반대** | José Valim: "sound types와 AOT는 찬성, 공유 가변 객체 스레드는 악몽" ([hraness](https://hraness.com/reading/threads-with-shared-mutable-objects-are-a-nightmare-on-javascripts-event-loop)) |
| **회의** | "JS에 타입·스레드·AOT를 넣으면 Java와 무엇이 다른가" |

---

## 10. 예제: AOT 친화 코드 vs 깨질 수 있는 코드

### 10.1 AOT에서도 동작이 같을 코드

타입을 속이는 코드가 없는 `Bun.serve` 예제입니다. AOT로 컴파일해도 동작은 같을 것으로 예상됩니다. 다만 오래 떠 있는 서버라 처리량보다는 **시작 시간과 메모리** 이득이 주로 보일 것입니다.

```typescript
// server.ts — 타입을 속이는 코드 없음
const server = Bun.serve({
  port: 3002,
  fetch(req) {
    console.log("hello");
    return new Response("hello");
  },
});

console.log(`${server.url}`);
```

### 10.2 AOT에서 결과가 달라지거나 거부될 코드

```typescript
// aot-pitfalls.ts — 6장 규칙에 걸리는 패턴 모음 (예상 동작)

// 1) 빌드 실패: 정적 타입 오류
const port: number = "8080";              // 기존: 실행 / AOT: 빌드 거부

// 2) 결과 변경: as가 실제 형변환
const raw: unknown = "5";
console.log((raw as number) + 1);         // 기존: "51" / AOT: 6

// 3) 런타임 에러: 경계 검증
interface User { name: string; age: number }
const user = JSON.parse('{"name":"a","age":"20"}') as User; // AOT: TypeError

// 4) 런타임 에러: 필수 필드 delete
const u: User = { name: "a", age: 1 };
delete (u as any).age;                    // AOT: 차단

// 5) 차단: 내장 프로토타입 몽키패칭
Array.prototype.map = function () { return []; }; // AOT: 차단

// 6) 거부: 구조만 같은 객체를 클래스 대신 전달
class Point { constructor(public x: number, public y: number) {} }
function len(p: Point) { return Math.hypot(p.x, p.y); }
len({ x: 3, y: 4 } as Point);             // 기존: 5 / AOT: 거부
```

### 10.3 지금 할 수 있는 대비

```bash
# 실행 전에 타입 검사 (Bun 1.4+)
bun check
bun build --check ./src/index.ts --outdir ./dist
```

---

## 11. 실무 권고사항

- **`as`를 타입 단언이 아닌 "거짓말" 용도로 쓰는 코드 정리:** 외부 입력은 `as` 대신 런타임 검증(스키마 검증)으로 좁히는 습관을 들이면 AOT 전환 시 결과 차이가 사라집니다.
- **`any` 경유 우회, 필수 필드 `delete`, 프로토타입 몽키패칭 제거:** 기존 런타임에서도 유지보수 위험이 큰 패턴입니다.
- **`bun check`를 CI에 추가:** AOT는 정적 타입 오류를 빌드 실패로 처리하므로, 지금부터 타입 오류 0을 유지하는 것이 가장 직접적인 대비입니다.
- **워크로드별 기대치 구분:** CLI처럼 짧게 뜨는 프로그램은 시작 시간·메모리 이득이 크고, 웜업이 끝난 장기 실행 서버는 이득이 작거나 없을 수 있습니다.
- **실행 파일 크기 예산 확인:** 초기 수치 기준 약 2배이므로 배포 크기 제약이 있다면 함께 고려해야 합니다.
- **출시 전까지는 관찰 단계:** 실험 기능이며 의미 변경이 포함되어 있으므로 프로덕션 채택은 정식 릴리스와 문서화 이후로 미루는 것이 안전합니다.

---

## 12. 사실 검토 · 출처

### 12.1 검토 메모

| 항목 | 상태 |
| :--- | :--- |
| 4부작 시리즈의 예시(`as`, `delete`, 프로토타입 등) | GeekNews 요약 기준. @bunjavascript 원본 스레드는 직접 확인하지 못함 |
| 내부 구조(파서 → typecheck 인트린식 → B3) | Jarred Sumner 트윗 기준 |
| 성능 수치 | Jarred Sumner 트윗 및 2차 보도 기준, 제3자 재현 없음 |
| 실행 파일 크기 430MB vs 304MB | 두 측정의 관계가 공개 설명되지 않음 |
| 10장 예제의 AOT 동작 | 공개 설명 기반 예상 동작, 미검증 |

### 12.2 출처

- [GeekNews — Bun, TypeScript를 네이티브 코드로 선행 컴파일하는 AOT 컴파일러 실험 공개](https://news.hada.io/topic?id=35124)
- [Bun Blog — Bun 1.4](https://bun.com/blog/bun-v1.4) · [Rewriting Bun in Rust](https://bun.com/blog/bun-in-rust)
- [Bun Docs — Single-file executable](https://bun.com/docs/bundler/executables) · [Bytecode Caching](https://bun.com/docs/bundler/bytecode) · [Type checking](https://bun.com/docs/runtime/check.md)
- [oven-sh/bun#9375 — native binary AOT 요청 (2024, closed)](https://github.com/oven-sh/bun/issues/9375)
- Jarred Sumner 트윗: [서버 JS 문제 제기](https://twiscan.com/en/x/jarredsumner/2103092824363401586) · [AOT 구조](https://twiscan.com/en/x/jarredsumner/2104754521121653039) · [타입 체커](https://twiscan.com/en/x/jarredsumner/2106734042095972482) · [벤치마크](https://nitter.cf/jarredsumner/status/2106719577275302344)
- [AI Coding Weekly — Bun AoT starts Claude Code 24% faster](https://aicodingweekly.news/stories/bun-aot-claude-code-24-percent-faster-startup-jarred-sumner)
- [frr.dev — Claude Code Native Build](https://www.frr.dev/posts/claude-code-native-build-bun/) · [flaviocopes — What happens when Bun runs a TypeScript file](https://flaviocopes.com/what-happens-when-bun-runs-typescript/)
- [Static Hermes](https://github.com/facebook/hermes/discussions/1137) · [Porffor](https://canadahonk-porffor.mintlify.app/introduction) · [AssemblyScript 논의](https://www.heidemann.dev/posts/the-future-of-typescript-is-wasm) · [torajs](https://torajs.com/)
- [hraness — José Valim on shared mutable threads](https://hraness.com/reading/threads-with-shared-mutable-objects-are-a-nightmare-on-javascripts-event-loop)
