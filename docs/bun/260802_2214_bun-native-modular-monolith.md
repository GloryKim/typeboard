# 폐쇄망에서 Bun 내장만으로 모듈러 모놀리스 — 외부 디펜던시 0, MSA 급 경계

> **문서 성격** — 설계·의사결정 보고서 (아키텍처 + 구현 레퍼런스)
> **대상 독자** — 폐쇄망(air-gapped)에 TS 백엔드를 올리되, MSA의 운영 복잡도는 피하고 싶은 엔지니어 / 아키텍트
> **핵심 제약** — ① 인터넷 없음(레지스트리 접근 불가) ② Bun 바이너리는 이미 설치되어 있음 ③ **외부 npm 의존성 0**
> **핵심 주장** — **하나의 프로세스** 안에서도 MSA에 준하는 **모듈 경계·도메인 이벤트·횡단관심사**를 세울 수 있고, **로컬 ACID 트랜잭션**이라는 MSA가 갖기 힘든 강점까지 덤으로 얻는다. 그리고 경계를 계약으로만 두면 **나중에 MSA로 쪼개는 것도 싸다.**
> **짝 문서** — MSA 버전은 `bun-native-msa.md`. 이 문서는 그 MSA 문서의 `shared/`(§10 파이프라인·DI·검증·예외필터)를 그대로 재사용한다.

---

## 목차

- [0. 요약 (Executive Summary)](#0-요약-executive-summary)
- [1. 모듈러 모놀리스란 무엇이고 언제 이기는가](#1-모듈러-모놀리스란-무엇이고-언제-이기는가)
- [2. MSA vs 모듈러 모놀리스 — 기능 대조 (밀리지 않음의 증명)](#2-msa-vs-모듈러-모놀리스--기능-대조-밀리지-않음의-증명)
- [3. 3대 원칙 — 경계 · 계약 · 이음새](#3-3대-원칙--경계--계약--이음새)
- [4. 전체 아키텍처 (단일 프로세스, 모듈 조립)](#4-전체-아키텍처-단일-프로세스-모듈-조립)
- [5. 폴더 구조 · 모듈 해부](#5-폴더-구조--모듈-해부)
- [6. 모듈 계약 (Public API vs Internal)](#6-모듈-계약-public-api-vs-internal)
- [7. 모듈 등록 · 조립 루트 (경량 DI)](#7-모듈-등록--조립-루트-경량-di)
- [8. 모듈 간 통신 ① — 동기 직접 호출 (계약 인터페이스)](#8-모듈-간-통신--동기-직접-호출-계약-인터페이스)
- [9. 모듈 간 통신 ② — 인프로세스 이벤트 버스 (브로커 대체)](#9-모듈-간-통신--인프로세스-이벤트-버스-브로커-대체)
- [10. 경계 강제 — 규약을 코드로 지키는 법](#10-경계-강제--규약을-코드로-지키는-법)
- [11. 라우팅 통합 — 하나의 Bun.serve에 모듈 routes 병합](#11-라우팅-통합--하나의-bunserve에-모듈-routes-병합)
- [12. 횡단 관심사 (DI·검증·Guard·Interceptor·예외필터)](#12-횡단-관심사-di검증guardinterceptor예외필터)
- [13. ★ 트랜잭션 — 모놀리스의 결정적 강점](#13--트랜잭션--모놀리스의-결정적-강점)
- [14. 데이터 · DB — 단일 DB에서의 소유권 규율](#14-데이터--db--단일-db에서의-소유권-규율)
- [15. 실시간 (WebSocket) 통합](#15-실시간-websocket-통합)
- [16. 설정 · 환경 변수](#16-설정--환경-변수)
- [17. 관측 · 헬스 · 로깅](#17-관측--헬스--로깅)
- [18. 테스트 (모듈 단위 + 통합)](#18-테스트-모듈-단위--통합)
- [19. 배포 — 단일 바이너리의 극단적 단순함](#19-배포--단일-바이너리의-극단적-단순함)
- [20. ★ 나중에 MSA로 쪼개기 (이음새 · Strangler)](#20--나중에-msa로-쪼개기-이음새--strangler)
- [21. 안티패턴 · 함정](#21-안티패턴--함정)
- [22. 최소 동작 예시 (User + Order 한 프로세스)](#22-최소-동작-예시-user--order-한-프로세스)
- [23. 리스크 · 완화](#23-리스크--완화)
- [24. 결론 · 선택 가이드](#24-결론--선택-가이드)
- [부록 A — MSA ↔ 모듈러 모놀리스 기능 대조표](#부록-a--msa--모듈러-모놀리스-기능-대조표)
- [부록 B — 모듈 추가 체크리스트](#부록-b--모듈-추가-체크리스트)
- [부록 C — "모놀리스가 진흙탕이 되는" 징후와 처방](#부록-c--모놀리스가-진흙탕이-되는-징후와-처방)

---

## 0. 요약 (Executive Summary)

**결론: 대부분의 폐쇄망 프로젝트는 여기서 시작해야 한다.**

- MSA의 실질 가치는 "여러 프로세스"가 아니라 **강한 모듈 경계**다. 그 경계는 **한 프로세스 안에서도** 계약(타입 인터페이스)·이벤트 버스·의존 규칙으로 세울 수 있다.
- 모듈러 모놀리스는 MSA 대비 **네트워크 지연 0, 배포 단위 1개, 디버깅 스택 하나, 로컬 트랜잭션 가능**이라는 강점을 갖는다. 특히 **로컬 ACID 트랜잭션**은 MSA가 사가(Saga)·아웃박스로 힘겹게 흉내 내야 하는 것을 **`db.transaction()` 한 줄**로 해결한다(§13).
- "MSA 못지 않는 기능"의 핵심 3종은 전부 무의존으로 구현된다:
  1. **모듈 경계** = 공개 계약(`contract.ts`) + 내부 은닉(§6)
  2. **비동기 팬아웃** = 인프로세스 이벤트 버스(Kafka/NATS 대체, §9)
  3. **횡단 관심사** = DI·검증·Guard·Interceptor·예외필터(MSA 문서 §10 재사용, §12)
- 그리고 경계를 **계약으로만** 두면, 나중에 특정 모듈을 별도 프로세스로 뽑을 때 **"인프로세스 호출 → `fetch` 호출"로 어댑터만 교체**하면 된다(§20). 즉 이 문서는 **"MSA로 가는 안전한 1단계"**이기도 하다.

| 질문 | 모듈러 모놀리스의 답 |
|---|---|
| 프로세스 수 | **1** (`Bun.serve` 하나) |
| 모듈 경계 | 계약 인터페이스 + 폴더 은닉 + 의존 규칙(§6, §10) |
| 모듈 간 동기 호출 | **함수 호출** (계약 인터페이스 경유, 네트워크 0) |
| 모듈 간 비동기 | **인프로세스 이벤트 버스**(§9) |
| 트랜잭션 | **로컬 ACID** — 여러 모듈을 한 트랜잭션에 (§13) ★ |
| DB | 단일 DB + **모듈별 테이블 소유권**(§14) |
| 배포 | **단일 바이너리/프로세스** (§19) — 폐쇄망 반입 최소 |
| MSA 전환 | **계약 뒤 어댑터만 교체**(§20) |
| 언제 이게 정답 | 팀·트래픽 중소, 강한 정합성 필요, 운영 인력 적음 |

**한 줄 권고:**
"프로세스를 나누지 말고 **모듈을 나눠라.** 경계는 `Bun.serve` 하나 안에서 **계약 + 이벤트 버스**로 세우고, 트랜잭션이라는 공짜 강점을 누려라. MSA는 경계가 검증되고 **정말 배포·스케일이 갈릴 때** 이음새(§20)를 따라 쪼개라."

---

## 1. 모듈러 모놀리스란 무엇이고 언제 이기는가

### 1.1 세 가지를 구분하자

```text
① 빅볼오브머드 모놀리스   : 한 프로세스 + 경계 없음  → 스파게티 (나쁨)
② 모듈러 모놀리스         : 한 프로세스 + 강한 모듈 경계 → 이 문서 (좋음)
③ 마이크로서비스(MSA)     : 여러 프로세스 + 네트워크 경계 → 짝 문서
```

②는 ①이 아니다. **한 프로세스라는 이유로 경계를 포기하지 않는다.** ③과의 유일한 본질적 차이는 "경계가 **네트워크**냐, **모듈 계약**이냐"뿐이다.

### 1.2 모듈러 모놀리스가 이기는 상황

| 조건 | 왜 모듈러 모놀리스인가 |
|---|---|
| 팀·트래픽이 중소 규모 | MSA 운영 비용(배포 파이프라인 N개, 분산 추적, 서비스 디스커버리)이 이득을 초과 |
| **강한 데이터 정합성 필요** | 여러 도메인을 **한 트랜잭션**으로 묶을 수 있음(§13). MSA는 사가 지옥 |
| 폐쇄망 · 운영 인력 적음 | 반입·기동·디버깅이 **하나의 아티팩트**로 끝(§19) |
| 도메인 경계가 아직 불확실 | 잘못 그은 경계를 **함수 리팩터로 고침**(MSA는 API 재계약 지옥) |
| 낮은 지연이 중요 | 모듈 간 호출이 **네트워크 왕복 0**, 나노초 함수 호출 |

### 1.3 MSA로 가야 하는 신호 (정직하게)

- 특정 모듈만 **독립 스케일**(CPU/메모리)이 절실하다
- 팀이 **조직적으로 갈려** 배포 주기가 충돌한다
- 한 모듈의 장애가 다른 모듈까지 죽이면 **절대 안 된다**(프로세스 격리 필요)
- 언어/런타임을 모듈별로 다르게 가야 한다

> 이 신호가 오기 전까지, **모듈러 모놀리스가 거의 항상 총유지비가 낮다.** 그리고 §20의 이음새를 지켜 두면 신호가 왔을 때 싸게 전환한다.

---

## 2. MSA vs 모듈러 모놀리스 — 기능 대조 (밀리지 않음의 증명)

"모듈러 모놀리스는 기능이 떨어진다"는 오해를 정면으로 반박한다.

| 기능 | MSA | 모듈러 모놀리스 (이 문서) | 판정 |
|---|---|---|---|
| 모듈 경계 | 네트워크 | 계약 인터페이스 + 은닉(§6) | **동등** |
| 독립 개발/소유권 | 서비스별 | 모듈별 폴더/계약(§5,6) | **동등** |
| 동기 통신 | HTTP `fetch` | 함수 호출(계약 경유, §8) | **모놀리스 우세**(지연 0) |
| 비동기/이벤트 팬아웃 | Kafka/NATS | 인프로세스 이벤트 버스(§9) | **동등** (규모 커지면 MSA) |
| 횡단 관심사(인증/검증/로깅) | 서비스별·게이트웨이 | DI·Guard·Interceptor(§12) | **동등** |
| **분산 트랜잭션** | 사가/아웃박스(복잡) | **로컬 ACID 한 줄**(§13) | **모놀리스 압승** |
| 관측/추적 | 분산 트레이싱 필수 | 단일 스택 로그(§17) | **모놀리스 단순** |
| 배포 | 서비스별 파이프라인 | 단일 아티팩트(§19) | **모놀리스 단순** |
| 독립 스케일 | 서비스별 | 프로세스 통째만 | **MSA 우세** |
| 장애 격리 | 프로세스 격리 | 모듈 격리(제한적)(§9.4) | **MSA 우세** |
| 폐쇄망 반입 | 다수 아티팩트 | **하나** | **모놀리스 우세** |
| 잘못된 경계 수정 | API 재계약 | 함수 리팩터 | **모놀리스 압승** |

**결론:** 순수 "독립 스케일"과 "프로세스 장애 격리"를 제외하면, 모듈러 모놀리스는 MSA에 **밀리지 않으며**, 트랜잭션·운영 단순성·반입·리팩터 유연성에서는 **더 낫다.** 그 두 예외가 실제로 아플 때만 MSA로 간다.

---

## 3. 3대 원칙 — 경계 · 계약 · 이음새

이 문서 전체를 관통하는 세 규칙.

### 3.1 원칙 ① — 모듈은 "은닉된 도메인"이다

```text
한 모듈 = 하나의 바운디드 컨텍스트
      = 자기 테이블을 소유
      = 자기 서비스/스토어는 밖에서 안 보임 (internal)
      = 밖에는 "계약(contract)"만 노출
```

### 3.2 원칙 ② — 모듈 간 결합은 "계약"으로만

```text
✅ order 모듈 → users 모듈의 contract 인터페이스 호출
❌ order 모듈 → users 모듈의 store/service 파일 직접 import
```

계약은 **인터페이스(타입)**다. 구현이 아니다. 이 규칙 하나가 §20의 MSA 전환을 가능케 한다.

### 3.3 원칙 ③ — 모든 경계는 "이음새(seam)"로 설계

모듈 간 호출은 **언제든 네트워크로 바뀔 수 있다고 가정**하고 짠다. 즉:
- 계약 메서드는 **비동기(`Promise`)**로 (나중에 `fetch`가 돼도 시그니처 안 바뀜)
- 계약은 **직렬화 가능한 데이터**만 주고받음 (클래스 인스턴스·함수 전달 금지)
- 다른 모듈의 DB 트랜잭션·객체 참조를 넘기지 않음(§13.4의 예외 규칙 참고)

> 이 세 원칙을 지키면 "모듈러 모놀리스"는 "MSA의 안전한 전 단계"가 된다. 지키지 않으면 ①의 빅볼오브머드로 미끄러진다.

---

## 4. 전체 아키텍처 (단일 프로세스, 모듈 조립)

### 4.1 논리 다이어그램

```text
┌──────────────────────────────────────────────────────────────┐
│                    단일 Bun 프로세스 (Bun.serve :4000)          │
│                                                                │
│  ┌──────────────── HTTP 계층 (통합 routes, §11) ─────────────┐ │
│  │  /api/users/*   /api/orders/*   /health   (WS upgrade)    │ │
│  └───────┬───────────────────┬──────────────────────────────┘ │
│          │ endpoint()        │  (Guard·Interceptor·필터 §12)  │
│  ┌───────▼─────────┐  ┌──────▼──────────┐                     │
│  │  users 모듈      │  │  orders 모듈     │   ...모듈 N         │
│  │  ┌───────────┐  │  │  ┌───────────┐  │                     │
│  │  │ contract  │◄─┼──┼──┤ service   │  │  ← 계약으로만 호출   │
│  │  │ (public)  │  │  │  └───────────┘  │     (함수, 지연 0)   │
│  │  ├───────────┤  │  │  ┌───────────┐  │                     │
│  │  │ service   │  │  │  │ store     │  │                     │
│  │  │ store     │  │  │  └───────────┘  │                     │
│  │  └───────────┘  │  │                 │                     │
│  └────────┬────────┘  └────────┬────────┘                     │
│           │  publish/subscribe (인프로세스 이벤트 버스, §9)     │
│  ┌────────▼────────────────────▼────────────────────────────┐ │
│  │              EventBus (동기/비동기 팬아웃)                  │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │   단일 DB (bun:sqlite / Bun.sql) — 모듈별 테이블 소유(§14)  │ │
│  │   users.*  |  orders.*   ← 한 트랜잭션으로 묶기 가능(§13)   │ │
│  └───────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### 4.2 부트 흐름

```text
main.ts
  1) 설정 로드 (Bun.env)                         §16
  2) DB 연결 1개 생성                              §14
  3) EventBus 생성                                §9
  4) 각 모듈 조립(create<Module>())               §7
       - store(db) → service(store, bus) → contract
  5) 모듈들의 routes 병합                          §11
  6) 이벤트 구독 등록(모듈 간 반응)                §9
  7) Bun.serve({ routes, websocket }) 기동
  8) onShutdown 등록 (server.stop → db.close)     §12
```

MSA는 이 부트를 **N번**(서비스마다) 한다. 모듈러 모놀리스는 **한 번**에 모두 조립한다 — 이게 운영 단순성의 근원이다.

---

## 5. 폴더 구조 · 모듈 해부

### 5.1 권장 레포 트리

```text
repo/
├── shared/                       # npm 아님 · 로컬 import (MSA 문서와 공유)
│   ├── http.ts                   # jsonOk / jsonErr / HttpError / readJson
│   ├── validate.ts               # 검증기 (ValidationPipe 대체)
│   ├── di.ts                     # 경량 DI 컨테이너 (선택)
│   ├── pipeline.ts               # endpoint() / Guard / Interceptor
│   ├── errors.ts                 # 전역 예외 필터
│   ├── lifecycle.ts              # graceful shutdown
│   ├── logger.ts                 # 구조화 로그
│   ├── event-bus.ts              # ★ 인프로세스 이벤트 버스 (§9)
│   └── types.ts
├── modules/                      # ★ 각 모듈 = 바운디드 컨텍스트
│   ├── users/
│   │   ├── contract.ts           # ★ 외부 공개 인터페이스 (유일한 진입점)
│   │   ├── events.ts             # 이 모듈이 발행/구독하는 이벤트 타입
│   │   ├── routes.ts             # 이 모듈의 HTTP 라우트 (internal)
│   │   ├── controller.ts         # HTTP 입출력 + 검증 (internal)
│   │   ├── service.ts            # 도메인 로직 (internal)
│   │   ├── store.ts              # 이 모듈 소유 테이블 접근 (internal)
│   │   ├── dto.ts                # 입력 스키마
│   │   └── index.ts              # createUsersModule() 조립 함수만 export
│   └── orders/
│       └── ... (동일 구조)
├── app/
│   ├── main.ts                   # 조립 루트 + Bun.serve
│   ├── registry.ts               # 모듈 등록/조회 (§7)
│   ├── http.ts                   # 모듈 routes 병합 (§11)
│   └── db.ts                     # 단일 DB 연결 (§14)
├── tools/capability-check.ts     # (MSA 문서 §3)
└── README.md
```

> **핵심:** `modules/users/`에서 **밖으로 export되는 건 `index.ts`의 `createUsersModule()`과 `contract.ts`의 타입뿐**이다. `service.ts`·`store.ts`는 모듈 밖에서 import하면 안 된다(§10에서 강제).

### 5.2 한 모듈의 구성요소 (Nest 대응)

| 파일 | 역할 | Nest 대응 | 가시성 |
|---|---|---|---|
| `contract.ts` | 다른 모듈이 쓰는 공개 인터페이스 | (Nest엔 없음 — 이게 경계) | **public** |
| `events.ts` | 발행/구독 이벤트 타입 | EventEmitter payload | public(타입) |
| `index.ts` | `createXModule()` 조립 | `@Module` | **public** |
| `routes.ts` | HTTP 라우트 선언 | `@Controller` 데코레이터 | internal |
| `controller.ts` | 요청→응답, 검증 | Controller 메서드 | internal |
| `service.ts` | 도메인 로직 | `@Injectable` Service | internal |
| `store.ts` | 소유 테이블 접근 | Repository | internal |
| `dto.ts` | 입력 스키마 | DTO + ValidationPipe | internal |

---

## 6. 모듈 계약 (Public API vs Internal)

계약은 이 아키텍처의 **심장**이다. 다른 모듈은 오직 이 인터페이스만 본다.

### 6.1 계약 정의

```ts
// modules/users/contract.ts
// ★ 밖으로 나가는 유일한 "행동" 표면. 구현(service/store)은 숨긴다.

export type UserDTO = {
  id: string;
  name: string;
  createdAt: string;
};

export interface UsersContract {
  // 나중에 fetch로 바뀌어도 시그니처 유지 → 전부 Promise, 직렬화 가능 데이터만
  getUser(id: string): Promise<UserDTO | null>;
  assertExists(id: string): Promise<void>; // 없으면 throw HttpError(404)
}
```

### 6.2 계약 구현은 service가 담당하되, "숨긴 채" 노출

```ts
// modules/users/service.ts  (internal)
import type { UsersContract, UserDTO } from "./contract";
import type { UserStore } from "./store";
import type { EventBus } from "../../shared/event-bus";
import { HttpError } from "../../shared/http";
import type { UserEvents } from "./events";

export class UserService implements UsersContract {
  constructor(private store: UserStore, private bus: EventBus<UserEvents>) {}

  async getUser(id: string): Promise<UserDTO | null> {
    const row = this.store.find(id);
    return row ? this.toDTO(row) : null;
  }

  async assertExists(id: string): Promise<void> {
    if (!this.store.find(id)) throw new HttpError(404, "user_not_found");
  }

  // 계약에 없는 내부 전용 메서드 (밖에서 안 보임 — 타입이 UsersContract로만 노출되므로)
  async create(name: string): Promise<UserDTO> {
    const id = crypto.randomUUID();
    const createdAt = new Date().toISOString();
    this.store.insert(id, name, createdAt);
    await this.bus.publish("user.created", { id, name }); // §9
    return { id, name, createdAt };
  }

  private toDTO(row: any): UserDTO {
    return { id: row.id, name: row.name, createdAt: row.created_at };
  }
}
```

### 6.3 조립 함수 — 모듈이 밖에 주는 것

```ts
// modules/users/index.ts
// ★ 모듈 밖으로 export하는 건 "조립 함수"와 "계약 타입"뿐.
import { UserStore } from "./store";
import { UserService } from "./service";
import { makeUserRoutes } from "./routes";
import type { EventBus } from "../../shared/event-bus";
import type { UsersContract } from "./contract";

export type { UsersContract, UserDTO } from "./contract";

export function createUsersModule(deps: { db: import("bun:sqlite").Database; bus: EventBus<any> }) {
  const store = new UserStore(deps.db);
  const service = new UserService(store, deps.bus);
  return {
    name: "users" as const,
    contract: service as UsersContract, // ← 계약 타입으로 좁혀서 노출
    routes: makeUserRoutes(service),     // ← HTTP는 통합 서버가 병합
    // service, store 는 절대 반환하지 않는다
  };
}
```

> **포인트:** `contract`는 `UsersContract`로 **타입을 좁혀** 반환한다. 그래서 다른 모듈은 `create()` 같은 내부 메서드에 접근할 수 없다 — 컴파일러가 막는다. 이것이 "런타임 없이 경계를 세우는" 첫 번째 장치다.

---

## 7. 모듈 등록 · 조립 루트 (경량 DI)

MSA 문서 §10.2의 DI 컨테이너를 써도 되지만, 모듈 단위에서는 **모듈 레지스트리**가 더 직관적이다.

### 7.1 레지스트리

```ts
// app/registry.ts
export class ModuleRegistry {
  private contracts = new Map<string, unknown>();

  provide<T>(name: string, contract: T): void {
    if (this.contracts.has(name)) throw new Error(`module already registered: ${name}`);
    this.contracts.set(name, contract);
  }

  get<T>(name: string): T {
    const c = this.contracts.get(name);
    if (!c) throw new Error(`module not found: ${name}`);
    return c as T;
  }
}
```

### 7.2 조립 루트 (main.ts)

```ts
// app/main.ts
import { Database } from "bun:sqlite";
import { EventBus } from "../shared/event-bus";
import { ModuleRegistry } from "./registry";
import { mergeRoutes } from "./http";
import { onShutdown } from "../shared/lifecycle";

import { createUsersModule, type UsersContract } from "../modules/users";
import { createOrdersModule } from "../modules/orders";

// 1) 인프라 (단일 인스턴스)
const db = new Database(Bun.env.DATABASE_PATH ?? "app.db");
db.exec("PRAGMA journal_mode = WAL;");
const bus = new EventBus();
const registry = new ModuleRegistry();

// 2) 모듈 조립 (의존 순서대로)
const users = createUsersModule({ db, bus });
registry.provide<UsersContract>("users", users.contract);

const orders = createOrdersModule({
  db,
  bus,
  users: registry.get<UsersContract>("users"), // ★ 계약 주입 (직접 import 아님)
});

const modules = [users, orders];

// 3) HTTP 통합
const routes = mergeRoutes(modules);

// 4) 모듈 간 이벤트 구독 (§9)
orders.subscribeTo?.(bus);

// 5) 기동
const server = Bun.serve({
  port: Number(Bun.env.PORT ?? 4000),
  routes: {
    "/health": { GET: () => Response.json({ ok: true, modules: modules.map((m) => m.name) }) },
    ...routes,
  },
  fetch() { return Response.json({ error: "not_found" }, { status: 404 }); },
});

console.log(`[app] :${server.port} modules=[${modules.map((m) => m.name).join(", ")}]`);

// 6) graceful shutdown
onShutdown(async () => { server.stop(); db.close(); });
```

> 이 `main.ts` 하나가 MSA의 `gateway + user-service + order-service`의 부트 3개를 대체한다. 의존 방향이 코드에 **명시적으로** 드러나는 게 장점이다.

---

## 8. 모듈 간 통신 ① — 동기 직접 호출 (계약 인터페이스)

주문 생성 시 유저 존재를 확인하는 시나리오. MSA에선 `fetch`, 모듈러 모놀리스에선 **함수 호출**이다 — 단, **계약을 통해서만**.

```ts
// modules/orders/service.ts
import type { UsersContract } from "../users";   // ← 계약 타입만 import (구현 아님)
import type { OrderStore } from "./store";
import type { EventBus } from "../../shared/event-bus";
import { HttpError } from "../../shared/http";

export class OrderService {
  constructor(
    private store: OrderStore,
    private users: UsersContract,   // ★ 주입된 계약 (직접 참조 아님)
    private bus: EventBus<any>,
  ) {}

  async createOrder(userId: string, item: string) {
    // 유저 검증 — 인프로세스지만 "원격처럼" 계약으로 (네트워크 왕복은 0)
    await this.users.assertExists(userId); // 없으면 HttpError(404) throw

    const id = crypto.randomUUID();
    this.store.insert(id, userId, item);
    await this.bus.publish("order.created", { id, userId, item });
    return { id, userId, item, status: "created" };
  }
}
```

**MSA와의 코드 차이:**

```ts
// MSA:                     await fetchJson(`${USER_BASE}/users/${userId}`);  // 네트워크
// 모듈러 모놀리스:          await this.users.assertExists(userId);           // 함수 호출
```

시그니처가 둘 다 `Promise`인 게 핵심이다. **§20에서 MSA로 뽑을 때, `UsersContract`의 구현만 `fetch` 어댑터로 갈아끼우면 `OrderService`는 한 글자도 안 바뀐다.**

| 장점 | 유의 |
|---|---|
| 네트워크 지연 0, 타입 안전(컴파일 체크) | 계약 밖 메서드 호출 금지(타입이 막음) |
| 디버깅 = 스택 트레이스 하나 | 순환 의존 주의(§10.3) |

---

## 9. 모듈 간 통신 ② — 인프로세스 이벤트 버스 (브로커 대체)

MSA에서 Kafka/NATS로 하던 **비동기 팬아웃**("주문 생성됨 → 알림·재고·통계가 각자 반응")을, **외부 브로커 없이** 인프로세스 이벤트 버스로 한다. 폐쇄망에서 브로커 반입·운영을 없애는 결정적 도구다.

### 9.1 타입 안전 이벤트 버스

```ts
// shared/event-bus.ts
type Handler<P> = (payload: P) => void | Promise<void>;

export class EventBus<E extends Record<string, unknown> = Record<string, unknown>> {
  private handlers = new Map<keyof E, Set<Handler<any>>>();

  on<K extends keyof E>(event: K, handler: Handler<E[K]>): () => void {
    if (!this.handlers.has(event)) this.handlers.set(event, new Set());
    this.handlers.get(event)!.add(handler);
    return () => this.handlers.get(event)!.delete(handler); // unsubscribe
  }

  /** 동기 대기 팬아웃: 모든 핸들러가 끝날 때까지 await (트랜잭션 내부용) */
  async publish<K extends keyof E>(event: K, payload: E[K]): Promise<void> {
    const hs = this.handlers.get(event);
    if (!hs) return;
    for (const h of hs) {
      await h(payload); // 하나 실패 시 전파 (§9.4에서 정책 선택)
    }
  }

  /** 비동기 발사 후 망각: 커밋 후 사이드이펙트용 (호출자를 막지 않음) */
  emit<K extends keyof E>(event: K, payload: E[K]): void {
    const hs = this.handlers.get(event);
    if (!hs) return;
    queueMicrotask(async () => {
      for (const h of hs) {
        try { await h(payload); }
        catch (e) { console.error(`[event:${String(event)}]`, e); }
      }
    });
  }
}
```

### 9.2 이벤트 타입 선언

```ts
// modules/users/events.ts
export type UserEvents = {
  "user.created": { id: string; name: string };
  "user.deleted": { id: string };
};

// modules/orders/events.ts
export type OrderEvents = {
  "order.created": { id: string; userId: string; item: string };
};

// app 에서 합집합으로 하나의 버스 타입 구성
// type AppEvents = UserEvents & OrderEvents;
```

### 9.3 구독 (모듈이 다른 모듈의 이벤트에 반응)

```ts
// modules/notifications/index.ts
import type { EventBus } from "../../shared/event-bus";

export function createNotificationsModule(deps: { bus: EventBus<any> }) {
  const bus = deps.bus;

  // "주문 생성됨"에 반응 — orders 모듈을 전혀 import하지 않고 이벤트로만 결합
  bus.on("order.created", async (p: { id: string; userId: string }) => {
    // 알림 발송 (내부 로직)
    console.log(`[noti] order ${p.id} for user ${p.userId}`);
  });

  return { name: "notifications" as const, contract: {}, routes: {} };
}
```

> **디커플링 수준이 MSA와 동일**하다: `notifications`는 `orders`의 존재조차 모른다. 이벤트 이름/페이로드(계약)로만 결합한다. **이것이 "메시지 브로커 못지 않은" 팬아웃이다.**

### 9.4 동기(publish) vs 비동기(emit) — 언제 무엇을

| 방식 | 시맨틱 | 언제 |
|---|---|---|
| `await bus.publish(...)` | 모든 핸들러 완료까지 대기, 실패 전파 | **트랜잭션 안**에서 강한 정합성이 필요할 때(§13) |
| `bus.emit(...)` | 발사 후 망각, 호출자 안 막음, 실패 격리 | 커밋 **후** 부수효과(알림·통계 등), 장애 격리 |

**장애 격리 한계(정직):** 인프로세스라 핸들러가 무한루프/크래시하면 프로세스 전체가 영향받을 수 있다. 이건 MSA의 프로세스 격리에 밀리는 지점이다(§2, §23의 R). 완화: `emit` 핸들러는 반드시 try/catch, 무거운 작업은 §9.5의 인프로세스 큐로.

### 9.5 (선택) 인프로세스 잡 큐 — "나중에" 처리

브로커 없이 지연/재시도 작업이 필요하면 얇은 큐를 둔다.

```ts
// shared/job-queue.ts — 인메모리 순차 처리 (프로세스 재시작 시 유실 주의)
type Job = () => Promise<void>;

export class JobQueue {
  private q: Job[] = [];
  private running = false;

  push(job: Job) { this.q.push(job); this.drain(); }

  private async drain() {
    if (this.running) return;
    this.running = true;
    while (this.q.length) {
      const job = this.q.shift()!;
      try { await job(); } catch (e) { console.error("[job]", e); }
    }
    this.running = false;
  }
}
```

> **영속성이 필요하면**(재시작해도 잃으면 안 되는 작업): 인메모리 대신 **DB 테이블 기반 아웃박스**로. `bun:sqlite`에 `jobs` 테이블을 두고, 트랜잭션 안에서 잡을 INSERT → 커밋 후 워커 루프가 폴링. 여전히 외부 브로커 0.

---

## 10. 경계 강제 — 규약을 코드로 지키는 법

"convention"만으로는 언젠가 누군가 `modules/users/store`를 직접 import한다. 폐쇄망에서 ESLint 플러그인 반입이 어려우니, **린트 없이** 경계를 지키는 장치를 겹겹이 둔다.

### 10.1 장치 ① — 타입 은닉 (컴파일러가 1차 방어)

§6.3처럼 `index.ts`가 **계약 타입으로 좁혀** 내보내면, 다른 모듈은 내부 메서드에 타입 레벨에서 접근 불가. 가장 저렴하고 강력한 장치.

### 10.2 장치 ② — 배럴(barrel) 규칙

```text
규칙: 모듈 밖에서는 "modules/<name>" (= index.ts) 와 "modules/<name>/contract" 만 import.
      "modules/<name>/service", "/store", "/routes" 를 밖에서 import하면 위반.
```

### 10.3 장치 ③ — 자체 경계 검사기 (외부 린터 0)

간단한 스크립트로 위반을 CI에서 잡는다. `bun`만으로.

```ts
// tools/check-boundaries.ts  —  bun tools/check-boundaries.ts
import { Glob } from "bun";

const glob = new Glob("modules/**/*.ts");
const violations: string[] = [];

// "다른 모듈의 내부 파일"을 import하는 패턴 탐지
// 허용: ./  (같은 모듈)   ../../shared/   ../<other>  (index)  ../<other>/contract
const badImport = /from\s+["']\.\.\/([a-z0-9-]+)\/(service|store|routes|controller|dto)["']/g;

for await (const file of glob.scan(".")) {
  const src = await Bun.file(file).text();
  let m: RegExpExecArray | null;
  while ((m = badImport.exec(src))) {
    violations.push(`${file}: imports internal '${m[2]}' of module '${m[1]}'`);
  }
}

if (violations.length) {
  console.error("BOUNDARY VIOLATIONS:\n" + violations.join("\n"));
  process.exit(1);
}
console.log("module boundaries OK");
```

> 이 30줄이 "Nx/모듈 경계 린터" 같은 도구의 핵심 90%를 폐쇄망에서 대체한다. 규칙이 명시적이라 팀이 이해하기도 쉽다.

### 10.4 장치 ④ — 순환 의존 방지

`users → orders → users` 계약 순환이 생기면 조립이 꼬인다. 규칙:
- **동기 계약 호출은 한 방향으로만**(예: orders → users 는 OK, users → orders 는 금지)
- 반대 방향 반응이 필요하면 **이벤트로**(users가 `order.created`를 구독) → 순환이 끊긴다

---

## 11. 라우팅 통합 — 하나의 Bun.serve에 모듈 routes 병합

각 모듈이 자기 `routes` 객체를 내면, `app`이 prefix를 붙여 하나로 합친다. MSA의 게이트웨이 프록시 대신 **함수로 병합**한다(네트워크 0).

### 11.1 모듈의 라우트 선언 (endpoint로 횡단관심사 조립 — MSA 문서 §10.8)

```ts
// modules/users/routes.ts
import { endpoint } from "../../shared/pipeline";
import { authGuard } from "../../shared/guards";
import { timing } from "../../shared/interceptors";
import { makeUserController } from "./controller";
import type { UserService } from "./service";

export function makeUserRoutes(service: UserService) {
  const ctrl = makeUserController(service);
  return {
    "/users/:id": {
      GET: endpoint({ guards: [authGuard], interceptors: [timing], handler: ctrl.getUser }),
    },
    "/users": {
      POST: endpoint({ interceptors: [timing], handler: ctrl.createUser }),
    },
  };
}
```

### 11.2 병합기 (prefix 부여)

```ts
// app/http.ts
type Mod = { name: string; routes: Record<string, unknown> };

export function mergeRoutes(modules: Mod[]) {
  const merged: Record<string, unknown> = {};
  for (const m of modules) {
    const prefix = `/api/${m.name}`; // 예: users → /api/users
    for (const [path, handlers] of Object.entries(m.routes)) {
      const full = path === "/" ? prefix : `${prefix}${path}`;
      if (merged[full]) throw new Error(`route collision: ${full}`);
      merged[full] = handlers;
    }
  }
  return merged;
  // 결과: "/api/users/users/:id" 가 되지 않도록 모듈 내부 경로 네이밍을 조정하거나,
  //       prefix를 "/api" 로만 두고 모듈이 "/users/:id" 를 완전경로로 선언하게 한다.
}
```

> **팁:** 충돌·중복 prefix를 피하려면 규칙 하나만 정하라 — "모듈 내부 `routes` 키는 도메인 명사 없이(`/:id`), prefix에서 도메인을 붙인다" 또는 "모듈이 완전 경로를 선언하고 병합기는 검증만 한다". 팀에서 하나로 고정.

### 11.3 MSA 게이트웨이와의 대응

| MSA | 모듈러 모놀리스 |
|---|---|
| 게이트웨이가 `/api/users` → user-service로 **프록시(fetch)** | `mergeRoutes`가 `/api/users` → users 모듈 핸들러로 **직결** |
| 요청 ID 부여·인증을 게이트웨이에서 | Interceptor/Guard를 `endpoint()`에서(§12) |
| 네트워크 홉 1회 | 홉 0 |

---

## 12. 횡단 관심사 (DI·검증·Guard·Interceptor·예외필터)

**MSA 문서 §10을 그대로 재사용한다.** `shared/`는 두 아키텍처가 공유하므로, 검증·Guard·Interceptor·예외필터·DI 컨테이너 코드를 다시 쓰지 않는다. 여기서는 **모듈러 모놀리스 맥락의 차이점**만 정리한다.

| 관심사 | 구현 위치 | 모놀리스에서의 차이 |
|---|---|---|
| 검증(ValidationPipe) | `shared/validate.ts` | 동일. 모듈 `dto.ts`에 스키마 |
| Guard(인증/인가) | `shared/guards.ts` | 동일. **게이트웨이가 없으니** 인증을 `endpoint()` Guard로 일괄 |
| Interceptor(로깅/타이밍) | `shared/interceptors.ts` | 동일. requestId는 서버 진입에서 1회 부여 |
| Exception Filter | `shared/errors.ts` | 동일. `HttpError` → JSON |
| Lifecycle | `shared/lifecycle.ts` | `onShutdown`에 `server.stop → db.close` |
| DI | `shared/di.ts` 또는 §7 레지스트리 | 모듈 조립엔 **레지스트리(§7)**가 더 자연스러움 |

핵심 컨트롤러 예시(검증까지):

```ts
// modules/users/controller.ts
import { jsonOk } from "../../shared/http";
import { validate } from "../../shared/validate";
import { createUserSchema } from "./dto";
import type { Ctx } from "../../shared/pipeline";
import type { UserService } from "./service";

export function makeUserController(service: UserService) {
  return {
    async getUser(ctx: Ctx) {
      const user = await service.getUser(ctx.params.id!);
      return user ? jsonOk(user) : jsonOk({ error: "not_found" }, 404);
    },
    async createUser(ctx: Ctx) {
      const dto = validate(createUserSchema, await ctx.req.json()); // §10.3
      const created = await service.create(dto.name);
      return jsonOk(created, 201);
    },
  };
}
```

> 요컨대, **NestJS 레벨 횡단관심사는 아키텍처(MSA vs 모놀리스)와 무관하게 동일한 `shared/` 코드로 해결**된다. 모놀리스라고 기능이 빠지지 않는다.

---

## 13. ★ 트랜잭션 — 모놀리스의 결정적 강점

여기가 "MSA 못지 않은"을 넘어 **"MSA보다 나은"** 지점이다.

### 13.1 문제: 여러 도메인을 원자적으로 바꿔야 할 때

"주문을 만들면서 동시에 유저의 포인트를 차감"한다고 하자.

- **MSA:** order-service와 user-service가 **다른 DB**. 원자성이 없다 → **사가(Saga)** 패턴, 보상 트랜잭션, 아웃박스, 최종 일관성. 코드·운영 폭증.
- **모듈러 모놀리스:** 두 모듈이 **같은 DB 연결**을 공유 → **로컬 트랜잭션 한 블록**.

### 13.2 로컬 트랜잭션 (bun:sqlite)

```ts
// modules/orders/service.ts — 트랜잭션 경계는 "유스케이스"가 소유
import { Database } from "bun:sqlite";

export class OrderService {
  constructor(
    private db: Database,
    private store: OrderStore,
    private users: UsersContract,       // 검증용 계약
    private pointsTx: PointsTxPort,     // ★ 트랜잭션 참여용 포트 (§13.4)
    private bus: EventBus<any>,
  ) {}

  async placeOrder(userId: string, item: string, cost: number) {
    await this.users.assertExists(userId);

    // ★ 한 트랜잭션: 주문 생성 + 포인트 차감이 모두 성공하거나 모두 롤백
    const tx = this.db.transaction((oid: string) => {
      this.store.insertTx(oid, userId, item);         // orders 테이블
      this.pointsTx.deductTx(userId, cost);           // users(points) 테이블
    });

    const orderId = crypto.randomUUID();
    tx(orderId); // 여기서 커밋. 하나라도 throw면 전부 롤백.

    // 커밋 후에만 외부에 알림 (실패해도 데이터 정합성엔 영향 없음)
    this.bus.emit("order.created", { id: orderId, userId, item });
    return { id: orderId, userId, item, status: "placed" };
  }
}
```

```ts
// Bun.sql(Postgres/MySQL)일 때
// await sql.begin(async (tx) => {
//   await store.insertTx(tx, ...);
//   await points.deductTx(tx, ...);
// });
```

**MSA로는 이 8줄이 사가 오케스트레이터 + 보상 로직 + 아웃박스 워커 + 멱등 처리 수백 줄이 된다.** 정합성이 중요한 도메인일수록 모듈러 모놀리스의 이 강점은 압도적이다.

### 13.3 트랜잭션 후 이벤트 규칙

```text
트랜잭션 안:  bus.publish (동기, 실패하면 롤백에 참여) — 신중히
트랜잭션 후:  bus.emit    (커밋된 사실에 대한 부수효과) — 기본값
```

**원칙:** "외부에 알리는 이벤트(알림·통계·메일)"는 **커밋 후 `emit`**. 롤백됐는데 알림이 나가는 사고를 막는다.

### 13.4 트랜잭션과 모듈 경계의 긴장 (중요 · 정직)

트랜잭션은 "같은 DB 연결"을 공유해야 하는데, 이는 §3의 "모듈은 자기 테이블만" 원칙과 **긴장**한다. 두 가지 규율로 관리:

1. **트랜잭션 참여 포트(port)를 계약에 명시** — orders가 users 테이블을 직접 건드리지 않는다. users 모듈이 `PointsTxPort.deductTx(userId, cost)`를 **계약으로 노출**하고, orders는 그 포트만 호출한다. 여전히 users의 store는 숨겨진다.

```ts
// modules/users/contract.ts (추가)
import type { Database } from "bun:sqlite";
export interface PointsTxPort {
  // 같은 트랜잭션 컨텍스트에서 실행되는 것을 전제로 한 동기 메서드
  deductTx(userId: string, amount: number): void;
}
```

2. **트랜잭션은 "오케스트레이션 모듈/유스케이스"가 소유** — 여러 모듈을 묶는 트랜잭션이 잦아지면, 그건 **경계가 잘못 그어졌다는 신호**다(§21). 그 두 모듈은 사실 하나의 바운디드 컨텍스트일 수 있다. 합치는 것을 고려하라.

> **MSA 전환 시:** 이 크로스-모듈 트랜잭션이 있던 자리가 정확히 "MSA로 쪼개면 사가가 필요해지는 곳"이다. 즉 **모듈러 모놀리스는 '어디가 분산 트랜잭션 비용이 큰지'를 미리 보여준다** — 쪼개기 전에 비용을 알 수 있다(§20).

---

## 14. 데이터 · DB — 단일 DB에서의 소유권 규율

### 14.1 원칙: 단일 DB, 모듈별 테이블 소유

```text
DB 연결은 하나 (app/db.ts) — 트랜잭션을 위해
그러나 테이블 소유권은 모듈별:
  users 모듈  → users, user_points   (이 모듈만 SQL 접근)
  orders 모듈 → orders, order_items  (이 모듈만 SQL 접근)

❌ orders의 store가 SELECT * FROM users
✅ orders는 UsersContract.getUser() 호출 (§8)
✅ 트랜잭션이 필요하면 PointsTxPort 등 계약 포트로만 (§13.4)
```

**"한 DB"지만 "한 스키마 진흙탕"이 아니다.** 소유권을 규율로 지키면, 나중에 모듈을 떼어낼 때 그 모듈의 테이블만 들고 나가면 된다(§20).

### 14.2 스키마 소유 강제 팁 (선택)

- 테이블 이름에 모듈 prefix: `users_profile`, `orders_line` → 소유가 눈에 보임
- Postgres면 **모듈별 스키마**(`users.profile`, `orders.line`)로 물리적 분리 → §20에서 DB까지 깔끔히 분리
- store 파일에서 접근하는 테이블 목록을 주석/상수로 명시 → §10.3 검사기로 "다른 모듈 테이블 접근" 탐지 확장 가능

### 14.3 DB 연결 (app/db.ts)

```ts
// app/db.ts
import { Database } from "bun:sqlite";

export function createDb(): Database {
  const db = new Database(Bun.env.DATABASE_PATH ?? "app.db");
  db.exec("PRAGMA journal_mode = WAL;");
  db.exec("PRAGMA foreign_keys = ON;");
  return db;
}

// 각 모듈은 조립 시 자기 테이블 마이그레이션을 실행 (아래 §14.4)
```

### 14.4 모듈별 마이그레이션 (소유 테이블만)

```ts
// modules/users/store.ts
import type { Database } from "bun:sqlite";

export class UserStore {
  constructor(private db: Database) {
    // 이 모듈이 소유한 테이블만 이 모듈이 생성
    db.exec(`
      CREATE TABLE IF NOT EXISTS users (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS user_points (
        user_id TEXT PRIMARY KEY, balance INTEGER NOT NULL DEFAULT 0);
    `);
  }
  find(id: string) { return this.db.query("SELECT * FROM users WHERE id=?").get(id); }
  insert(id: string, name: string, createdAt: string) {
    this.db.query("INSERT INTO users (id,name,created_at) VALUES (?,?,?)").run(id, name, createdAt);
  }
  // 트랜잭션 포트 구현 (§13.4) — 같은 db 연결이라 상위 tx에 참여
  deductTx(userId: string, amount: number) {
    this.db.query("UPDATE user_points SET balance = balance - ? WHERE user_id=?").run(amount, userId);
  }
}
```

---

## 15. 실시간 (WebSocket) 통합

모놀리스는 WS도 **같은 프로세스**에서 처리한다. 이벤트 버스(§9)와 결합하면 강력하다: "도메인 이벤트 → WS 브로드캐스트"가 프로세스 내부에서 즉시.

```ts
// app/main.ts (websocket 통합 발췌)
const server = Bun.serve({
  port: Number(Bun.env.PORT ?? 4000),
  routes: { /* ...§7 */ },
  fetch(req, srv) {
    if (new URL(req.url).pathname === "/ws" && srv.upgrade(req)) return;
    return Response.json({ error: "not_found" }, { status: 404 });
  },
  websocket: {
    open(ws) { ws.subscribe("orders"); },
    message() {},
    close() {},
  },
});

// 도메인 이벤트 → WS 팬아웃 (브로커 없이, 인프로세스)
bus.on("order.created", (p) => {
  server.publish("orders", JSON.stringify({ type: "order.created", data: p }));
});
```

> MSA라면 "order-service → (브로커) → ws-service → 클라이언트"였을 경로가, 모놀리스에선 **`bus.on` 한 줄**이다. 지연도 홉도 없다.

---

## 16. 설정 · 환경 변수

MSA 문서 §14와 동일하게 `Bun.env`(+ `.env` 자동 로드). 모놀리스는 **설정이 하나로 모인다**는 게 장점.

```ts
// app/config.ts
function required(key: string): string {
  const v = Bun.env[key];
  if (!v) { console.error(`${key} required`); process.exit(1); }
  return v;
}

export const config = {
  port: Number(Bun.env.PORT ?? 4000),
  dbPath: Bun.env.DATABASE_PATH ?? "app.db",
  jwtSecret: required("JWT_SECRET"),
  // 모듈별 설정도 여기서 네임스페이스로: users: { ... }, orders: { ... }
};
```

```bash
# .env (하나면 됨 — MSA처럼 서비스마다 흩어지지 않음)
PORT=4000
DATABASE_PATH=./data/app.db
JWT_SECRET=change-me
```

---

## 17. 관측 · 헬스 · 로깅

### 17.1 헬스 (모듈별 상태 집계)

```ts
// 각 모듈이 health()를 선택적으로 제공, app이 집계
"/health": {
  GET: () => {
    const results = modules.map((m) => ({ name: m.name, ok: m.health?.() ?? true }));
    const ok = results.every((r) => r.ok);
    return Response.json({ ok, modules: results }, { status: ok ? 200 : 503 });
  },
},
```

### 17.2 로깅 — 추적이 오히려 쉽다

- MSA는 요청 하나가 여러 프로세스를 거쳐 **분산 추적**이 필수. 모놀리스는 **한 스택 트레이스**에 다 나온다.
- 그래도 requestId는 부여해 로그 상관관계를 유지: 서버 진입 Interceptor에서 `crypto.randomUUID()` 1회.
- 구조화 JSON(`shared/logger.ts`)을 stdout으로 → 내부 수집기(폐쇄망).

```ts
// 모듈 경계를 넘는 호출도 같은 프로세스라 스택에 다 찍힘 → 디버깅 난이도 ↓
bus.on("order.created", (p) => log("info", "event", { event: "order.created", ...p }));
```

---

## 18. 테스트 (모듈 단위 + 통합)

`bun test`(내장, 반입 0). 모듈러 모놀리스는 **모듈을 격리 테스트**하기 쉽다(계약이 mock 지점).

### 18.1 모듈 단위 — 계약을 mock으로 주입

```ts
// modules/orders/service.test.ts
import { test, expect, mock } from "bun:test";
import { OrderService } from "./service";

test("존재하지 않는 유저면 주문 실패", async () => {
  const users = { assertExists: mock(async () => { throw new Error("404"); }) };
  const store = { insert: mock(() => {}) };
  const bus = { publish: mock(async () => {}), emit: mock(() => {}) };
  const svc = new OrderService(store as any, users as any, bus as any);

  await expect(svc.createOrder("nope", "book")).rejects.toThrow();
  expect(store.insert).not.toHaveBeenCalled();
});
```

### 18.2 이벤트 버스 테스트

```ts
import { test, expect } from "bun:test";
import { EventBus } from "../../shared/event-bus";

test("publish는 구독자에게 전달", async () => {
  const bus = new EventBus<{ "x": { n: number } }>();
  let got = 0;
  bus.on("x", (p) => { got = p.n; });
  await bus.publish("x", { n: 42 });
  expect(got).toBe(42);
});
```

### 18.3 통합 — 전체 앱 부트 후 HTTP

```ts
// e2e/app.test.ts — 실제 조립된 앱을 인메모리 DB로 기동
import { test, expect, beforeAll, afterAll } from "bun:test";

let proc: ReturnType<typeof Bun.spawn>;
beforeAll(async () => {
  proc = Bun.spawn(["bun", "app/main.ts"], {
    env: { ...process.env, PORT: "4900", DATABASE_PATH: ":memory:" },
  });
  await Bun.sleep(300);
});
afterAll(() => proc.kill());

test("주문 생성 e2e (유저→주문 한 프로세스)", async () => {
  const u = await fetch("http://127.0.0.1:4900/api/users", {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: "kim" }),
  }).then((r) => r.json());

  const res = await fetch("http://127.0.0.1:4900/api/orders", {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ userId: u.id, item: "book" }),
  });
  expect(res.status).toBe(201);
});
```

> **모놀리스 테스트 강점:** e2e가 **프로세스 하나**만 띄우면 된다. MSA는 서비스 3개 + 게이트웨이를 다 띄워야 같은 시나리오를 검증한다.

---

## 19. 배포 — 단일 바이너리의 극단적 단순함

### 19.1 단일 실행 (폐쇄망 최적)

```bash
bun app/main.ts
# 또는 단일 바이너리
bun build --compile app/main.ts --outfile app
./app
```

**반입물이 하나다.** MSA는 서비스 이미지 N개 + 게이트웨이 + compose. 모듈러 모놀리스는 **바이너리 1개(또는 이미지 1개)**.

### 19.2 Dockerfile

```dockerfile
FROM oven/bun:1
WORKDIR /app
COPY shared ./shared
COPY modules ./modules
COPY app ./app
ENV PORT=4000
EXPOSE 4000
CMD ["bun", "app/main.ts"]
```

`dependencies` 비어있음(§외부 0) → `bun install` 단계 자체가 없음 → **빌드가 인터넷을 안 탐**.

### 19.3 스케일 (수평 복제)

모놀리스도 **여러 인스턴스로 복제**해 로드밸런서 뒤에 둘 수 있다(무상태 HTTP라면). "모놀리스 = 스케일 못 함"은 오해다. 못 하는 건 **모듈별 개별 스케일**이지, 전체 복제는 자유롭다.

```text
        ┌─ app 인스턴스 1 ─┐
LB ────┼─ app 인스턴스 2 ─┼──→ (공유 DB / 또는 읽기 복제본)
        └─ app 인스턴스 3 ─┘
주의: 인프로세스 이벤트 버스(§9)는 "인스턴스 내부"만 팬아웃한다.
      인스턴스 간 이벤트가 필요해지면 그때가 브로커(또는 MSA) 도입 신호.
```

---

## 20. ★ 나중에 MSA로 쪼개기 (이음새 · Strangler)

모듈러 모놀리스의 진짜 값어치: **잘못 쪼개는 위험 없이 시작하고, 정말 필요할 때 싸게 전환**.

### 20.1 전환이 싼 이유 — 이미 경계가 있다

§3의 세 원칙을 지켰다면, 한 모듈을 서비스로 뽑는 건 **계약 뒤의 어댑터 교체**다.

```ts
// AS-IS (모놀리스): orders가 users 계약을 "직접 구현체"로 주입받음
const orders = createOrdersModule({ users: usersModule.contract, ... });

// TO-BE (users를 별도 서비스로 뽑음): 계약 구현만 fetch 어댑터로 교체
class UsersHttpAdapter implements UsersContract {          // ★ 계약은 그대로
  constructor(private base: string) {}
  async getUser(id: string) {
    return fetchJson(`${this.base}/users/${id}`).catch(() => null);
  }
  async assertExists(id: string) {
    const r = await fetch(`${this.base}/users/${id}`, { signal: AbortSignal.timeout(2000) });
    if (r.status === 404) throw new HttpError(404, "user_not_found");
    if (!r.ok) throw new HttpError(503, "user_service_unavailable");
  }
}
const orders = createOrdersModule({ users: new UsersHttpAdapter(Bun.env.USER_SERVICE_URL!), ... });
```

**`OrderService`는 한 글자도 안 바뀐다.** 계약이 이음새였기 때문. 이게 스트랭글러(Strangler Fig) 패턴의 무의존 구현이다.

### 20.2 전환 체크리스트

```text
[ ] 뽑을 모듈이 계약(contract)으로만 소비되고 있었는가? (§10 검사기로 확인)
[ ] 그 모듈과의 크로스-모듈 트랜잭션(§13.4)이 있었는가?
      → 있다면 여기가 사가가 필요해지는 곳. 전환 비용을 미리 계산.
[ ] 그 모듈이 구독하던 이벤트를, 이제 네트워크(브로커/HTTP)로 전달할 방법 결정
[ ] 그 모듈의 테이블을 별도 DB로 이전 (§14.2에서 스키마 분리해 뒀다면 수월)
[ ] 계약 구현을 HTTP 어댑터로 교체 (§20.1)
[ ] MSA 문서(bun-native-msa.md)로 합류
```

### 20.3 전환 판단 트리

```text
지금 아픈 게 무엇인가?
├─ "특정 모듈만 스케일이 필요" .......... → 그 모듈만 서비스로 (부분 전환)
├─ "배포 주기가 팀별로 충돌" ............ → 조직 경계 따라 분리
├─ "한 모듈 장애가 전체를 죽임" ......... → 격리 필요 → 서비스로
└─ 위 중 아무것도 아님 .................. → 모놀리스 유지 (전환하지 마라)
```

> **전환은 목표가 아니라 옵션이다.** 많은 시스템이 모듈러 모놀리스로 평생 잘 산다. 이 문서의 규율은 "MSA로 가려고"가 아니라 "**갈 수 있게 열어두려고**" 지키는 것이다.

---

## 21. 안티패턴 · 함정

| 함정 | 왜 나쁜지 | 대안 |
|---|---|---|
| 모듈 간 `service`/`store` 직접 import | 경계 붕괴 = 빅볼오브머드 | 계약만 (§6), 검사기(§10.3) |
| 계약에 클래스 인스턴스/함수 넘김 | 직렬화 불가 = MSA 전환 봉쇄 | 데이터(DTO)만 (§3.3) |
| 크로스-모듈 트랜잭션 남발 | 경계가 틀렸다는 신호 | 모듈 합치기 검토 (§13.4) |
| 모든 통신을 이벤트로 | 흐름 추적 불가·순환 | 질의는 동기 계약, 통지는 이벤트 (§9.4) |
| 계약 순환(A↔B 동기) | 조립 꼬임 | 한 방향 계약 + 역방향은 이벤트 (§10.4) |
| 이벤트 핸들러에서 throw 방치 | 프로세스 전체 영향 | `emit` 핸들러 try/catch 필수 (§9.4) |
| `emit`을 트랜잭션 커밋 전에 | 롤백돼도 알림 나감 | 커밋 후 emit (§13.3) |
| 한 스키마에 모든 테이블 섞기 | 소유권 소멸 | 모듈 prefix/스키마 (§14.2) |
| 모듈 100개로 잘게 쪼개기 | 조립·인지 부하 | 2~6 바운디드 컨텍스트부터 |
| "어차피 나중에 MSA니까" 미리 fetch화 | 조기 복잡도·지연 | 계약만 지키고 인프로세스 유지 (§20) |

---

## 22. 최소 동작 예시 (User + Order 한 프로세스)

앞 섹션 코드를 이어 붙이면 아래 흐름이 **하나의 프로세스**에서 동작한다.

### 22.1 호출 흐름

```text
Client
  POST http://localhost:4000/api/orders   { userId, item }
       │
       ▼  mergeRoutes → orders 모듈 endpoint (Guard·Interceptor·필터)
       │
       ▼  OrderService.createOrder()
       │     ├─ users.assertExists(userId)      ← 계약 함수 호출 (네트워크 0)
       │     ├─ store.insert(...)               ← orders 테이블
       │     └─ bus.emit("order.created", ...)  ← 인프로세스 팬아웃
       │            └─ notifications 모듈이 반응 (orders를 모름)
       │            └─ WS "orders" 채널로 브로드캐스트 (§15)
       ▼
  201 { order }
```

이 전체가 **함수 호출 스택 하나**다. MSA였다면 gateway→order-service→(fetch)→user-service→(브로커)→noti-service의 네트워크 5홉.

### 22.2 트랜잭션이 낀 버전 (§13)

```text
POST /api/orders  { userId, item, cost }
  → OrderService.placeOrder()
       db.transaction(() => {
         orders.insertTx(...)         // orders 테이블
         users.deductTx(userId, cost) // users 테이블 (PointsTxPort 계약)
       })  ← 원자적. 하나 실패 시 전부 롤백.
       커밋 후: bus.emit("order.created")
```

**MSA로는 불가능하거나 사가가 필요한 이 원자성이, 모듈러 모놀리스에선 공짜다.**

---

## 23. 리스크 · 완화

| # | 리스크 | 영향 | 완화 |
|---|---|---|---|
| R1 | **장애 격리 약함** — 한 모듈 크래시/무한루프가 프로세스 전체에 영향 | 가용성 | `emit` 핸들러 try/catch(§9.4), 무거운 작업 큐 분리, 치명 모듈은 §20으로 조기 분리 |
| R2 | **독립 스케일 불가** — 모듈별 리소스 스케일 안 됨 | 성능 | 전체 수평복제(§19.3), 핫스팟 모듈만 서비스로(§20) |
| R3 | **경계 침식** — 시간이 지나며 계약 무시하고 직접 참조 | 유지보수 | 검사기 CI 게이트(§10.3), 타입 은닉(§10.1), 리뷰 규칙 |
| R4 | **크로스-모듈 트랜잭션 남용** → 사실상 모놀리스 결합 | 전환성 | §13.4 규율, 잦으면 경계 재설계 |
| R5 | **인프로세스 이벤트 유실** — 재시작 시 인메모리 큐 손실 | 정합성 | 영속 필요 시 DB 아웃박스 테이블(§9.5) |
| R6 | **인스턴스 간 이벤트 안 감** — 복제 시 버스는 인스턴스 로컬 | 확장 | 그 시점이 브로커/MSA 도입 신호(§19.3) |
| R7 | **버전 종속(Bun)** | 기동 실패 | 능력 매트릭스 확인(MSA 문서 §3) |
| R8 | **"잠깐 패키지 하나만" 유혹** | 제로 디펜 붕괴 | CI에서 dependencies 비어있음 검사(MSA 문서 §22.1) |

### 23.1 경계 건강도 CI 게이트 (권장)

```bash
bun tools/check-boundaries.ts      # §10.3 — 모듈 내부 직접 import 차단
bun -e '/* dependencies 0 검사 (MSA 문서 §22.1) */'
bunx tsc --noEmit                  # 계약 타입 은닉이 지켜지는지
bun test                           # 모듈 단위 + e2e
```

---

## 24. 결론 · 선택 가이드

### 24.1 모듈러 모놀리스로 가라 — 이럴 때

- 팀·트래픽이 중소 규모이고 운영 인력이 적다 (폐쇄망은 특히)
- **강한 데이터 정합성**(여러 도메인 원자적 변경)이 필요하다 → §13
- 도메인 경계가 아직 확정적이지 않다 (리팩터로 고치고 싶다)
- 반입·배포·디버깅을 **하나의 아티팩트**로 단순화하고 싶다
- "나중에 MSA 가능성"은 열어두되 지금 그 복잡도는 사기 싫다

→ **하나의 `Bun.serve` + `modules/*` + 계약 + 이벤트 버스 + 단일 DB(트랜잭션)**

### 24.2 MSA를 (부분이라도) 고려하라 — 이럴 때

- 특정 모듈만 **독립 스케일**이 절실
- **팀 조직**이 갈려 배포 주기가 충돌
- 한 모듈 장애의 **프로세스 격리**가 필수
→ §20의 이음새를 따라 **그 모듈만** 뽑아라. 전체를 MSA로 만들 필요는 없다.

### 24.3 착수 전 확정할 결정

1. **바운디드 컨텍스트** — 초기 모듈을 2~6개로. 무엇이 한 모듈인가?
2. **모듈 간 통신 기본값** — 질의는 동기 계약, 통지는 이벤트. 팀 규칙으로 고정.
3. **트랜잭션 경계** — 어떤 유스케이스가 크로스-모듈 원자성이 필요한가? (그게 많으면 경계 재검토)
4. **DB 소유권 표기** — 테이블 prefix / 스키마 분리 정책
5. **전환 열어둘까** — §3 세 원칙(특히 계약은 Promise·직렬화 데이터)을 초기부터 지킬지

---

## 부록 A — MSA ↔ 모듈러 모놀리스 기능 대조표

| 관심사 | MSA (`bun-native-msa.md`) | 모듈러 모놀리스 (이 문서) |
|---|---|---|
| 실행 단위 | 프로세스 N개 | **프로세스 1개** |
| 경계 | 네트워크(HTTP/WS) | **계약 인터페이스 + 은닉**(§6,10) |
| 동기 통신 | `fetch` | **함수 호출(계약 경유)**(§8) |
| 비동기/이벤트 | 브로커(Kafka/NATS) | **인프로세스 EventBus**(§9) |
| 실시간 | 전용 WS 서비스 | **같은 프로세스 WS + 버스**(§15) |
| 라우팅 통합 | 게이트웨이 프록시 | **mergeRoutes 함수**(§11) |
| 횡단 관심사 | `shared/`(§10) | **동일 `shared/`**(§12) |
| 트랜잭션 | 사가/아웃박스 | **로컬 ACID**(§13) ★ |
| DB | 서비스별 DB | 단일 DB + 소유권(§14) |
| 관측 | 분산 추적 | 단일 스택 로그(§17) |
| 배포 | 이미지 N + compose | **아티팩트 1개**(§19) |
| 스케일 | 서비스별 | 전체 복제 / 부분 전환(§19.3,20) |
| 장애 격리 | 프로세스 | 모듈(제한적)(§9.4,23) |
| 폐쇄망 반입 | 다수 | **최소** |
| 상호 전환 | ← §20 이음새 → | ← §20 이음새 → |

## 부록 B — 모듈 추가 체크리스트

- [ ] `modules/<name>/` 생성: `contract.ts` `index.ts` `service.ts` `store.ts` `routes.ts` `controller.ts` `dto.ts` `events.ts`
- [ ] `contract.ts`에 공개 인터페이스만 (전부 `Promise`, 직렬화 데이터)
- [ ] `index.ts`의 `create<Name>Module()`이 **계약 타입으로 좁혀** 반환 (service/store 노출 금지)
- [ ] 소유 테이블만 이 모듈 store에서 생성/접근 (§14)
- [ ] 다른 모듈은 **주입된 계약**으로만 호출 (직접 import 금지)
- [ ] 발행/구독 이벤트를 `events.ts`에 타입 선언, 순환은 이벤트로 끊기(§10.4)
- [ ] `routes`를 `endpoint({...})`로 조립(검증·Guard·Interceptor·필터)
- [ ] `app/main.ts` 조립 루트에 등록 + `mergeRoutes`
- [ ] 크로스-모듈 트랜잭션이 필요하면 포트를 계약에 명시(§13.4)
- [ ] 모듈 단위 + e2e 테스트(§18)
- [ ] `bun tools/check-boundaries.ts` 통과(§10.3)

## 부록 C — "모놀리스가 진흙탕이 되는" 징후와 처방

| 징후 | 진단 | 처방 |
|---|---|---|
| import 그래프가 스파게티 | 경계 침식 | 검사기 CI 게이트화(§10.3), 계약으로 되돌리기 |
| 크로스-모듈 트랜잭션이 도처에 | 경계 오설정 | 두 모듈 병합 or 재분할(§13.4) |
| 한 모듈 바꾸면 전부 깨짐 | 계약 아닌 내부 결합 | 계약 안정화, 내부 은닉 강화(§6) |
| 이벤트가 너무 많아 흐름 추적 불가 | 이벤트 남용 | 질의는 동기 계약으로 되돌림(§9.4) |
| 빌드/테스트가 느려짐 | 모듈 과다·결합 | 모듈 수 축소, 병렬 테스트 |
| 배포 때마다 전 모듈 리스크 | 이게 진짜 아프면 | §20으로 핫 모듈만 분리 |

---

**요약:** 모듈러 모놀리스는 "경계를 포기한 모놀리스"가 아니다. **한 프로세스 안에서 계약·이벤트 버스·의존 규칙으로 MSA에 준하는 경계**를 세우고, **로컬 ACID 트랜잭션**이라는 MSA가 갖기 힘든 강점까지 얻는다. 실행은 `Bun.serve` 하나, 모듈 간은 **계약 함수 호출 + 인프로세스 이벤트**, 횡단관심사는 MSA와 **같은 `shared/`**로 자급한다. 그리고 계약을 이음새로 지켜 두면, 정말 필요한 날 **어댑터만 교체해 MSA로** 걸어 나갈 수 있다 — 폐쇄망 제로 디펜던시에서 가장 합리적인 출발점이자, 대개는 종착점이다.
