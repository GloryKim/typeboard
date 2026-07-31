# 대규모 선착순 예매 시스템 — 완전판

## 출처 : https://www.youtube.com/watch?v=EC2Xqyq6vg4

> **단일 합본.** 대기열·입장·예매·폴링·Redis/Lua/MQ 트레이드오프·예시 코드·팩트체크를 한 파일에 담았다.  
> 원자료: 대용량 선착순 예매 아키텍처 강의·Q&A·SRT 폴링 관찰을 **기술 문서로 재구성** (유튜브 말투 제거).  
> 본문은 **설계 패턴 설명**이며, KTX/SRT 내부 구현의 공식 스펙 공개가 아니다. 실측·추론은 그 사실을 명시한다.  
> **재검토:** 승격 조회 구멍·Watermark·보안/fail-open·용량·상용 대기열·해시 태그·멱등 예매·Redis 장애 정책·스케줄러 리더·동일 score/ZADD NX 정정·SRT 필드 보강 (§21).

---

## 목차

1. [한 줄 요약](#1-한-줄-요약)
2. [문제 정의](#2-문제-정의)
3. [왜 API+RDB만으로는 부족한가](#3-왜-apirdb만으로는-부족한가)
4. [3단계 분리 설계](#4-3단계-분리-설계)
5. [단계 1 — 대기열 (Redis ZSET)](#5-단계-1--대기열-redis-zset)
6. [단계 2 — 입장 (Scheduler → Active)](#6-단계-2--입장-scheduler--active)
7. [단계 3 — 예매 (원자적 좌석 차감)](#7-단계-3--예매-원자적-좌석-차감)
8. [특정 좌석 선점](#8-특정-좌석-선점)
9. [폴링 vs WebSocket vs SSE](#9-폴링-vs-websocket-vs-sse)
10. [대기열에 Kafka/MQ가 아닌 Redis인 이유](#10-대기열에-kafkamq가-아닌-redis인-이유)
11. [인프라·논블로킹 API](#11-인프라논블로킹-api)
12. [실측 관찰 (SRT 폴링 — 추론)](#12-실측-관찰-srt-폴링--추론)
13. [엔드투엔드 시퀀스](#13-엔드투엔드-시퀀스)
14. [실패·정합성·운영 함정](#14-실패정합성운영-함정)
15. [보안·클라이언트 오동작·관측](#15-보안클라이언트-오동작관측)
16. [용량 산정·부하 테스트](#16-용량-산정부하-테스트)
17. [예시 코드 모음](#17-예시-코드-모음)
18. [토이 시뮬레이터 (전체)](#18-토이-시뮬레이터-전체)
19. [설계 체크리스트](#19-설계-체크리스트)
20. [팩트체크·정정 노트](#20-팩트체크정정-노트)
21. [재검토 로그 (누락·오류)](#21-재검토-로그-누락오류)

---

## 1. 한 줄 요약

```text
① 오픈 순간 트래픽은 Redis 대기열로 흡수하고 RDB에 직행시키지 않는다
② 스케줄러가 뒤단이 감당 가능한 N명만 Active(입장 가능)로 승격한다
③ 예매 확정 시 Redis Lua(또는 동등 원자 연산)로 잔여 좌석을 조건부 차감한다
④ (선택) 성공 건만 MQ → 워커 → RDB 로 비동기 기록한다
⑤ 대기 순번 UI는 대규모에서 WebSocket보다 폴링이 유리한 경우가 많다
⑥ 결제·좌석 맵 선택은 폭주 구간 밖으로 빼 복잡도를 줄인다
```

| 요구 | 의미 |
|------|------|
| 대용량 | 순간 동시 접속이 DB·락 한계를 넘을 수 있음 |
| 선착순 | 먼저 줄 선 사용자가 먼저 입장할 자격 |
| 초과 예매 금지 | 좌석/시간대 한도 초과 불가 |
| 중복 방지 | 같은 사용자·같은 좌석 이중 점유 방지 |
| 단순화 | 결제·상세 좌석을 뒤로 미루면 장애면↓ |

---

## 2. 문제 정의

명절 기차표처럼 **정해진 시각에 수백만이 동시에 “접수”** 하는 시스템은 다음이 동시에 필요하다.

- **공정한 순서**(또는 최소한 추적 가능한 줄)
- **재고 한도** 준수
- **순간 부하**가 영속 DB를 죽이지 않을 것
- 사용자에게 **대기 순번 피드백**(UX)

“API를 많이 늘리면 된다”만으로는 부족하다. 예매의 진실(누가 표를 가졌는지)은 보통 **RDB**에 남고, 쓰기는 한곳(또는 소수 primary)에 모인다.

---

## 3. 왜 API+RDB만으로는 부족한가

### 3.1 일반적인 확장

```text
사용자 → API 1대 → RDB
사용자 → LB → API N대 → RDB   ← API만 스케일 아웃
```

API 프로세스/컨테이너는 수평 확장이 비교적 쉽다.  
그러나 수백만이 동시에 **예매 트랜잭션**을 치면 병목은 대개:

- DB 커넥션 풀 고갈  
- 동일 행/동일 재고에 대한 **락 경합**  
- WAL·디스크 I/O  

### 3.2 RDB에 대한 정확한 표현 (과장 주의)

| 말 | 교정 |
|----|------|
| “RDB는 스케일 아웃이 불가능하다” | **부정확.** 읽기 복제, 샤딩(Citus/Vitess 등), NewSQL 등이 있다 |
| “예매는 무조건 RDB” | **관행에 가깝다.** 강한 일관성·감사·조인에 유리해서 최종 원장으로 쓰는 경우가 많다 |
| “NoSQL이면 해결” | **아님.** 쓰기 분산이 쉬워져도 재고 원자성·순서 UX는 별도 설계가 필요 |

정리: **전통적 단일 primary RDB에 수백만 동시 쓰기를 그대로 붓는 것**이 위험한 것이지, “RDB=못 씀”이 아니다.  
그래서 **DB 앞에 유량 제어·대기열**을 둔다.

---

## 4. 3단계 분리 설계

```text
[1] 대기열 등록  : 모든 접수 → Redis ZSET (DB 미접촉)
[2] 입장 처리    : 스케줄러가 앞에서 N명 → Active(+TTL)
[3] 예매 처리    : Active만 좌석 차감 → RDB 또는 MQ→RDB
```

| 단계 | 비유 | 목표 |
|------|------|------|
| 대기열 | 매표소 밖 긴 줄 | 쓰기를 메모리로 흡수, 순위 유지 |
| Active | 창구 앞 소수 | 뒤단 TPS에 맞춰 유입 제한 |
| 예매 | 창구에서 표 발권 | 재고 원자성 + 영속화 |

**원칙:** 줄은 길어도 되고, **창구(RDB) 앞은 항상 짧게** 유지한다.

```text
         ┌──────────────┐
 Click → │ Queue API    │ → Redis ZSET (waiting)
         └──────────────┘
                │ polling (rank / can_enter)
         ┌──────────────┐
         │ Scheduler    │ → ZPOPMIN N → Active
         └──────────────┘
                │
         ┌──────────────┐
 Enter → │ Booking API  │ → Lua remaining -= n
         └──────────────┘
                │
         ┌──────┴──────┐
         ▼             ▼
        RDB      MQ → Worker → RDB
```

---

## 5. 단계 1 — 대기열 (Redis ZSET)

### 5.1 왜 Redis Sorted Set인가

- 메모리 기반 → 낮은 지연  
- **score 기준 정렬** 유지  
- `ZRANK` / `ZCARD` / `ZREM` / `ZPOPMIN` 등 **대기열 운영**에 맞는 연산  
- 클러스터로 확장 가능 (단, **핫 키** 문제는 남음 → §14)

### 5.2 등록 흐름

1. 사용자가 “예매 접수” (로그인은 나중일 수 있음)  
2. API가 **UUID(또는 로그인 user_id)** 발급/확정  
3. `ZADD waiting:<event> <score> <member>`  
4. 식별자를 클라이언트에 저장(쿠키/로컬스토리지 등)  
5. 이후 폴링은 그 식별자로 순위 조회  

**score 선택 (중요):**

| 방식 | 장점 | 단점 |
|------|------|------|
| `time.time()` (API 서버 시각) | 구현 쉬움 | **서버 간 시계 스큐**로 순서 뒤틀림 가능 |
| Redis `TIME` | 서버 단일 시계에 가깝게 | 네트워크 RTT 영향은 있음 |
| `INCR` 논리 시퀀스 | 단조·공정 | “절대 시각”은 아님. 선착순 정의로 충분하면 우수 |

선착순의 정의를 “누가 먼저 Redis에 기록됐는가”로 두면 **단조 시퀀스**가 가장 깔끔하다.

**시각 score 주의:** `time.time()`은 밀리초 미만이 같고 서버 스큐가 겹치면 **동일 score**가 난다 (§5.4).  
또한 IEEE754 double로 저장되므로 극단적으로 큰 정수 시퀀스는 정밀도 한계가 있다(실무 대기열 규모에서는 보통 문제 없음).

### 5.3 멱등성 — 연타·새로고침

ZSET member를 `user_id`로 두면 **같은 사용자는 한 줄만** 가질 수 있다.  
다만 **기본 `ZADD`는 기존 member의 score를 갱신**한다.  
연타 시 score가 바뀌면 **순위가 뒤로 밀리거나 앞으로 당겨질 수 있다.**

권장:

```text
ZADD key NX score member
```

- `NX`: member가 **없을 때만** 추가 → 연타해도 최초 순위 유지 (멱등)  
- 이미 있으면 0 반환, 순위는 그대로  

### 5.4 순번 조회

| 명령 | 의미 |
|------|------|
| `ZRANK key member` | 오름차순 순위 (0 = 맨 앞) |
| `ZREVRANK` | 내림차순이 필요할 때 |
| `ZCARD key` | 전체 인원 |

```text
ahead  = rank                 # 내 앞에 있는 사람 수
behind = total - rank - 1     # 내 뒤에 있는 사람 수 (대기열에 있을 때)
```

복잡도: Redis ZSET의 rank는 스키프리스트 기반으로 **대체로 O(log N)** 수준으로 빠르다.  
(정확한 Big-O는 버전·구현 문서를 따르되, “전체 스캔”은 아니다.)

**동일 score:** Redis는 score가 같으면 **member 문자열의 사전식 순서**로 정렬한다.  
“누가 먼저 줄 섰는지”와 무관한 순서가 섞일 수 있으므로, 선착순에는 **유일 단조 score(시퀀스)** 를 권장한다.

### 5.5 예시 — 대기열 클래스

```python
"""waiting_queue.py"""
from __future__ import annotations

import uuid
from typing import Any

import redis


class WaitingQueue:
    def __init__(self, r: redis.Redis, event_id: str):
        self.r = r
        # 해시 태그: Cluster에서 waiting/active/seq를 같은 슬롯에 두려면 {event_id} 사용
        self.key = f"{{{event_id}}}:waiting"
        self.seq_key = f"{{{event_id}}}:waiting:seq"

    def _next_score(self) -> int:
        # 단조 시퀀스 = 시계 스큐에 덜 민감
        return int(self.r.incr(self.seq_key))

    def enqueue(self, user_id: str | None = None) -> dict[str, Any]:
        uid = user_id or str(uuid.uuid4())
        # 이미 있으면 INCR 생략 가능 (§5.6). 여기서는 단순화.
        if self.r.zrank(self.key, uid) is not None:
            rank = self.r.zrank(self.key, uid)
            total = self.r.zcard(self.key)
            return {
                "user_id": uid,
                "newly_added": False,
                "rank": rank,
                "ahead": rank,
                "behind": (total - rank - 1) if rank is not None else None,
                "total": total,
            }
        score = self._next_score()
        added = self.r.zadd(self.key, {uid: score}, nx=True)
        rank = self.r.zrank(self.key, uid)
        total = self.r.zcard(self.key)
        return {
            "user_id": uid,
            "newly_added": bool(added),
            "rank": rank,
            "ahead": rank,
            "behind": (total - rank - 1) if rank is not None else None,
            "total": total,
        }

    def status(self, user_id: str) -> dict[str, Any] | None:
        rank = self.r.zrank(self.key, user_id)
        if rank is None:
            return None
        total = self.r.zcard(self.key)
        return {
            "user_id": user_id,
            "rank": rank,
            "ahead": rank,
            "behind": total - rank - 1,
            "total": total,
        }

    def leave(self, user_id: str) -> int:
        return int(self.r.zrem(self.key, user_id))
```

### 5.6 등록 시 자주 놓치는 디테일

1. **`INCR` 후 `ZADD NX`:** 이미 줄에 있는 사용자도 `INCR`을 먼저 호출하면 **시퀀스 번호가 낭비**된다.  
   최적화: `ZRANK`로 존재 확인 → 없으면 `INCR`+`ZADD NX`. (확인과 추가 사이에 레이스가 있으므로, 최종은 여전히 `NX`가 안전망.)
2. **로그인 전 UUID / 로그인 후 user_id 병합:** 익명으로 줄 선 뒤 로그인하면 member가 두 개가 될 수 있다.  
   → 로그인 시 **익명 토큰→계정 이관**(`ZREM` 익명 + `ZADD NX` 계정, 기존 score 유지) 정책을 미리 정한다.
3. **멀티 탭:** member=`user_id`면 같은 줄 공유(의도된 멱등). 탭마다 다른 UUID면 **한 사람이 여러 자리** — 봇과 동일 계열 남용.
4. **대기열 내구성:** Redis AOF/RDB·복제 지연 중 장애 시 줄이 유실될 수 있다.  
   “줄 복구 불가 허용 vs 재접수”를 제품 정책으로 명시하고, 가능하면 **복제본 + 장애 시 fail-closed(접수 중단)**.

---

## 6. 단계 2 — 입장 (Scheduler → Active)

### 6.1 역할

주기적 워커(예: 1초):

1. 부하 테스트로 정한 **배치 크기 N** (뒤단 TPS보다 여유 있게)  
2. 대기열 **앞에서 N명** 추출  
3. Active 저장소에 넣고 **입장 유효 시간(TTL)** 부여  

예: 뒤단이 초당 100건이면 60~80만 승격.

### 6.2 Active의 의미

- Active = **예매 UI 진입 자격**  
- 폴링이 `can_enter=true`를 보면 예매 페이지로 이동  
- TTL(관찰 사례로 수 분대): 방치·이탈 사용자 회수  

> “KTX가 내부적으로 3분 TTL”은 **강의 관찰/추정**이지 공식 공개 스펙이 아니다.  
> 운영에서는 상품·UX에 맞게 설정한다.

### 6.3 Active 누적 문제

스케줄러가 계속 올리면 Active가 쌓일 수 있다. 완화:

1. Active **TTL/만료 score**로 자동 제거  
2. 사용자별 예매 완료 시각이 달라 동시 확정이 완전 동기화되기 어렵다  
3. 최종 재고는 **Lua 한도**로만 통과  
4. (선택) MQ로 DB 쓰기 완충  

“대기열 이탈을 즉시 감지”하는 하트비트는 정확하지만,  
수백만 폴링마다 쓰기를 넣으면 **입장·예매와 자원 경쟁**이 난다 → §9.4.

### 6.4 원자성 — ZPOPMIN과 Active 추가를 분리하면 위험

```text
ZPOPMIN waiting N     # 줄에서 제거됨
(여기서 프로세스 크래시)
ZADD active ...       # 실행 못 함 → 사용자 유실
```

**잘못된 패턴:** 두 명령을 파이프라인으로만 이어 붙이기.  
**권장:** 한 Lua(또는 MULTI/EXEC + 주의 깊은 설계)로 “대기열 제거 + Active 등록”을 묶는다.

```lua
-- promote.lua
-- KEYS[1]=waiting  KEYS[2]=active
-- ARGV[1]=N  ARGV[2]=expire_at_unix

local n = tonumber(ARGV[1])
local exp = tonumber(ARGV[2])
local popped = redis.call('ZPOPMIN', KEYS[1], n)
local out = {}
for i = 1, #popped, 2 do
  local member = popped[i]
  redis.call('ZADD', KEYS[2], exp, member)
  table.insert(out, member)
end
return out
```

Redis Cluster에서는 **KEYS가 동일 해시 슬롯**이어야 한다.  
예: `{event123}:waiting`, `{event123}:active` 처럼 **해시 태그**를 맞춘다.

### 6.5 Active 저장 방식

| 방식 | TTL | 비고 |
|------|-----|------|
| `SET active:{event}:{uid} 1 EX 180` | 키 단위 TTL 네이티브 | 키 수 = Active 수 |
| `ZADD active:{event} expire_at uid` | score로 만료 시각 | 키 1개, `ZREMRANGEBYSCORE`로 청소 |
| `SADD`만 | 멤버 TTL 없음 | 만료 로직 별도 필요 |

**사실:** Redis는 ZSET **멤버별 EXPIRE를 지원하지 않는다.** TTL은 **키 단위**.  
멤버 만료는 score에 만료시각을 넣는 방식이 흔하다.

### 6.6 승격 후 상태 조회 — 자주 빠뜨리는 구멍

`ZPOPMIN`으로 승격하면 사용자는 **waiting에서 사라진다.**

```text
폴링이 waiting만 보면 → rank=null → “줄에서 사라짐?” 오해 / 버그
올바른 조회 순서:
  1) active인가? → can_enter=true (예매 UI)
  2) waiting인가? → rank/ahead 표시
  3) 둘 다 아님 → 만료·완료·미등록 등 상태 머신으로 분기
```

### 6.7 대안: Watermark(커서) 방식

대기열에서 **물리적으로 pop하지 않고**, “score ≤ S 인 사람까지 입장 가능”으로 표시할 수도 있다.

```text
admit_score = 현재까지 허용된 최대 score (또는 순번)
can_enter = (my_score <= admit_score)
ZRANK는 계속 유효 (줄에 남아 있음)
입장 완료·이탈 시에만 ZREM
```

| | ZPOPMIN → Active | Watermark |
|--|------------------|-----------|
| waiting 유지 | 승격 시 제거 | 유지 가능 |
| 구현 | Active 저장소 필요 | 커서 1개로 단순 |
| 중복 입장 방지 | Active+TTL | score 비교 + 예매 시 락 |
| 대규모 ZCARD | pop 후 감소 | 이탈 전까지 큼 |

둘 다 유효하다. 본 문서 기본안은 강의와 맞춰 **pop + Active**이고,  
폴링은 반드시 **active를 먼저** 본다.

### 6.8 “입장”과 “예매 확정”은 다른 층

- **입장(Active)** = UI에 들어갈 자격 (유량 제어)  
- **예매 확정** = 재고 차감 + 원장 기록  

Active에 올랐다고 좌석이 보장되는 것은 **아니다.**  
최종 보장은 Lua 재고(+ RDB)다. Active가 좌석 수보다 많아도,  
확정 단계에서 잔여가 없으면 실패한다 — 이게 의도된 백프레셔다.

---

## 7. 단계 3 — 예매 (원자적 좌석 차감)

### 7.1 왜 DB 락만으로 부족한가

`SELECT … FOR UPDATE`로도 재고 보호는 가능하다.  
다만 핫 로우에 대량 동시 접근이 모이면 DB가 다시 병목이다.  
**잔여 좌석 카운터**를 Redis에서 원자적으로 다루는 패턴이 널리 쓰인다.  
최종 원장은 여전히 RDB에 남기는 경우가 많다 (감사·정산·조인).

### 7.2 왜 `DECRBY`만으로 부족한가 (N장)

```text
잔여 2, 요청 3
DECRBY 3 → -1   # Redis는 음수를 허용
이후 보정(+1)…  # 레이스·복잡도
```

필요 로직은 **조건부 차감**:

```text
IF remaining >= need THEN remaining -= need; OK
ELSE FAIL
```

Redis 단일 명령만으로는 “N장 조건부”가 없어서,  
**Lua 스크립트**(또는 Redis Function)로 묶는 것이 일반적이다.

### 7.3 Lua 예시 (검증된 패턴)

Redis는 Lua 실행 동안 해당 인스턴스에서 **다른 명령과 인터리빙하지 않음**을 보장한다 (공식 문서: script atomic execution).  
→ check-and-set이 한 스크립트 안에서 안전하다.

```lua
-- KEYS[1] = remaining:<trip>
-- ARGV[1] = need
local remaining = tonumber(redis.call('GET', KEYS[1]) or '-1')
local need = tonumber(ARGV[1])
if remaining < 0 then
  return {-1, remaining}  -- not initialized
end
if remaining < need then
  return {0, remaining}
end
remaining = remaining - need
redis.call('SET', KEYS[1], remaining)
return {1, remaining}
```

```python
LUA_RESERVE = """
local remaining = tonumber(redis.call('GET', KEYS[1]) or '-1')
local need = tonumber(ARGV[1])
if remaining < 0 then return {-1, remaining} end
if remaining < need then return {0, remaining} end
remaining = remaining - need
redis.call('SET', KEYS[1], remaining)
return {1, remaining}
"""

def try_reserve(r: redis.Redis, trip_id: str, seats: int) -> tuple[int, int]:
    """returns (status, remaining) status: 1 ok, 0 fail, -1 uninitialized"""
    status, rem = r.eval(LUA_RESERVE, 1, f"remaining:{trip_id}", seats)
    return int(status), int(rem)
```

오픈 전 `SET remaining:<trip> <초기좌석>` 필수.

### 7.4 성공 후 RDB / MQ

```text
Lua 성공
  ├─ 동기: Booking API → RDB INSERT
  └─ 비동기: MQ publish → Worker → RDB
```

앞단에서 이미 N으로 유입을 줄였다면 RDB 직행도 가능하다.  
더 완충·재시도가 필요하면 MQ.

**정합성 함정:** Lua로 좌석은 깎였는데 RDB/MQ 기록이 실패하면 **재고만 줄고 주문 없음**.  
→ Outbox, 보상 트랜잭션(재고 복구), “예약 의도” 로그 필수 (§14).

### 7.5 결제·좌석 맵은 뒤로

폭주 구간에 결제·좌석 UI를 넣으면 타임아웃·취소·부분 실패가 폭증한다.  
실무적으로는:

1. **인원·열차(시간대) 홀드**만 폭주 경로에서  
2. **좌석 지정·결제**는 소수만 들어와 수행  

### 7.6 예매 확정 멱등성 (더블 클릭·재시도)

사용자는 “확정”을 연타하거나, 타임아웃 후 재시도한다.

```text
요청1: Lua 성공 → RDB 쓰기 전 타임아웃 → 클라 재시도
요청2: Lua 또 성공하면 좌석이 두 번 깎임  ← 위험
```

권장:

| 장치 | 역할 |
|------|------|
| `reservation_id` (클라/서버 발급, 멱등키) | 동일 키면 두 번째 요청은 **이전 결과 재사용** |
| Lua 안에서 `SET hold:{rid} 1 NX EX …` | 스크립트 진입 시 이미 홀드면 “이미 성공” 반환 |
| RDB UNIQUE(reservation_id) | 원장 이중 INSERT 차단 |
| 성공 후 Active에서 제거 | 같은 Active로 재예매 루프 방지 |

“재고 Lua만”이 아니라 **비즈니스 멱등키**가 있어야 완성이다.

### 7.7 재고 키 설계

```text
remaining:{trip_id}           # 열차·시간대 단위 카운터 (핫 키 주의)
remaining:{trip}:{cabin}      # 등급별 분리 시 핫 키 분산 + 정책 단순화
```

오픈 전 초기화·오픈 중 임의 `SET` 금지(운영 실수로 재고 폭증/증발).  
모니터링: 음수·초기화(-1)·급격한 점프 알림.

---

## 8. 특정 좌석 선점

좌석 번호까지 고를 때:

```text
SET seat:<trip>:<seat_id> <user_id> NX EX 180
```

- `NX`: 없을 때만 성공 → 원자적 선점  
- `EX`: 미완료 시 자동 해제  

```python
def hold_seat(r: redis.Redis, trip: str, seat: str, user_id: str, ttl: int = 180) -> bool:
    return bool(r.set(f"seat:{trip}:{seat}", user_id, nx=True, ex=ttl))
```

확정 시 RDB 반영 + 키 정리/전환.  
수량 카운터(Lua)와 좌석 키(`SET NX`)를 **함께** 쓸 때는 두 자원의 순서를 고정하고,  
실패 시 보상 순서를 문서화해야 한다.

---

## 9. 폴링 vs WebSocket vs SSE

### 9.1 비교

| 방식 | 모델 | 대규모 대기 순번 UI |
|------|------|---------------------|
| WebSocket | 연결 유지, 양방향 | 연결 수·메모리·LB 한도, **재접속 폭풍** 위험 |
| SSE | 서버→클라 스트림 | 연결 유지 부담 유사 |
| Polling | 요청 후 종료 (stateless) | **수평 확장 용이**, 구현 단순 |

대기열 UX는 “순번 / 입장 가능” 정도의 **단방향·주기 확인**이면 충분한 경우가 많다.

### 9.2 비용 직관

```text
WS 50만  = 전화기 50만 대 열어 두기
Poll 50만 = 짧은 편지를 나눠 처리
```

- 연결 유지: 소켓 객체·파일 디스크립터·LB max connection  
- 순간 단선 → 수십만이 동시에 reconnect ≈ **자기 DDoS**  
- 폴링 1회 ≈ `ZRANK` 수준의 **초경량**이면, “자주 물어도” 연결 유지보다 싼 구간이 생김  

**단정 금지:** WebSocket이 “항상 나쁘다”가 아니다.  
채팅·시세·게임처럼 **저지연 양방향**이 본질이면 WS/SSE가 맞다.  
**대규모·단순 상태 조회**에서는 폴링이 자주 이긴다.

### 9.2.1 Long Polling (중간 지점)

요청을 보낸 뒤 서버가 **변화가 있을 때까지 짧게 붙잡았다가** 응답하는 방식이다.

- 일반 폴링보다 즉시성에 가깝고  
- WS만큼 연결을 영구 유지하지는 않음  
- 다만 워커/커넥션을 수 초 점유하므로 **대기열이 수백만이면** 여전히 부담  

대규모 순번 UI의 1순위는 여전히 **짧은 주기 + 초경량 GET 폴링**인 경우가 많다.

### 9.3 Adaptive · Jitter · 서버 지시 TTL

1. 순번 멀면 주기↑, 가까우면 주기↓  
2. 사용자마다 난수(jitter)로 동시 폭주 완화  
3. 응답에 `ttl_ms`를 실어 **서버가 다음 폴링 시각을 통제** (SRT 관찰 패턴)

```javascript
async function pollLoop(userId) {
  while (true) {
    const res = await fetch(`/api/queue/status?uid=${encodeURIComponent(userId)}`);
    const data = await res.json();
    render(data);
    if (data.can_enter) {
      location.href = "/booking";
      return;
    }
    const delay = (data.ttl_ms ?? adaptiveDelay(data.ahead)) + Math.random() * 400;
    await new Promise((r) => setTimeout(r, delay));
  }
}

function adaptiveDelay(ahead) {
  if (ahead > 100000) return 10000;
  if (ahead > 10000) return 5000;
  if (ahead > 1000) return 2000;
  return 1000;
}
```

폴링 API 예:

```json
{
  "ahead": 15230,
  "behind": 410000,
  "total": 425231,
  "can_enter": false,
  "ttl_ms": 2000
}
```

### 9.4 이탈 감지 (Heartbeat) vs 허용

| 접근 | 장점 | 단점 |
|------|------|------|
| 폴링마다 생존 갱신 | 죽은 클라 빠른 제거 | ZSET 멤버 TTL 불가 → 유저별 키 폭증 또는 쓰기 폭증 |
| 허수 허용 + Active TTL | 읽기 위주, 단순 | Active에 빈 자리 잠깐 존재 |

**사실 확인:** Redis `EXPIRE`는 **키 단위**. ZSET 멤버마다 TTL을 네이티브로 걸 수 없다.  
채팅/게임의 presence와 대기열은 요구가 다르다.

### 9.5 Sticky 세션과 폴링

폴링 자체는 **stateless**라 sticky가 필수는 아니다.  
다만 SRT 관찰처럼 **상용 대기열 노드·세션 친화성**이 있으면 sticky 쿠키가 보일 수 있다 (§12).  
자작 Redis 대기열 + 공유 Redis면 API는 아무 인스턴스로 가도 된다.  
sticky에 의존한 로컬 메모리 대기열은 **노드 장애 시 줄 유실**이므로 비권장.

---

## 10. 대기열에 Kafka/MQ가 아닌 Redis인 이유

MQ도 FIFO로 “처리 순서”는 만들 수 있다.  
그러나 **대기열 UX·운영**까지 포함하면 ZSET이 맞는 경우가 많다.

| 기능 | Redis ZSET | 전형적 MQ (Kafka 등) |
|------|------------|----------------------|
| 내 순번 즉시 | `ZRANK` | 네이티브 약함 (파티션 오프셋 ≠ 유저 순위) |
| 전체 인원 | `ZCARD` | 별도 카운터/외부 저장소 |
| 연타 멱등 | member=user + `ZADD NX` | 메시지 중복 → 컨슈머 중복제거 |
| 중간 퇴장 | `ZREM` | 중간 삭제 어려움 (토픽 특성) |
| 앞에서 N명 | `ZPOPMIN N` | consume은 가능 |
| 본업 | 실시간 랭킹·상태 | 로그 스트리밍·비동기 버퍼 |

비유:

- MQ ≈ **터널** — 줄은 서지만 터널 안 “몇 번째?”는 알기 어렵다  
- ZSET ≈ **랭킹 명단** — 줄 자체를 데이터 구조로 든다  

**실무 조합 (자주 쓰임):**

```text
앞단 Redis ZSET = 대기 조회·관리
뒷단 MQ        = 예매 성공 후 RDB/알림/로그 비동기
```

“Kafka로 대기열 절대 불가”는 **과장**.  
가능하지만 **순위 UI를 위해 외부 인덱스(Redis 등)를 또 두는** 경우가 많아,  
처음부터 ZSET으로 대기열을 잡는 편이 단순하다.

---

## 11. 인프라·논블로킹 API

앱만 잘 짜도 NIC·커널 큐·프록시 커넥션이 막히면 무의미하다.

| 계층 | 예시 |
|------|------|
| OS/NIC | somaxconn, backlog, 버퍼 |
| LB/Nginx | worker_connections, keepalive, upstream |
| API | WebFlux/Netty 등 **비블로킹**으로 Redis I/O 대기 시 스레드 절약 |

대기열 API는 CPU보다 **Redis RTT 대기**가 병목인 경우가 많다.

### 11.1 Redis 장애 시 정책

| 정책 | 동작 | 비고 |
|------|------|------|
| **fail-closed** | 접수·폴링·예매 중단/대기 안내 | 재고·순서 보호에 안전. UX는 나쁨 |
| fail-open | Redis 없이 예매 직행 | **초과 예매·불공정** 위험. 대기열 목적과 충돌 |
| degraded | 읽기 전용 안내 + 신규 접수 중지 | 타협안 |

대기열의 존재 이유는 RDB 보호이므로, 앞단 Redis가 죽으면 **예매를 열어 버리는 것**은 대개 최악의 선택이다.

### 11.2 스케줄러(승격 워커) 이중 기동

워커가 2개면 `ZPOPMIN`이 **2N** 승격될 수 있다(Lua로 묶여도 배치가 두 번).

대응:

- Redis `SET promote:lock:{event} NX EX …` 리더 선출  
- 또는 k8s/리더 일렉션으로 **단일 스케줄러**  
- N을 동적으로 줄이는 피드백 루프와 병행  

---

## 12. 실측 관찰 (SRT 폴링 — 추론)

강의에서 개발자 도구로 SRT 대기 화면을 본 **관찰·추론**이다.  
**공식 스펙이 아니다.**

관찰로 언급된 포인트:

| 관찰 | 해석(가설) |
|------|------------|
| 상용 대기열(NetFunnel 등) 흔적 | 직접 Redis를 안 짜고 **검증된 대기열 제품**을 쓸 수도 있음 |
| HTTP/1.1 | 구형 브라우저·호환성 고려 가능 |
| 폴링 | WS/SSE가 아님 |
| 응답에 TTL | **다음 폴링 주기 서버 통제** |
| ahead / behind 유사 필드 | 내 앞·뒤 인원 UX |
| host ≠ referer | 대기열 도메인과 예매 도메인 분리 |
| Content-Type: JavaScript | JSONP 스타일 또는 서버가 실행 스크립트를 내려줌 |
| sticky 쿠키 흔적 | LB에서 동일 노드 고정 |
| 응답 필드 TPS | 일반 TPS가 아니라 **입장 처리율(네퍼널 내부)** 가설 |
| 응답마다 key 갱신 | 대기 토큰 회전·재사용 방지 가설 |
| nWait / nNext | ahead / behind UX |

해석 후보 (확정 아님):

1. 레거시·크로스 도메인 → JSONP  
2. 서버가 콜백/함수 호출을 내려 **클라 제어**  
3. sticky로 세션 친화성  

**교훈:** 폴링 + 서버 지시 주기 + (필요 시) 상용 대기열은 실전 선택지다.  
“반드시 자작 Redis”가 유일한 답은 아니다.

---

## 13. 엔드투엔드 시퀀스

```text
T0  수백만 클릭 “접수”
      → Queue API: id 발급, ZADD NX waiting (RDB 미접촉)
      → Client: id 저장, /status 폴링

T1  Scheduler (예: 1s)
      → Lua: ZPOPMIN N + Active 등록(만료시각)
      
T2  Client can_enter=true
      → 예매 UI (인원·열차 선택)

T3  확정
      → Lua: remaining 조건부 차감
      → 성공 시 RDB 또는 MQ→RDB
      → 실패 시 안내 / 재시도 정책

T4  (소수) 좌석 지정 · 결제
```

오픈 전: N, TTL, 폴링 주기, Redis 메모리, LB 한도, **봇 E2E 부하 테스트**.

---

## 14. 실패·정합성·운영 함정

| 함정 | 내용 | 대응 |
|------|------|------|
| 시계 스큐 | API 서버별 `time()` score | Redis TIME / INCR 시퀀스 |
| ZADD score 갱신 | 연타 시 순위 변동 | `ZADD NX` |
| promote 비원자 | pop 후 크래시로 유실 | Lua로 waiting↔active 묶기 |
| Cluster 슬롯 | Lua 다중 키 거부 | 해시 태그 `{event}:*` |
| 핫 키 | 초대형 단일 ZSET | 이벤트/구간 샤딩, 상용 대기열 |
| Lua 성공·DB 실패 | 재고만 감소 | Outbox, 보상, 멱등 주문키 |
| Active 허수 | 이탈자 자리 | TTL + 재고 Lua가 최종 방어 |
| MQ 중복 | at-least-once | 주문 idempotency key |
| 폴링 쓰로틀 | 동일 주기 동기화 | jitter + 서버 ttl_ms |
| 승격 후 waiting만 조회 | rank=null 오동작 | **active 먼저** 조회 (§6.6) |
| Lua 스크립트 과다 | Redis 싱글스레드 블로킹 | 스크립트 짧게, 배치 제한 |
| 동일 score | 순위가 member 사전순에 의존 | 시퀀스로 score 유일화 |
| Active=좌석 보장으로 오해 | 과매 착각 | 확정은 Lua+RDB (§6.8) |
| 예매 더블 클릭 | 좌석 이중 차감 | reservation_id + Lua NX 홀드 (§7.6) |
| Redis 장애 fail-open | 대기열 우회 폭주 | fail-closed / degraded (§11.1) |
| 익명→로그인 member 불일치 | 한 사람 두 줄 | 이관 정책 (§5.6) |
| promote 워커 2중 | 입장량 2N | 리더 락 (§11.2) |
| 시각 score 정밀도/스큐 | 순서 뒤틀림 | INCR 시퀀스 |

### 14.1 Outbox / 보상 (조금 더)

권장 순서 예:

```text
1) reservation_id(멱등키) 발급
2) Lua: 재고 차감 + (가능하면) “홀드 로그” 키 기록
3) Outbox 테이블/스트림에 “RDB에 쓸 주문” 기록 (같은 트랜잭션 경계 설계)
4) 워커가 RDB INSERT (idempotent)
5) 실패 시: 재시도 → 최종 실패 시 재고 복구 Lua
```

“Lua만 성공하고 끝”은 운영에서 재고 드리프트를 만든다.

### 14.2 MULTI/EXEC vs Lua

| | MULTI/EXEC | Lua |
|--|------------|-----|
| 여러 명령 일괄 | 가능 | 가능 |
| 중간에 읽은 값으로 분기 | `WATCH` 필요 (낙관적) | 스크립트 안에서 자연스러움 |
| N장 조건부 차감 | WATCH+재시도 복잡 | **한 스크립트가 단순** |

대기열 승격·재고 차감처럼 **read-modify-write**는 Lua가 실무에서 흔하다.

### 14.3 수량 카운터 + 좌석 `SET NX` 순서

둘을 같이 쓰면 **부분 성공**이 난다.

권장 순서 예:

```text
1) Active/토큰 검증
2) Lua: remaining 조건부 차감 (또는 “홀드 수량”)
3) SET seat:… NX EX  (실패 시 remaining 복구)
4) 확정 시 RDB + seat 키 전환(홀드→확정) / TTL 제거
```

반대로 좌석만 잡고 수량 차감에 실패하면 **유령 선점**이 생긴다.  
실패 경로마다 보상 순서를 표로 문서화하고 테스트한다.

---

## 15. 보안·클라이언트 오동작·관측

### 15.1 식별자 보안

- 단순 추측 가능한 순번을 URL에 노출하지 말 것  
- UUID만으로도 부족할 수 있음 → **서명된 대기 토큰**(HMAC) + 만료  
- 폴링마다 토큰 재발급(회전)하면 재사용·공유 링크 악용을 완화 (SRT 관찰과 유사한 UX)  
- Active/예매 API는 토큰 검증 필수 (위조 `can_enter` 방지)

### 15.2 “비행기 모드 후 바로 뚫림”류

클라이언트 버그·페일오픈 가능성이 높다.

```text
폴링 실패 N회
  → (잘못된 클라) “대기열 장애”로 간주
  → 예매 페이지로 강제 이동  ← 위험 (fail-open)
```

서버는 **반드시 active/토큰으로만** 예매 API를 열어줘야 한다.  
클라 리다이렉트만 믿으면 우회된다.

정상 타이밍과 겹치면 “운 좋게 입장”처럼 보일 수도 있다.  
반복 재현되면 **클라 fail-open + 서버 검증 부재**를 의심한다.

### 15.3 청크 응답·연결 유지 (관찰)

대기 중 다음 단계 HTML/데이터가 늦을 때, 서버가 **작은 페이로드를 주기적으로** 내려  
브라우저 타임아웃·프록시 idle cut을 피하는 사례가 보고된다.  
(청크드 전송 / keepalive 트릭 — 구현·추론이 갈릴 수 있음)

요지: **장시간 대기 UX**에서는 “한 방에 큰 응답”보다  
연결·타임아웃 전략을 따로 설계해야 한다.

### 15.4 관측 지표

| 지표 | 용도 |
|------|------|
| waiting ZCARD | 줄 길이 |
| promote N / 초 | 입장률 |
| active 수 | 창구 앞 혼잡 |
| Lua 성공/실패율 | 재고 고갈·오류 |
| 폴링 QPS·p99 | 상태 API 용량 |
| DB/MQ 지연·재시도 | 원장 정합 |
| 예매 확정 TPS | 뒤단 한계 |

---

## 16. 용량 산정·부하 테스트

### 16.1 입장 배치 N

```text
N ≈ floor( 뒤단_지속가능_TPS × safety_factor )
safety_factor 예: 0.6 ~ 0.8
```

뒤단 = 예매 API + Redis Lua + (RDB 또는 MQ 컨슈머).  
**병목이 바뀌면 N을 다시 잰다.**

### 16.2 폴링 QPS (대략)

```text
poll_qps ≈ waiting_users / avg_interval_sec
예: 50만 명 × (1/5초) ≈ 10만 QPS
```

adaptive면 평균 간격이 커져 QPS↓.  
상태 API는 Redis rank만 타게 해 **DB를 보지 말 것**.

### 16.3 오픈 전 테스트

1. 봇으로 접수→폴링→입장→예매 E2E  
2. 연타·중복 UUID·토큰 위조  
3. Redis failover / 클러스터 슬롯 이동  
4. MQ 지연·중복 컨슘  
5. 재고 0 근처 레이스  
6. 스케줄러 이중 기동(리더 선출) — **promote 이중 실행** 방지  

상용 대기열(NetFunnel 등)을 쓰면 앱 부담은 줄지만,  
**연동·장애·비용·벤더 종속성** 트레이드오프는 남는다.

---

## 17. 예시 코드 모음

### 17.1 입장 워커 (Lua 사용 권장)

```python
PROMOTE_LUA = """
local n = tonumber(ARGV[1])
local exp = tonumber(ARGV[2])
local popped = redis.call('ZPOPMIN', KEYS[1], n)
local out = {}
for i = 1, #popped, 2 do
  local member = popped[i]
  redis.call('ZADD', KEYS[2], exp, member)
  table.insert(out, member)
end
return out
"""

def promote(r: redis.Redis, event_id: str, n: int, ttl_sec: int) -> list[str]:
    import time
    waiting = f"{{{event_id}}}:waiting"
    active = f"{{{event_id}}}:active"
    exp = time.time() + ttl_sec
    members = r.eval(PROMOTE_LUA, 2, waiting, active, n, exp)
    # 만료 청소 (lazy)
    r.zremrangebyscore(active, "-inf", time.time())
    return [m.decode() if isinstance(m, bytes) else m for m in members]

def is_active(r: redis.Redis, event_id: str, user_id: str) -> bool:
    import time
    score = r.zscore(f"{{{event_id}}}:active", user_id)
    return score is not None and float(score) >= time.time()
```

### 17.2 상태 API 의사코드

```python
def queue_status(r, event_id: str, user_id: str) -> dict:
    """반드시 active를 먼저 본다. (ZPOPMIN 승격 후 waiting에 없음)"""
    if is_active(r, event_id, user_id):
        return {
            "state": "active",
            "can_enter": True,
            "ahead": 0,
            "ttl_ms": 1000,
        }
    q = WaitingQueue(r, event_id)
    st = q.status(user_id)
    if st is None:
        return {"state": "absent", "can_enter": False, "ttl_ms": 3000}
    return {
        "state": "waiting",
        **st,
        "can_enter": False,
        "ttl_ms": 2000 if st["ahead"] > 1000 else 1000,
    }
```

---

## 18. 토이 시뮬레이터 (전체)

Redis 없이 3단계 개념을 검증하는 인메모리 모델.

```python
"""sim_ticketing.py — 개념 검증용 (프로덕션 아님)"""
from __future__ import annotations

import heapq
import time
import uuid
from dataclasses import dataclass, field


@dataclass(order=True)
class WaitItem:
    score: float
    user_id: str = field(compare=False)


class ToySystem:
    def __init__(self, seats: int, admit_per_tick: int = 3, active_ttl: float = 3.0):
        self.waiting: list[WaitItem] = []
        self.active: dict[str, float] = {}
        self.remaining = seats
        self.admit_per_tick = admit_per_tick
        self.active_ttl = active_ttl
        self.db_orders: list[tuple[str, int]] = []
        self._seq = 0

    def enqueue(self) -> str:
        self._seq += 1
        uid = str(uuid.uuid4())[:8]
        heapq.heappush(self.waiting, WaitItem(float(self._seq), uid))
        return uid

    def rank(self, user_id: str) -> int | None:
        for i, item in enumerate(sorted(self.waiting)):
            if item.user_id == user_id:
                return i
        return None

    def tick_admit(self) -> list[str]:
        now = time.time()
        self.active = {u: exp for u, exp in self.active.items() if exp > now}
        promoted: list[str] = []
        for _ in range(self.admit_per_tick):
            if not self.waiting:
                break
            item = heapq.heappop(self.waiting)
            self.active[item.user_id] = now + self.active_ttl
            promoted.append(item.user_id)
        return promoted

    def try_book(self, user_id: str, seats: int) -> bool:
        now = time.time()
        if self.active.get(user_id, 0) < now:
            return False
        if self.remaining < seats:
            return False
        self.remaining -= seats
        self.db_orders.append((user_id, seats))
        del self.active[user_id]
        return True


if __name__ == "__main__":
    sys = ToySystem(seats=10, admit_per_tick=2)
    users = [sys.enqueue() for _ in range(8)]
    print("first ranks:", {u: sys.rank(u) for u in users[:3]})

    for t in range(5):
        got = sys.tick_admit()
        print(
            f"tick{t} promote={got} "
            f"wait={len(sys.waiting)} active={list(sys.active)}"
        )
        for u in list(sys.active):
            if sys.try_book(u, 1):
                print(f"  booked {u}, seats_left={sys.remaining}")

    print("DB orders:", sys.db_orders)
```

```bash
python3 sim_ticketing.py
```

---

## 19. 설계 체크리스트

- [ ] 오픈 순간 쓰기가 RDB로 직행하지 않는가  
- [ ] score가 시계 스큐에 안전한가 (`NX` + 시퀀스/Redis TIME)  
- [ ] 연타가 순위를 바꾸지 않는가 (`ZADD NX`)  
- [ ] 입장 N이 부하 테스트 기반인가  
- [ ] waiting→active 승격이 원자적인가 (Lua + 해시 태그)  
- [ ] **폴링이 active를 먼저 조회하는가** (pop 이후 waiting 부재)  
- [ ] Active 만료 정책이 있는가  
- [ ] Active ≠ 좌석 보장임을 제품/API가 반영하는가  
- [ ] N장 예매가 조건부 원자 차감인가 (Lua)  
- [ ] Lua 성공 후 DB/MQ 실패 보상·멱등키가 있는가  
- [ ] 예매 API가 클라 리다이렉트가 아닌 **서버 토큰/active**로 인가하는가  
- [ ] 결제·좌석 맵이 폭주 경로 밖인가  
- [ ] 폴링이 adaptive/jitter/서버 ttl을 쓰는가  
- [ ] 대기열(ZSET)과 후처리(MQ) 역할이 분리됐는가  
- [ ] 스케줄러 단일 리더(이중 promote 방지)가 있는가  
- [ ] LB/커널/Redis 핫 키까지 포함한 통합 부하 테스트·봇 시나리오가 있는가  
- [ ] 예매 **멱등키**(더블 클릭·타임아웃 재시도)가 있는가 (§7.6)  
- [ ] Redis 장애 시 **fail-closed/degraded**가 정의됐는가 (§11.1)  
- [ ] 익명 UUID → 로그인 이관 정책이 있는가 (§5.6)  
- [ ] 수량 차감 + 좌석 NX **보상 순서**가 테스트됐는가 (§14.3)  

---

## 20. 팩트체크·정정 노트

이 섹션은 합본 과정에서 **틀리거나 과장되기 쉬운 말**을 고친 기록이다.

| # | 이전/위험 표현 | 판정 | 교정 |
|---|----------------|------|------|
| 1 | RDB는 스케일 아웃이 안 된다 | 과장 | 단일 primary **쓰기** 한계가 핵심. 복제·샤딩은 존재 |
| 2 | ZADD만 하면 연타해도 순위 유지 | **틀림** (기본은 score 갱신) | **`ZADD NX`** 필요 |
| 3 | ZSET 멤버에 TTL을 건다 | **틀림** | 키 TTL만 네이티브. 멤버는 score 만료 패턴 |
| 4 | ZPOPMIN 후 파이프라인으로 active 추가면 충분 | 위험 | 크래시 시 유실 → **Lua로 묶기** |
| 5 | Lua는 항상 전역 원자 | 부분 맞음 | 인스턴스 내 원자. **Cluster는 동일 슬롯 키만** |
| 6 | DECRBY로 N장 예매 | 부적절 | 음수·레이스 → **조건부 Lua** |
| 7 | WebSocket은 대규모에 쓰면 안 된다 | 과장 | 대기 **순번 조회**에선 폴링이 유리한 경우가 많음 |
| 8 | Kafka로는 대기열 불가 | 과장 | 가능하나 순위 UX에 외부 상태 저장이 필요해지는 경우가 많음 |
| 9 | KTX가 Redis ZSET+Lua를 쓴다 | **미검증** | 공개 아키텍처 아님. **패턴 설명**으로 읽을 것 |
| 10 | SRT가 JSONP인 이유 | 추론 | 패킷 관찰 기반 가설. 확정 아님 |
| 11 | 폴링이 항상 더 싸다 | 조건부 | 요청이 무거우면(매 폴링 DB) 오히려 나쁠 수 있음. **초경량+주기 조절**이 전제 |
| 12 | Lua 성공 = 예매 완료 | 불완전 | RDB/MQ 기록·보상까지가 완료 |

| 13 | promote 후 waiting만 보면 된다 | **틀림** | active 우선 (§6.6) |
| 14 | Active면 좌석 확정 | **틀림** | 유량 자격일 뿐. 확정은 재고 Lua |
| 15 | MULTI면 조건부 차감 OK | 불충분하기 쉬움 | read-modify-write는 Lua/`WATCH` 설계 필요 |
| 16 | 클라가 예매 페이지로만 보내면 충분 | **위험** | 서버 인가 없이 fail-open |
| 17 | ZRANK는 항상 “대기 시간 순” | 조건부 | **동일 score**면 member 문자열 순. 시퀀스 권장 |
| 18 | 자작 Redis만이 정답 | 과장 | NetFunnel 등 상용 대기열도 선택지 |
| 19 | 같은 score면 ZADD가 자동 멱등 | **불완전** | 멤버 중복은 막지만, **기본 ZADD는 score 갱신**. 순위 고정은 **`NX`** |
| 20 | Redis 죽으면 예매 열어라 | **위험** | fail-closed가 대기열 목적과 맞음 (§11.1) |
| 21 | INCR마다 호출해도 문제없다 | 낭비·미세 이슈 | 기존 유저는 rank 조회로 스킵 (§5.6) |
| 22 | 스케줄러 여러 대 = 더 좋음 | 조건부 | 락 없으면 입장량 폭주 (§11.2) |

### 공식·문서에 가까운 근거

- Redis Lua: 스크립트 실행의 atomic semantics (Redis programmability 문서)  
- ZSET 멤버 TTL 미지원: Redis EXPIRE는 키 단위 (커뮤니티·문서 공통)  
- `ZADD NX`, `ZPOPMIN`, `ZRANK`, `SET NX EX`: Redis 명령 스펙  
- Cluster Lua: 동일 슬롯 키만 (hash tags)  
- 동일 score 시 member 사전식 정렬: Redis Sorted Set 스펙

### 이 문서의 범위

```text
맞는 것: 유량 제어 + 대기 상태 + 원자 재고라는 설계 축
틀린 읽기: “이 글 = KTX 소스코드”
```

---

## 21. 재검토 로그 (누락·오류)

이번 패스에서 **원본 강의·Q&A 대비 빠졌거나 위험했던 점**을 보완했다.

### 누락 → 추가

| 항목 | 위치 |
|------|------|
| 승격 후 waiting 부재 → 폴링 버그 | §6.6, §17.2 |
| Watermark 대안 | §6.7 |
| Active ≠ 좌석 보장 | §6.8 |
| Outbox/보상 절차 | §14.1 |
| MULTI vs Lua | §14.2 |
| 토큰·서명, fail-open(비행기 모드류) | §15 |
| 청크/장시간 대기 UX | §15.3 |
| 관측 지표·N·폴링 QPS 산정 | §16 |
| 봇 부하 테스트·스케줄러 리더 | §16.3, §19 |
| 상용 대기열(NetFunnel) 선택지 | §12, §16.3 |
| WaitingQueue 해시 태그 키 | §5.5 |

### 오류·위험 표현 → 교정

| 내용 | 조치 |
|------|------|
| 상태 API가 waiting만 봄 | active 우선으로 수정 |
| 키 네이밍이 Cluster Lua와 불일치 | `{event}` 태그 통일 |
| 팩트체크에 승격/Active 오해 미기재 | §20 행 13–18 추가 |
| 본문 §9.5 참조(없는 절) | §9.4로 수정 |
| §20 “공식 근거” 블록 중복 | 하나로 병합 |
| 강의 “같은 score면 ZADD 멱등” | **불완전** → `ZADD NX` + 시퀀스 유일 score로 정정 (§20-19) |
| 스키프리스트 표기 중 키릴 문자 혼입 | “스키프리스트”로 교정 |

### 2차 재검토에서 추가

| 항목 | 위치 |
|------|------|
| 동일 score → member 사전순 | §5.4 |
| 시각 score 정밀도·스큐 | §5.2 |
| INCR 낭비·로그인 이관·내구성 | §5.6 |
| 예매 멱등·재고 키 | §7.6–7.7 |
| Long polling | §9.2.1 |
| Sticky vs 공유 Redis | §9.5 |
| Redis 장애 fail-closed | §11.1 |
| promote 리더 락 | §11.2 |
| SRT TPS/key 회전 필드 | §12 |
| 좌석+수량 보상 순서 | §14.3 |
| 팩트체크 19–22 | §20 |

---

## 끝 — 기억용 공식

```text
대규모 선착순 예매
  = 대기열로 흡수
  + 입장량으로 유량 제어
  + 원자적 재고 차감
  + 무거운 진실은 RDB (가능하면 비동기)
  + 상태 조회는 가벼운 폴링
  + 확정은 멱등키로 끝내기
  + 앞단 장애 시 fail-closed

연결을 유지하는 비용 > 자주 묻는 비용
  이 성립하는 구간 → 대기열에 폴링
```
