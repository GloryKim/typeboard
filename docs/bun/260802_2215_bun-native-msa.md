# 폐쇄망에서 Bun 내장만으로 NestJS 레벨 TS 서버 구축 — 기술 보고서

> **문서 성격** — 설계·의사결정 보고서 (아키텍처 + 구현 레퍼런스)
> **대상 독자** — 폐쇄망(air-gapped)에 TS 백엔드를 올려야 하는 서버 엔지니어 / 아키텍트
> **핵심 제약** — ① 인터넷 없음(레지스트리 접근 불가) ② Bun 바이너리는 이미 설치되어 있음 ③ **외부 npm 의존성 0** (node_modules 없이 소스만으로 동작)
> **목표** — NestJS가 주는 **경계·규약·횡단관심사(DI·검증·Guard·Interceptor·예외필터)** 수준을, **Bun 내장 API + 직접 작성한 얇은 TS**만으로 재현
> **버전 기준** — Bun 1.2.x~1.3.x 기능을 전제로 하되, *실제 설치된 버전에서 기능 확인*을 절차로 못박음(§3)

---

## 목차

- [0. 요약 (Executive Summary)](#0-요약-executive-summary)
- [1. 목적과 범위](#1-목적과-범위)
- [2. 폐쇄망 운영 전제 (가장 먼저 정리해야 할 것)](#2-폐쇄망-운영-전제-가장-먼저-정리해야-할-것)
- [3. 설치된 Bun 능력 확인 — 버전 매트릭스](#3-설치된-bun-능력-확인--버전-매트릭스)
- [4. 왜 NestJS / Express를 안 쓰는가 (그리고 무엇을 포기하는가)](#4-왜-nestjs--express를-안-쓰는가-그리고-무엇을-포기하는가)
- [5. MSA가 실제로 해결해야 하는 것](#5-msa가-실제로-해결해야-하는-것)
- [6. 전체 아키텍처](#6-전체-아키텍처)
- [7. 서비스 단위 설계 (NestJS 흉내)](#7-서비스-단위-설계-nestjs-흉내)
- [8. 외부 0 — Bun 내장 도구 맵](#8-외부-0--bun-내장-도구-맵)
- [9. 라우팅 — 내장 routes 우선 · PureRouter는 선택](#9-라우팅--내장-routes-우선--purerouter는-선택)
- [10. ★ NestJS 레벨 도달 — 기능 패리티 직접 구현](#10--nestjs-레벨-도달--기능-패리티-직접-구현)
- [11. 미들웨어 · 공통 계층](#11-미들웨어--공통-계층)
- [12. 서비스 간 통신](#12-서비스-간-통신)
- [13. 게이트웨이 (API Gateway)](#13-게이트웨이-api-gateway)
- [14. 설정 · 환경 변수 · 시크릿](#14-설정--환경-변수--시크릿)
- [15. 데이터 · DB 경계](#15-데이터--db-경계)
- [16. 헬스 · 관측 · 장애](#16-헬스--관측--장애)
- [17. 테스트 (bun test)](#17-테스트-bun-test)
- [18. 배포 · Docker · 프로세스 모델](#18-배포--docker--프로세스-모델)
- [19. 모노레포 vs 멀티레포](#19-모노레포-vs-멀티레포)
- [20. 최소 동작 예시 (User + Order + Gateway)](#20-최소-동작-예시-user--order--gateway)
- [21. 안티패턴 · 함정](#21-안티패턴--함정)
- [22. 리스크 · 완화 (보고서)](#22-리스크--완화-보고서)
- [23. 결론 · 남은 결정](#23-결론--남은-결정)
- [부록 A — Express / Nest / Bun 제로 대조](#부록-a--express--nest--bun-제로-대조)
- [부록 B — 서비스 추가 체크리스트](#부록-b--서비스-추가-체크리스트)
- [부록 C — 폐쇄망 반입 체크리스트](#부록-c--폐쇄망-반입-체크리스트)

---

## 0. 요약 (Executive Summary)

**결론: 가능하다. 그리고 폐쇄망에서는 오히려 이 방식이 정석에 가깝다.**

- 인터넷이 없으면 `bun add`가 애초에 안 된다. 즉 **"디펜던시 0"는 우리가 지키는 규율이 아니라 환경이 강제하는 사실**이다. 이 문서는 그 제약을 **약점이 아니라 설계 기준**으로 삼는다.
- NestJS/Express가 주는 실질 가치는 "런타임 마법"이 아니라 **코드가 커져도 무너지지 않는 규약(모듈 경계·DI·검증·Guard·Interceptor·예외필터)**이다. 이 규약은 **프레임워크 없이도 얇은 TS로 재현**할 수 있고, 그 구현체를 §10에 전부 제시한다.
- 실행 엔진은 전부 **`Bun.serve`** 하나로 수렴한다. 라우팅은 **내장 `routes`**(Bun ≥ 1.2.3), 서비스 간 통신은 **내장 `fetch`**, 실시간은 **내장 WebSocket**, DB는 **`bun:sqlite` / `Bun.sql`**, 해시는 **`Bun.password`**, 설정은 **`Bun.env`(+ `.env` 자동 로드)** — 전부 바이너리에 포함되어 있어 반입할 패키지가 없다.

| 질문 | 답 |
|---|---|
| Express 대신? | **`Bun.serve` + 내장 `routes`** (Bun **≥ 1.2.3**) |
| NestJS 대신? | **폴더/모듈 규약 + §10의 경량 DI·검증·Guard·Interceptor·예외필터** (데코레이터·프레임워크 런타임 없음) |
| HTTP 클라이언트? | **내장 `fetch`** (axios 불필요) |
| 실시간? | **`Bun.serve` WebSocket** (socket.io 불필요) |
| `.env`? | **`Bun.env` / `process.env`** (`.env` 자동 로드 → dotenv 불필요) |
| 검증(ValidationPipe)? | **직접 작성한 스키마 검증기** (§10.3) |
| DI 컨테이너? | **경량 토큰 기반 컨테이너 직접 구현** (§10.2) — 필요할 때만 |
| DB 드라이버? | **`bun:sqlite`(완전 내장) / `Bun.sql`(Postgres·MySQL 내장 클라)** — 외부 `pg`/`mysql2` 없이 |
| 가장 큰 리스크 | **버전 종속** — 폐쇄망은 업그레이드가 비싸다. 반입 시점의 Bun 버전에 기능이 묶인다(§3, §22) |

**한 줄 권고:**
"프레임워크를 반입하려 하지 말고, **서비스마다 `Bun.serve({ routes })` 하나**로 시작하라. NestJS 레벨이 필요한 부분(DI·검증·Guard·Interceptor·예외필터)만 **§10의 얇은 유틸을 골라 붙여라.** 두껍게 Nest를 재발명하지 마라."

---

## 1. 목적과 범위

### 1.1 이 문서가 답하는 질문

1. **인터넷이 없는데** TS 서버를 어떻게 올리나? (→ §2 반입·설치 전제)
2. **외부 라이브러리 없이** Express 수준의 웹서버를 어떻게 짜나? (→ §8, §9)
3. 더 나아가 **NestJS 수준의 구조/횡단관심사**(DI·검증·Guard·Interceptor·예외필터·라이프사이클)를 어떻게 자급하나? (→ §10)
4. 그걸 **MSA**로 어떻게 나누고 잇나? (→ §5~§13)

### 1.2 다루는 것 / 다루지 않는 것

| 다룬다 | 다루지 않는다 |
|---|---|
| Bun 내장 API만으로의 서버·라우팅·통신·DB | 특정 오케스트레이터(K8s) 상세 운영 |
| NestJS 핵심 개념의 무의존 재현 | ORM 재발명 (SQL 직접 사용 권장) |
| 폐쇄망 반입·버전 고정 절차 | OpenTelemetry/APM SDK 연동 (패키지 필요) |
| 경량 DI·검증·Guard·Interceptor·예외필터 코드 | 메시지 브로커(Kafka/NATS) 클라이언트 |

> **핵심 구분:** "**라이브러리 0**"과 "**코드 공유 0**"은 다르다. 라우터·검증기·에러 포맷 같은 공통 코드는 **레포 안 `shared/` 폴더로 공유**한다 — 이건 외부 패키지가 아니다.

---

## 2. 폐쇄망 운영 전제 (가장 먼저 정리해야 할 것)

> 이 섹션이 이 문서의 진짜 출발점이다. 코드보다 **반입·버전·검증 절차**를 먼저 못박아야 한다. 폐쇄망에서 가장 비싼 실수는 "개발 PC(인터넷 O)에서 되던 게 운영망(인터넷 X)에서 안 되는 것"이다.

### 2.1 "디펜던시 0"는 규율이 아니라 사실이다

```text
인터넷 있음:  bun add express   → registry.npmjs.org 에서 다운로드 → node_modules
인터넷 없음:  bun add express   → 네트워크 실패. 애초에 불가능.
```

따라서 폐쇄망에서 선택지는 사실상 둘뿐이다:

1. **패키지를 오프라인으로 반입**한다 (사설 레지스트리 미러 / `node_modules` 통째 tar / `bun install --offline` + 로컬 캐시). → 운영 부담·보안 스캔·버전 표류.
2. **처음부터 외부 패키지를 안 쓴다** (이 문서). → 반입 대상이 **Bun 바이너리 + 우리 소스**로 끝난다.

이 문서는 (2)를 택한다. 그 결과 **반입 산출물이 극단적으로 단순**해진다.

### 2.2 반입 대상 목록 (이게 전부다)

| 반입물 | 형태 | 비고 |
|---|---|---|
| Bun 바이너리 | 단일 실행 파일 (OS/arch 일치) | **버전 고정**. §3에서 능력 확인 |
| 애플리케이션 소스 | `.ts` 파일들 + `shared/` | node_modules **없음** |
| (선택) 타입체크용 `typescript` | 오프라인 tarball | *런타임엔 불필요*, CI 타입체크만 |
| (선택) 베이스 이미지 | `oven/bun:1` 등을 사내 레지스트리에 미러 | Docker 쓸 때 |

> **원칙:** 런타임에 필요한 건 `bun` + `.ts`뿐이다. `bun`은 `.ts`를 별도 빌드 없이 실행한다(트랜스파일 내장). `tsc`/`ts-node`/번들러를 반입할 이유가 없다. 단, **타입 체크**(`tsc --noEmit`)를 CI 게이트로 쓰고 싶으면 `typescript`만 오프라인 반입한다(런타임 의존 아님).

### 2.3 Bun 반입·설치 확인

Bun은 설치되어 있다고 했으니, 반입 후 **가장 먼저 할 일은 버전·능력 확인**이다.

```bash
bun --version          # 예: 1.2.19 — 이 숫자가 기능 가용성을 결정한다 (§3)
bun --revision         # 정확한 빌드
which bun && file $(which bun)
```

### 2.4 소스 반입·전달 절차 (권장)

```text
[개발망: 인터넷 O]
  1) 외부 패키지 0 확인:  package.json 의 dependencies/devDependencies 비었는지
  2) 타입 체크 통과:      bunx tsc --noEmit   (또는 반입한 tsc)
  3) 테스트 통과:         bun test
  4) 산출물 묶기:         소스 디렉터리 tar + 체크섬(sha256)

        │  물리 매체 / 단방향 게이트웨이 / 승인된 전송로
        ▼
[운영망: 인터넷 X]
  5) 체크섬 검증 후 해제
  6) bun --version 으로 §3 능력 재확인
  7) bun <service>/main.ts 기동 (또는 compile 바이너리)
```

### 2.5 런타임 자체에 대한 보안·네트워크 검토

폐쇄망은 "아웃바운드가 없어야 정상"이다. 반입 전에 확인:

- **텔레메트리/자동 업데이트 차단** — Bun은 명시 없이 코드를 phone-home 하지 않지만, `bun upgrade`는 인터넷을 탄다. 운영 스크립트/이미지에서 자동 업그레이드 경로를 제거한다.
- **아웃바운드 없음 검증** — 기동 후 실제로 외부로 나가는 소켓이 없는지 확인(방화벽/`lsof`/감사 로그). 우리 코드의 `fetch`는 **내부 서비스 URL만** 향하게 설계(§12).
- **`.env`/시크릿은 이미지에 굽지 않는다** — 런타임 주입(§14).

### 2.6 폐쇄망이 설계에 주는 실제 제약 (요약)

| 제약 | 설계 반영 |
|---|---|
| `bun add` 불가 | 외부 패키지 0 (강제) → §8 내장 맵으로 자급 |
| 업그레이드 비쌈 | **버전 고정 + 능력 매트릭스 확인**(§3), 최신 기능에 과의존 금지 |
| 외부 서비스 없음 | 관측은 **stdout JSON + 내부 수집**(§16), APM SDK 배제 |
| 외부 CA/시각 동기 제한 가능 | 내부 TLS·시각 동기 정책을 인프라와 합의 |
| 반입 오버헤드 | 산출물을 **바이너리+소스**로 최소화, 체크섬 필수 |

---

## 3. 설치된 Bun 능력 확인 — 버전 매트릭스

> 폐쇄망에서 **가장 흔한 사고**: "문서엔 되는데 우리 Bun에선 없는 기능." 업그레이드가 비싼 환경이므로, **설치된 버전에서 기능 존재를 코드로 확인**하는 절차를 표준화한다.

### 3.1 기능별 최소 버전 (대략치 — 반드시 실측으로 확정)

| 기능 | 대략 도입 | 이 문서에서의 역할 | 없을 때 대안 |
|---|---|---|---|
| `Bun.serve` | 오래됨(안정) | 모든 서비스의 실행 엔진 | (필수) |
| `Bun.serve({ routes })` (`:param`·메서드별·`*`) | **1.2.3** | 라우팅 기본값 | §9.2 PureRouter |
| `bun:sqlite` | 오래됨(안정) | 서비스별 내장 DB | 파일 JSON(데모만) |
| `Bun.password` | 오래됨 | 비밀번호 해시 | (없으면 재검토) |
| `.env` 자동 로드 / `Bun.env` | 오래됨 | 설정 | 수동 파싱 |
| WebSocket (`websocket`) | 오래됨 | 실시간/이벤트 | (필수 시 확인) |
| `Bun.sql` (Postgres) | ~1.2 | 본격 RDB | `bun:sqlite`로 후퇴 |
| `Bun.sql` (MySQL/MariaDB) | ~1.3 | 본격 RDB | 위와 동일 |
| `Bun.redis` | ~1.2.9+ | 캐시/락 | 인메모리/DB로 후퇴 |
| `Bun.YAML` | ~1.2.x | YAML 설정 | JSON 설정 |
| `bun build --compile` | 안정 | 단일 바이너리 배포 | 소스+런타임 |
| `bun test` | 안정 | 테스트 | (필수 권장) |

> 위 "대략 도입" 열은 **참고용**이다. 폐쇄망에서는 인터넷으로 릴리스 노트를 못 볼 수 있으니, **아래 3.2의 자가진단을 실제 실행**해 가용 기능을 확정하는 것을 절차로 삼는다.

### 3.2 능력 자가진단 스니펫 (기동 전 1회)

```ts
// tools/capability-check.ts  —  bun tools/capability-check.ts
const report: Record<string, boolean> = {};

// routes 지원 여부: 실제로 라우트 서버를 잠깐 띄워 확인하는 대신,
// 버전 파싱으로 1차 판단하고, 애매하면 try/catch로 실측한다.
const [maj, min, pat] = Bun.version.split(".").map(Number);
const gte = (a: number, b: number, c: number) =>
  maj > a || (maj === a && (min > b || (min === b && pat >= c)));

report["routes(>=1.2.3)"] = gte(1, 2, 3);
report["Bun.sql"] = typeof (Bun as any).sql !== "undefined";
report["Bun.redis"] = typeof (Bun as any).redis !== "undefined";
report["Bun.YAML"] = typeof (Bun as any).YAML !== "undefined";
report["Bun.password"] = typeof Bun.password?.hash === "function";

// bun:sqlite 는 import 가능 여부로 확인
try {
  await import("bun:sqlite");
  report["bun:sqlite"] = true;
} catch {
  report["bun:sqlite"] = false;
}

console.log(JSON.stringify({ bunVersion: Bun.version, report }, null, 2));

// 필수 기능이 없으면 기동 자체를 막는다 (폐쇄망 fail-fast)
const required = ["Bun.password", "bun:sqlite"];
const missing = required.filter((k) => !report[k]);
if (missing.length) {
  console.error("REQUIRED CAPABILITIES MISSING:", missing);
  process.exit(1);
}
```

### 3.3 degrade 전략 (버전이 낮을 때)

```text
routes 없음(1.2.3 미만)  → §9.2 PureRouter 로 라우팅 (정규식 한 겹)
Bun.sql 없음            → bun:sqlite 파일 DB 로 시작, 나중에 이관
Bun.redis 없음          → 인메모리 Map + TTL, 또는 sqlite 캐시 테이블
Bun.YAML 없음           → 설정을 JSON 으로 (JSON.parse)
```

**규칙:** "없는 기능을 억지로 재현"하려고 외부 패키지를 반입하는 순간 §2의 전제가 깨진다. **내장 대체 → 후퇴 옵션**만 쓴다.

---

## 4. 왜 NestJS / Express를 안 쓰는가 (그리고 무엇을 포기하는가)

### 4.1 Express → 키우다 Nest가 나온 이유

| Express가 주는 것 | Nest가 메운 구멍 |
|---|---|
| `app.get/post` 라우팅 | 모듈·컨트롤러·프로바이더 **경계** |
| 미들웨어 체인 | DI, Guard, Pipe, Interceptor **규칙** |
| 빠른 프로토타입 | 팀 규모에서 **폴더가 안 무너지게** |

즉 Nest의 가치는 "기술이 마법"이 아니라 **코드가 커져도 길을 잃지 않는 규약**이다. **그 규약만 가져오면** 프레임워크 런타임은 필요 없다 — 그리고 폐쇄망에선 애초에 그 런타임을 반입하기도 싫다.

### 4.2 제로 디펜던시로 가면 얻는 것

```text
Express + Nest + axios + dotenv + socket.io + class-validator + typeorm ...
  → node_modules 수백 MB~, 취약점 스캔 대상 폭증, 폐쇄망 반입 지옥

Bun.serve + routes + fetch + Bun.env + bun:sqlite (+ §10의 얇은 유틸)
  → bun 런타임 + 우리 소스. 앱 의존성 레이어 없음.
```

| 이득 | 설명 |
|---|---|
| 반입물 최소 | 컨테이너/서버에 `bun` + `.ts`뿐. npm 트리·취약점 스캔 대상 없음 |
| 폐쇄망 친화 | `bun add` 불필요 → §2가 단순해짐 |
| 콜드 스타트 | 프레임워크 부트 스택 없음 |
| 성능 상한 | `Bun.serve`가 Express 미들웨어 스택보다 유리한 경우가 많음 |
| 공급망 리스크↓ | 우리가 안 짠 코드가 프로세스 안에 거의 없음 |

### 4.3 의식적으로 버리는 것 (그리고 §10에서 되찾는 것)

| Nest 기능 | 제로 디펜에서 | 되찾는 곳 |
|---|---|---|
| 데코레이터 라우팅 | `routes` 객체 / 수동 등록 | §9 |
| DI 컨테이너 | 생성자 주입 or 경량 컨테이너 | **§10.2** |
| ValidationPipe | 직접 스키마 검증 | **§10.3** |
| Guards | 핸들러 앞단 함수 | **§10.4** |
| Interceptors | 고차 함수 래퍼 | **§10.5** |
| Exception Filter | 전역 에러 → Response 매퍼 | **§10.6** |
| Lifecycle Hooks | 명시적 init/shutdown | **§10.7** |
| OpenAPI 자동 | 수동 스펙(md/JSON) | (수기) |
| ORM | SQL 직접 · `bun:sqlite` · `Bun.sql` | §15 |

> **핵심 태도:** 편의 기능은 **얇게 직접** 짠다. "미들웨어 플랫폼", "DI 프레임워크"를 두껍게 재발명하면 Nest를 다시 반입하는 것과 유지비가 같아진다.

---

## 5. MSA가 실제로 해결해야 하는 것

마이크로서비스는 "폴더를 많이 나누는 것"이 아니다. **배포·장애·스케일 경계**를 나누는 것이다.

| 과제 | 모놀리스 | Bun MSA (목표) |
|---|---|---|
| 프로세스 | 하나 | 서비스마다 하나 (`Bun.serve`) |
| 포트 | 하나 | 서비스별 포트 또는 Unix socket |
| 장애 전파 | 전체가 죽음 | 한 서비스 다운 ≠ 전체 다운 (게이트웨이만 degrade) |
| 배포 | 통째 | 서비스 단위 |
| 데이터 | 한 DB 스키마 | **서비스별 DB** (권장) |
| 통신 | 함수 호출 | **HTTP `fetch` / WebSocket / 메시지** |

### 5.1 이 문서의 MSA 최소 정의

```text
1 서비스 = 1 실행 진입점(main.ts) = 1 Bun.serve = 1 책임 도메인
서비스 간 = 네트워크 경계 (같은 머신 포트라도 "원격"으로 취급)
공유 코드 = 레포 내 shared/ 로컬 import (npm publish 없음)
```

> **폐쇄망 특이점:** 외부 서비스 디스커버리 인프라(Consul 등)를 못 반입할 수 있다. 그래서 서비스 발견은 **env 고정 URL / 컨테이너 DNS**로 단순화한다(§12.3). 이건 타협이 아니라 폐쇄망에서 더 견고한 선택이다.

---

## 6. 전체 아키텍처

### 6.1 논리 다이어그램

```text
                    ┌─────────────────────────────────────┐
                    │           Clients / BFF / UI         │
                    └─────────────────┬───────────────────┘
                                      │ HTTP
                                      ▼
                    ┌─────────────────────────────────────┐
                    │         API Gateway (Bun.serve)      │
                    │  - 라우팅 / 인증 헤더 전달            │
                    │  - 타임아웃 · 재시도(선택) · 헬스집계  │
                    └───────┬─────────────┬───────────────┘
                            │             │
               fetch HTTP   │             │  fetch HTTP
                            ▼             ▼
              ┌──────────────────┐   ┌──────────────────┐
              │  user-service    │   │  order-service   │
              │  Bun.serve:4001  │   │  Bun.serve:4002  │
              │  routes/ctrl/db  │   │  routes/ctrl/db  │
              └────────┬─────────┘   └────────┬─────────┘
                       │                      │
                       ▼                      ▼
                 user DB / sqlite        order DB / sqlite
                       ▲                      │
                       │     (필요 시) fetch  │
                       └────── user ←─────────┘
                              "주문 생성 시 유저 검증"
```

### 6.2 프로세스 배치 (로컬 개발)

```text
터미널 A  bun services/gateway/main.ts       → :4000
터미널 B  bun services/user-service/main.ts  → :4001
터미널 C  bun services/order-service/main.ts → :4002
```

프로덕션도 같은 구성을 프로세스 매니저 / Docker Compose / K8s로 올린다. **런타임은 전부 Bun, 의존성 트리는 없다.**

### 6.3 경계 규칙

| 규칙 | 내용 |
|---|---|
| 동기 기본 | 요청-응답은 **HTTP JSON** (`fetch`) |
| 실시간 | 알림·푸시·스트림은 **WebSocket** |
| 공유 금지 | 다른 서비스 DB에 직접 SQL 금지 → **API만** |
| 타임아웃 | 모든 아웃바운드 `fetch`에 `AbortSignal.timeout` |
| 헬스 | 각 서비스 `/health`, 게이트웨이는 집계 가능 |
| 폐쇄 | 아웃바운드는 **내부 서비스 URL로만** (외부 인터넷 없음) |

---

## 7. 서비스 단위 설계 (NestJS 흉내)

Nest의 Module / Controller / Service 감각을 **파일 분리**로 옮긴다. 데코레이터·DI 컨테이너는 기본적으로 없고, 필요할 때만 §10.2의 경량 컨테이너를 쓴다.

### 7.1 권장 레포 트리

```text
repo/
├── shared/                      # npm 아님 · 로컬 import만
│   ├── http.ts                  # jsonOk / jsonErr / fetchJson / HttpError
│   ├── validate.ts              # 검증기 (ValidationPipe 대체) — §10.3
│   ├── di.ts                    # 경량 DI 컨테이너 (선택) — §10.2
│   ├── pipeline.ts              # Guard/Interceptor 합성 — §10.4~10.5
│   ├── errors.ts                # 전역 예외 필터 매퍼 — §10.6
│   ├── lifecycle.ts             # graceful shutdown — §10.7
│   ├── logger.ts                # 구조화 로그 — §16
│   ├── router.ts                # (선택) PureRouter — §9.2
│   └── types.ts
├── services/
│   ├── gateway/
│   │   ├── main.ts              # Bun.serve 진입
│   │   ├── routes.ts
│   │   └── proxy.ts             # 업스트림 fetch 맵
│   ├── user-service/
│   │   ├── main.ts
│   │   ├── routes.ts
│   │   ├── controller.ts        # 요청→응답 (HTTP 입출력만)
│   │   ├── service.ts           # 비즈니스 로직
│   │   ├── dto.ts               # 입력 스키마 (검증 대상)
│   │   └── store.ts             # DB / 메모리
│   └── order-service/
│       └── ... (동일 구조)
├── tools/
│   └── capability-check.ts      # §3.2
├── docker-compose.yml           # 선택
└── README.md
```

### 7.2 Nest 개념 ↔ Bun 제로 매핑

| Nest | Bun 제로 | 구현 위치 |
|---|---|---|
| `@Module` | 서비스 폴더 + `main.ts`가 조립 | §7 |
| `@Controller` | `controller.ts`의 함수 | §20 |
| `@Injectable` Service | `service.ts`의 클래스/함수 (+ 선택적 DI) | §10.2 |
| `@Get(':id')` | `routes: { "/users/:id": { GET } }` | §9 |
| `ValidationPipe` / DTO | `validate.ts` + `dto.ts` | §10.3 |
| `@UseGuards` | 파이프라인 앞단 Guard | §10.4 |
| `@UseInterceptors` | 고차 함수 래퍼 | §10.5 |
| `ExceptionFilter` | 전역 에러 매퍼 | §10.6 |
| `OnModuleInit` / `OnModuleDestroy` | 명시적 init/shutdown 훅 | §10.7 |
| `ConfigModule` | `Bun.env` (+ `.env` 자동) | §14 |
| `HttpModule` / axios | `fetch` | §12 |
| `@WebSocketGateway` | `Bun.serve({ websocket })` | §12.2 |

### 7.3 의존 방향 (중요)

```text
main.ts
  → routes.ts (라우트 등록)
    → controller.ts (HTTP 입출력 + 검증)
      → service.ts (도메인)
        → store.ts (영속화)

❌ controller가 다른 서비스의 store를 import
✅ service가 fetch로 다른 서비스 HTTP 호출
```

같은 레포여도 **프로세스 밖 서비스를 파일로 직접 import하지 않는다.** (`order-service`가 `user-service/store`를 import하면 MSA가 모놀리스로 붕괴한다.)

### 7.4 한 서비스 `main.ts` 골격

```ts
// services/user-service/main.ts
import * as ctrl from "./controller";
import { onShutdown } from "../../shared/lifecycle";
import { closeDb } from "./store";

const port = Number(Bun.env.PORT ?? 4001);

const server = Bun.serve({
  port,
  hostname: "0.0.0.0",
  routes: {
    "/health": { GET: () => Response.json({ ok: true, service: "user" }) },
    "/users/:id": { GET: (req) => ctrl.getUser(req, req.params) },
    "/users": { POST: (req) => ctrl.createUser(req) },
  },
  fetch() {
    return Response.json({ error: "not_found" }, { status: 404 });
  },
});

console.log(`[user-service] :${port}`);

// Nest의 OnModuleDestroy 대체 (§10.7)
onShutdown(async () => {
  server.stop();       // 새 연결 차단
  closeDb();           // 리소스 정리
  console.log("[user-service] stopped");
});
```

서비스 부트는 이 정도로 끝이다. Nest `NestFactory.create`에 해당하는 게 `Bun.serve` 한 블록이다.

---

## 8. 외부 0 — Bun 내장 도구 맵

MSA에서 자주 쓰는 기능을 **무엇으로 대체할지**만 고정한다. (가용성은 §3에서 실측)

| 필요 | 예전에 쓰던 것 (반입 불가) | Bun 내장 (반입 불필요) |
|---|---|---|
| HTTP 서버 | Express / Fastify / Nest | **`Bun.serve`** |
| 라우팅 | express.Router | **`Bun.serve({ routes })`** (≥1.2.3) · §9.2 PureRouter |
| HTTP 클라 | axios / got | **`fetch`** |
| WebSocket | socket.io / ws | **`Bun.serve` `websocket`** |
| env | dotenv | **`Bun.env`** (+ `.env` 자동 로드) |
| JSON 파싱 | body-parser | **`req.json()` / `Response.json`** |
| multipart | multer | **`req.formData()`** |
| 파일 | fs + 라이브러리 | **`Bun.file` / `Bun.write`** |
| 비밀번호 해시 | bcrypt | **`Bun.password`** |
| SQLite | better-sqlite3 | **`bun:sqlite`** |
| Postgres/MySQL | pg / mysql2 / prisma | **`Bun.sql`** (Postgres ~1.2, MySQL ~1.3) |
| Redis | ioredis | **`Bun.redis`** (~1.2.9+ · Cluster 등 제한) |
| YAML 설정 | js-yaml | **`Bun.YAML`** |
| UUID | uuid | **`crypto.randomUUID()`** |
| 해시/서명 | crypto libs | **WebCrypto `crypto.subtle`** |
| 검증 | class-validator / zod | **직접 작성 (§10.3)** |
| 테스트 | jest | **`bun test`** |
| 단일 바이너리 | pkg / esbuild | **`bun build --compile`** |

**폐쇄망에서 없어도 버티는 것:** ORM, DI 프레임워크, API 문서 생성기, 메시지 브로커 클라이언트(초기).
**초기에 반드시 정할 것:** DB 종류(§15), 서비스 발견 방식(§12.3), 반입 시점 Bun 버전(§3).

---

## 9. 라우팅 — 내장 `routes` 우선 · PureRouter는 선택

**틀린 전제(구버전 자료):** "`Bun.serve`엔 Express식 라우팅이 없다 → 무조건 직접 짜야 한다."
**현재(Bun ≥ 1.2.3):** `routes` 옵션으로 `:id` 파라미터·메서드별 핸들러·와일드카드가 **내장**이다. MSA 서비스의 기본값은 이쪽이다.

### 9.1 권장 — `Bun.serve({ routes })`

```ts
Bun.serve({
  port: 4001,
  routes: {
    "/health": { GET: () => Response.json({ ok: true, service: "user" }) },
    "/users/:id": {
      GET: (req) => Response.json({ userId: req.params.id }),
    },
    "/users": {
      POST: async (req) => {
        const body = await req.json();
        return Response.json({ status: "created", data: body }, { status: 201 });
      },
    },
    "/files/*": (req) => new Response(`file: ${req.params["*"]}`),
  },
  fetch() {
    return Response.json({ error: "not_found" }, { status: 404 });
  },
});
```

| 기능 | 내장 `routes` |
|---|---|
| `:id` 파라미터 | ✅ `req.params.id` |
| 메서드별 핸들러 | ✅ `{ GET, POST, ... }` |
| 와일드카드 | ✅ `/files/*` |
| 미매칭 fallback | ✅ `fetch` |
| Express식 미들웨어 체인 | ❌ → §10~11 래퍼로 얇게 |

라우트를 파일로 나누고 싶으면 `routes.ts`에서 객체를 export해 `main.ts`에 넘긴다. **별도 라우터 클래스는 필수가 아니다.**

### 9.2 선택 — PureRouter (커스텀 체인·구버전)

Bun 1.2.3 미만이거나, 라우터 한곳에서 `compose([...])` 체인을 돌리고 싶을 때의 **옵션**이다.

```ts
// shared/router.ts
import { HttpError } from "./http";

type Handler = (req: Request, params: Record<string, string>) => Response | Promise<Response>;
type Route = { method: string; regex: RegExp; keys: string[]; handler: Handler };

export class PureRouter {
  private routes: Route[] = [];

  add(method: string, path: string, handler: Handler) {
    const keys: string[] = [];
    const regexPath = path.replace(/:([^/]+)/g, (_, key) => {
      keys.push(key);
      return "([^/]+)";
    });
    this.routes.push({ method: method.toUpperCase(), regex: new RegExp(`^${regexPath}$`), keys, handler });
  }
  get(p: string, h: Handler) { this.add("GET", p, h); }
  post(p: string, h: Handler) { this.add("POST", p, h); }
  put(p: string, h: Handler) { this.add("PUT", p, h); }
  delete(p: string, h: Handler) { this.add("DELETE", p, h); }

  async resolve(req: Request): Promise<Response> {
    const url = new URL(req.url);
    const method = req.method.toUpperCase();
    const route = this.routes.find((r) => r.method === method && r.regex.test(url.pathname));
    if (!route) return Response.json({ error: "not_found" }, { status: 404 });

    const matches = url.pathname.match(route.regex);
    const params: Record<string, string> = {};
    route.keys.forEach((key, i) => { params[key] = decodeURIComponent(matches![i + 1]!); });

    try {
      return await route.handler(req, params);
    } catch (err) {
      if (err instanceof HttpError) {
        return Response.json({ error: err.code, detail: err.detail }, { status: err.status });
      }
      console.error("[router]", err);
      return Response.json({ error: "internal" }, { status: 500 });
    }
  }
}
```

**하지 말 것:** path-to-regexp, koa-router 등 재도입(폐쇄망에선 애초에 반입도 안 됨). 내장 `routes`로 부족할 때만 정규식 한 겹이면 충분하다.

---

## 10. ★ NestJS 레벨 도달 — 기능 패리티 직접 구현

> 이 섹션이 "**express의 msa 용도인 nestjs 레벨**"이라는 요청의 핵심이다. NestJS의 진짜 가치인 **DI · 검증(Pipe) · Guard · Interceptor · 예외필터 · 라이프사이클**을, 외부 패키지 0으로 재현한다. **전부 필요한 게 아니다 — §10.8 도입 판단표를 보고 골라 쓴다.**

### 10.0 패리티 개요 — 무엇을 얼마나 만들 것인가

| Nest 기능 | 무의존 구현 난이도 | 언제 필요 | 이 문서의 구현 |
|---|---|---|---|
| Controller/Service 분리 | 매우 낮음 | 항상 | 파일 분리(§7) |
| **검증(ValidationPipe)** | 낮음 | **거의 항상** | §10.3 |
| **Guard(인증/인가)** | 낮음 | 보호 API 있으면 | §10.4 |
| **Interceptor(로깅/타이밍)** | 낮음 | 관측 필요 시 | §10.5 |
| **Exception Filter** | 낮음 | **거의 항상** | §10.6 |
| **Lifecycle(graceful)** | 낮음 | 배포/DB 쓰면 | §10.7 |
| **DI 컨테이너** | 중간 | 규모 커질 때만 | §10.2 |
| 데코레이터 메타 | 중간~높음 | 대개 불필요 | §10.8(비권장) |

> 우선순위: **검증·예외필터·라이프사이클**은 초기에 넣어라(작고 이득 큼). **DI 컨테이너**는 서비스가 커지고 생성자 배선이 아파질 때 넣어라. **데코레이터 기반 메타프로그래밍**은 대개 넣지 마라(유지비가 이득을 넘는다).

### 10.1 가장 단순한 형태 — DI 없이 생성자 주입

작은 서비스라면 컨테이너 없이 **명시적 생성자 주입**이 가장 명확하다.

```ts
// services/user-service/wire.ts  —  "조립 루트"(Composition Root)
import { UserStore } from "./store";
import { UserService } from "./service";
import { UserController } from "./controller";

const store = new UserStore(Bun.env.DATABASE_PATH ?? "user.db");
const service = new UserService(store);
export const userController = new UserController(service);
```

이 방식이 **NestJS DI의 90%를 대체**한다. 의존이 명시적이고 테스트에서 mock 주입이 쉽다. **여기서 배선이 지겨워질 때만** §10.2로 간다.

### 10.2 경량 DI 컨테이너 (필요할 때만)

토큰 기반, 싱글턴/팩토리 지원, 순환참조 감지. Reflect-metadata·데코레이터 없이.

```ts
// shared/di.ts
type Token<T> = symbol & { __t?: T };
export const token = <T>(name: string): Token<T> => Symbol(name) as Token<T>;

type Provider<T> = { factory: (c: Container) => T; singleton?: boolean };

export class Container {
  private providers = new Map<symbol, Provider<any>>();
  private singletons = new Map<symbol, any>();
  private resolving = new Set<symbol>();

  register<T>(t: Token<T>, factory: (c: Container) => T, opts: { singleton?: boolean } = {}) {
    this.providers.set(t, { factory, singleton: opts.singleton ?? true });
    return this;
  }

  resolve<T>(t: Token<T>): T {
    if (this.singletons.has(t)) return this.singletons.get(t);
    const p = this.providers.get(t);
    if (!p) throw new Error(`No provider for ${String(t)}`);
    if (this.resolving.has(t)) throw new Error(`Circular dependency: ${String(t)}`);

    this.resolving.add(t);
    const instance = p.factory(this);
    this.resolving.delete(t);

    if (p.singleton) this.singletons.set(t, instance);
    return instance;
  }
}
```

```ts
// tokens.ts
import { token } from "../../shared/di";
import type { UserStore } from "./store";
import type { UserService } from "./service";

export const TOKENS = {
  UserStore: token<UserStore>("UserStore"),
  UserService: token<UserService>("UserService"),
};

// wire.ts
import { Container } from "../../shared/di";
import { TOKENS } from "./tokens";
import { UserStore } from "./store";
import { UserService } from "./service";

export const container = new Container()
  .register(TOKENS.UserStore, () => new UserStore(Bun.env.DATABASE_PATH ?? "user.db"))
  .register(TOKENS.UserService, (c) => new UserService(c.resolve(TOKENS.UserStore)));

// 사용
const svc = container.resolve(TOKENS.UserService);
```

> **경고:** 이 30줄이 "NestJS 레벨 DI"의 실용 상한이다. `@Injectable` 자동 스캔·스코프(REQUEST/TRANSIENT)·모듈 그래프까지 재현하려 들지 마라 — 그 순간 유지비가 Nest 반입보다 커진다.

### 10.3 검증 — ValidationPipe / DTO 대체

class-validator 없이, 순수 함수 검증기. DTO는 그냥 타입 + 검증 스키마다.

```ts
// shared/validate.ts
import { HttpError } from "./http";

export type Rule<T> = (v: unknown, field: string) => T;

export const v = {
  string(opts: { min?: number; max?: number; pattern?: RegExp } = {}): Rule<string> {
    return (raw, field) => {
      if (typeof raw !== "string") throw new HttpError(400, "validation", { field, msg: "must be string" });
      if (opts.min != null && raw.length < opts.min) throw new HttpError(400, "validation", { field, msg: `min ${opts.min}` });
      if (opts.max != null && raw.length > opts.max) throw new HttpError(400, "validation", { field, msg: `max ${opts.max}` });
      if (opts.pattern && !opts.pattern.test(raw)) throw new HttpError(400, "validation", { field, msg: "pattern" });
      return raw;
    };
  },
  number(opts: { min?: number; max?: number; int?: boolean } = {}): Rule<number> {
    return (raw, field) => {
      const n = typeof raw === "string" ? Number(raw) : raw;
      if (typeof n !== "number" || Number.isNaN(n)) throw new HttpError(400, "validation", { field, msg: "must be number" });
      if (opts.int && !Number.isInteger(n)) throw new HttpError(400, "validation", { field, msg: "must be int" });
      if (opts.min != null && n < opts.min) throw new HttpError(400, "validation", { field, msg: `min ${opts.min}` });
      if (opts.max != null && n > opts.max) throw new HttpError(400, "validation", { field, msg: `max ${opts.max}` });
      return n;
    };
  },
  optional<T>(rule: Rule<T>): Rule<T | undefined> {
    return (raw, field) => (raw == null ? undefined : rule(raw, field));
  },
};

// 스키마 = 필드별 규칙 맵
export type Schema<T> = { [K in keyof T]: Rule<T[K]> };

export function validate<T>(schema: Schema<T>, input: unknown): T {
  if (typeof input !== "object" || input == null) {
    throw new HttpError(400, "validation", { msg: "body must be object" });
  }
  const out = {} as T;
  for (const key in schema) {
    out[key] = schema[key]((input as any)[key], key);
  }
  return out;
}
```

```ts
// services/user-service/dto.ts
import { v, type Schema } from "../../shared/validate";

export type CreateUserDto = { name: string; age?: number };

export const createUserSchema: Schema<CreateUserDto> = {
  name: v.string({ min: 1, max: 50 }),
  age: v.optional(v.number({ int: true, min: 0, max: 150 })),
};

// controller.ts 에서
// const dto = validate(createUserSchema, await req.json());
```

> 이게 NestJS `class-validator + ValidationPipe`의 실무 80%다. 검증 실패는 `HttpError(400)` → §10.6 전역 필터가 일관된 JSON으로 변환한다.

### 10.4 Guard — 인증/인가

Nest `CanActivate`를 "Response를 반환하면 차단, 아무것도 안 반환하면 통과" 규칙의 함수로.

```ts
// shared/pipeline.ts
export type Ctx = { req: Request; params: Record<string, string>; state: Record<string, unknown> };
export type Guard = (ctx: Ctx) => Response | void | Promise<Response | void>;
export type Handler = (ctx: Ctx) => Response | Promise<Response>;

export function withGuards(guards: Guard[], handler: Handler): Handler {
  return async (ctx) => {
    for (const g of guards) {
      const blocked = await g(ctx);
      if (blocked) return blocked; // Response 반환 = 차단
    }
    return handler(ctx);
  };
}
```

```ts
// shared/guards.ts
import type { Guard } from "./pipeline";

export const authGuard: Guard = (ctx) => {
  const auth = ctx.req.headers.get("authorization");
  if (!auth?.startsWith("Bearer ")) {
    return Response.json({ error: "unauthorized" }, { status: 401 });
  }
  // 토큰 검증(내장 crypto.subtle 로 서명 확인 등) 후 사용자 주입
  ctx.state.userId = /* verify(token) */ "u_123";
};

export const roleGuard = (role: string): Guard => (ctx) => {
  if (ctx.state.role !== role) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
};
```

### 10.5 Interceptor — 횡단 관심사 (로깅·타이밍·응답 변형)

Nest `NestInterceptor`(전/후 감쌈)를 고차 함수로.

```ts
// shared/pipeline.ts (계속)
export type Interceptor = (ctx: Ctx, next: () => Promise<Response>) => Promise<Response>;

export function withInterceptors(interceptors: Interceptor[], handler: Handler): Handler {
  return async (ctx) => {
    let i = -1;
    const dispatch = (idx: number): Promise<Response> => {
      if (idx <= i) throw new Error("next() called twice");
      i = idx;
      const fn = interceptors[idx];
      if (!fn) return Promise.resolve(handler(ctx));
      return fn(ctx, () => dispatch(idx + 1));
    };
    return dispatch(0);
  };
}
```

```ts
// shared/interceptors.ts
import type { Interceptor } from "./pipeline";
import { log } from "./logger";

export const timing: Interceptor = async (ctx, next) => {
  const t = performance.now();
  const res = await next();
  log("info", "request", {
    method: ctx.req.method,
    path: new URL(ctx.req.url).pathname,
    status: res.status,
    ms: +(performance.now() - t).toFixed(1),
    requestId: ctx.req.headers.get("x-request-id"),
  });
  return res;
};
```

### 10.6 Exception Filter — 전역 예외 → 일관된 JSON

Nest `ExceptionFilter`를 "어떤 에러든 Response로" 매핑하는 한 함수로. **모든 핸들러를 이걸로 감싼다.**

```ts
// shared/errors.ts
import { HttpError } from "./http";
import { log } from "./logger";
import type { Handler, Ctx } from "./pipeline";

export function withErrorFilter(handler: Handler): Handler {
  return async (ctx: Ctx) => {
    try {
      return await handler(ctx);
    } catch (err) {
      if (err instanceof HttpError) {
        return Response.json({ error: err.code, detail: err.detail }, { status: err.status });
      }
      log("error", "unhandled", { msg: String(err), stack: (err as Error)?.stack });
      return Response.json({ error: "internal" }, { status: 500 });
    }
  };
}
```

### 10.7 Lifecycle — init / graceful shutdown

Nest `OnModuleInit` / `OnApplicationShutdown` 대체. 폐쇄망 무중단 배포·DB 정리에 필수.

```ts
// shared/lifecycle.ts
type Hook = () => void | Promise<void>;
const shutdownHooks: Hook[] = [];
let shuttingDown = false;

export function onShutdown(hook: Hook) {
  shutdownHooks.push(hook);
}

async function runShutdown(signal: string) {
  if (shuttingDown) return;
  shuttingDown = true;
  console.log(`[lifecycle] ${signal} → graceful shutdown`);
  for (const h of shutdownHooks.reverse()) {
    try { await h(); } catch (e) { console.error("[shutdown hook]", e); }
  }
  process.exit(0);
}

process.on("SIGTERM", () => runShutdown("SIGTERM"));
process.on("SIGINT", () => runShutdown("SIGINT"));
```

`main.ts`에서 `onShutdown(() => { server.stop(); closeDb(); })` — §7.4 참고.

### 10.8 최종 조립 — 파이프라인 한 줄

Guard → Interceptor → ErrorFilter → Handler를 조립하는 헬퍼. 내장 `routes`와 그대로 물린다.

```ts
// shared/pipeline.ts (계속)
import { withErrorFilter } from "./errors";

export function endpoint(opts: {
  guards?: Guard[];
  interceptors?: Interceptor[];
  handler: Handler;
}) {
  const { guards = [], interceptors = [], handler } = opts;
  const composed = withErrorFilter(
    withInterceptors(interceptors, withGuards(guards, handler)),
  );
  // Bun routes 시그니처(req)에 맞춰 Ctx로 변환
  return (req: Request & { params?: Record<string, string> }) =>
    composed({ req, params: req.params ?? {}, state: {} });
}
```

```ts
// routes.ts — Nest 컨트롤러 데코레이터에 대응하는 "선언"
import { endpoint } from "../../shared/pipeline";
import { authGuard } from "../../shared/guards";
import { timing } from "../../shared/interceptors";
import * as ctrl from "./controller";

export const routes = {
  "/users/:id": {
    GET: endpoint({
      guards: [authGuard],
      interceptors: [timing],
      handler: (ctx) => ctrl.getUser(ctx),
    }),
  },
  "/users": {
    POST: endpoint({
      interceptors: [timing],
      handler: (ctx) => ctrl.createUser(ctx),
    }),
  },
};
```

> **이것이 "NestJS 레벨"의 실체다.** `@UseGuards(AuthGuard) @UseInterceptors(TimingInterceptor)`가 `endpoint({ guards, interceptors })` 한 줄이 됐을 뿐, 개념·경계·테스트 용이성은 동일하다. 데코레이터/메타데이터/리플렉션은 **전혀 없다.**

### 10.9 데코레이터를 쓸 것인가? (대개 No)

Bun은 TS 데코레이터를 실행할 수 있다. 하지만 폐쇄망 무의존 프로젝트에서 데코레이터 기반 메타프로그래밍(`@Get()` 자동 수집 등)은 권장하지 않는다:

- Nest식 데코레이터는 보통 `reflect-metadata`(외부 패키지)에 기댄다 → 반입 대상 증가.
- 자체 데코레이터 프레임워크를 만들면 §4.3에서 버린 복잡도가 그대로 돌아온다.
- 위 `endpoint({...})` **명시적 선언**이 더 읽기 쉽고 디버깅·테스트가 쉽다.

**결론: 명시적 함수 조립을 기본으로. 데코레이터는 팀이 강하게 원하고 유지비를 감수할 때만.**

---

## 11. 미들웨어 · 공통 계층

§10에서 Guard/Interceptor/ErrorFilter를 정의했으니, 여기선 나머지 공통 헬퍼만.

```ts
// shared/http.ts
export function jsonOk(data: unknown, status = 200) {
  return Response.json(data, { status });
}
export function jsonErr(code: string, status: number, detail?: unknown) {
  return Response.json({ error: code, detail }, { status });
}

export class HttpError extends Error {
  constructor(public status: number, public code: string, public detail?: unknown) {
    super(code);
  }
}

export async function readJson<T>(req: Request): Promise<T> {
  try {
    return (await req.json()) as T;
  } catch {
    throw new HttpError(400, "invalid_json");
  }
}

export async function fetchJson(url: string, init: RequestInit & { timeoutMs?: number } = {}) {
  const { timeoutMs = 3000, ...rest } = init;
  const res = await fetch(url, { ...rest, signal: AbortSignal.timeout(timeoutMs) });
  const body = await res.json().catch(() => null);
  if (!res.ok) throw new HttpError(res.status, "upstream_error", body);
  return body;
}
```

```ts
// shared/logger.ts — 구조화 로그 (§16)
type Level = "debug" | "info" | "warn" | "error";
export function log(level: Level, msg: string, fields: Record<string, unknown> = {}) {
  console.log(JSON.stringify({ ts: new Date().toISOString(), level, msg, ...fields }));
}
```

> 얇게 유지한다. "미들웨어 플랫폼"을 만들면 Nest를 다시 쓰는 것과 유지비가 같아진다.

---

## 12. 서비스 간 통신

### 12.1 동기 HTTP (기본 · 추천 시작점)

```ts
const USER_BASE = Bun.env.USER_SERVICE_URL ?? "http://127.0.0.1:4001";

export async function assertUserExists(userId: string) {
  return fetchJson(`${USER_BASE}/users/${userId}`, { timeoutMs: 2000 });
}
```

| 장점 | 단점 |
|---|---|
| 디버깅 쉬움 · 도구 불필요 | 동기 결합 · 상대 다운 시 실패 |
| Bun `fetch`만으로 끝 | 트래픽 많으면 게이트웨이/캐시 필요 |

**초기 MSA는 HTTP만으로 충분**한 경우가 많다. Kafka/NATS 클라이언트는 폐쇄망 반입·"디펜던시 0"과 정면충돌한다.

### 12.2 WebSocket (실시간 · 내부 이벤트)

```ts
const server = Bun.serve({
  port: 4010,
  fetch(req, srv) {
    if (srv.upgrade(req)) return; // upgrade 성공 시 Response 반환 안 함
    return new Response("ws only", { status: 400 });
  },
  websocket: {
    open(ws) { ws.subscribe("orders"); },
    message(_ws, message) { server.publish("orders", message); },
    close(_ws) {},
  },
});
```

**요청-응답 비즈니스는 HTTP, 이벤트 팬아웃은 WS**로 역할을 나누면 단순하다.

### 12.3 서비스 디스커버리 (폐쇄망 현실)

| 단계 | 방법 | 언제 |
|---|---|---|
| 0 | **환경변수 고정 URL** `USER_SERVICE_URL=http://user:4001` | 로컬·Compose |
| 1 | **Docker/K8s DNS** 서비스 이름 (`http://user-service:4001`) | 컨테이너 오케 |
| 2 | 게이트웨이만 알고 내부는 고정 맵 | 서비스 수 적음 |
| 3 | 자체 `/registry` (비추천·복잡) | 정말 필요할 때 |

**폐쇄망 제로 디펜에서는 1단계(DNS + env)가 정답에 가깝다.** 동적 디스커버리 프레임워크는 반입도 어렵고 대개 불필요하다.

### 12.4 타임아웃 · 재시도 (최소)

```ts
export async function fetchJsonRetry(url: string, init?: RequestInit, retries = 1) {
  let last: unknown;
  for (let i = 0; i <= retries; i++) {
    try { return await fetchJson(url, { ...init, timeoutMs: 2000 }); }
    catch (e) { last = e; }
  }
  throw last;
}
```

재시도는 **멱등 GET**에만. POST 주문 생성에 무분별 재시도 금지. 서킷브레이커는 연속 실패 카운터 + 일정 시간 503 반환 정도면 초기 충분.

---

## 13. 게이트웨이 (API Gateway)

클라이언트는 **게이트웨이 한 곳**만 본다. 내부 포트는 노출하지 않는다.

### 13.1 역할

| 함 | 안 함 |
|---|---|
| 경로 → 업스트림 프록시 | 두꺼운 비즈니스 로직 |
| 인증 토큰 검증(선택) | 모든 서비스 DB 접근 |
| CORS · 요청 ID 부여 | 각 도메인 상세 validation |
| `/health` 집계 | |

### 13.2 리버스 프록시 스케치

```ts
// services/gateway/proxy.ts
const UPSTREAM: Record<string, string> = {
  "/api/users": Bun.env.USER_SERVICE_URL ?? "http://127.0.0.1:4001",
  "/api/orders": Bun.env.ORDER_SERVICE_URL ?? "http://127.0.0.1:4002",
};

export async function proxy(req: Request): Promise<Response> {
  const url = new URL(req.url);
  const entry = Object.entries(UPSTREAM).find(([prefix]) =>
    url.pathname === prefix || url.pathname.startsWith(prefix + "/"));
  if (!entry) return Response.json({ error: "not_found" }, { status: 404 });

  const [, base] = entry;
  const stripped = url.pathname.replace(/^\/api/, "") || "/";
  const target = new URL(stripped + url.search, base);

  const headers = new Headers(req.headers);
  headers.set("x-request-id", crypto.randomUUID());
  headers.delete("host");

  const init: RequestInit = { method: req.method, headers, signal: AbortSignal.timeout(5000) };
  if (req.method !== "GET" && req.method !== "HEAD") init.body = await req.arrayBuffer();

  const res = await fetch(target, init);
  return new Response(res.body, { status: res.status, headers: res.headers });
}
```

게이트웨이도 **외부 라이브러리 0**이다. nginx를 앞에 둬도 되고, Bun 게이트웨이만으로도 소규모는 충분하다.

---

## 14. 설정 · 환경 변수 · 시크릿

```ts
const port = Number(Bun.env.PORT ?? 4001);
const userUrl = Bun.env.USER_SERVICE_URL;
if (!userUrl) {
  console.error("USER_SERVICE_URL required");
  process.exit(1); // 폐쇄망 fail-fast: 잘못된 설정으로 조용히 뜨지 않기
}
```

- 프로젝트 루트의 `.env`를 자동 로드 → `Bun.env` / `process.env` → **dotenv 불필요**
- 시크릿은 **이미지에 굽지 말고** Compose/K8s secret · 런타임 env로 주입(§2.5)
- YAML이 필요하면 `Bun.YAML.parse(await Bun.file("config.yaml").text())`(가용성 §3), 없으면 JSON

```bash
# user-service           # order-service                     # gateway
PORT=4001                 PORT=4002                            PORT=4000
DATABASE_PATH=./data/     USER_SERVICE_URL=http://127…:4001    USER_SERVICE_URL=http://127…:4001
  user.db                 DATABASE_PATH=./data/order.db        ORDER_SERVICE_URL=http://127…:4002
```

---

## 15. 데이터 · DB 경계

### 15.1 원칙

```text
서비스당 DB(또는 스키마) 분리
다른 서비스 테이블 JOIN 금지
필요 데이터는 API로 가져오거나 읽기 모델(캐시) 복제
```

### 15.2 폐쇄망·외부 0 관점의 선택

| DB | Bun 경로 | 반입 부담 | MSA 적합 |
|---|---|---|---|
| **SQLite** (`bun:sqlite`) | 완전 내장 | **0** (DB 엔진까지 내장) | 단일 인스턴스·엣지·중소 |
| **Postgres** | `Bun.sql` (~1.2) | 서버는 별도, 드라이버 0 | 본격 프로덕션 |
| **MySQL/MariaDB** | `Bun.sql` (~1.3) | 서버는 별도, 드라이버 0 | 동일 |
| Redis | `Bun.redis` (~1.2.9+) | 서버는 별도, 드라이버 0 | 캐시·락 |
| 파일 JSON | `Bun.file` | 0 | 데모만 |

> **폐쇄망 강점:** `bun:sqlite`는 **DB 엔진 자체가 Bun에 내장**되어, 별도 DB 서버조차 반입·기동할 필요가 없다. 프로토타입·소규모·엣지에서 압도적으로 단순하다. 트래픽/HA가 필요하면 Postgres 서버만 세우고 드라이버는 `Bun.sql`로 — **외부 `pg`/`mysql2` 반입 0.**

### 15.3 store 예시 (sqlite)

```ts
// services/user-service/store.ts
import { Database } from "bun:sqlite";

let db: Database;
export class UserStore {
  constructor(path: string) {
    db = new Database(path);
    db.exec(`CREATE TABLE IF NOT EXISTS users (
      id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT NOT NULL);`);
  }
  find(id: string) { return db.query("SELECT * FROM users WHERE id = ?").get(id); }
  insert(id: string, name: string) {
    db.query("INSERT INTO users (id, name, created_at) VALUES (?, ?, ?)")
      .run(id, name, new Date().toISOString());
  }
}
export function closeDb() { db?.close(); }
```

---

## 16. 헬스 · 관측 · 장애

### 16.1 헬스체크

```ts
"/health": { GET: () => Response.json({ ok: true, service: "user", ts: Date.now() }) }
```

게이트웨이 집계:

```ts
async function deepHealth() {
  const targets = [Bun.env.USER_SERVICE_URL, Bun.env.ORDER_SERVICE_URL]
    .filter(Boolean).map((b) => `${b}/health`);
  const results = await Promise.all(targets.map(async (u) => {
    try { const r = await fetch(u!, { signal: AbortSignal.timeout(1000) }); return { url: u, ok: r.ok }; }
    catch { return { url: u, ok: false }; }
  }));
  const ok = results.length > 0 && results.every((x) => x.ok);
  return Response.json({ ok, results }, { status: ok ? 200 : 503 });
}
```

### 16.2 로그 (폐쇄망 관측 전략)

- 구조화: `log("info", "request", { requestId, ... })` (§11의 `shared/logger.ts`)
- `x-request-id`를 게이트웨이에서 부여·전파 → 서비스 간 추적
- **외부 APM/OTel exporter는 대개 패키지가 필요 → 폐쇄망 제로 디펜에서는 배제.** 대신 **stdout JSON을 내부 로그 수집기(사내에 이미 있는 것)**로 흘린다. 관측은 인프라 레이어에서 해결하고, 앱은 **깨끗한 JSON을 뱉는 것**까지만 책임진다.

### 16.3 장애 모드

| 상황 | 기대 동작 |
|---|---|
| user-service 다운 | order 생성 시 502/503 + 명확한 `upstream_error` |
| gateway만 다운 | 내부 서비스는 살아 있어도 외부 차단 |
| DB 파일 락/손상 | `/health`를 503으로 (준비 프로브 실패) |

---

## 17. 테스트 (bun test)

NestJS의 강점 중 하나가 테스트 생태계다. **`bun test`는 내장이라 반입 0**이고, jest 유사 API를 제공한다.

### 17.1 단위 테스트 (service — DI 덕에 mock 주입 쉬움)

```ts
// services/user-service/service.test.ts
import { test, expect, mock } from "bun:test";
import { UserService } from "./service";

test("createUser는 이름을 저장하고 id를 반환", async () => {
  const store = { insert: mock(() => {}), find: mock(() => null) };
  const svc = new UserService(store as any);
  const user = await svc.create("kim");
  expect(user.name).toBe("kim");
  expect(store.insert).toHaveBeenCalledTimes(1);
});
```

### 17.2 검증 로직 테스트

```ts
import { test, expect } from "bun:test";
import { validate } from "../../shared/validate";
import { createUserSchema } from "./dto";

test("이름 없으면 검증 실패", () => {
  expect(() => validate(createUserSchema, {})).toThrow();
});
```

### 17.3 통합 테스트 (실제 서버 기동 → fetch)

```ts
// e2e/user.test.ts
import { test, expect, beforeAll, afterAll } from "bun:test";

let proc: ReturnType<typeof Bun.spawn>;
beforeAll(async () => {
  proc = Bun.spawn(["bun", "services/user-service/main.ts"], { env: { ...process.env, PORT: "4901" } });
  await Bun.sleep(300); // 기동 대기
});
afterAll(() => proc.kill());

test("POST /users → 201", async () => {
  const res = await fetch("http://127.0.0.1:4901/users", {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: "kim" }),
  });
  expect(res.status).toBe(201);
});
```

```bash
bun test          # 전부 실행 (반입한 소스만으로, 인터넷 불필요)
```

> **폐쇄망 CI 게이트:** `bun test` + `bunx tsc --noEmit`(반입한 typescript) 통과를 반입 전 필수 관문으로(§2.4).

---

## 18. 배포 · Docker · 프로세스 모델

### 18.1 최소 Dockerfile

```dockerfile
# 폐쇄망: oven/bun:1 을 사내 레지스트리에 미러해 두고 참조
FROM oven/bun:1
WORKDIR /app
COPY shared ./shared
COPY services/user-service ./services/user-service
ENV PORT=4001
EXPOSE 4001
CMD ["bun", "services/user-service/main.ts"]
```

`package.json`의 dependencies가 **비어 있거나 없어도** 된다 (`bun add`로 채우지 않으면 됨). → `bun install` 단계 자체가 불필요 → **폐쇄망 빌드가 인터넷을 안 탄다.**

### 18.2 Compose 스케치

```yaml
services:
  gateway:
    build: { context: ., dockerfile: Dockerfile.gateway }
    ports: ["4000:4000"]
    environment:
      USER_SERVICE_URL: http://user:4001
      ORDER_SERVICE_URL: http://order:4002
  user:
    build: { context: ., dockerfile: Dockerfile.user }
    environment: { PORT: "4001" }
  order:
    build: { context: ., dockerfile: Dockerfile.order }
    environment: { PORT: "4002", USER_SERVICE_URL: http://user:4001 }
```

서비스 발견 = **Compose DNS 이름**.

### 18.3 컴파일 바이너리 (Bun 런타임조차 안 깔고 싶을 때)

```bash
bun build --compile services/user-service/main.ts --outfile user-service
```

- 반입물이 **단일 실행 파일**이 되어 폐쇄망 배포가 더 단순해질 수 있다.
- 주의: 대상 OS/arch로 빌드해야 하고, `shared/` import 경로가 컴파일에 포함되는지 빌드 한 번으로 검증할 것. `bun:sqlite` 등 내장은 함께 포함된다.

---

## 19. 모노레포 vs 멀티레포

| | 모노레포 (추천 시작) | 멀티레포 |
|---|---|---|
| `shared/` | 경로 import | git submodule / 복사 |
| 배포 | 서비스별 Docker 컨텍스트 | 레포별 CI |
| 폐쇄망 제로 디펜 | 유지 쉬움 (반입 1덩어리) | "공유 패키지 publish" 유혹 → 사설 레지스트리 필요 |

**폐쇄망에서 외부 0을 지키려면 모노레포 + `shared/`가 압도적으로 덜 아프다** (반입 산출물이 하나로 떨어진다). 팀이 커지면 서비스 레포를 쪼개되, `shared`는 복사하거나 사내 아티팩트로 — 그 순간 "npm 0" 정책을 재정의한다.

---

## 20. 최소 동작 예시 (User + Order + Gateway)

### 20.1 user-service controller (Ctx 기반)

```ts
// services/user-service/controller.ts
import { jsonOk, jsonErr } from "../../shared/http";
import { validate } from "../../shared/validate";
import { createUserSchema } from "./dto";
import type { Ctx } from "../../shared/pipeline";
import { userController } from "./wire"; // §10.1 조립 루트

export async function getUser(ctx: Ctx) {
  const user = userController.find(ctx.params.id!);
  return user ? jsonOk(user) : jsonErr("user_not_found", 404);
}

export async function createUser(ctx: Ctx) {
  const dto = validate(createUserSchema, await ctx.req.json()); // 검증 (§10.3)
  const created = userController.create(dto.name);              // 서비스 (§10.1)
  return jsonOk(created, 201);
}
```

### 20.2 order-service — 타 서비스 호출

```ts
// services/order-service/service.ts
import { fetchJson, HttpError } from "../../shared/http";

const USER_BASE = Bun.env.USER_SERVICE_URL ?? "http://127.0.0.1:4001";

export async function createOrder(userId: string, item: string) {
  try {
    await fetchJson(`${USER_BASE}/users/${userId}`); // 동기 HTTP로 유저 검증 (MSA 경계)
  } catch (e) {
    if (e instanceof HttpError && e.status === 404) throw new HttpError(400, "invalid_user");
    throw new HttpError(503, "user_service_unavailable");
  }
  const id = crypto.randomUUID();
  return { id, userId, item, status: "created" };
}
```

### 20.3 호출 흐름

```text
Client → POST http://localhost:4000/api/orders
           │
           ▼  gateway proxy → http://order:4002/orders
           │
           │  fetch GET http://user:4001/users/:id  (유저 검증)
           ▼
        user-service → 200 → order 저장 → 201 { order }
```

이 한 흐름이 **Express 모놀리스의 함수 호출**을 **네트워크 경계**로 바꾼 전부다.

---

## 21. 안티패턴 · 함정

| 함정 | 왜 나쁜지 | 대안 |
|---|---|---|
| "인터넷 잠깐 열어서 axios만…" | 폐쇄망·제로 디펜 전제 붕괴 | `fetch` (§8) |
| order가 `../user-service/store` import | 배포·장애 독립 붕괴 | `fetch`만 (§12) |
| 공유 DB에 모든 테이블 | 스키마 결합 = 모놀리스 | DB per service (§15) |
| Nest급 데코레이터/리플렉션 재발명 | 유지비 > 이득, 외부 패키지 유혹 | `endpoint({...})` 명시 조립 (§10.8) |
| DI 컨테이너부터 깔고 시작 | 조기 추상화 | 생성자 주입 먼저 (§10.1) |
| 모든 통신을 WS | 디버깅·LB 고통 | 기본 HTTP (§12) |
| POST에 자동 재시도 | 중복 주문 | 멱등키 or 재시도 금지 (§12.4) |
| 게이트웨이에 비즈니스 로직 | 다시 모놀리스 | 프록시·인증만 (§13) |
| 최신 Bun 기능에 과의존 | 폐쇄망은 업그레이드가 비쌈 | 능력 매트릭스 확인·degrade (§3) |
| 서비스 20개부터 시작 | 운영 비용 폭발 | **2~4 바운디드 컨텍스트**부터 |

### 21.1 "MSA vs 모듈러 모놀리스"

Bun 하나로 **폴더만 나눠 한 프로세스**에 올리는 것도 가능하다 — 그건 MSA가 아니라 **모듈러 모놀리스**이고, 초기엔 더 나을 수 있다.

```text
지금 팀·트래픽이 작다  → 한 Bun.serve + domains/user, domains/order 폴더
                        나중에 프로세스 분리 시 routes만 다른 main으로 이관
이미 배포 단위·스케일이 다르다 → 이 문서의 멀티 프로세스 MSA
```

제로 디펜을 지키면서도 **과한 MSA는 독**이다. 경계를 먼저 그리고, 프로세스는 필요할 때 쪼갠다.

---

## 22. 리스크 · 완화 (보고서)

| # | 리스크 | 영향 | 완화 |
|---|---|---|---|
| R1 | **버전 종속** — 반입 시점 Bun 버전에 기능이 묶임 | 특정 API 부재로 기동 실패 | §3 능력 매트릭스 자가진단 + fail-fast, degrade 경로 사전 정의 |
| R2 | **업그레이드 비용** — 폐쇄망은 Bun 교체가 비쌈 | 보안 패치 지연 | 버전 고정 + 반입 주기 정례화, 최신 기능 과의존 금지 |
| R3 | **관측 생태계 부재** — APM SDK 반입 어려움 | 트러블슈팅 난이도↑ | stdout 구조화 JSON + `x-request-id` + 내부 수집기(§16) |
| R4 | **직접 구현 유지보수** — 검증·DI·Guard를 우리가 짬 | 버그·인력 종속 | 얇게 유지, 테스트(§17)로 고정, §10 범위를 넘지 않기 |
| R5 | **Bun 생태계/드라이버 성숙도** — `Bun.sql` 등 상대적으로 새로움 | 엣지 케이스 | 프로토타입은 `bun:sqlite`(성숙), 프로덕션 전 부하·회귀 테스트 |
| R6 | **인력 스킬** — 팀이 Nest에 익숙 | 학습 곡선 | §10에서 Nest 개념 1:1 매핑 제공, 명시적 조립으로 진입장벽↓ |
| R7 | **공급망(우리 코드)** — 우리가 짠 코드가 취약점 | 보안 | 코드 리뷰·테스트, 그러나 외부 의존 0이라 스캔 표면 자체가 작음 |
| R8 | **"잠깐 패키지 하나만" 유혹** | 제로 디펜 붕괴 → 사설 레지스트리 필요 | 정책으로 명문화, CI에서 dependencies 비어있음 검사 |

### 22.1 CI에서 "디펜던시 0" 강제 (권장 게이트)

```bash
# dependencies/devDependencies 가 비어있지 않으면 실패
bun -e '
  const p = await Bun.file("package.json").json().catch(() => ({}));
  const dep = { ...(p.dependencies||{}), ...(p.devDependencies||{}) };
  if (Object.keys(dep).length) { console.error("EXTERNAL DEPS FOUND:", dep); process.exit(1); }
  console.log("zero-dependency OK");
'
```

---

## 23. 결론 · 남은 결정

### 23.1 이렇게 가면 맞다

- 폐쇄망이라 `bun add`가 애초에 안 된다 / 반입물을 극단적으로 줄이고 싶다
- JS/TS로 남기되 Express·Nest 런타임을 반입하기 싫다
- JSON API + 서비스 간 HTTP (+ 필요 시 WS)
- NestJS의 **구조·검증·Guard·Interceptor·예외필터**는 원하지만 프레임워크 종속은 싫다

→ **`Bun.serve({ routes })` + §10 파이프라인 + env/DNS URL + (`bun:sqlite` | `Bun.sql`)**

### 23.2 다시 프레임워크를 고려할 때

- 팀 전원이 Nest 데코레이터·OpenAPI·가드 생태계 생산성이 최우선이고
- ORM·큐·관리 UI까지 npm 생태계를 적극 쓸 계획이며
- **폐쇄망 반입 파이프라인(사설 레지스트리 미러)을 이미 운영 중**이다

→ 그때는 "제로 디펜" 목표 자체를 재정의하는 편이 낫다. (하지만 폐쇄망이라면 대개 이 문서 방향이 총유지비가 낮다.)

### 23.3 착수 전 확정할 결정 3가지

1. **반입 시점 Bun 버전** — 고정할 버전과 §3 능력 매트릭스 실측 결과는?
2. **DB** — 서비스별 `bun:sqlite` 파일로 시작? 아니면 Postgres/MySQL(`Bun.sql`)?
3. **스케일 형태** — 지금 멀티 프로세스 MSA인가, 모듈러 모놀리스 후 분리인가?
4. **NestJS 패리티 범위** — §10 중 무엇을 초기에 넣나? (권장: 검증·예외필터·라이프사이클부터, DI는 나중)

---

## 부록 A — Express / Nest / Bun 제로 대조

| 관심사 | Express | NestJS | Bun 제로 MSA (폐쇄망) |
|---|---|---|---|
| 서버 | `http` + express | Nest 위에 express/fastify | **`Bun.serve`** |
| 구조 | 자유 (쉽게 무너짐) | Module 강제 | **폴더 규약 + §10 파이프라인** |
| DI | 없음 | 강력(자동) | **생성자 주입 / 경량 컨테이너(§10.2)** |
| 라우팅 | 내장 | 데코레이터 | **`routes`(≥1.2.3)** / PureRouter |
| 검증 | 수동/미들웨어 | ValidationPipe | **직접 검증기(§10.3)** |
| Guard/Interceptor | 미들웨어 | 데코레이터 | **함수 합성(§10.4~10.5)** |
| 예외처리 | 에러 미들웨어 | ExceptionFilter | **전역 매퍼(§10.6)** |
| 설정 | dotenv | ConfigModule | **`Bun.env`** |
| 서비스 호출 | axios | HttpService | **`fetch`** |
| WS | socket.io 등 | `@WebSocketGateway` | **내장 websocket** |
| 테스트 | jest | jest + Test 유틸 | **`bun test`(내장)** |
| 외부 디펜던시 | 많음 | 더 많음 | **0** |
| 폐쇄망 반입 | 어려움 | 매우 어려움 | **바이너리+소스만** |

## 부록 B — 서비스 추가 체크리스트

- [ ] `services/<name>/main.ts`에 `Bun.serve` + 포트 env + `onShutdown`(§10.7)
- [ ] `routes.ts` / `controller.ts` / `service.ts` / `dto.ts` / `store.ts` 분리
- [ ] 입력 검증 스키마 작성(§10.3), 핸들러는 `endpoint({...})`로 조립(§10.8)
- [ ] `/health` 제공
- [ ] 게이트웨이 `UPSTREAM`에 prefix 추가
- [ ] `*_SERVICE_URL` env 문서화
- [ ] 다른 서비스 store import 안 함 (fetch만)
- [ ] 아웃바운드 `fetch`에 timeout
- [ ] 단위/통합 테스트 추가(§17)
- [ ] Docker/Compose 서비스·DNS 이름 추가
- [ ] **`bun add`로 새 패키지 안 넣음** (CI 게이트 §22.1 통과)

## 부록 C — 폐쇄망 반입 체크리스트

- [ ] 반입 Bun 버전 고정 · `bun --version` / `--revision` 기록
- [ ] 운영망에서 `tools/capability-check.ts` 실행 → 필수 능력 확인(§3.2)
- [ ] `package.json` dependencies/devDependencies 비어있음 확인(§22.1)
- [ ] `bunx tsc --noEmit` 타입 체크 통과 (반입한 typescript)
- [ ] `bun test` 통과
- [ ] 소스 tar + **sha256 체크섬** 생성 → 운영망에서 검증
- [ ] 베이스 이미지(`oven/bun:1`) 사내 레지스트리 미러 확인(Docker 시)
- [ ] 시크릿은 이미지에 없고 런타임 주입인지 확인(§2.5)
- [ ] 자동 업그레이드/아웃바운드 경로 차단 확인(§2.5)

---

**요약:** 폐쇄망에서는 "디펜던시 0"이 이상이 아니라 **환경이 강제하는 조건**이다. 그 조건 위에서 실행은 전부 `Bun.serve`로, 라우팅은 **내장 `routes`**로, 서비스 간은 **`fetch` + env/DNS**로 잇는다. NestJS가 주던 값어치 — **검증·Guard·Interceptor·예외필터·DI·라이프사이클** — 은 프레임워크를 반입하지 않고 **§10의 얇은 TS로 골라 자급**한다. 최신 기능에 과의존하지 말고, 반입 시점 **Bun 버전의 능력을 실측(§3)**해 degrade 경로를 미리 정해 두는 것이 폐쇄망 운영의 핵심이다.
