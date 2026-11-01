# 대규모 선착순 예매 시스템의 접속 대기열 구조 분석과 Bun 내장 API 구현

## 요약

명절 승차권 예매는 전 국민이 같은 시각에 한정된 좌석을 두고 동시에 접속하는, 국내에서 가장 극단적인 형태의 선착순 트래픽이다. 2026년 추석 승차권 예매는 9월 7일부터 11일까지 진행되며, 이 기간에는 예매 서버의 처리 능력을 크게 넘는 접속이 짧은 시간에 몰린다. 본 보고서는 브라우저 개발자 도구로 관찰할 수 있는 접속 대기 화면의 요청과 응답을 근거로 대기열의 동작 구조를 재구성하고, 이를 Bun 내장 API만으로 구현하는 방법을 제시한다.

관찰된 통신에는 세 가지 요청 필드가 있다. 대기열 시스템이 정의한 기능 식별자로 보이는 `opcode`, 사용자 식별값을 암호화·해싱한 것으로 보이는 `key`, 다중 서버 중 특정 서버로 요청을 고정하는 `sticky`다. 클라이언트는 응답에 담긴 TTL 값(실제 대기열 2초, 대기가 없는 상태 5초)마다 상태를 다시 조회하는 폴링 방식으로 동작하며, 응답에는 내 앞 대기 인원, 내 뒤 대기 인원, 초당 처리량(TPS), TTL이 포함된다. 실제 대기열 응답은 JSONP 형식이었고, 예매 화면에 입장한 뒤에는 3분 안에 예매를 마쳐야 하며 시간이 지나면 자동으로 퇴장된다.

이 구조는 입장 제어(admission control)로 요약된다. 대기열은 예매 서버가 감당할 수 있는 속도로만 사용자를 입장시키고, 입장한 사용자의 체류 시간을 3분으로 제한해 동시 사용자 수의 상한을 보장한다. 서버가 응답마다 TTL을 내려 폴링 주기를 조절하는 방식은 대기 인원이 수십만 명일 때 대기열 서버 자체가 무너지지 않게 하는 핵심 장치다. 구현 예제는 `Bun.serve`의 라우트와 WebSocket, `bun:sqlite`, Bun 내장 `RedisClient`, Web Crypto, `bun:test`만 사용하며 외부 패키지에 의존하지 않는다.

## 1. 배경과 분석 방법

### 1.1 명절 승차권 예매 트래픽의 특성

| 항목 | 특성 |
| --- | --- |
| 기간 | 2026년 추석 승차권 예매, 9월 7일 ~ 11일 |
| 대상 | 전 국민, 노선·일자별 예매 시작 시각에 동시 접속 |
| 자원 | 열차별 좌석이라는 한정되고 대체 불가능한 재고 |
| 부하 형태 | 예매 시작 직후 수 초 안에 최대치에 도달하는 스파이크 |
| 실패 비용 | 중복 판매, 결제 후 좌석 없음, 서버 다운 시 전 국민적 불만 |

일반적인 웹 서비스의 부하는 시간대에 따라 완만하게 오르내리지만, 선착순 예매는 시작 시각 직전까지 거의 0이었다가 시작과 동시에 평소의 수백 배로 치솟는다. 오토 스케일링으로 서버를 늘리기에는 시간이 부족하고, 늘린다 해도 좌석 재고라는 공유 자원은 늘어나지 않는다. 따라서 처리 능력을 늘리는 대신 **처리 능력에 맞춰 유입을 줄 세우는** 접근, 즉 접속 대기열이 필요하다.

### 1.2 분석 방법: 브라우저 개발자 도구

대기열 시스템의 내부 구현은 공개되어 있지 않다. 그러나 브라우저와 대기열 서버 사이의 통신은 크롬 개발자 도구의 네트워크 탭으로 관찰할 수 있다. 요청 파라미터와 응답 본문의 값을 보면 시스템이 대기열을 어떻게 구현하고, 사용자의 차례를 어떻게 확인하는지 상당 부분 추론할 수 있다.

```text
접속 대기 화면 진입
  → 개발자 도구 네트워크 탭에서 대기열 서버로 가는 요청 확인
  → 요청 파라미터(opcode, key, sticky 등) 확인
  → 응답 본문(대기 인원, TPS, TTL 등) 확인
  → 요청 간격과 응답 값의 변화로 동작 구조 추론
```

본 보고서의 2장은 이렇게 관찰된 사실이고, 4장 이후는 관찰을 근거로 재구성한 설계다. 재구성한 내용은 실제 내부 구현과 다를 수 있다.

### 1.3 관찰 환경

| 환경 | 관찰 내용 |
| --- | --- |
| 사전 체험 서비스 | 로그인을 요구한다. 본 예매는 사전 로그인을 하지 않도록 되어 있어 차이가 있다. |
| 대기열 표시 | 체험 사용자가 적어 대기열 없이 곧바로 예매 화면으로 넘어간다. 체험에서는 대기 상태를 경험할 수 없다. |
| 클라이언트 경고 | 뒤로 가기나 개발자 도구를 사용하면 로그아웃될 수 있다는 안내가 표시된다. |
| 세션 제한 | 예매 화면 입장 후 3분 안에 예매를 마쳐야 하며, 3분이 지나면 자동으로 퇴장된다. |

본 예매에서 사전 로그인을 막는 이유는 공개되어 있지 않다. 예매 시작 전에 로그인 세션을 미리 확보해 두는 행위를 막고, 인증 요청이 대기열 통과 이후로 분산되게 하려는 목적으로 추정할 수 있다.

## 2. 관찰 결과

### 2.1 요청 파라미터

| 필드 | 관찰 | 추정 역할 |
| --- | --- | --- |
| `opcode` | 요청마다 포함되는 코드 값 | 대기열 시스템이 정의한 기능 식별자. 하나의 엔드포인트에서 진입, 상태 조회, 완료 등을 구분 |
| `key` | 사용자별로 다른 긴 문자열 | 사용자 식별값과 대기 정보를 암호화·해싱한 값. 번호표 역할 |
| `sticky` | 서버를 가리키는 값 | 다중 로드 밸런싱 서버 중 예매 시점에 배정된 특정 서버로 이후 요청을 고정 |

`sticky`가 존재한다는 것은 대기 상태가 서버마다 따로 관리될 가능성을 시사한다. 사용자의 대기 정보가 특정 서버의 메모리에 있다면, 그 사용자의 이후 요청은 반드시 같은 서버로 가야 한다.

### 2.2 폴링 주기

클라이언트는 서버에 연결을 유지하지 않고, 일정 간격으로 상태를 다시 묻는 폴링 방식으로 동작한다.

| 상태 | 폴링 간격 | 응답의 TTL 값 |
| --- | --- | --- |
| 대기열이 없는 예매 화면 (사전 체험) | 약 5초 | 5 |
| 실제 대기열 | 약 2초 | 2 |

요청 간격이 응답의 TTL 값과 일치한다. 즉 클라이언트가 폴링 주기를 스스로 정하는 것이 아니라 **서버가 응답마다 다음 조회 시점을 지시**한다.

### 2.3 응답 필드

| 필드 | 대기열 없는 상태의 값 | 실제 대기열에서의 의미 |
| --- | --- | --- |
| 내 앞 대기 인원 | 의미 있는 값 없음 | 내 차례까지 남은 사람 수 |
| 내 뒤 대기 인원 | 의미 있는 값 없음 | 나보다 늦게 들어온 사람 수 |
| TPS | 0 | 초당 입장(처리) 인원 |
| TTL | 5 | 다음 상태 조회까지의 초 단위 간격 |

대기열 없는 상태의 응답은 대기 상태가 아니라 예매 화면에 입장한 뒤의 응답이므로 TPS가 0이고 대기 인원 값도 의미가 없다. 실제 대기열에서는 내 앞과 뒤의 인원, 초당 처리량이 채워지고 TTL은 2초로 짧아진다. 클라이언트는 이 값으로 "앞에 몇 명, 예상 대기 시간 몇 분" 같은 화면을 그린다.

### 2.4 응답 형식

실제 대기열에서는 응답이 JSONP 형식이었고, 이때도 `sticky` 값이 유지되었다. JSONP는 다른 도메인의 서버에서 데이터를 받기 위해 `<script>` 태그로 자바스크립트 함수 호출 형태의 응답을 받는 방식이다. 대기열 서버가 예매 서비스와 다른 도메인에서 운영되며, CORS가 보편화되기 전부터 이어진 호환 방식을 유지하고 있는 것으로 볼 수 있다.

### 2.5 세션 타임아웃

예매 화면에 입장한 사용자는 3분 안에 예매를 마쳐야 한다. 3분이 지나면 서버가 자동으로 세션을 끊고 화면에서 퇴장시킨다. 이 제한은 4.4에서 설명하듯 동시 사용자 수의 상한을 보장하는 장치다.

### 2.6 관찰의 한계

- 대기열 서버와 예매 서버의 내부 구현, 저장소, 서버 대수는 알 수 없다.
- 사전 체험은 대기 상태를 제공하지 않으므로 실제 대기열 응답은 과거 예매 시점의 관찰에 의존한다.
- 필드의 의미는 값의 변화로 추론한 것이며 공식 명세가 아니다.

## 3. 접속 대기열이 필요한 이유

### 3.1 대기열이 없을 때의 연쇄 장애

```text
예매 시작 시각
  → 수십만 명이 동시에 예매 서버 접속
  → WAS 스레드와 DB 커넥션 고갈, 응답 지연
  → 사용자의 새로고침과 재시도로 요청이 몇 배로 증폭
  → 좌석 테이블 행 락 경합, 트랜잭션 타임아웃
  → 서버 다운, 아무도 예매하지 못함
```

선착순 시스템에서 가장 나쁜 결과는 일부가 늦게 예매하는 것이 아니라 모두가 실패하는 것이다. 사용자는 응답이 늦으면 새로고침을 누르므로, 과부하는 스스로를 증폭시킨다.

### 3.2 입장 제어로서의 대기열

대기열은 예매 서버 앞에서 입장 속도를 통제하는 장치다.

```text
             유입: 초당 수만 명
                    │
          ┌─────────▼─────────┐
          │   접속 대기열      │  번호표 발급, 순서 보장, 상태 안내
          └─────────┬─────────┘
                    │ 입장: 초당 N명 (예매 서버 처리 능력에 맞춤)
          ┌─────────▼─────────┐
          │   예매 서버        │  좌석 조회, 선점, 결제
          └───────────────────┘
```

| 대기열이 보장하는 것 | 방법 |
| --- | --- |
| 예매 서버 보호 | 처리 능력 이상으로 입장시키지 않는다 |
| 공정성 | 먼저 온 순서대로 번호표를 발급하고 그 순서로 입장시킨다 |
| 사용자 경험 | 남은 인원과 예상 시간을 보여 줘 새로고침을 억제한다 |
| 재시도 억제 | 새로고침해도 기존 번호표를 유지해 증폭을 막는다 |

### 3.3 리틀의 법칙으로 본 3분 제한

예매 화면에 동시에 머무는 사용자 수는 리틀의 법칙으로 계산된다.

```text
동시 입장 사용자 수(L) = 초당 입장 인원(λ) × 평균 체류 시간(W)
```

체류 시간에 상한이 없으면 한 사용자가 화면을 열어 둔 채 떠나도 자리가 계속 점유되어 L이 끝없이 늘 수 있다. 3분 제한은 W의 최댓값을 180초로 고정해, 입장 속도만 통제하면 동시 사용자 수가 다음 값을 넘지 않음을 보장한다.

```text
L ≤ λ × 180초
예: 초당 200명 입장 → 동시 사용자 최대 36,000명
```

반대로 예매 서버가 감당할 수 있는 동시 사용자 수가 정해져 있다면 입장 속도는 다음과 같이 정한다.

```text
예매 서버 처리량 2,000 req/s, 세션당 평균 10회 요청, 평균 체류 60초
  → 세션당 요청률 = 10 / 60 ≈ 0.167 req/s
  → 수용 가능한 동시 세션 = 2,000 / 0.167 ≈ 12,000
  → 초당 입장 인원 λ = 12,000 / 60 = 200명
  → 대기자 100만 명의 마지막 순번 대기 시간 ≈ 1,000,000 / 200 = 5,000초 ≈ 83분
```

이 계산이 대기열 응답의 TPS 값과 예상 대기 시간의 근거가 된다.

## 4. 관찰 결과로 재구성한 아키텍처

### 4.1 구성 요소

```text
브라우저
  │  ① 진입 (opcode=진입)                    ┌────────────────────────┐
  ├───────────────────────────────────────▶ │ 대기열 서버 (별도 도메인) │
  │  ◀── key(번호표), sticky, 대기 상태 ──── │  번호표 발급            │
  │                                          │  입장 스케줄러 (초당 N명)│
  │  ② 상태 조회 반복 (opcode=조회, TTL마다) │  상태 계산              │
  ├───────────────────────────────────────▶ │                        │
  │  ◀── 앞/뒤 인원, TPS, TTL / 입장 토큰 ── └────────────────────────┘
  │
  │  ③ 입장 토큰 제출                        ┌────────────────────────┐
  ├───────────────────────────────────────▶ │ 예매 서버 (WAS)         │
  │  ◀── 좌석 조회, 선점, 결제 (3분 이내) ── │  입장 토큰 검증          │
  │                                          │  좌석 선점·확정 트랜잭션 │
  │  ④ 완료 또는 3분 만료 (opcode=완료)      └───────────┬────────────┘
  └──▶ 대기열 서버: 입장 슬롯 반환                       │
                                                ┌──────▼──────┐
                                                │  데이터베이스 │
                                                └─────────────┘
```

### 4.2 요청 흐름

```text
1. 진입    : 브라우저가 대기열 서버에 opcode=진입 요청
             → 서버가 순번을 발급하고 서명된 key와 sticky 값을 반환
2. 대기    : 브라우저가 TTL초마다 opcode=조회 요청 (key, sticky 포함)
             → 서버가 내 앞/뒤 인원, TPS, 다음 TTL을 반환
3. 입장    : 입장 스케줄러가 내 순번까지 입장 커서를 전진시키면
             → 조회 응답에 입장 토큰(만료 시각 포함)이 실려 옴
4. 예매    : 브라우저가 입장 토큰을 예매 서버에 제출
             → 예매 서버가 토큰 서명과 만료를 검증하고 3분 세션 시작
5. 퇴장    : 예매 완료 시 opcode=완료로 슬롯 반환, 또는 3분 만료 시 자동 회수
             → 반환된 슬롯만큼 다음 대기자가 입장
```

### 4.3 각 필드의 설계 이유

| 필드 | 설계 이유 |
| --- | --- |
| `opcode` | 하나의 엔드포인트로 진입·조회·완료를 처리하면 대기열 서버를 별도 도메인에 두고 스크립트 한 종류로 연동하기 쉽다. JSONP처럼 GET만 가능한 방식에서도 기능을 구분할 수 있다. |
| `key` | 순번을 평문으로 주면 사용자가 숫자를 바꿔 새치기할 수 있다. 서버가 서명하거나 암호화한 번호표를 발급하면 위변조를 검출할 수 있고, 서버는 key만으로 사용자와 순번을 확인한다. |
| `sticky` | 대기 상태를 서버 메모리에 두면 저장소 왕복 없이 빠르게 응답할 수 있지만, 사용자는 항상 같은 서버로 가야 한다. sticky 값으로 로드 밸런서가 요청을 해당 서버에 고정한다. |
| TTL | 대기자 전원이 같은 주기로 폴링하면 대기열 서버의 부하가 대기 인원에 비례해 커진다. 서버가 TTL을 내려 주면 혼잡도와 순번에 따라 폴링 주기를 조절할 수 있다(5.5). |
| TPS | 초당 입장 인원을 알려 주면 클라이언트가 `앞 인원 / TPS`로 예상 대기 시간을 계산해 보여 줄 수 있다. |
| 앞/뒤 인원 | 앞 인원은 남은 대기를, 뒤 인원은 이탈하면 손해라는 정보를 줘 새로고침과 이탈을 억제한다. |
| JSONP | 다른 도메인의 대기열 서버를 `<script>` 태그로 호출하는 방식이다. 구형 브라우저 호환에는 유리하지만, 현재는 CORS를 설정한 JSON 응답이 더 안전한 선택이다(5.10). |

이와 같은 구성은 국내 공공기관과 예매 서비스에서 쓰이는 상용 접속 대기 솔루션들의 일반적인 형태와도 유사하다.

## 5. 핵심 설계 요소

### 5.1 번호표 발급: 원자적 순번 증가

번호표는 동시에 수만 건이 발급되어도 중복되거나 건너뛰면 안 된다. 단일 서버라면 메모리 카운터 증가로 충분하고, 여러 서버라면 Redis의 `INCR`처럼 원자적으로 증가하는 공유 카운터를 쓴다. 같은 사용자가 새로고침하거나 여러 탭을 열어도 기존 번호를 돌려줘야 증폭을 막을 수 있으므로, 사용자 식별값과 번호의 매핑도 함께 저장한다.

### 5.2 입장 제어: 입장 커서 방식

대기자를 하나씩 큐에서 꺼내는 대신, "몇 번까지 입장했는가"를 나타내는 입장 커서 하나만 전진시킨다.

```text
번호표:   1  2  3  4  5  6  7  8  9  10 ... 1,000,000
                      ▲                          ▲
               입장 커서(cursor)             마지막 발급(lastIssued)

매초: cursor를 min(TPS, 남은 슬롯)만큼 전진
      내 번호 ≤ cursor 이면 입장
```

커서 방식은 대기자가 100만 명이어도 상태가 숫자 두 개로 요약되므로 입장 처리와 상태 계산이 매우 가볍다.

### 5.3 앞·뒤 인원과 예상 대기 시간

```text
내 앞 인원      = 내 번호 - cursor - 1
내 뒤 인원      = lastIssued - 내 번호
예상 대기 시간  = 내 앞 인원 / TPS
```

이탈한 사용자도 번호는 차지하고 있으므로 앞 인원은 실제보다 약간 많게 계산된다. 이탈자는 5.7의 방식으로 커서가 지나갈 때 건너뛴다.

### 5.4 입장 속도와 동시 사용자 상한

입장 스케줄러는 매초 두 가지 한도 중 작은 값만큼만 입장시킨다.

```text
이번 초 입장 인원 = min(설정 TPS, 최대 동시 사용자 - 현재 입장 중인 사용자)
```

TPS 한도는 예매 서버로 들어가는 유입 속도를, 동시 사용자 한도는 3.3의 L 상한을 지킨다. 예매를 빨리 끝내고 나가는 사용자가 많으면 슬롯이 빨리 반환되어 입장이 빨라진다.

### 5.5 적응형 TTL: 폴링 부하 제어

대기열 서버가 받는 조회 요청 수는 다음과 같다.

```text
초당 조회 요청 = Σ (대기자 수 / 각자의 TTL)
```

대기자 100만 명이 모두 2초마다 조회하면 대기열 서버는 초당 50만 건을 받는다. 차례가 먼 사용자에게 긴 TTL을, 가까운 사용자에게 짧은 TTL을 주면 부하가 크게 줄어든다. 초당 입장 인원이 500명일 때 다음 기준을 적용한 예시는 아래와 같다.

| 예상 대기 시간 | 해당 순번 (TPS 500) | 인원 | TTL | 초당 조회 |
| --- | --- | --- | --- | --- |
| 10초 미만 | 앞 5,000명 미만 | 5,000 | 2초 | 2,500 |
| 10초 ~ 60초 | 5,000 ~ 30,000 | 25,000 | 5초 | 5,000 |
| 60초 ~ 600초 | 30,000 ~ 300,000 | 270,000 | 10초 | 27,000 |
| 600초 이상 | 300,000 이상 | 700,000 | 30초 | 23,333 |
| 합계 | | 1,000,000 | | 약 57,833 |

모두 TTL 2초일 때의 50만 건 대비 약 8.7배 감소한다. 여기에 클라이언트가 TTL에 무작위 지터를 더하면 조회 요청이 특정 순간에 몰리지 않는다.

### 5.6 입장 토큰

입장한 사용자는 예매 서버에 "대기열을 통과했다"는 증거를 제출해야 한다. 대기열 서버와 예매 서버가 공유하는 비밀 키로 서명한 토큰을 쓰면 예매 서버는 대기열 서버에 묻지 않고도 검증할 수 있다.

| 토큰 내용 | 목적 |
| --- | --- |
| 사용자 식별값 (해시) | 토큰 양도·공유 방지 |
| 순번 | 감사와 추적 |
| 만료 시각 (입장 + 3분) | 세션 제한을 토큰 자체에 내장 |
| 종류 (번호표 / 입장) | 번호표를 입장 토큰으로 오용하는 것 방지 |

### 5.7 이탈자 처리와 슬롯 회수

| 상황 | 처리 |
| --- | --- |
| 대기 중 창을 닫음 | 마지막 조회 시각을 기록하고, 커서가 그 번호에 도달했을 때 최대 TTL의 3배 이상 조회가 없었다면 입장시키지 않고 건너뜀 |
| 입장 후 예매 완료 | 완료 요청으로 슬롯을 즉시 반환 |
| 입장 후 3분 경과 | 스케줄러가 만료된 세션을 회수하고, 예매 서버의 좌석 선점도 같은 시각에 만료 |

이탈 판정 기준이 TTL보다 짧으면 정상적으로 기다리는 사용자를 이탈자로 오판한다. 따라서 이탈 판정 시간은 최대 TTL의 배수로 잡아야 한다.

### 5.8 스티키 세션과 공유 저장소

| 방식 | 장점 | 단점 |
| --- | --- | --- |
| 서버 메모리 + sticky | 저장소 왕복이 없어 빠르고 단순 | 서버 장애 시 해당 서버의 대기 순번 유실, 서버 간 부하 불균형, 전체 순서는 서버별로만 보장 |
| 공유 저장소(Redis) | 서버가 무상태가 되어 아무 서버나 응답 가능, 장애 시에도 순번 유지, 전역 순서 보장 | 저장소가 단일 병목이 될 수 있어 원자적 스크립트와 키 설계가 중요 |

관찰된 `sticky` 필드는 메모리 방식 또는 서버별 캐시를 사용하는 구조를 시사한다. 6장은 메모리 방식, 7장은 Redis 방식으로 구현한다.

### 5.9 부정 사용 방지

| 위협 | 대응 |
| --- | --- |
| 순번 위변조 | 서명된 key, 서버 측 검증 |
| 다중 탭·다중 기기 | 사용자 식별값당 번호표 하나 |
| 매크로·자동화 | 조회 빈도 제한(TTL보다 빠른 요청 거부), 행동 기반 탐지, 필요 시 추가 인증 |
| 개발자 도구를 이용한 조작 | 클라이언트 경고와 로그아웃은 억제 효과만 있다. 모든 판단은 서버가 검증한다 |
| 입장 토큰 공유 | 토큰에 사용자 식별값을 넣고 예매 서버의 로그인 사용자와 대조 |

뒤로 가기나 개발자 도구 사용 시 로그아웃될 수 있다는 안내는 클라이언트 측 억제 장치다. 브라우저에서 실행되는 코드는 사용자가 얼마든지 바꿀 수 있으므로, 보안 경계는 항상 서버의 서명 검증과 상태 검증에 두어야 한다.

### 5.10 JSONP와 CORS

| 항목 | JSONP | CORS + JSON |
| --- | --- | --- |
| 원리 | `<script src>`로 함수 호출 코드를 받아 실행 | `fetch`로 JSON을 받고, 서버가 허용 출처를 헤더로 명시 |
| 메서드 | GET만 가능 | 모든 메서드 |
| 오류 처리 | HTTP 상태 코드 확인이 어려움 | 상태 코드와 본문으로 명확히 처리 |
| 보안 | 응답이 스크립트로 실행되므로 콜백 이름 주입, 응답 변조 위험 | 데이터로만 해석되어 안전 |

새로 만든다면 CORS를 설정한 JSON 응답을 쓰는 것이 좋다. 호환을 위해 JSONP를 지원해야 한다면 콜백 이름을 엄격한 정규식으로 검증하고, `Content-Type: text/javascript`와 `X-Content-Type-Options: nosniff`를 설정한다.

### 5.11 재고 정합성: 좌석 선점과 확정

대기열은 예매 서버로의 유입을 줄일 뿐, 좌석 중복 판매를 막아 주지는 않는다. 입장한 사용자들끼리도 같은 좌석을 동시에 고를 수 있으므로 예매 서버는 좌석 상태를 원자적으로 바꿔야 한다.

```text
free ──선점(조건부 UPDATE)──▶ held (held_by, held_until=세션 만료 시각)
held ──확정(조건부 UPDATE + 주문 INSERT, 하나의 트랜잭션)──▶ sold
held ──held_until 경과──▶ 다른 사용자가 선점 가능 (free로 간주)
```

선점은 "현재 비어 있거나 선점이 만료된 경우에만 바꾼다"는 조건을 `UPDATE`의 `WHERE` 절에 넣고, 변경된 행 수가 1인지로 성공을 판정한다. 읽고 나서 쓰는 방식은 두 사용자가 동시에 빈 좌석을 읽고 둘 다 성공하는 경쟁 조건을 만든다.

## 6. Bun 내장 API 구현: 단일 서버 버전

### 6.1 구성과 사용하는 내장 API

```text
ticketing/
├── shared/
│   └── token.ts            서명 토큰 (Web Crypto HMAC)
├── queue/
│   ├── memory-queue.ts     메모리 대기열 코어
│   ├── redis-queue.ts      Redis 대기열 코어 (7장)
│   ├── server.ts           대기열 서버 (Bun.serve, 포트 3001)
│   └── ws-server.ts        WebSocket 푸시 버전 (8장)
├── booking/
│   ├── seats.ts            좌석 선점·확정 (bun:sqlite)
│   ├── server.ts           예매 서버 (Bun.serve, 포트 3000)
│   └── public/index.html   브라우저 클라이언트
├── load/
│   └── load.ts             부하 생성기 (9장)
└── test/
    └── ticketing.test.ts   정합성 테스트 (bun:test)
```

| 내장 API | 용도 |
| --- | --- |
| `Bun.serve` (`routes`, `websocket`) | 대기열 서버, 예매 서버, WebSocket 푸시 |
| `req.cookies` | 로그인 쿠키 읽기·쓰기 |
| `bun:sqlite` (`Database`, `transaction`) | 좌석 재고와 주문 저장 |
| `RedisClient` (`bun`) | 다중 서버 대기열의 공유 상태 |
| `Bun.CryptoHasher` | 사용자 식별값 해싱 |
| `crypto.subtle` (Web Crypto) | 번호표와 입장 토큰의 HMAC 서명·검증 |
| `Bun.file` | 정적 HTML 제공 |
| `Bun.env`, `Bun.argv` | 설정과 실행 인자 |
| `Bun.sleep`, `Bun.nanoseconds` | 부하 생성기의 대기와 시간 측정 |
| `bun:test` | 순서·한도·중복 판매 방지 테스트 |

외부 npm 패키지는 사용하지 않는다. 7장의 Redis 버전만 Redis 서버가 필요하다.

### 6.2 서명 토큰: `shared/token.ts`

번호표(`ticket`)와 입장 토큰(`admission`)을 같은 형식으로 서명한다. 본문은 base64url로 인코딩한 JSON이고, 서명은 HMAC-SHA256이다. 대기열 서버와 예매 서버는 환경 변수 `QUEUE_SECRET`으로 같은 키를 공유한다.

```ts
const encoder = new TextEncoder();

export type TokenKind = "ticket" | "admission";

export type Claims = {
  kind: TokenKind;
  sub: string;
  no: number;
  exp: number;
};

const key = await crypto.subtle.importKey(
  "raw",
  encoder.encode(Bun.env.QUEUE_SECRET ?? "dev-only-secret-change-me"),
  { name: "HMAC", hash: "SHA-256" },
  false,
  ["sign", "verify"],
);

const toBase64Url = (bytes: Uint8Array) => Buffer.from(bytes).toString("base64url");

export async function sign(claims: Claims): Promise<string> {
  const body = toBase64Url(encoder.encode(JSON.stringify(claims)));
  const signature = new Uint8Array(await crypto.subtle.sign("HMAC", key, encoder.encode(body)));
  return `${body}.${toBase64Url(signature)}`;
}

export async function verify(token: string, kind: TokenKind, now = Date.now()): Promise<Claims | null> {
  const [body, signature] = token.split(".");
  if (!body || !signature) return null;

  const valid = await crypto.subtle.verify(
    "HMAC",
    key,
    Buffer.from(signature, "base64url"),
    encoder.encode(body),
  );
  if (!valid) return null;

  const claims = JSON.parse(Buffer.from(body, "base64url").toString("utf8")) as Claims;
  if (claims.kind !== kind || claims.exp < now) return null;
  return claims;
}

export function userKey(userId: string): string {
  return new Bun.CryptoHasher("sha256")
    .update(Bun.env.USER_KEY_SALT ?? "dev-salt")
    .update(userId)
    .digest("hex")
    .slice(0, 32);
}
```

`crypto.subtle.verify`는 서명을 상수 시간으로 비교하므로 타이밍 공격에 안전하다. `userKey`는 관찰된 `key` 필드처럼 사용자 식별값을 그대로 노출하지 않고 해시로 바꾼다.

### 6.3 대기열 코어: `queue/memory-queue.ts`

5.2~5.7의 설계를 그대로 옮긴 메모리 대기열이다. 입장 커서, 마지막 발급 번호, 대기자 맵, 입장자 맵, 사용자별 번호 맵으로 구성된다.

```ts
export type QueueOptions = {
  tps: number;
  maxActive: number;
  sessionMs: number;
  staleMs: number;
};

export type QueueStatus =
  | { state: "waiting"; no: number; ahead: number; behind: number; tps: number; ttl: number; etaSec: number }
  | { state: "admitted"; no: number; expiresAt: number }
  | { state: "expired"; no: number };

export interface Queue {
  enter(sub: string): number | Promise<number>;
  status(no: number): QueueStatus | Promise<QueueStatus>;
  tick(): number | Promise<number>;
  release(no: number): void | Promise<void>;
}

type Waiting = { sub: string; lastSeen: number };
type Active = { sub: string; expiresAt: number };

export const MAX_TTL = 30;

export function pollTtl(ahead: number, tps: number): number {
  const etaSec = ahead / Math.max(tps, 1);
  if (etaSec >= 600) return MAX_TTL;
  if (etaSec >= 60) return 10;
  if (etaSec >= 10) return 5;
  return 2;
}

export class MemoryQueue implements Queue {
  private lastIssued = 0;
  private cursor = 0;
  private admittedLastTick = 0;
  private readonly waiting = new Map<number, Waiting>();
  private readonly active = new Map<number, Active>();
  private readonly bySub = new Map<string, number>();

  constructor(private readonly opts: QueueOptions) {}

  get snapshot() {
    return {
      lastIssued: this.lastIssued,
      cursor: this.cursor,
      waiting: this.waiting.size,
      active: this.active.size,
      tps: this.admittedLastTick,
    };
  }

  enter(sub: string, now = Date.now()): number {
    const existing = this.bySub.get(sub);
    if (existing !== undefined) return existing;

    const no = ++this.lastIssued;
    this.waiting.set(no, { sub, lastSeen: now });
    this.bySub.set(sub, no);
    return no;
  }

  status(no: number, now = Date.now()): QueueStatus {
    const active = this.active.get(no);
    if (active) return { state: "admitted", no, expiresAt: active.expiresAt };

    const waiting = this.waiting.get(no);
    if (!waiting) return { state: "expired", no };

    waiting.lastSeen = now;
    const ahead = Math.max(0, no - this.cursor - 1);
    const rate = this.admittedLastTick || this.opts.tps;
    return {
      state: "waiting",
      no,
      ahead,
      behind: this.lastIssued - no,
      tps: this.admittedLastTick,
      ttl: pollTtl(ahead, rate),
      etaSec: Math.ceil(ahead / rate),
    };
  }

  tick(now = Date.now()): number {
    for (const [no, session] of this.active) {
      if (session.expiresAt <= now) this.release(no);
    }

    let budget = Math.min(this.opts.tps, this.opts.maxActive - this.active.size);
    let admitted = 0;

    while (budget > 0 && this.cursor < this.lastIssued) {
      const no = ++this.cursor;
      const entry = this.waiting.get(no);
      if (!entry) continue;

      this.waiting.delete(no);
      if (now - entry.lastSeen > this.opts.staleMs) {
        this.bySub.delete(entry.sub);
        continue;
      }

      this.active.set(no, { sub: entry.sub, expiresAt: now + this.opts.sessionMs });
      budget--;
      admitted++;
    }

    this.admittedLastTick = admitted;
    return admitted;
  }

  release(no: number): void {
    const entry = this.active.get(no) ?? this.waiting.get(no);
    if (!entry) return;

    this.active.delete(no);
    this.waiting.delete(no);
    if (this.bySub.get(entry.sub) === no) this.bySub.delete(entry.sub);
  }
}
```

| 메서드 | 대응하는 opcode | 동작 |
| --- | --- | --- |
| `enter` | 진입 | 사용자당 번호표 하나 발급, 재진입 시 기존 번호 반환 |
| `status` | 조회 | 마지막 조회 시각 갱신, 앞/뒤 인원·TPS·TTL 계산, 입장 여부 반환 |
| `tick` | (스케줄러) | 만료 세션 회수 후 TPS와 남은 슬롯 중 작은 값만큼 커서 전진, 이탈자 건너뜀 |
| `release` | 완료 | 슬롯 반환, 사용자 매핑 제거 |

### 6.4 대기열 서버: `queue/server.ts`

관찰된 구조처럼 단일 엔드포인트 `/queue`에서 `opcode`로 기능을 구분하고, `callback` 파라미터가 있으면 JSONP로 응답한다. opcode 값은 본 예제에서 정한 것이다.

```ts
import { RedisClient } from "bun";
import { MemoryQueue, MAX_TTL, type Queue, type QueueOptions } from "./memory-queue";
import { RedisQueue } from "./redis-queue";
import { sign, userKey, verify } from "../shared/token";

const OPCODE = { ENTER: "5101", STATUS: "5002", LEAVE: "5004" } as const;
const NODE_ID = Bun.env.NODE_ID ?? "q1";
const BOOKING_ORIGIN = Bun.env.BOOKING_ORIGIN ?? "http://localhost:3000";
const CALLBACK_NAME = /^[A-Za-z_$][\w$]{0,63}$/;

const options: QueueOptions = {
  tps: Number(Bun.env.QUEUE_TPS ?? 50),
  maxActive: Number(Bun.env.QUEUE_MAX_ACTIVE ?? 500),
  sessionMs: 3 * 60_000,
  staleMs: MAX_TTL * 3 * 1000,
};

const queue: Queue =
  Bun.env.QUEUE_BACKEND === "redis"
    ? new RedisQueue(new RedisClient(Bun.env.REDIS_URL ?? "redis://localhost:6379"), "chuseok", options, NODE_ID)
    : new MemoryQueue(options);

setInterval(() => {
  Promise.resolve(queue.tick()).catch((err) => console.error("tick failed", err));
}, 1000);

const corsHeaders = {
  "Access-Control-Allow-Origin": BOOKING_ORIGIN,
  "Access-Control-Allow-Credentials": "true",
  "Cache-Control": "no-store",
};

function reply(req: Request, body: unknown, status = 200): Response {
  const callback = new URL(req.url).searchParams.get("callback");
  if (callback && CALLBACK_NAME.test(callback)) {
    return new Response(`${callback}(${JSON.stringify(body)});`, {
      status,
      headers: {
        "Content-Type": "text/javascript; charset=utf-8",
        "X-Content-Type-Options": "nosniff",
        "Cache-Control": "no-store",
      },
    });
  }
  return Response.json(body, { status, headers: corsHeaders });
}

const server = Bun.serve({
  port: Number(Bun.env.PORT ?? 3001),
  routes: {
    "/queue": async (req) => {
      const params = new URL(req.url).searchParams;
      const opcode = params.get("opcode");

      if (opcode === OPCODE.ENTER) {
        const uid = req.cookies.get("uid");
        if (!uid) return reply(req, { error: "login required" }, 401);

        const sub = userKey(uid);
        const no = await queue.enter(sub);
        const key = await sign({ kind: "ticket", sub, no, exp: Date.now() + 6 * 60 * 60_000 });
        return reply(req, { key, sticky: NODE_ID, ...(await queue.status(no)) });
      }

      const key = params.get("key");
      const ticket = key ? await verify(key, "ticket") : null;
      if (!ticket) return reply(req, { error: "invalid key" }, 403);

      if (opcode === OPCODE.STATUS) {
        const status = await queue.status(ticket.no);
        if (status.state === "admitted") {
          const admission = await sign({ kind: "admission", sub: ticket.sub, no: ticket.no, exp: status.expiresAt });
          return reply(req, { ...status, admission });
        }
        return reply(req, { ...status, sticky: NODE_ID });
      }

      if (opcode === OPCODE.LEAVE) {
        await queue.release(ticket.no);
        return reply(req, { state: "left", no: ticket.no });
      }

      return reply(req, { error: "unknown opcode" }, 400);
    },

    "/queue/metrics": () =>
      Response.json(queue instanceof MemoryQueue ? { node: NODE_ID, ...queue.snapshot } : { node: NODE_ID }),
  },
  fetch: () => new Response("not found", { status: 404 }),
});

console.log(`queue server ${NODE_ID} on ${server.url}`);
```

| 응답 필드 | 예시 | 의미 |
| --- | --- | --- |
| `key` | `eyJraW5k...` | 서명된 번호표. 이후 모든 요청에 포함 |
| `sticky` | `q1` | 이 번호표를 발급한 대기열 서버 |
| `state` | `waiting` / `admitted` / `expired` | 현재 상태 |
| `ahead`, `behind` | `1532`, `48210` | 내 앞·뒤 인원 |
| `tps` | `50` | 직전 1초 입장 인원 |
| `ttl` | `5` | 다음 조회까지 초 |
| `etaSec` | `31` | 예상 대기 시간 |
| `admission` | `eyJraW5k...` | 입장 토큰 (입장 시에만) |

`localhost`에서는 쿠키가 포트를 구분하지 않으므로, 예매 서버(3000)가 설정한 `uid` 쿠키가 대기열 서버(3001) 요청에도 전송된다. 운영 환경에서 대기열 서버를 다른 도메인에 둔다면 로그인 쿠키 대신 예매 서버가 발급한 서명 토큰을 진입 요청에 실어 보내는 방식으로 바꾼다.

### 6.5 좌석 선점과 확정: `booking/seats.ts`

5.11의 조건부 `UPDATE`와 트랜잭션을 `bun:sqlite`로 구현한다. `strict: true`로 열면 SQL의 `$id` 같은 파라미터를 `{ id }` 객체로 바인딩할 수 있다.

```ts
import { Database } from "bun:sqlite";

export function openDb(path = ":memory:"): Database {
  const db = new Database(path, { create: true, strict: true });
  db.run("PRAGMA journal_mode = WAL");
  db.run("PRAGMA busy_timeout = 5000");
  db.run(`
    CREATE TABLE IF NOT EXISTS seats (
      id         TEXT PRIMARY KEY,
      train      TEXT NOT NULL,
      status     TEXT NOT NULL CHECK (status IN ('free', 'held', 'sold')),
      held_by    TEXT,
      held_until INTEGER
    )`);
  db.run(`
    CREATE TABLE IF NOT EXISTS orders (
      id         INTEGER PRIMARY KEY AUTOINCREMENT,
      seat_id    TEXT NOT NULL UNIQUE REFERENCES seats(id),
      buyer      TEXT NOT NULL,
      created_at INTEGER NOT NULL
    )`);
  return db;
}

export function seed(db: Database, train: string, cars = 4, seatsPerCar = 60): void {
  const insert = db.query("INSERT OR IGNORE INTO seats (id, train, status) VALUES ($id, $train, 'free')");
  db.transaction(() => {
    for (let car = 1; car <= cars; car++) {
      for (let n = 1; n <= seatsPerCar; n++) {
        insert.run({ id: `${train}-${car}-${n}`, train });
      }
    }
  })();
}

export function listSeats(db: Database, train: string, now = Date.now()) {
  return db
    .query(`
      SELECT id,
             CASE WHEN status = 'held' AND held_until < $now THEN 'free' ELSE status END AS status
      FROM seats
      WHERE train = $train
      ORDER BY id`)
    .all({ train, now }) as { id: string; status: "free" | "held" | "sold" }[];
}

export function holdSeat(db: Database, seatId: string, buyer: string, until: number, now = Date.now()): boolean {
  const hold = db.transaction(() => {
    const taken = db
      .query(`
        UPDATE seats
        SET status = 'held', held_by = $buyer, held_until = $until
        WHERE id = $id
          AND (status = 'free' OR (status = 'held' AND held_until < $now))`)
      .run({ id: seatId, buyer, until, now });

    if (taken.changes !== 1) return false;

    db.query(`
        UPDATE seats
        SET status = 'free', held_by = NULL, held_until = NULL
        WHERE held_by = $buyer AND status = 'held' AND id <> $id`)
      .run({ buyer, id: seatId });
    return true;
  });
  return hold.immediate();
}

export function confirmSeat(db: Database, seatId: string, buyer: string, now = Date.now()): number | null {
  const confirm = db.transaction(() => {
    const sold = db
      .query(`
        UPDATE seats
        SET status = 'sold', held_until = NULL
        WHERE id = $id AND status = 'held' AND held_by = $buyer AND held_until >= $now`)
      .run({ id: seatId, buyer, now });

    if (sold.changes !== 1) return null;

    const order = db
      .query("INSERT INTO orders (seat_id, buyer, created_at) VALUES ($id, $buyer, $now)")
      .run({ id: seatId, buyer, now });
    return Number(order.lastInsertRowid);
  });
  return confirm.immediate();
}
```

| 함수 | 원자성 보장 방법 |
| --- | --- |
| `holdSeat` | `WHERE` 절의 상태 조건과 `changes === 1` 판정. 성공했을 때만 같은 사용자의 이전 선점을 해제해 사용자당 좌석 하나 유지 |
| `confirmSeat` | 선점자 본인, 선점 미만료 조건을 건 `UPDATE`와 주문 `INSERT`를 하나의 트랜잭션으로 묶음 |
| `immediate()` | 트랜잭션 시작 시 쓰기 락을 먼저 잡아 동시 쓰기 간 교착을 방지 |
| `orders.seat_id UNIQUE` | 애플리케이션 버그가 있어도 DB 수준에서 중복 주문 차단 |

선점 만료 시각 `held_until`은 입장 토큰의 만료 시각과 같게 설정한다. 3분 세션이 끝나는 순간 좌석도 자동으로 다른 사용자에게 열린다.

### 6.6 예매 서버: `booking/server.ts`

예매 서버는 입장 토큰을 검증한 요청만 좌석 선점과 확정을 허용한다. 대기열 서버에 매번 묻지 않고 서명만으로 검증하므로 대기열 서버의 부하와 무관하게 동작한다.

```ts
import { confirmSeat, holdSeat, listSeats, openDb, seed } from "./seats";
import { verify } from "../shared/token";

const db = openDb(Bun.env.DB_PATH ?? "booking.sqlite");
seed(db, "KTX-101");

async function admitted(req: Request) {
  const token = req.headers.get("x-admission");
  return token ? verify(token, "admission") : null;
}

const server = Bun.serve({
  port: Number(Bun.env.PORT ?? 3000),
  routes: {
    "/": new Response(Bun.file(new URL("./public/index.html", import.meta.url))),

    "/login": {
      POST: async (req) => {
        const { name } = (await req.json()) as { name?: string };
        if (!name) return Response.json({ error: "name required" }, { status: 400 });
        req.cookies.set("uid", name, { httpOnly: true, sameSite: "lax", path: "/" });
        return Response.json({ ok: true });
      },
    },

    "/api/seats/:train": (req) => Response.json(listSeats(db, req.params.train)),

    "/api/hold": {
      POST: async (req) => {
        const claims = await admitted(req);
        if (!claims) return Response.json({ error: "not admitted" }, { status: 403 });

        const { seatId } = (await req.json()) as { seatId: string };
        return holdSeat(db, seatId, claims.sub, claims.exp)
          ? Response.json({ held: seatId, until: claims.exp })
          : Response.json({ error: "seat taken" }, { status: 409 });
      },
    },

    "/api/confirm": {
      POST: async (req) => {
        const claims = await admitted(req);
        if (!claims) return Response.json({ error: "session expired" }, { status: 403 });

        const { seatId } = (await req.json()) as { seatId: string };
        const orderId = confirmSeat(db, seatId, claims.sub);
        return orderId
          ? Response.json({ orderId, seatId })
          : Response.json({ error: "hold expired or not yours" }, { status: 409 });
      },
    },
  },
  fetch: () => new Response("not found", { status: 404 }),
});

console.log(`booking server on ${server.url}`);
```

`/login`은 예제를 위한 단순한 로그인이다. 실제 서비스에서는 비밀번호 검증(`Bun.password.verify`)과 서명된 세션 쿠키를 사용해야 한다.

### 6.7 브라우저 클라이언트: `booking/public/index.html`

클라이언트는 서버가 내려 준 TTL에 지터를 더해 조회하고, 입장 후에는 토큰의 만료 시각으로 3분 카운트다운을 표시한다.

```html
<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8" />
  <title>승차권 예매</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 720px; margin: 2rem auto; }
    #seats { display: grid; grid-template-columns: repeat(10, 1fr); gap: 4px; }
    #seats button[data-status="free"] { background: #d1fae5; }
    #seats button[data-status="held"] { background: #fef3c7; }
    #seats button[data-status="sold"] { background: #fee2e2; }
  </style>
</head>
<body>
  <h1>추석 승차권 예매</h1>
  <input id="name" placeholder="사용자 이름" />
  <button id="login">로그인</button>
  <button id="enter" disabled>예매 대기열 진입</button>
  <p id="status"></p>
  <p id="timer"></p>
  <div id="seats"></div>

  <script type="module">
    const QUEUE = "http://localhost:3001/queue";
    const OPCODE = { ENTER: "5101", STATUS: "5002", LEAVE: "5004" };
    const $ = (id) => document.getElementById(id);
    let ticketKey = null;
    let admission = null;

    async function queueCall(params) {
      const res = await fetch(`${QUEUE}?${new URLSearchParams(params)}`, { credentials: "include" });
      return res.json();
    }

    $("login").onclick = async () => {
      await fetch("/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: $("name").value }),
      });
      $("enter").disabled = false;
    };

    $("enter").onclick = async () => {
      let s = await queueCall({ opcode: OPCODE.ENTER });
      ticketKey = s.key;

      while (s.state === "waiting") {
        $("status").textContent =
          `대기 중: 앞 ${s.ahead}명, 뒤 ${s.behind}명, 초당 ${s.tps}명 입장, 예상 ${s.etaSec}초 (다음 확인 ${s.ttl}초 후)`;
        await new Promise((r) => setTimeout(r, s.ttl * 1000 + Math.random() * 500));
        s = await queueCall({ opcode: OPCODE.STATUS, key: ticketKey });
      }

      if (s.state !== "admitted") {
        $("status").textContent = "대기 시간이 만료되었습니다. 다시 진입해 주세요.";
        return;
      }

      admission = s.admission;
      $("status").textContent = "입장했습니다. 3분 안에 예매를 완료해 주세요.";
      startTimer(s.expiresAt);
      await renderSeats();
    };

    function startTimer(expiresAt) {
      const id = setInterval(() => {
        const left = Math.max(0, Math.round((expiresAt - Date.now()) / 1000));
        $("timer").textContent = `남은 시간 ${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`;
        if (left === 0) {
          clearInterval(id);
          admission = null;
          $("status").textContent = "시간이 초과되어 퇴장되었습니다.";
          $("seats").innerHTML = "";
        }
      }, 1000);
    }

    async function renderSeats() {
      const seats = await (await fetch("/api/seats/KTX-101")).json();
      $("seats").innerHTML = "";
      for (const seat of seats) {
        const b = document.createElement("button");
        b.textContent = seat.id.split("-").slice(-2).join("-");
        b.dataset.status = seat.status;
        b.disabled = seat.status !== "free";
        b.onclick = () => book(seat.id);
        $("seats").append(b);
      }
    }

    async function book(seatId) {
      const headers = { "Content-Type": "application/json", "x-admission": admission };
      const hold = await fetch("/api/hold", { method: "POST", headers, body: JSON.stringify({ seatId }) });
      if (!hold.ok) return renderSeats();

      const confirm = await fetch("/api/confirm", { method: "POST", headers, body: JSON.stringify({ seatId }) });
      const result = await confirm.json();
      $("status").textContent = confirm.ok ? `예매 완료: ${result.seatId} (주문 ${result.orderId})` : result.error;

      await queueCall({ opcode: OPCODE.LEAVE, key: ticketKey });
      admission = null;
    }
  </script>
</body>
</html>
```

### 6.8 실행

```powershell
$env:QUEUE_SECRET = "change-me-to-a-long-random-value"
$env:QUEUE_TPS = "5"
$env:QUEUE_MAX_ACTIVE = "20"

bun run queue/server.ts
bun run booking/server.ts
```

브라우저 탭 여러 개에서 서로 다른 이름으로 로그인해 대기열에 진입하면, `QUEUE_TPS=5` 설정에 따라 초당 5명씩 입장하고 나머지는 앞 인원이 줄어드는 대기 화면을 보게 된다. 개발자 도구 네트워크 탭에서는 2장에서 관찰한 것과 같은 형태의 요청(`opcode`, `key`)과 응답(`ahead`, `behind`, `tps`, `ttl`, `sticky`)을 확인할 수 있다.

## 7. Redis로 확장한 다중 서버 구현

### 7.1 키 설계

여러 대기열 서버가 하나의 전역 순서를 공유하도록 상태를 Redis로 옮긴다. 모든 키에 같은 해시 태그 `{chuseok}`를 붙여 Redis Cluster에서도 같은 슬롯에 배치되게 해야 Lua 스크립트로 여러 키를 원자적으로 다룰 수 있다.

| 키 | 타입 | 내용 |
| --- | --- | --- |
| `q:{chuseok}:seq` | String | 마지막 발급 번호 (`INCR`) |
| `q:{chuseok}:cursor` | String | 입장 커서 |
| `q:{chuseok}:seen` | Sorted Set | 대기자. 멤버 = 번호, 점수 = 마지막 조회 시각 |
| `q:{chuseok}:active` | Sorted Set | 입장자. 멤버 = 번호, 점수 = 세션 만료 시각 |
| `q:{chuseok}:tps` | String | 직전 틱의 입장 인원 |
| `q:{chuseok}:leader` | String | 입장 스케줄러 실행권 (`SET NX PX`) |
| `q:{chuseok}:user:<sub>` | String | 사용자별 번호 (만료 6시간) |

### 7.2 원자적 스크립트

진입과 입장 처리는 여러 키를 읽고 쓰므로 Lua 스크립트로 원자성을 보장한다.

**진입 스크립트**: 기존 번호가 아직 대기 중이거나 입장 중이면 그대로 돌려주고, 아니면 새 번호를 발급한다.

```lua
-- KEYS: user, seq, seen, active   ARGV: now, userTtlSec
local existing = redis.call('GET', KEYS[1])
if existing and (redis.call('ZSCORE', KEYS[3], existing) or redis.call('ZSCORE', KEYS[4], existing)) then
  return tonumber(existing)
end
local no = redis.call('INCR', KEYS[2])
redis.call('SET', KEYS[1], no, 'EX', ARGV[2])
redis.call('ZADD', KEYS[3], ARGV[1], no)
return no
```

**입장 스크립트**: 만료 세션을 회수하고, TPS와 남은 슬롯 중 작은 값만큼 커서를 전진시키며 이탈자를 건너뛴다.

```lua
-- KEYS: seq, cursor, seen, active, tps   ARGV: now, tps, maxActive, sessionMs, staleMs
local now = tonumber(ARGV[1])
redis.call('ZREMRANGEBYSCORE', KEYS[4], '-inf', now)

local room = tonumber(ARGV[3]) - redis.call('ZCARD', KEYS[4])
local budget = math.min(tonumber(ARGV[2]), room)
local cursor = tonumber(redis.call('GET', KEYS[2]) or '0')
local last = tonumber(redis.call('GET', KEYS[1]) or '0')
local admitted, scanned = 0, 0

while budget > 0 and cursor < last and scanned < 10000 do
  cursor = cursor + 1
  scanned = scanned + 1
  local seen = redis.call('ZSCORE', KEYS[3], cursor)
  if seen then
    redis.call('ZREM', KEYS[3], cursor)
    if now - tonumber(seen) <= tonumber(ARGV[5]) then
      redis.call('ZADD', KEYS[4], now + tonumber(ARGV[4]), cursor)
      budget = budget - 1
      admitted = admitted + 1
    end
  end
end

redis.call('SET', KEYS[2], cursor)
redis.call('SET', KEYS[5], admitted)
return admitted
```

`scanned` 상한은 이탈자가 대량으로 몰려 있을 때 스크립트 하나가 Redis를 오래 점유하지 않도록 한 번에 훑는 범위를 제한한다.

### 7.3 Redis 대기열 코어: `queue/redis-queue.ts`

Bun 내장 `RedisClient`의 `send`로 임의의 Redis 명령을 보낸다. 동시에 보낸 명령은 자동으로 파이프라인 처리된다.

```ts
import type { RedisClient } from "bun";
import { pollTtl, type Queue, type QueueOptions, type QueueStatus } from "./memory-queue";

const ENTER_SCRIPT = `
local existing = redis.call('GET', KEYS[1])
if existing and (redis.call('ZSCORE', KEYS[3], existing) or redis.call('ZSCORE', KEYS[4], existing)) then
  return tonumber(existing)
end
local no = redis.call('INCR', KEYS[2])
redis.call('SET', KEYS[1], no, 'EX', ARGV[2])
redis.call('ZADD', KEYS[3], ARGV[1], no)
return no`;

const TICK_SCRIPT = `
local now = tonumber(ARGV[1])
redis.call('ZREMRANGEBYSCORE', KEYS[4], '-inf', now)
local room = tonumber(ARGV[3]) - redis.call('ZCARD', KEYS[4])
local budget = math.min(tonumber(ARGV[2]), room)
local cursor = tonumber(redis.call('GET', KEYS[2]) or '0')
local last = tonumber(redis.call('GET', KEYS[1]) or '0')
local admitted, scanned = 0, 0
while budget > 0 and cursor < last and scanned < 10000 do
  cursor = cursor + 1
  scanned = scanned + 1
  local seen = redis.call('ZSCORE', KEYS[3], cursor)
  if seen then
    redis.call('ZREM', KEYS[3], cursor)
    if now - tonumber(seen) <= tonumber(ARGV[5]) then
      redis.call('ZADD', KEYS[4], now + tonumber(ARGV[4]), cursor)
      budget = budget - 1
      admitted = admitted + 1
    end
  end
end
redis.call('SET', KEYS[2], cursor)
redis.call('SET', KEYS[5], admitted)
return admitted`;

function queueKeys(event: string) {
  const prefix = `q:{${event}}`;
  return {
    seq: `${prefix}:seq`,
    cursor: `${prefix}:cursor`,
    seen: `${prefix}:seen`,
    active: `${prefix}:active`,
    tps: `${prefix}:tps`,
    leader: `${prefix}:leader`,
    user: (sub: string) => `${prefix}:user:${sub}`,
  };
}

export class RedisQueue implements Queue {
  private readonly keys: ReturnType<typeof queueKeys>;

  constructor(
    private readonly redis: RedisClient,
    event: string,
    private readonly opts: QueueOptions,
    private readonly nodeId: string,
  ) {
    this.keys = queueKeys(event);
  }

  async enter(sub: string): Promise<number> {
    const k = this.keys;
    const no = await this.redis.send("EVAL", [
      ENTER_SCRIPT, "4", k.user(sub), k.seq, k.seen, k.active,
      String(Date.now()), String(6 * 60 * 60),
    ]);
    return Number(no);
  }

  async status(no: number): Promise<QueueStatus> {
    const k = this.keys;
    const member = String(no);
    const [activeExp, seenAt, cursor, last, tps] = await Promise.all([
      this.redis.send("ZSCORE", [k.active, member]),
      this.redis.send("ZSCORE", [k.seen, member]),
      this.redis.get(k.cursor),
      this.redis.get(k.seq),
      this.redis.get(k.tps),
    ]);

    if (activeExp !== null) return { state: "admitted", no, expiresAt: Number(activeExp) };
    if (seenAt === null) return { state: "expired", no };

    await this.redis.send("ZADD", [k.seen, "XX", String(Date.now()), member]);

    const ahead = Math.max(0, no - Number(cursor ?? 0) - 1);
    const observed = Number(tps ?? 0);
    const rate = observed || this.opts.tps;
    return {
      state: "waiting",
      no,
      ahead,
      behind: Number(last ?? 0) - no,
      tps: observed,
      ttl: pollTtl(ahead, rate),
      etaSec: Math.ceil(ahead / rate),
    };
  }

  async tick(): Promise<number> {
    const k = this.keys;
    const lock = await this.redis.send("SET", [k.leader, this.nodeId, "NX", "PX", "900"]);
    if (lock !== "OK") return 0;

    const admitted = await this.redis.send("EVAL", [
      TICK_SCRIPT, "5", k.seq, k.cursor, k.seen, k.active, k.tps,
      String(Date.now()), String(this.opts.tps), String(this.opts.maxActive),
      String(this.opts.sessionMs), String(this.opts.staleMs),
    ]);
    return Number(admitted);
  }

  async release(no: number): Promise<void> {
    const member = String(no);
    await Promise.all([
      this.redis.send("ZREM", [this.keys.active, member]),
      this.redis.send("ZREM", [this.keys.seen, member]),
    ]);
  }
}
```

### 7.4 다중 서버 실행과 sticky의 의미 변화

```powershell
$env:QUEUE_BACKEND = "redis"
$env:REDIS_URL = "redis://localhost:6379"

$env:NODE_ID = "q1"; $env:PORT = "3001"; bun run queue/server.ts
$env:NODE_ID = "q2"; $env:PORT = "3011"; bun run queue/server.ts
$env:NODE_ID = "q3"; $env:PORT = "3021"; bun run queue/server.ts
```

모든 서버가 매초 스케줄러 실행을 시도하지만 `SET NX PX 900`을 획득한 한 서버만 입장 스크립트를 실행하므로 입장 속도는 서버 대수와 무관하게 `QUEUE_TPS`로 유지된다. 대기 상태가 Redis에 있으므로 어느 서버가 조회 요청을 받아도 같은 결과를 돌려준다. 이 구성에서 `sticky` 값은 요청 라우팅에 필수가 아니라 디버깅과 추적용 정보가 된다.

| 항목 | 메모리 버전 (6장) | Redis 버전 (7장) |
| --- | --- | --- |
| 순서 보장 범위 | 서버별 | 전역 |
| 로드 밸런서 | sticky 라우팅 필수 | 아무 서버로 분산 가능 |
| 서버 장애 | 해당 서버 대기자 순번 유실 | 다른 서버가 즉시 대체 |
| 조회 응답 비용 | 메모리 접근 | Redis 왕복 (파이프라인) |
| 확장 | 서버 추가 시 대기자 재배치 불가 | 서버 추가만으로 조회 처리량 증가 |

## 8. 실시간 푸시 대안: WebSocket 브로드캐스트

### 8.1 폴링과 푸시의 비교

입장 커서 방식에서는 모든 대기자에게 필요한 정보가 사실상 같다. 현재 커서, 마지막 발급 번호, TPS만 알면 각자 자기 번호로 앞·뒤 인원을 계산할 수 있다. 따라서 서버가 매초 이 세 숫자를 모든 연결에 한 번씩 브로드캐스트하면 개별 조회 요청이 필요 없다.

| 항목 | TTL 폴링 | WebSocket 브로드캐스트 |
| --- | --- | --- |
| 요청 수 | 대기자 수 / TTL (초당) | 연결 유지, 서버가 초당 1회 발행 |
| 갱신 지연 | 최대 TTL | 최대 1초 |
| 서버 자원 | 요청당 HTTP 처리 | 연결당 메모리, 파일 디스크립터 |
| 중간 장비 | 일반 HTTP로 어디서나 동작 | 프록시·방화벽의 WebSocket 지원 필요 |
| 이탈 감지 | 조회 중단으로 감지 | 연결 종료로 즉시 감지 |

수십만 연결을 유지하는 비용과 인프라 호환성 때문에 대규모 공공 서비스는 여전히 TTL 폴링을 많이 쓴다. 두 방식을 함께 두고 WebSocket이 안 되는 환경에서 폴링으로 내려가는 구성도 가능하다.

### 8.2 구현: `queue/ws-server.ts`

Bun의 WebSocket은 토픽 구독과 `server.publish`를 내장하고 있어, 브로드캐스트를 반복문 없이 한 번의 호출로 처리한다.

```ts
import { MemoryQueue } from "./memory-queue";

const queue = new MemoryQueue({
  tps: Number(Bun.env.QUEUE_TPS ?? 50),
  maxActive: Number(Bun.env.QUEUE_MAX_ACTIVE ?? 500),
  sessionMs: 3 * 60_000,
  staleMs: 90_000,
});

const server = Bun.serve({
  port: Number(Bun.env.PORT ?? 3002),
  fetch(req, server) {
    if (new URL(req.url).pathname === "/queue/ws" && server.upgrade(req)) return;
    return new Response("not found", { status: 404 });
  },
  websocket: {
    open(ws) {
      ws.subscribe("queue");
      ws.send(JSON.stringify({ type: "snapshot", ...queue.snapshot }));
    },
    message() {},
    close() {},
  },
});

setInterval(() => {
  queue.tick();
  const { cursor, lastIssued, tps } = queue.snapshot;
  server.publish("queue", JSON.stringify({ type: "tick", cursor, lastIssued, tps }));
}, 1000);

console.log(`queue ws on ${server.url}`);
```

클라이언트는 진입 시 받은 자기 번호로 화면을 계산한다.

```js
const myNo = 15320;
const ws = new WebSocket("ws://localhost:3002/queue/ws");
ws.onmessage = (event) => {
  const { cursor, lastIssued, tps } = JSON.parse(event.data);
  const ahead = Math.max(0, myNo - cursor - 1);
  const behind = lastIssued - myNo;
  const eta = tps > 0 ? Math.ceil(ahead / tps) : "-";
  document.getElementById("status").textContent = `앞 ${ahead}명, 뒤 ${behind}명, 예상 ${eta}초`;
};
```

브로드캐스트 메시지에는 개인 정보가 없으므로 입장 토큰은 여전히 개별 HTTP 조회로 받아야 한다. 커서가 내 번호를 지나면 그때 한 번만 조회 요청을 보내면 된다.

## 9. 검증

### 9.1 정합성 테스트: `test/ticketing.test.ts`

대기열의 순서와 한도, 좌석의 중복 판매 방지, 토큰 위변조 검출을 `bun:test`로 검증한다.

```ts
import { describe, expect, test } from "bun:test";
import { MemoryQueue } from "../queue/memory-queue";
import { confirmSeat, holdSeat, openDb, seed } from "../booking/seats";
import { sign, verify } from "../shared/token";

const NOW = 1_000_000;

describe("MemoryQueue", () => {
  test("발급 순서대로 입장하고 초당 입장 수를 넘지 않는다", () => {
    const q = new MemoryQueue({ tps: 3, maxActive: 100, sessionMs: 180_000, staleMs: 90_000 });
    const nos = Array.from({ length: 10 }, (_, i) => q.enter(`u${i}`, NOW));

    expect(q.tick(NOW)).toBe(3);
    expect(q.status(nos[2]!, NOW).state).toBe("admitted");
    expect(q.status(nos[3]!, NOW)).toMatchObject({ state: "waiting", ahead: 0, behind: 6 });
  });

  test("같은 사용자가 다시 진입하면 기존 번호를 받는다", () => {
    const q = new MemoryQueue({ tps: 1, maxActive: 10, sessionMs: 180_000, staleMs: 90_000 });
    expect(q.enter("same", NOW)).toBe(q.enter("same", NOW));
  });

  test("동시 입장 한도를 넘지 않는다", () => {
    const q = new MemoryQueue({ tps: 10, maxActive: 2, sessionMs: 180_000, staleMs: 90_000 });
    for (let i = 0; i < 5; i++) q.enter(`u${i}`, NOW);
    expect(q.tick(NOW)).toBe(2);
    expect(q.tick(NOW + 1000)).toBe(0);
  });

  test("조회가 끊긴 대기자는 건너뛴다", () => {
    const q = new MemoryQueue({ tps: 5, maxActive: 10, sessionMs: 180_000, staleMs: 90_000 });
    const gone = q.enter("gone", NOW);
    const alive = q.enter("alive", NOW);
    q.status(alive, NOW + 100_000);

    expect(q.tick(NOW + 100_000)).toBe(1);
    expect(q.status(gone, NOW + 100_000).state).toBe("expired");
    expect(q.status(alive, NOW + 100_000).state).toBe("admitted");
  });

  test("세션이 만료되면 슬롯이 다음 대기자에게 넘어간다", () => {
    const q = new MemoryQueue({ tps: 1, maxActive: 1, sessionMs: 1000, staleMs: 90_000 });
    q.enter("a", NOW);
    const b = q.enter("b", NOW);

    expect(q.tick(NOW)).toBe(1);
    expect(q.tick(NOW + 500)).toBe(0);
    expect(q.tick(NOW + 1000)).toBe(1);
    expect(q.status(b, NOW + 1000).state).toBe("admitted");
  });
});

describe("좌석 선점과 확정", () => {
  test("같은 좌석은 한 명만 선점한다", () => {
    const db = openDb(":memory:");
    seed(db, "T", 1, 1);
    const until = Date.now() + 180_000;

    const winners = Array.from({ length: 1000 }, (_, i) => holdSeat(db, "T-1-1", `buyer-${i}`, until))
      .filter(Boolean);
    expect(winners).toHaveLength(1);
  });

  test("만료된 선점은 다른 사용자가 가져갈 수 있다", () => {
    const db = openDb(":memory:");
    seed(db, "T", 1, 1);

    expect(holdSeat(db, "T-1-1", "a", NOW + 10, NOW)).toBe(true);
    expect(holdSeat(db, "T-1-1", "b", NOW + 200, NOW + 5)).toBe(false);
    expect(holdSeat(db, "T-1-1", "b", NOW + 200, NOW + 20)).toBe(true);
  });

  test("선점이 만료된 뒤에는 확정할 수 없고, 확정은 한 번만 된다", () => {
    const db = openDb(":memory:");
    seed(db, "T", 1, 2);

    holdSeat(db, "T-1-1", "a", NOW + 10, NOW);
    expect(confirmSeat(db, "T-1-1", "a", NOW + 20)).toBeNull();

    holdSeat(db, "T-1-2", "b", NOW + 1000, NOW);
    expect(confirmSeat(db, "T-1-2", "b", NOW + 10)).not.toBeNull();
    expect(confirmSeat(db, "T-1-2", "b", NOW + 20)).toBeNull();
  });

  test("한 사용자는 좌석을 하나만 선점한다", () => {
    const db = openDb(":memory:");
    seed(db, "T", 1, 2);

    holdSeat(db, "T-1-1", "a", NOW + 1000, NOW);
    holdSeat(db, "T-1-2", "a", NOW + 1000, NOW);
    expect(holdSeat(db, "T-1-1", "b", NOW + 1000, NOW)).toBe(true);
  });
});

describe("서명 토큰", () => {
  test("변조된 토큰은 거부한다", async () => {
    const token = await sign({ kind: "ticket", sub: "u", no: 1, exp: Date.now() + 60_000 });
    const [body, sig] = token.split(".");
    const forged = Buffer.from(JSON.stringify({ kind: "ticket", sub: "u", no: 0, exp: Date.now() + 60_000 }))
      .toString("base64url");

    expect(await verify(`${body}.${sig}`, "ticket")).not.toBeNull();
    expect(await verify(`${forged}.${sig}`, "ticket")).toBeNull();
  });

  test("번호표를 입장 토큰으로 쓸 수 없고, 만료된 토큰은 거부한다", async () => {
    const ticket = await sign({ kind: "ticket", sub: "u", no: 1, exp: Date.now() + 60_000 });
    const expired = await sign({ kind: "admission", sub: "u", no: 1, exp: Date.now() - 1 });

    expect(await verify(ticket, "admission")).toBeNull();
    expect(await verify(expired, "admission")).toBeNull();
  });
});
```

```powershell
bun test
```

| 테스트 | 검증하는 설계 |
| --- | --- |
| 발급 순서·초당 입장 수 | 5.2 입장 커서, 5.4 TPS 한도, 5.3 앞·뒤 인원 계산 |
| 재진입 시 기존 번호 | 5.1 사용자당 번호표 하나 |
| 동시 입장 한도 | 5.4 최대 동시 사용자 |
| 이탈자 건너뛰기 | 5.7 이탈 판정 |
| 세션 만료 후 슬롯 이동 | 2.5 3분 제한, 5.7 슬롯 회수 |
| 좌석 단일 선점 | 5.11 조건부 UPDATE |
| 선점 만료와 확정 | 5.11 선점 만료, 확정 트랜잭션 |
| 토큰 변조·종류·만료 | 5.6 입장 토큰 |

### 9.2 부하 생성기: `load/load.ts`

실제 사용자처럼 진입하고, 서버가 지시한 TTL마다 조회하고, 입장하면 잠시 예매한 뒤 슬롯을 반환하는 가상 사용자를 동시에 실행한다. 외부 부하 도구 없이 Bun의 `fetch`와 `Bun.sleep`만 사용한다.

```ts
const QUEUE_URL = Bun.env.QUEUE_URL ?? "http://localhost:3001/queue";
const USERS = Number(Bun.argv[2] ?? 2000);
const CONCURRENCY = Number(Bun.argv[3] ?? 500);
const TTL_SCALE = Number(Bun.env.TTL_SCALE ?? 1);
const OPCODE = { ENTER: "5101", STATUS: "5002", LEAVE: "5004" } as const;

type Result = { waitMs: number; polls: number; admitted: boolean };
type Reply = { state: string; key?: string; ttl?: number };

let requests = 0;

async function call(uid: string, params: Record<string, string>): Promise<Reply> {
  requests++;
  const res = await fetch(`${QUEUE_URL}?${new URLSearchParams(params)}`, {
    headers: { Cookie: `uid=${uid}` },
  });
  return (await res.json()) as Reply;
}

async function virtualUser(i: number): Promise<Result> {
  const uid = `load-${i}`;
  const started = Bun.nanoseconds();

  let reply = await call(uid, { opcode: OPCODE.ENTER });
  const key = reply.key!;
  let polls = 0;

  while (reply.state === "waiting") {
    await Bun.sleep((reply.ttl ?? 2) * 1000 * TTL_SCALE);
    reply = await call(uid, { opcode: OPCODE.STATUS, key });
    polls++;
  }

  const waitMs = (Bun.nanoseconds() - started) / 1e6;
  const admitted = reply.state === "admitted";
  if (admitted) {
    await Bun.sleep(1000 + Math.random() * 4000);
    await call(uid, { opcode: OPCODE.LEAVE, key });
  }
  return { waitMs, polls, admitted };
}

function percentile(sorted: number[], p: number): number {
  return sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))] ?? 0;
}

const results: Result[] = [];
let next = 0;
const started = Bun.nanoseconds();

await Promise.all(
  Array.from({ length: CONCURRENCY }, async () => {
    while (next < USERS) {
      results.push(await virtualUser(next++));
    }
  }),
);

const elapsedSec = (Bun.nanoseconds() - started) / 1e9;
const waits = results.map((r) => r.waitMs).sort((a, b) => a - b);

console.table({
  users: USERS,
  concurrency: CONCURRENCY,
  admitted: results.filter((r) => r.admitted).length,
  elapsedSec: Number(elapsedSec.toFixed(1)),
  requests,
  requestsPerSec: Math.round(requests / elapsedSec),
  avgPolls: Number((results.reduce((s, r) => s + r.polls, 0) / results.length).toFixed(1)),
  waitP50Sec: Number((percentile(waits, 50) / 1000).toFixed(1)),
  waitP99Sec: Number((percentile(waits, 99) / 1000).toFixed(1)),
});
```

```powershell
$env:QUEUE_TPS = "50"
bun run queue/server.ts

bun run load/load.ts 2000 500
```

| 확인 항목 | 기대 결과 |
| --- | --- |
| `admitted` | `USERS`와 같아야 함 (이탈자가 없으므로 전원 입장) |
| 입장 속도 | 서버의 `/queue/metrics`에서 `tps`가 `QUEUE_TPS`를 넘지 않음 |
| `waitP99Sec` | 대략 `CONCURRENCY / QUEUE_TPS`초 근처 |
| `requestsPerSec` | 대기자 수와 TTL에 비례. TTL 기준을 바꿔 5.5의 감소 효과 확인 |

### 9.3 장애 시나리오 점검

| 시나리오 | 확인 방법 | 기대 동작 |
| --- | --- | --- |
| 대기열 서버 1대 중단 (Redis 버전) | 부하 중 서버 하나 종료 | 다른 서버가 조회를 받아 순번 유지 |
| 대기열 서버 1대 중단 (메모리 버전) | 부하 중 서버 하나 종료 | 해당 서버 대기자만 `expired` 응답 후 재진입 |
| Redis 지연 | 네트워크 지연 주입 | 조회 응답 지연, 입장 속도는 유지 |
| 예매 서버 과부하 | `QUEUE_TPS`를 과대 설정 | 예매 API 지연 증가 → 입장 속도를 낮춰야 함을 확인 |
| 대량 이탈 | 가상 사용자 일부를 중간에 중단 | 커서가 이탈자를 건너뛰고 입장 속도 유지 |

## 10. 운영

### 10.1 지표

| 지표 | 의미 | 조치 기준 |
| --- | --- | --- |
| 대기 인원 (`lastIssued - cursor`) | 대기열 길이 | 예상 대기 시간 공지 |
| 초당 입장 인원 | 실제 입장 속도 | 설정 TPS와의 차이 확인 |
| 입장 중 사용자 수 | 동시 세션 | 최대 동시 사용자 근접 시 TPS 조정 검토 |
| 세션 만료 비율 | 3분 내 예매 실패 비율 | 높으면 예매 화면 성능이나 UX 문제 |
| 이탈 비율 | 차례 전에 떠난 사용자 | 대기 시간 대비 이탈 추세 |
| 대기열 조회 RPS | 폴링 부하 | TTL 기준 조정 |
| 예매 API P99 지연 | 예매 서버 상태 | 지연 증가 시 TPS 하향 |
| 선점 충돌률 (409 비율) | 인기 좌석 경합 | 좌석 추천·분산 UI 검토 |

### 10.2 동적 TPS 조정

설정 TPS를 고정값으로 두면 예매 서버가 느려질 때 대응할 수 없다. 예매 서버의 P99 지연을 기준으로 TPS를 자동 조정하면 대기열이 예매 서버의 실제 상태에 맞춰 입장 속도를 바꾼다.

```ts
function adjustTps(current: number, bookingP99Ms: number, min = 10, max = 500): number {
  if (bookingP99Ms > 800) return Math.max(min, Math.floor(current * 0.7));
  if (bookingP99Ms < 300) return Math.min(max, Math.ceil(current * 1.1));
  return current;
}
```

느릴 때는 크게 줄이고 빠를 때는 조금씩 늘리는 비대칭 조정은 혼잡 제어에서 쓰는 방식과 같다. 급격한 진동을 막기 위해 조정 주기를 10초 이상으로 둔다.

### 10.3 장애 정책: 대기열 서버가 실패하면

| 정책 | 동작 | 위험 |
| --- | --- | --- |
| 실패 시 차단 (fail-closed) | 대기열 서버 장애 시 예매 서버도 입장 토큰 없는 요청을 거부 | 예매가 중단되지만 예매 서버와 재고는 보호 |
| 실패 시 개방 (fail-open) | 대기열 장애 시 토큰 검증을 생략하고 직접 입장 | 예매 서버로 전체 트래픽이 몰려 연쇄 장애 |

선착순 예매처럼 유입이 처리 능력을 크게 넘는 서비스는 실패 시 차단이 원칙이다. 예매 서버가 대기열 서버에 묻지 않고 서명만으로 토큰을 검증하는 6.6의 구조는, 대기열 서버가 잠시 멈춰도 이미 입장한 사용자의 예매는 계속 진행되게 한다.

### 10.4 정적 자원 분리

대기 화면의 HTML, 스크립트, 이미지는 CDN에서 제공하고, 대기열 서버는 조회 API만 처리하게 한다. 수십만 명이 대기 화면을 새로고침할 때 정적 자원 요청이 대기열 서버로 가면 조회 처리 능력이 그만큼 줄어든다.

## 11. 브라우저 개발자 도구로 직접 관찰하기

### 11.1 관찰 절차

| 단계 | 방법 |
| --- | --- |
| 1 | 크롬에서 F12로 개발자 도구를 열고 네트워크 탭 선택 |
| 2 | `Preserve log`를 켜서 페이지 이동 후에도 기록 유지 |
| 3 | 대기 화면 진입 후 반복되는 요청을 찾음. JSONP는 `JS` 필터, JSON은 `Fetch/XHR` 필터에 나타남 |
| 4 | 요청의 `Payload` 탭에서 `opcode`, `key`, `sticky` 같은 파라미터 확인 |
| 5 | `Response` 탭에서 대기 인원, TPS, TTL 값 확인 |
| 6 | 요청 목록의 시간 간격과 응답의 TTL 값이 일치하는지 비교 |
| 7 | `Timing` 탭에서 대기열 서버의 응답 시간 확인 |

### 11.2 관찰 시 유의 사항

- 서비스에 따라 뒤로 가기나 개발자 도구 사용 시 로그아웃될 수 있다. 실제로 예매해야 한다면 예매를 먼저 마치고 관찰한다.
- 관찰은 자신의 브라우저에서 오가는 통신을 보는 것에 그쳐야 한다. 요청을 조작하거나 자동화 도구로 반복 호출하는 행위는 서비스 약관 위반이며, 다른 사용자의 공정한 기회를 침해하고 업무 방해에 해당할 수 있다.
- 6.8의 예제를 로컬에서 실행하면 같은 형태의 통신을 제약 없이 관찰하고 실험할 수 있다.

## 12. 점검표

| 영역 | 점검 항목 |
| --- | --- |
| 입장 제어 | 예매 서버 처리 능력과 리틀의 법칙으로 초당 입장 인원과 최대 동시 사용자를 산정했는가? |
| 세션 제한 | 입장 후 체류 시간 상한(예: 3분)을 두고, 만료 시 슬롯과 좌석 선점을 함께 회수하는가? |
| 번호표 | 사용자당 번호표 하나를 원자적으로 발급하고, 재진입 시 기존 번호를 돌려주는가? |
| 위변조 | 번호표와 입장 토큰을 서버가 서명하고, 종류·만료·사용자를 검증하는가? |
| 폴링 부하 | 서버가 TTL로 폴링 주기를 지시하고, 순번에 따라 TTL을 조정하며, 클라이언트가 지터를 더하는가? |
| 이탈 처리 | 이탈 판정 시간이 최대 TTL보다 충분히 긴가? 커서가 이탈자를 건너뛰는가? |
| 상태 저장 | 메모리 방식이라면 sticky 라우팅과 서버 장애 영향을, Redis 방식이라면 원자적 스크립트와 해시 태그를 갖췄는가? |
| 스케줄러 | 다중 서버에서 입장 처리가 한 곳에서만 실행되도록 실행권을 제어하는가? |
| 재고 정합성 | 좌석 선점을 조건부 UPDATE로, 확정을 트랜잭션으로 처리하고 DB 제약으로 중복을 막는가? |
| 응답 형식 | 새 시스템은 CORS + JSON을 쓰고, JSONP가 필요하면 콜백 이름을 검증하는가? |
| 부정 사용 | 클라이언트 측 억제와 별개로 모든 판단을 서버에서 검증하는가? |
| 장애 정책 | 대기열 장애 시 실패 시 차단 정책과, 입장 토큰의 독립 검증이 가능한가? |
| 검증 | 순서·한도·중복 판매 방지를 테스트하고, 부하 생성기로 입장 속도와 폴링 RPS를 확인했는가? |
| 지표 | 대기 인원, 입장 속도, 세션 만료율, 이탈률, 조회 RPS, 예매 API 지연을 수집하는가? |

## 결론

명절 승차권 예매의 접속 대기열은 처리 능력을 늘리는 대신 유입을 처리 능력에 맞추는 입장 제어 시스템이다. 브라우저에서 관찰되는 `opcode`, `key`, `sticky` 요청 필드와 앞·뒤 대기 인원, TPS, TTL 응답 필드, 3분 세션 제한은 각각 단일 엔드포인트의 기능 구분, 위변조 방지 번호표, 서버별 상태 고정, 서버 주도 폴링 제어, 동시 사용자 상한 보장이라는 설계 의도로 해석된다.

핵심은 세 가지로 정리된다. 첫째, 입장 커서와 초당 입장 인원으로 예매 서버의 유입을 통제하고, 세션 시간 제한으로 리틀의 법칙의 체류 시간을 고정해 동시 사용자 수를 보장한다. 둘째, 서버가 순번에 따라 TTL을 조절해 대기자가 수십만 명이어도 대기열 서버 자신이 과부하에 빠지지 않게 한다. 셋째, 대기열은 유입만 통제할 뿐이므로 좌석 재고의 정합성은 예매 서버가 조건부 갱신과 트랜잭션으로 별도로 보장해야 한다.

본 보고서의 구현은 이 구조를 Bun 내장 API만으로 재현한다. `Bun.serve`의 라우트로 opcode 기반 대기열 서버와 예매 서버를, Web Crypto로 서명 토큰을, `bun:sqlite` 트랜잭션으로 좌석 선점과 확정을, 내장 `RedisClient`와 Lua 스크립트로 다중 서버 전역 대기열을, WebSocket 토픽 발행으로 푸시 방식을, `bun:test`와 `fetch` 기반 부하 생성기로 검증을 구성했다. 외부 패키지 없이도 대규모 선착순 시스템의 핵심 구조를 설계하고 실험할 수 있다.

## 참고 자료

- 코딩하는기술사, 「대규모 트래픽에 참여할 시간입니다 | 9월 7일 추석 기차표 예매」, 2026. <https://www.youtube.com/watch?v=nvHEaoYzApA>
- [Bun Docs: HTTP server (`Bun.serve`)](https://bun.com/docs/api/http)
- [Bun Docs: WebSockets](https://bun.com/docs/api/websockets)
- [Bun Docs: SQLite (`bun:sqlite`)](https://bun.com/docs/api/sqlite)
- [Bun Docs: Redis client](https://bun.com/docs/api/redis)
- [Bun Docs: Test runner (`bun:test`)](https://bun.com/docs/cli/test)
- [MDN: SubtleCrypto](https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto)
- [Redis Docs: Scripting with Lua](https://redis.io/docs/latest/develop/interactive/programmability/eval-intro/)
