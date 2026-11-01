# 모든 개발자가 알아야 할 지연시간(Latency) 숫자 — 심층 보고서

출처 : https://www.youtube.com/watch?v=WbzMtyyOQpM

> **문서 성격**: "Latency Numbers Every Programmer Should Know"로 알려진 컴퓨터 시스템 연산별 지연시간 표의 **역사·의미·실무 활용**을 정리한 기술 보고서.
> **한 줄 요지**: 코더(coder)와 아키텍트(architect)를 가르는 것은 "코드를 잘 쓰는가"가 아니라 **"근거 있는 의사결정을 할 수 있는가"**이며, 그 근거의 핵심 도구가 바로 이 지연시간 숫자에 대한 **차수(order of magnitude) 감각**이다.

---

## 0. 핵심 요약 (TL;DR)

- 시스템 설계에서 "느리다/빠르다"는 직관이 아니라 **숫자(나노초/마이크로초/밀리초)**로 말할 수 있어야 한다.
- 이 표는 한 사람의 발명이 아니라 **Peter Norvig(2001) → Jeff Dean(2009) → Jeff Boner/jboner Gist(2012) → Colin Scott(시각화)**로 이어진 **업계 공동 자산**이다.
- 외워야 할 것은 정확한 값이 아니라 **차수의 서열과 격차**:
  - **CPU/캐시(ns) ≪ 메모리(~100ns) ≪ SSD(수십~수백 µs) ≪ 데이터센터 왕복(~500µs) ≪ 디스크 seek(~10ms) ≪ 대륙 간 왕복(~150ms)**
- 연도가 지나며 **절댓값은 변해도 상대적 격차(배수)와 서열은 거의 유지**된다.
- 실무 활용: **캐시 도입 판단, 마이크로서비스 분리 비용 산정, 시스템 디자인 면접** 등에서 정량적 근거를 제공.

---

## 1. 문제의식: 코더 vs 아키텍트

- 전형적 회의 장면: 주니어가 "DB 조회가 느리니 Redis 캐시를 도입하자"고 제안 → 시니어가 "**DB 응답이 얼마나 느린데? 캐시로 얼마나 빨라지는데?**"라고 물으면 정량적으로 답하지 못한다.
- **핵심**: 코드는 이미 AI가 잘 만든다. 진짜 개발자(아키텍트)의 경쟁력은 **기준과 근거에 입각한 의사결정 능력**이다.
- 이 보고서가 다루는 "지연시간 숫자"는 그 의사결정의 **객관적 기준점** 역할을 한다.

---

## 2. 이 표는 어떤 질문에 답하는가

컴퓨팅 시스템의 시간 비용(CPU·메모리·디스크·네트워크)을 정량화한 이 표는 다음과 같은 **아키텍처 질문**에 답한다.

- 캐시를 도입할 가치가 있는가?
- 네트워크 왕복(round trip)을 한 번 줄이면 얼마나 빨라지는가?
- 데이터를 메모리에 두는 것이 정말 그렇게 큰 차이를 만드는가?
- 리전(region)을 분리하면 응답이 얼마나 느려지는가?
- SSD가 RAM을 대체할 수 있는가?

> 이 수치들은 백엔드·시스템 설계의 표준 레퍼런스로, DDIA(*Designing Data-Intensive Applications*), 시스템 디자인 면접서(예: Alex Xu), ACM Queue, Google Research·Cornell 학술자료 등에서 **공통 인용**된다.

---

## 3. 계보: 누가, 언제, 왜 만들었나

이 표는 **한 사람의 천재성이 아니라 업계 전체가 함께 다듬어 온 공동 자산**이다.

| 인물 | 시점 | 기여 | 관점 |
|---|---|---|---|
| **Peter Norvig** | 2001 | 최초 정리(11개 수치) | **단일 PC**의 로컬 하드웨어 물리적 제약 |
| **Jeff Dean** | 2009 (LADIS 키노트) | 분산 시스템 시대에 맞게 확장(압축·데이터센터 왕복 등) | **구글 규모 분산 인프라** |
| **Jeff Boner (jboner)** | 2012 | Jeff Dean 강연을 Gist로 정리 + **SSD 항목 2개 추가** → 사실상 업계 표준 표 완성 | **SSD 시대** |
| **Colin Scott** | ~2012–2020 | 하드웨어 추세를 **시간의 함수로 모델링**, 연도 슬라이더 인터랙티브 시각화 | **시간에 따른 변화** |

### 3-1. Peter Norvig (2001) — 출발점

- 구글 리서치 디렉터 출신, AI 교과서 *Artificial Intelligence: A Modern Approach*(AIMA)의 공저자.
- 에세이 **["Teach Yourself Programming in Ten Years"](https://norvig.com/21-days.html)**에서 "24시간/21일 만에 배우기" 류의 책들을 비판하며 **"진짜 프로그래밍 숙련에는 10년이 걸린다"**고 주장.
- 이 에세이 부록에 붙은 **"Approximate timing for various operations on a typical PC"** 표가 오늘날 이 숫자의 **출발점**. 총 **11개** 수치 수록.

#### Norvig의 원본 11개 수치 (typical PC 기준)

| 연산 | 지연시간 (ns) | 환산 |
|---|---:|---|
| 典型 명령 실행 (execute typical instruction) | 1 | 1 ns |
| L1 캐시 참조 | 0.5 | 0.5 ns |
| 분기 예측 실패 (branch mispredict) | 5 | 5 ns |
| L2 캐시 참조 | 7 | 7 ns |
| Mutex lock/unlock | 25 | 25 ns |
| 메인 메모리 참조 | 100 | 100 ns |
| 1 Gbps 네트워크로 2KB 전송 | 20,000 | 20 µs |
| 메모리에서 1MB 순차 읽기 | 250,000 | 250 µs |
| 디스크 seek (새 위치 탐색) | 8,000,000 | 8 ms |
| 디스크에서 1MB 순차 읽기 | 20,000,000 | 20 ms |
| 패킷 왕복 US ↔ Europe | 150,000,000 | 150 ms |

> **Norvig의 진짜 메시지**: 이 숫자를 **암기하라는 것이 아니다.** 중요한 것은 **나노초 → 마이크로초 → 밀리초로 넘어가는 차수(order of magnitude)의 감각**이다. 이 감각이 있는 사람과 없는 사람의 시스템 설계는 완전히 달라진다. 새로운 기술이 나와도 "이 정도 비용이 들겠구나"를 **추정**할 수 있게 된다.

### 3-2. Jeff Dean (2009) — 분산 시스템으로 확장

- MapReduce, Bigtable, Spanner, TensorFlow 등의 공동 설계자. 업계의 "살아있는 전설".
- **LADIS 2009**(대규모 분산 시스템 학회) 키노트 **"Designs, Lessons and Advice from Building Large Distributed Systems"**([자료](https://research.cs.cornell.edu/ladis2009/talks/dean-keynote-ladis2009.pdf))에서 Norvig의 표를 **구글의 분산 시스템 경험을 반영해 확장**.
- 핵심 어록: **"중요한 것은 실제로 만들어 보지 않고도 시스템 성능을 추정할 수 있는 능력이다(the ability to estimate the performance of a system design without actually having to build it)."** — *back-of-the-envelope calculation*.

#### Jeff Dean이 추가/변경한 대표 항목

| 연산 | 지연시간 | 의미 |
|---|---:|---|
| **1KB를 Zippy로 압축** | 3,000 ns (3 µs) | 압축의 시간 비용을 정량화 |
| **같은 데이터센터 내부 왕복(RTT)** | 500,000 ns (500 µs) | 분산 시스템의 새로운 계층 |
| 디스크 seek | 10,000,000 ns (10 ms) | Norvig보다 보수적 값 |

#### 압축 트레이드오프 예시 (Jeff Dean 표의 진가)

네트워크·디스크 I/O가 병목이던 Bigtable/MapReduce 환경에서 "**압축해서 전송할지**"를 정량적으로 판단하는 예:

```text
[가정] 1KB 데이터를 네트워크로 전송

(A) 그냥 전송:
    10 µs  (전송)

(B) 압축 후 전송:
    3 µs   (1KB Zippy 압축)
  + 5 µs   (절반 크기로 줄어든 데이터 전송)
  = 8 µs

→ 8 µs < 10 µs 이므로 압축이 이득.
   "압축 비용 < 절감되는 전송 비용"일 때만 압축이 의미 있다.
```

> 즉 이 표는 **"압축이 전송보다 빨라야 의미가 있다"**는 트레이드오프를 **직접 만들어 보지 않고도** 판단하게 해준다.

### 3-3. jboner Gist (2012) — 업계 표준판

- Akka 창시자 **Jonas Bonér**가 Jeff Dean의 2009 강연을 **[GitHub Gist](https://gist.github.com/jboner/2841832)**로 정리하면서 **SSD 항목 2개**(SSD 4K 랜덤 읽기, SSD 1MB 순차 읽기)를 추가.
- 이 버전이 현재 **가장 널리 인용되는 사실상의 표준표**.

#### "Latency Numbers Every Programmer Should Know" (~2012, jboner Gist)

| 연산 | ns | µs | ms | 비고 |
|---|---:|---:|---:|---|
| L1 cache reference | 0.5 | | | |
| Branch mispredict | 5 | | | |
| L2 cache reference | 7 | | | 14× L1 |
| Mutex lock/unlock | 25 | | | |
| Main memory reference | 100 | | | 20× L2, 200× L1 |
| Compress 1KB with Zippy | 3,000 | 3 | | |
| Send 1KB over 1 Gbps network | 10,000 | 10 | | |
| **Read 4KB randomly from SSD** | 150,000 | 150 | | ~1GB/s SSD |
| Read 1MB sequentially from memory | 250,000 | 250 | | |
| Round trip within same datacenter | 500,000 | 500 | | |
| **Read 1MB sequentially from SSD** | 1,000,000 | 1,000 | 1 | 4× memory |
| Disk seek | 10,000,000 | 10,000 | 10 | 20× datacenter RTT |
| Read 1MB sequentially from disk | 20,000,000 | 20,000 | 20 | 80× memory, 20× SSD |
| Send packet CA→Netherlands→CA | 150,000,000 | 150,000 | 150 | |

### 3-4. Colin Scott — 시간의 함수로 시각화

- UC Berkeley의 Colin Scott이 이 수치들을 **연도(하드웨어 추세)의 함수로 모델링**하여, 연도 슬라이더를 움직이면 해당 시점의 지연시간이 자동 계산되는 인터랙티브 웹페이지를 제작.
- **[Latency Numbers Every Programmer Should Know (interactive)](https://colin-scott.github.io/personal_website/research/interactive_latency.html)**
- **진짜 가치**: 슬라이더 하나로 **약 30년의 하드웨어 변화**를 보여주면서도, **차수의 서열(CPU 캐시 → 메모리 → SSD → 디스크 → 네트워크)은 연도가 바뀌어도 유지**됨을 직관적으로 체감시킨다.
- **한계**: 하드웨어 발전을 단순화해 모델링했기 때문에, **메모리·네트워크처럼 개선 속도가 제한적인 영역**에서는 실제 벤치마크와 차이가 날 수 있다. 정확한 값이 필요하면 최신 벤치마크를 참조해야 한다.

---

## 4. 차수 감각을 키우는 직관 (Humanized Scale)

지연시간은 사람이 체감하기엔 너무 작다. **모든 값에 약 10억 배(×1,000,000,000)**를 곱해 "만약 1 CPU 사이클이 1초라면"으로 바꾸면 차수 격차가 직관적으로 다가온다. (개략적·설명용 근사)

| 연산 | 실제 | 인간 척도(×10⁹) |
|---|---:|---|
| L1 캐시 참조 (0.5 ns) | 0.5 ns | 약 0.5초 |
| 메인 메모리 참조 (100 ns) | 100 ns | 약 1분 40초 |
| SSD 1MB 순차 읽기 (1 ms) | 1,000,000 ns | 약 11.6일 |
| 데이터센터 왕복 (500 µs) | 500,000 ns | 약 5.8일 |
| 디스크 seek (10 ms) | 10,000,000 ns | 약 4개월 |
| 대륙 간 왕복 (150 ms) | 150,000,000 ns | 약 4.75년 |

> 메모리 접근이 "1분 40초"라면, 대륙 간 네트워크 왕복은 "약 5년"이다. **왜 네트워크 호출을 줄여야 하는지**가 몸으로 이해된다.

---

## 5. 실무 의사결정 사례

이 수치들의 목적은 **"시스템을 직접 만들기 전에 성능을 추정(estimate)"**하는 것이다.

### 5-1. 캐시 도입 의사결정 — "Redis 캐시를 도입할 가치가 있는가?"

- 상황: HDD 기반 레거시 DB 조회가 느려 Redis 캐시 도입을 검토.
- 인용할 수치:
  - **HDD 디스크 seek ≈ 10 ms (밀리초)**
  - **데이터센터 내부 RTT ≈ 500 µs (마이크로초)**
  - **Redis(메모리) 조회 ≈ 수백 ns (나노초)**
- 판단: 밀리초 ↔ 마이크로초 ↔ 나노초로 **차수 자체가 다르다.** → HDD 조회 대비 메모리 조회가 압도적으로 빠르므로 **캐시 도입 가치가 크다.**
- 단서: **SSD 기반 DB라면** 성능 격차가 줄어드므로, 그때는 지연시간이 아니라 **부하 분산(throughput) 관점**으로 재검토해야 한다.

### 5-2. 마이크로서비스(MSA) 분리의 진짜 비용

- 질문: "이 기능을 별도 마이크로서비스로 분리하는 게 좋을까?"
- 인용할 수치:
  - **로컬 메모리 내 함수 호출 ≈ 수 ns**
  - **동일 데이터센터 내 네트워크 호출 ≈ 500 µs**
  - → 둘의 차이는 **약 10만 배 이상** (나노초 vs 마이크로초). 여기에 **직렬화/역직렬화 비용**까지 추가된다.
- 판단: 기능 분리 자체가 호출당 **최소 10만 배의 비용**을 추가한다. **호출 빈도가 높다면 분리에 신중**해야 하며, "무조건 분리가 정답"은 아니다.
- 단서: MSA는 지연시간만이 아니라 **확장성·배포 독립성·조직 구조** 등 다른 아키텍처 관점도 함께 고려해야 하는 결정이다. 위는 **지연시간 관점의 한 축**일 뿐이다.

### 5-3. 시스템 디자인 면접 — "초당 100만 건(1M QPS) 처리"

- 예: "URL 단축 서비스를 100만 QPS로 처리하려면 어떻게 설계하겠는가?" (트위터/인스타그램급 초대형 규모)
- 수치 감각이 있는 개발자가 떠올리는 세 숫자:
  1. **메모리 조회 ≈ 100 ns → 이론상 단일 서버로 초당 ~1,000만 건.** 메모리에서 끝나는 작업이면 한 대로도 충분.
  2. **디스크/DB 조회 ≈ 수 ms → 단일 서버로는 초당 수백 건도 버겁다.** 100만 QPS에선 한계가 명확.
  3. **네트워크 호출 ≈ 500 µs → 서비스 간 호출 횟수를 줄이지 않으면 병목.**
- 결론적 논증: **왜 캐시가 필수인지, 왜 샤딩이 필요한지, 왜 호출을 최소화해야 하는지**를 모두 **숫자로 정량화**해 설명할 수 있다. 이것이 코더와 아키텍트의 차이다.

---

## 6. 활용 시 주의점 (4가지)

1. **정확한 값을 암기하려 하지 말라.** 이 수치는 **차수 수준의 근사값**이며 실제 환경과 정확히 일치하지 않는다. 핵심은 **ns / µs / ms 단위 차이의 감각**.
2. **전부 외울 필요 없다. 몇 개만 알아도 충분하다.** 최소한 다음만 기억하면 대부분의 의사결정이 가능:
   - **L1 캐시 · 메인 메모리 · SSD · 데이터센터 RTT · 디스크 · 대륙 간 전송**
3. **절댓값이 아니라 상대적 격차(배수)에 집중하라.** 연도마다 절댓값은 변해도 **비율(예: RAM이 SSD보다 대략 수백 배 빠르다)**은 대체로 유지된다.
4. **지연시간(latency)만이 전부가 아니다.** 실무에서는 **처리량(throughput)**을 함께 고려해야 한다. 배치·병렬·순차 처리로 지연 비용을 상쇄할 수 있다. (예: 여러 호출을 **배치로 묶으면** 네트워크 왕복 횟수가 줄어 실효 비용이 감소)

---

## 7. 결론: 바이브 코딩 시대에 더 중요한 아키텍트 역량

- AI가 상당한 수준의 코드를 매우 빠르게 만들어 내는 것은 사실이다. 그러나:
  - **구조 설계와 하중 계산을 모르는 건축가**,
  - **재료와 불의 성질을 모르는 요리사**,
  - **악보를 못 읽는 작곡가**가 있을 수 없듯,
  - 개발의 **본질(시간 비용·차수 감각)**을 모른 채 AI에만 의존해 쏟아낸 코드는 결국 **저품질 시스템**이 되어 되돌아온다.
- **누구나 개발하는 시대일수록, 본질을 이해하는 아키텍트 역량이 진짜 경쟁력**이다. 기본에 충실하고 중요한 것에 집중하자.

---

## 부록 A. 핵심 숫자 치트시트 (최소 암기 세트)

| 계층 | 대표 지연시간 | 차수 |
|---|---:|---|
| L1 캐시 | ~0.5 ns | 나노초 |
| 메인 메모리 | ~100 ns | 나노초 |
| SSD 랜덤 읽기 | ~150 µs | 마이크로초 |
| 데이터센터 내부 왕복(RTT) | ~500 µs | 마이크로초 |
| 디스크(HDD) seek | ~10 ms | 밀리초 |
| 대륙 간 패킷 왕복 | ~150 ms | 밀리초 |

> 기억법: **캐시(ns) → 메모리(ns) → SSD/DC(µs) → 디스크(ms) → 대륙 간(ms)**, 계층을 한 단계 내려갈 때마다 **대략 100~1000배씩** 느려진다.

## 부록 B. 참고 자료 (원문 링크)

- Peter Norvig, *Teach Yourself Programming in Ten Years* (2001): https://norvig.com/21-days.html
- Jeff Dean, LADIS 2009 Keynote — *Designs, Lessons and Advice from Building Large Distributed Systems*: https://research.cs.cornell.edu/ladis2009/talks/dean-keynote-ladis2009.pdf
- Jonas Bonér (jboner), *Latency Numbers Every Programmer Should Know* Gist (2012): https://gist.github.com/jboner/2841832
- Colin Scott, *Latency Numbers Every Programmer Should Know* (interactive): https://colin-scott.github.io/personal_website/research/interactive_latency.html
- 함께 인용되는 표준 문헌: *Designing Data-Intensive Applications*(Martin Kleppmann), 시스템 디자인 면접서(Alex Xu), ACM Queue.

> 본 표의 수치는 차수 감각을 위한 근사값입니다. 특정 하드웨어/연도의 정확한 값이 필요하면 최신 벤치마크와 Colin Scott의 인터랙티브 도구를 함께 참조하십시오.
