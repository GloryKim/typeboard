> 런타임: **Bun** (`Bun.serve`, `Bun.connect`, `Bun.hash.crc32` 사용)
> 총 995줄, 단일 파일 프로그램

---

## 1. 개요

이 파일은 **LiDAR 기반 군중/배회(Crowd/Roaming) 탐지 데이터를 중계하는 실시간 서버**다.
파일 최하단의 단 한 줄이 전체 프로그램의 진입점이다.

```ts
new Server().start();   // 995번째 줄
```

동작을 한 문장으로 요약하면 다음과 같다.

> **업스트림 "브릿지(bridge)" WebSocket에 접속해 LiDAR 좌표 데이터를 받아서,
> (1) 브라우저(프론트엔드)로 WebSocket/HTTP로 실시간 중계하고,
> (2) 조건에 맞는 이벤트를 Kafka로 발행한다.**

주석(`// Go: ...`, `Go asFloat/...`)과 `crowd-bun`, `crowd.ts` 같은 문자열을 보면
**원래 Go로 작성돼 있던 서버를 Bun/TypeScript로 포팅한 것**임을 알 수 있다.
외부 Kafka 라이브러리를 쓰지 않고 Kafka 와이어 프로토콜을 직접 구현한 점이 이 파일의 가장 큰 특징이다.

### 데이터 흐름 (전체 그림)

```
[LiDAR 브릿지]                         [프론트엔드 브라우저]
  ws://127.0.0.1:8765                    /ws/coordinates (WS)
        │                                /coordinates    (HTTP GET)
        │  tracks / crowds JSON                 ▲
        ▼                                       │ snapshot / delta
  ┌───────────────────────────────────────────────────────┐
  │                     Server (this file)                 │
  │   applyBridgeJSON() → live Map(마커) → broadcast()      │
  │        │                                               │
  │        └─→ kafkaCh 큐 → kafkaLoop() → KafkaProducer     │
  └───────────────────────────────────────────────────────┘
                            │
                            ▼
                   [Kafka 브로커 클러스터]
                    2.2.2.2:30000~30002
```

---



## 2. 파일 구조 (섹션별)

파일은 주석 구분선(`// ----`)으로 논리 블록이 나뉘어 있다.


| 구간(줄)   | 섹션                | 역할                                                          |
| ------- | ----------------- | ----------------------------------------------------------- |
| 1–13    | 유틸                | `log`, `sleep`                                              |
| 15–36   | env helpers       | 환경변수 읽기 (`env`, `envFloat`, `envInt`)                       |
| 38–110  | types & JSON 강제변환 | `Marker`, `LidarSignal` 타입, `asFloat/asInt/asBool/asString` |
| 112–200 | 도메인 헬퍼            | ID 생성, 방향 계산, 시간 포맷, JSON 직렬화                               |
| 202–525 | Kafka 와이어 프로토콜    | `KafkaBuf`, `KafkaReader`, `KafkaProducer` 등 직접 구현          |
| 527–981 | `Server` 클래스      | 서버 본체 (브릿지 수신 · 브로드캐스트 · Kafka 루프 · HTTP)                   |
| 983–995 | main              | `kafkaBrokers()`, 진입점                                       |


---



## 3. 유틸리티 함수 (1–36줄)



### `log` (1–9줄)

자체 타임스탬프 로거. 형식이 특이하다.

```
YYYY/MM/DD HH:MM:SS.mmm000 <msg>
```

`getMilliseconds()`(3자리) 뒤에 `000`을 붙여 **마이크로초 자리(6자리)를 흉내낸다**.
즉 밀리초까지만 실제 값이고 나머지 3자리는 항상 `000`이다. (아마 Go의 로그 포맷을 맞추기 위한 것)

### `sleep(ms)` (11–13줄)

`setTimeout` 기반 Promise. 코드 전반의 `await sleep(...)`에 사용된다.

### env 헬퍼 (19–36줄)

- `env(k, def)`: 값이 `undefined`거나 **빈 문자열이면** 기본값 사용.
- `envFloat` / `envInt`: 파싱 실패(`NaN`/비유한값)면 기본값으로 폴백. `Number.isFinite` 검증이 들어 있어 안전하다.

---



## 4. 타입 정의 (42–79줄)



### `Marker` (44–60줄)

프론트엔드에 뿌리는 **화면 표시용 마커** 한 개를 나타낸다.

- 위치: `x, y, z`
- 식별: `marker_id`(문자열), `sensor_id`, `object_id`
- 종류: `point_type` (`"track"` 또는 `"crowd"`)
- 상태 플래그: `is_roaming`, `is_crowding`, `is_crowd_suspicious`
- 선택 필드(`?`): `roam_state`, `crowd_state`, `radius`, `occupant_count`
- `created_at`: 생성 시각 문자열



### `LidarSignal` (62–72줄)

**Kafka로 내보낼 이벤트** 한 건. `type`은 `"Roaming"` 또는 `"Crowd"` 두 가지뿐이다.
`objectId`, `locationLine`, `occupantCount`, `image`, `direction`은 선택 필드.

### Kafka 관련 상수 (74–79줄)

```ts
KAFKA_EVENT_ID    = "202608"                    // 이벤트 그룹 ID
KAFKA_LOCATION_LINE = "2"                        // 위치(호선 등)
KAFKA_CAMERA_IDX  = 3                            // 카메라 인덱스
KAFKA_IMAGE_URL   = "http://1.1.1.1:12344/image" // 이미지 URL (플레이스홀더성 값)
KAFKA_QUEUE_CAP   = 4096                          // 내부 Kafka 큐 최대 길이
```

`1.1.1.1`, `2.2.2.2`(브로커) 같은 값은 실제 주소라기보다 **자리표시자(placeholder)** 로 보인다.

---



## 5. JSON 강제 변환 함수 (85–110줄)

브릿지에서 오는 JSON은 타입이 들쭉날쭉할 수 있으므로(숫자가 문자열로 오는 등), Go의 `asFloat/asInt/...`를 옮긴 관대한 변환기를 둔다.

- `asFloat(v)` **→** `[number, boolean]`: 유한한 number이거나, 파싱 가능한 비어있지 않은 문자열이면 `[값, true]`. 아니면 `[0, false]`. **성공 여부를 튜플로 반환**하는 게 핵심.
- `asInt(v)`: `asFloat` 후 `Math.trunc`(버림). 실패 시 0.
- `asBool(v)`: boolean은 그대로, number는 `!==0`, 문자열은 `"true"`/`"1"`만 참.
- `asString(v)`: 문자열이면 그대로, `null`/`undefined`는 `""`, 나머지는 `String()`.

---



## 6. 도메인 헬퍼 (112–200줄)



### ID 생성 (112–118줄)

마커의 고유 ID를 만든다. 센서/오브젝트별로 유일하다.

```ts
liveTrackID(2, 15)  →  "s2-live-track-15"
liveCrowdID(2, 15)  →  "s2-live-crowd-15"
```



### `directionFromTheta(theta)` (120–127줄)

각도(도 단위)를 한글 방위로 변환한다. 먼저 `theta % 360` 후 음수 보정.


| 각도 범위             | 방향  |
| ----------------- | --- |
| 315°~360°, 0°~45° | 동   |
| 45°~135°          | 북   |
| 135°~225°         | 서   |
| 225°~315°         | 남   |


> 주의: 통상적인 수학 좌표계(0°=동/양의 x축)에서 90°는 "북"으로 매핑되어 있다. 반시계 방향으로 동→북→서→남 순서다.



### 시간 포맷 함수 (129–156줄)

- `nowKSTCompact()`: `Intl.DateTimeFormat`으로 **서울 시간대(Asia/Seoul)** 기준 `YYYYMMDDHHMMSS` 14자리 문자열 생성. Kafka 이벤트의 `startDatetime`에 사용.
- `rfc3339Nano(d)`: ISO 문자열을 **나노초(9자리)** 정밀도 RFC3339로 변환. 실제 정밀도는 밀리초(3자리)까지고 뒤 6자리는 `0`으로 채운다. 마커 `created_at`, 스냅샷 `timestamp` 등에 사용.
- `rfc3339(d)`: 소수 초를 아예 제거한 표준 RFC3339 (`...SSZ`). `/health`의 `timestamp`에 사용.



### `crowdDirection(obj)` (158–164줄)

군중 방향 결정 우선순위:

1. `obj.direction` 문자열이 있으면 그대로,
2. 없으면 `obj.theta_t`를 각도로 보고 `directionFromTheta`,
3. 그것도 없으면 기본값 **"동"**.



### 직렬화 함수 (166–200줄)

- `markerToJSON(m)`: 필수 필드를 먼저 복사하고, 선택 필드(`roam_state`, `crowd_state`, `radius`, `occupant_count`)는 **truthy할 때만** 추가한다.
  - ⚠️ 주의점: `if (m.radius)` / `if (m.occupant_count)` 조건 때문에 값이 **정확히** `0`**이면 출력에서 빠진다**. 반경 0이나 인원수 0은 표현되지 않는다.
- `lidarSignalToJSON(sig)`: 마찬가지로 선택 필드는 `!== undefined`일 때만 넣는다. 이쪽은 `undefined` 검사라 `0`도 살아남는다.

---



## 7. Kafka 와이어 프로토콜 구현 (202–525줄)

**이 파일에서 가장 무거운 부분.** 외부 npm/Kafka 라이브러리 없이, 원시 TCP 소켓(`Bun.connect`)으로 Kafka 프로토콜을 손수 구현했다. 구버전(v0/v1/v3) API를 사용한다.

### 7.1 `concatChunks` (206–215줄)

여러 `Uint8Array` 청크를 하나로 이어붙이는 헬퍼. 버퍼 조립/수신 누적에 반복 사용된다.

### 7.2 `KafkaBuf` (217–270줄) — 빅엔디안 직렬화 버퍼

Kafka는 **빅엔디안(big-endian)**을 쓰므로 `DataView`의 `setInt16/32/64(..., false)`로 기록한다.

- `writeInt8/16/32/64`: 정수 기록. `writeInt64`는 `BigInt`로 변환.
- `writeString`: **길이(int16) + UTF-8 바이트** (Kafka의 문자열 표현).
- `writeBytes`: **길이(int32) + 데이터**.
- `writeNullableBytes`: `null`이면 길이 `-1`만 기록.
- `writeRaw`: 길이 없이 원시 바이트 추가.
- `toUint8Array`: 지금까지 쌓인 청크를 하나로 합쳐 반환.



### 7.3 `KafkaReader` (272–331줄) — 역직렬화

`KafkaBuf`의 반대. 오프셋 `off`를 들고 순차적으로 읽는다.

- `readInt8/16/32/64`, `readString`(길이<0이면 `""`), `readNullableString`(길이<0이면 `null`), `readBytes`, `skip(n)`.
- 문자열/바이트 읽기는 `subarray`로 **복사 없이** 슬라이스한다.



### 7.4 `parseBroker` (333–336줄)

`"host:port"` 문자열을 파싱. `lastIndexOf(":")` 를 써서 IPv6 등 콜론이 여럿인 주소도 마지막 콜론만 포트 구분자로 취급한다.

### 7.5 `encodeKafkaMessage` (338–356줄) — 레코드셋(MessageSet) 인코딩

구버전 Kafka 메시지 포맷을 조립한다.

1. **inner**: `magic=1`(int8), `attributes=0`(int8), `key=null`(nullable bytes), `value`(bytes) → payload
2. payload의 **CRC32**를 `Bun.hash.crc32`로 계산(`>>> 0`로 부호없는 32비트화)
3. **message**: `CRC(int32)` + payload
4. **set**: `offset=0`(int64) + `message`(bytes)



### 7.6 `buildRequest` (358–372줄) — 요청 프레이밍

모든 Kafka 요청 공통 헤더를 붙인다.

```
[int32 전체길이]
  [int16 apiKey][int16 apiVersion][int32 correlationId]
  [string clientId="crowd-bun"]
  [body...]
```

`kafkaCorrelation`(358줄)은 모듈 전역 카운터로 요청마다 1씩 증가한다.

### 7.7 `kafkaRoundTrip` (374–422줄) — TCP 왕복 통신

`Bun.connect`로 TCP 접속 → 요청 전송 → 응답 수신을 하나의 Promise로 감싼다.

- **길이 프리픽스 프레이밍**: 첫 4바이트로 전체 응답 길이를 읽어 `expected = 길이 + 4`를 정하고, 그만큼 다 모이면 완료.
- **타임아웃**(기본 8초): 시간 초과 시 reject.
- `settled` 플래그로 **중복 resolve/reject 방지**. `open/data/close/error` 콜백과 `.catch`가 모두 `finish`/`reject`를 부를 수 있어 방어 장치가 필요하다.
- 응답을 다 받으면 `_sock.end()`로 소켓을 닫는다.



### 7.8 `kafkaFetchLeader` (430–481줄) — 파티션 리더 조회 (Metadata API)

- **apiKey=3(Metadata), version=1** 요청을 만들어(body: 토픽 1개) 브로커 목록을 순회한다.
- 응답을 파싱해 **브로커 목록**(id→host:port)과 **토픽/파티션 정보**를 읽는다.
- `errCode===0 && part===0`인 **파티션 0의 리더 브로커**를 찾아 그 host/port를 반환한다.
- 모든 브로커 조회 실패 시 **첫 번째 브로커 주소로 폴백**(479–480줄).
- 파싱 과정에서 replica/isr 목록은 읽어서 버리고(466·468줄), rack 문자열도 스킵한다(448줄) — 오프셋을 정확히 맞추기 위해 필요한 read다.



### 7.9 `KafkaProducer` (483–525줄) — 프로듀서

- 생성자에서 브로커 목록/토픽 저장, `brokerBytes`(브로커별 전송 바이트 통계)를 0으로 초기화.
- `publish(value)`:
  1. `leaderCache`가 없으면 `kafkaFetchLeader`로 리더를 찾아 캐싱.
  2. `encodeKafkaMessage`로 레코드셋 생성.
  3. **Produce 요청(apiKey=0, version=3)** body 구성:
    - `-1`(transactional/…), `acks=1`, `timeout=8000`, 토픽 1개, 토픽명, 파티션 1개, 파티션 0, 레코드셋.
  4. `kafkaRoundTrip`으로 전송하고 성공하면 바이트 통계 갱신.
  5. **실패 시** `leaderCache`**를 null로 리셋**하고 예외를 던진다 → 다음 발행 때 리더를 다시 조회(리더 변경/장애 대응).

> 참고: `body.writeInt32(-1)` 등 매직 넘버들은 특정 Kafka API 버전의 필드 순서에 하드코딩돼 있다. 응답 본문을 파싱하지 않으므로(전송만 확인) 브로커 에러코드는 검사하지 않는다.

---



## 8. `Server` 클래스 (533–981줄)

프로그램의 본체. 상태·타이머·클라이언트·Kafka 큐를 모두 들고 있다.

### 8.1 주요 필드 (534–554줄)

- `bridgeURL`, `httpPort`, `sensorID`, `displayTTLms`: 설정값.
- `live: Map<string, Marker>`: **현재 화면에 떠 있는 마커들**(marker_id → Marker).
- `seq`: 프론트로 보내는 메시지 시퀀스 번호.
- `lastBridge`: 마지막으로 브릿지 데이터를 받은 시각.
- `clients: Set<WsClient>`: 접속한 브라우저 WebSocket들.
- `expireTimers` / `expireGen`: 마커별 **만료 타이머**와 **세대(generation) 카운터**.
- `kafkaEnabled`, `kafkaProducer`, `kafkaTopic`, `kafkaCh`(내부 큐), `kafkaPeriodMs`: Kafka 관련.
- `lastRoamPub: Map<number, number>` / `lastCrowdPub: number`: **발행 rate-limit(스로틀)** 용 마지막 발행 시각.
- `bridgeChain: Promise<void>`: 브릿지 메시지를 **순차 처리**하기 위한 Promise 체인(뒤에서 설명).



### 8.2 생성자 (556–584줄) — 환경변수 설정


| 환경변수                                    | 기본값                   | 의미                                           |
| --------------------------------------- | --------------------- | -------------------------------------------- |
| `HTTP_PORT`                             | `1111`                | HTTP/WS 서버 포트 (`:` 접두어 허용)                   |
| `MARKER_DISPLAY_SEC` / `MARKER_TTL_SEC` | `2`                   | 마커 표시 유지 시간(초). 앞이 0 이하면 뒤로 폴백, 그래도 0 이하면 2초 |
| `BRIDGE_WS`                             | `ws://127.0.0.1:8765` | 업스트림 브릿지 주소                                  |
| `SENSOR_ID`                             | `2`                   | 기본 센서 ID                                     |
| `KAFKA_PERIOD_SEC`                      | `5`                   | Kafka 발행 최소 간격(스로틀 주기)                       |
| `KAFKA_DISABLE`                         | `0`                   | `"1"`이면 Kafka 비활성화                           |
| `KAFKA_LIDAR_TOPIC`                     | `lidar-topic`         | Kafka 토픽명                                    |
| `KAFKA_BROKERS`                         | `2.2.2.2:30000~30002` | 브로커 목록(쉼표 구분, `kafkaBrokers()`에서 파싱)         |


Kafka가 켜져 있으면 `KafkaProducer`를 만들고 설정을 로그로 남긴다.

### 8.3 마커 만료 메커니즘 (586–624줄) — 핵심 로직

LiDAR 마커는 계속 갱신되지 않으면 화면에서 사라져야 한다. 이를 **세대(generation) 기반 타이머**로 구현했다.

- `scheduleExpireLocked(id)` (591–600줄): 해당 마커의 `expireGen`을 +1 하고, **이전 타이머를 취소**한 뒤 `displayTTLms` 후 만료되도록 새 타이머를 건다. 마커가 갱신될 때마다 호출되므로 **TTL이 계속 연장**된다.
- `onMarkerExpire(id, gen)` (602–624줄):
  - **세대 검증**: `expireGen.get(id) !== gen`이면 무시. → 그 사이 마커가 갱신돼(세대가 올라가) 이 타이머는 **낡은 것**이므로 아무 일도 안 한다. (오래된 타이머가 최신 마커를 잘못 지우는 것을 방지)
  - 여전히 유효하면 `live`에서 삭제하고 타이머/세대 정보 정리 후, 프론트로 `live_remove: [id]` **delta 메시지**를 브로드캐스트한다.

> 이 세대 카운터 패턴이 이 파일에서 가장 정교한 동시성 방어 장치다. `clearTimeout`만으로는 이미 큐에 들어간 콜백을 못 막는 경우가 있어 세대 비교를 함께 쓴다.



### 8.4 스냅샷 생성 (626–647줄)

- `buildSnapshotLocked(now)`: `seq`를 올리고 `live`의 모든 마커를 `markerToJSON`으로 변환해 **전체 스냅샷** payload를 만든다. 특이하게 마커 배열을 `live_markers`, `markers` **두 필드에 중복**으로 넣고, `trail_markers`는 항상 빈 배열이다(주석대로 trail 기능 미사용).
  - 공통 메타: `source:"crowd"`, `map_version:2`, `use_crowd_map:true`, `markers_ttl_ms`.
- `latestSnapshot()`: 마커가 하나도 없으면 `[{}, false]`, 있으면 `[스냅샷, true]`.



### 8.5 브로드캐스트 (649–663줄)

`clients` 집합의 모든 WebSocket에 JSON 문자열을 보낸다. `[...this.clients]`**로 복사본을 순회**하여 순회 중 삭제해도 안전하게 하고, `send` 실패 시 소켓을 닫고 집합에서 제거한다.

### 8.6 브릿지 메시지 처리 (665–810줄) — 데이터 수신의 핵심



#### 순차 처리 큐 (665–667줄)

```ts
queueBridgeJSON(raw) {
  this.bridgeChain = this.bridgeChain.then(() => this.applyBridgeJSON(raw));
}
```

`bridgeChain` Promise에 체인으로 이어붙여 **메시지를 도착 순서대로, 겹치지 않게 한 번에 하나씩** 처리한다. `applyBridgeJSON`이 `await`(Kafka enqueue)을 포함하므로 병렬 실행 시 상태가 꼬일 수 있는데, 이 체인이 그것을 방지한다.

#### `applyBridgeJSON(raw)` (669–810줄)

1. 문자열/ArrayBuffer를 텍스트로 만들고 `JSON.parse`. **파싱 실패면 조용히 무시**(674–679줄).
2. `sensor_id`를 읽되 0 이하면 서버 기본 `sensorID` 사용(682–683줄).
3. `lastBridge` 갱신, 이번 배치에서 발행할 `kafkaOut` 배열 준비.

내부에 세 개의 클로저를 둔다.

`upsertMarker(m)` (688–691줄): `live`에 마커를 넣고(**얕은 복사** `{...m}`) 만료 타이머를 재설정.

`ingestTrack(obj)` (693–733줄) — 개별 추적 대상(사람) 처리:

- `x, y` 없으면 무시. `z`, `object_id`, `roam_state` 파싱.
- `is_roaming`이 true거나 `roam_state==="ROAMING"`이면 배회로 판단. `roam_state`가 비면 상태에 맞춰 `"ROAMING"`/`"NORMAL"` 채움.
- `point_type:"track"` 마커를 upsert.
- **배회 상태일 때 Kafka 발행 (스로틀 적용)**: 같은 `object_id`의 마지막 발행 후 `kafkaPeriodMs`(기본 5초) 이내면 스킵(720–722줄). 아니면 `lastRoamPub` 갱신하고 `type:"Roaming"` 시그널을 `kafkaOut`에 **push(뒤에 추가)**.

`ingestCrowd(obj)` (735–783줄) — 군중 처리:

- `x, y` 없으면 무시.
- `crowd_state` 결정: 명시값 없으면 `is_crowding`→`"CROWDING"`, `is_crowd_suspicious`→`"SUSPICIOUS"`. 둘 다 아니면 **처리 중단**(745줄).
- `z`, `radius`, `object_id`, `occupant_count`(0 이하면 1로 보정) 파싱.
- `point_type:"crowd"` 마커를 upsert (`is_crowding`/`is_crowd_suspicious`를 상태에 맞춰 세팅).
- **군중 Kafka 발행 (스로틀 적용)**: `lastCrowdPub` 기준 `kafkaPeriodMs` 이내면 스킵. 아니면 `type:"Crowd"` 시그널을 `kafkaOut`에 **unshift(맨 앞에 추가)** → 군중 이벤트를 배회보다 우선 처리하려는 의도. `direction`은 `crowdDirection`으로 결정.

**입력 형태 3가지 지원** (785–805줄):

- `payload.tracks` 배열 → 각각 `ingestTrack`
- `payload.crowds` 배열 → 각각 `ingestCrowd`
- **단일 객체 형태**(배열 없이 최상위에 `x`가 바로 있는 경우): `crowd_state`/`is_crowding`/`is_crowd_suspicious`가 있으면 군중으로, 아니고 `object_id`가 있으면 추적으로 처리. 다양한 브릿지 페이로드 스키마에 유연하게 대응.

1. 마지막에 **스냅샷 브로드캐스트**(807–808줄) 후 `enqueueKafka(kafkaOut)`을 `await`.

> 즉 프론트로는 **매 브릿지 메시지마다 전체 스냅샷**을 보내고, 마커가 TTL로 사라질 때만 `live_remove` **delta**를 별도로 보낸다.



### 8.7 Kafka 큐잉 (812–855줄)

`enqueueKafka(sigs)` (812–837줄) — 큐에 넣기:

- Kafka 비활성/빈 배열이면 즉시 반환.
- `Crowd` **타입**: 큐가 꽉 찼으면(`>= 4096`) 최대 **200ms 동안 1ms씩 대기하며 자리 나기를 기다림**. 그래도 안 나면 `"kafka queue stalled, drop Crowd"` 로그 후 드롭. 자리 나면 push. → **군중 이벤트를 최대한 살리려는 backpressure**.
- **그 외(**`Roaming`**)**: 큐가 꽉 찼으면 **즉시 드롭**(`"kafka queue full, drop ..."`). 기다리지 않음.

즉 **Crowd > Roaming 우선순위**가 여기서도 드러난다(unshift + 대기 vs push + 즉시 드롭).

`kafkaLoop()` (839–855줄) — 소비 루프:

- 무한 루프로 `kafkaCh.shift()`(맨 앞) 하나씩 꺼냄. 없으면 1ms sleep.
- 시그널을 JSON→UTF-8 바이트로 만들어 `kafkaProducer.publish`. 성공/실패 모두 로그. **실패해도 루프는 계속 돈다**(개별 실패가 전체를 멈추지 않음).

> `enqueueKafka`(생산)와 `kafkaLoop`(소비)가 `kafkaCh` 배열 하나를 공유하는 **단일 생산자-소비자 큐** 구조. `start()`에서 `kafkaLoop()`을 한 번 띄워둔다.



### 8.8 HTTP 핸들러 (857–889줄)

- `handleHealth()` (`/health`): 상태 JSON — `status`, `service:"crowd"`, 현재 마커 수, 브라우저 클라이언트 수, 브릿지 주소, Kafka 활성 여부, 마지막 브릿지 수신 시각, 현재 시각.
- `handleCoordinates(req)` (`/coordinates`):
  - `OPTIONS`(CORS preflight)면 허용 헤더 반환.
  - 최신 스냅샷을 반환하되 마커가 없으면 **404** `{error:"no coordinates"}`. 모든 응답에 `Access-Control-Allow-Origin: `*(전체 허용).



### 8.9 브라우저 WebSocket 콜백 (891–907줄)

- `onBrowserOpen(ws)`: 클라이언트 집합에 추가하고, **접속 즉시 현재 스냅샷을 1회 전송**(초기 화면 채우기).
- `onBrowserClose(ws)`: 집합에서 제거.



### 8.10 브릿지 접속 루프 (909–941줄)

`bridgeLoop()`은 **무한 재접속 루프**다.

1. `new WebSocket(bridgeURL)`로 접속. **핸드셰이크 10초 타임아웃**.
2. 접속되면 `message` 이벤트마다 `queueBridgeJSON`으로 넘긴다. `close`/`error`면 내부 Promise를 resolve해 루프를 다시 돈다.
3. 접속 자체 실패면 3초 대기 후 재시도, 정상 종료 후엔 2초 대기 후 재접속.

→ **브릿지가 죽어도 서버는 계속 살아서 재연결을 시도**한다.

### 8.11 서버 기동 (943–980줄)

`start()`:

1. `kafkaLoop()`, `bridgeLoop()`을 백그라운드로 띄운다(`void`, await 안 함).
2. `Bun.serve` 로 HTTP 서버를 `0.0.0.0:httpPort`에 연다.
  - `/ws/coordinates`: `server.upgrade(req)`로 **WebSocket 업그레이드**. 실패면 500.
  - `/health`, `/coordinates`: 각 핸들러. 그 외는 **404 "Not Found"**.
  - `websocket` 핸들러: `open`→`onBrowserOpen`, `close`→`onBrowserClose`, `message`**는 무시**(주석: Go에서도 읽기 전용, 들어오는 프레임은 소비만). 즉 프론트→서버 방향 메시지는 처리하지 않는다.
3. 리스닝 정보와 라우팅 요약을 로그로 남긴다.

---



## 9. main (983–995줄)

- `kafkaBrokers()`: `KAFKA_BROKERS` 환경변수를 쉼표로 분리·trim·빈 값 제거해 브로커 배열 반환. 기본 `["2.2.2.2:30000","2.2.2.2:30001","2.2.2.2:30002"]`.
- 마지막 줄 `new Server().start();` — 인스턴스를 만들고 즉시 기동.

---



## 10. 종합 정리



### 이 서버가 하는 일 (요약)


| #   | 기능                                     | 관련 코드                                                               |
| --- | -------------------------------------- | ------------------------------------------------------------------- |
| 1   | 업스트림 브릿지 WS에 무한 재접속하며 LiDAR JSON 수신    | `bridgeLoop`, `queueBridgeJSON`                                     |
| 2   | tracks/crowds/단일객체 형태를 파싱해 마커로 변환      | `applyBridgeJSON`, `ingestTrack`, `ingestCrowd`                     |
| 3   | 마커를 TTL(기본 2초) 동안만 유지, 만료 시 프론트에 제거 알림 | `scheduleExpireLocked`, `onMarkerExpire`                            |
| 4   | 프론트로 WS/HTTP를 통해 스냅샷·delta 실시간 중계      | `broadcast`, `handleCoordinates`, `Bun.serve`                       |
| 5   | 배회/군중 이벤트를 스로틀 걸어 Kafka로 발행            | `enqueueKafka`, `kafkaLoop`, `KafkaProducer`                        |
| 6   | 외부 라이브러리 없이 Kafka 프로토콜 직접 구현           | `KafkaBuf`, `KafkaReader`, `kafkaFetchLeader`, `encodeKafkaMessage` |




### 설계상 두드러지는 특징

- **의존성 제로 지향**: Bun 내장 기능(`Bun.serve`/`connect`/`hash`)만으로 HTTP·WS·Kafka를 전부 처리. `package.json` 없이도 돌아가는 단일 파일.
- **Go → TS 포팅**: 함수 이름/주석/로그 포맷이 원본 Go 서버를 그대로 따라간다.
- **정교한 만료 처리**: 세대(generation) 카운터로 오래된 타이머의 오작동을 방지.
- **순차 처리 보장**: `bridgeChain` Promise 체인으로 async 처리의 순서·원자성 유지.
- **우선순위/백프레셔**: Crowd 이벤트를 Roaming보다 우선(큐 앞 삽입 + 대기), Roaming은 큐 포화 시 즉시 드롭.
- **장애 내성**: 브릿지·Kafka 실패가 프로세스를 죽이지 않고 재시도/재조회로 복구.



### 잠재적 유의점 (이 파일 범위 내에서 관찰되는 것)

- `markerToJSON`에서 `if (m.radius)` / `if (m.occupant_count)` truthy 검사 때문에 **값이** `0`**이면 직렬화에서 누락**된다.
- `KafkaProducer.publish`는 Produce **응답 본문을 파싱하지 않아** 브로커가 반환하는 에러코드를 확인하지 않는다(전송 성공=발행 성공으로 간주).
- `brokerBytes` 통계(514–519줄)는 수집만 하고 어디에도 노출/사용되지 않는다.
- `2.2.2.2`, `1.1.1.1`, `KAFKA_EVENT_ID="202608"` 등은 실제 운영값이라기보다 **플레이스홀더**로 보이며, 배포 시 환경변수/상수로 교체가 필요해 보인다.
- 프론트→서버 WebSocket 메시지는 의도적으로 무시하므로 **단방향(서버→프론트) 스트리밍** 전용이다.


# 코드

```typescript
function log(msg: string, ...args: unknown[]): void {
  const t = new Date();
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  const ts =
    `${t.getFullYear()}/${p(t.getMonth() + 1)}/${p(t.getDate())} ` +
    `${p(t.getHours())}:${p(t.getMinutes())}:${p(t.getSeconds())}.` +
    `${String(t.getMilliseconds()).padStart(3, "0")}000`;
  console.log(`${ts} ${msg}`, ...args);
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// ---------------------------------------------------------------------------
// env helpers
// ---------------------------------------------------------------------------

function env(k: string, def: string): string {
  const v = process.env[k];
  return v !== undefined && v !== "" ? v : def;
}

function envFloat(k: string, def: number): number {
  const v = process.env[k];
  if (v === undefined || v === "") return def;
  const f = Number.parseFloat(v);
  return Number.isFinite(f) ? f : def;
}

function envInt(k: string, def: number): number {
  const v = process.env[k];
  if (v === undefined || v === "") return def;
  const n = Number.parseInt(v, 10);
  return Number.isFinite(n) ? n : def;
}

// ---------------------------------------------------------------------------
// types
// ---------------------------------------------------------------------------

type JsonMap = Record<string, unknown>;

interface Marker {
  marker_id: string;
  sensor_id: number;
  x: number;
  y: number;
  z: number;
  point_type: string;
  object_id: number;
  is_roaming: boolean;
  roam_state?: string;
  crowd_state?: string;
  is_crowding: boolean;
  is_crowd_suspicious: boolean;
  radius?: number;
  occupant_count?: number;
  created_at: string;
}

interface LidarSignal {
  eventId: string;
  type: "Roaming" | "Crowd";
  startDatetime: string;
  objectId?: number;
  locationLine?: string;
  occupantCount?: number;
  cameraIdx: number;
  image?: string;
  direction?: string;
}

const KAFKA_EVENT_ID = "202608";
const KAFKA_LOCATION_LINE = "2";
const KAFKA_CAMERA_IDX = 3;
const KAFKA_IMAGE_URL = "http://1.1.1.1:12344/image";

const KAFKA_QUEUE_CAP = 4096;

// ---------------------------------------------------------------------------
// JSON coercion (Go asFloat/asInt/asBool/asString)
// ---------------------------------------------------------------------------

function asFloat(v: unknown): [number, boolean] {
  if (typeof v === "number" && Number.isFinite(v)) return [v, true];
  if (typeof v === "string" && v !== "") {
    const f = Number.parseFloat(v);
    if (Number.isFinite(f)) return [f, true];
  }
  return [0, false];
}

function asInt(v: unknown): number {
  const [f, ok] = asFloat(v);
  return ok ? Math.trunc(f) : 0;
}

function asBool(v: unknown): boolean {
  if (typeof v === "boolean") return v;
  if (typeof v === "number") return v !== 0;
  if (typeof v === "string") return v === "true" || v === "1";
  return false;
}

function asString(v: unknown): string {
  if (typeof v === "string") return v;
  if (v === null || v === undefined) return "";
  return String(v);
}

function liveTrackID(sensorID: number, objectID: number): string {
  return `s${sensorID}-live-track-${objectID}`;
}

function liveCrowdID(sensorID: number, objectID: number): string {
  return `s${sensorID}-live-crowd-${objectID}`;
}

function directionFromTheta(theta: number): string {
  let t = theta % 360;
  if (t < 0) t += 360;
  if (t >= 315 || t < 45) return "동";
  if (t < 135) return "북";
  if (t < 225) return "서";
  return "남";
}

function nowKSTCompact(): string {
  const fmt = new Intl.DateTimeFormat("en-GB", {
    timeZone: "Asia/Seoul",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
  const parts = fmt.formatToParts(new Date());
  const g = (t: string) => parts.find((p) => p.type === t)?.value ?? "00";
  return `${g("year")}${g("month")}${g("day")}${g("hour")}${g("minute")}${g("second")}`;
}

function rfc3339Nano(d: Date): string {
  const iso = d.toISOString();
  const dot = iso.indexOf(".");
  if (dot < 0) return iso.replace("Z", ".000000000Z");
  const ms = iso.slice(dot + 1, iso.length - 1);
  return `${iso.slice(0, dot)}.${(ms + "000000").slice(0, 9)}Z`;
}

function rfc3339(d: Date): string {
  const iso = d.toISOString();
  return iso.replace(/\.\d{3}Z$/, "Z");
}

function crowdDirection(obj: JsonMap): string {
  const d = asString(obj["direction"]);
  if (d !== "") return d;
  const [theta, ok] = asFloat(obj["theta_t"]);
  if (ok) return directionFromTheta(theta);
  return "동";
}

function markerToJSON(m: Marker): Marker {
  const o: Marker = {
    marker_id: m.marker_id,
    sensor_id: m.sensor_id,
    x: m.x,
    y: m.y,
    z: m.z,
    point_type: m.point_type,
    object_id: m.object_id,
    is_roaming: m.is_roaming,
    is_crowding: m.is_crowding,
    is_crowd_suspicious: m.is_crowd_suspicious,
    created_at: m.created_at,
  };
  if (m.roam_state) o.roam_state = m.roam_state;
  if (m.crowd_state) o.crowd_state = m.crowd_state;
  if (m.radius) o.radius = m.radius;
  if (m.occupant_count) o.occupant_count = m.occupant_count;
  return o;
}

function lidarSignalToJSON(sig: LidarSignal): Record<string, unknown> {
  const o: Record<string, unknown> = {
    eventId: sig.eventId,
    type: sig.type,
    startDatetime: sig.startDatetime,
    cameraIdx: sig.cameraIdx,
  };
  if (sig.objectId !== undefined) o.objectId = sig.objectId;
  if (sig.locationLine !== undefined) o.locationLine = sig.locationLine;
  if (sig.occupantCount !== undefined) o.occupantCount = sig.occupantCount;
  if (sig.image !== undefined) o.image = sig.image;
  if (sig.direction !== undefined) o.direction = sig.direction;
  return o;
}

// ---------------------------------------------------------------------------
// Kafka wire protocol (Bun.connect, no external libs)
// ---------------------------------------------------------------------------

function concatChunks(chunks: Uint8Array[]): Uint8Array {
  const n = chunks.reduce((s, c) => s + c.length, 0);
  const out = new Uint8Array(n);
  let off = 0;
  for (const c of chunks) {
    out.set(c, off);
    off += c.length;
  }
  return out;
}

class KafkaBuf {
  private chunks: Uint8Array[] = [];

  writeInt8(n: number): void {
    const b = new Uint8Array(1);
    b[0] = n & 0xff;
    this.chunks.push(b);
  }

  writeInt16(n: number): void {
    const b = new Uint8Array(2);
    new DataView(b.buffer).setInt16(0, n, false);
    this.chunks.push(b);
  }

  writeInt32(n: number): void {
    const b = new Uint8Array(4);
    new DataView(b.buffer).setInt32(0, n, false);
    this.chunks.push(b);
  }

  writeInt64(n: number): void {
    const b = new Uint8Array(8);
    new DataView(b.buffer).setBigInt64(0, BigInt(n), false);
    this.chunks.push(b);
  }

  writeString(s: string): void {
    const enc = new TextEncoder().encode(s);
    this.writeInt16(enc.length);
    this.chunks.push(enc);
  }

  writeBytes(data: Uint8Array): void {
    this.writeInt32(data.length);
    this.chunks.push(data);
  }

  writeNullableBytes(data: Uint8Array | null): void {
    if (data === null) {
      this.writeInt32(-1);
      return;
    }
    this.writeBytes(data);
  }

  writeRaw(data: Uint8Array): void {
    this.chunks.push(data);
  }

  toUint8Array(): Uint8Array {
    return concatChunks(this.chunks);
  }
}

class KafkaReader {
  private view: DataView;
  private off = 0;

  constructor(private buf: Uint8Array) {
    this.view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  }

  readInt8(): number {
    const v = this.view.getInt8(this.off);
    this.off += 1;
    return v;
  }

  readInt16(): number {
    const v = this.view.getInt16(this.off, false);
    this.off += 2;
    return v;
  }

  readInt32(): number {
    const v = this.view.getInt32(this.off, false);
    this.off += 4;
    return v;
  }

  readInt64(): number {
    const v = this.view.getBigInt64(this.off, false);
    this.off += 8;
    return Number(v);
  }

  readString(): string {
    const len = this.readInt16();
    if (len < 0) return "";
    const s = this.buf.subarray(this.off, this.off + len);
    this.off += len;
    return new TextDecoder().decode(s);
  }

  readNullableString(): string | null {
    const len = this.readInt16();
    if (len < 0) return null;
    const s = this.buf.subarray(this.off, this.off + len);
    this.off += len;
    return new TextDecoder().decode(s);
  }

  readBytes(): Uint8Array {
    const len = this.readInt32();
    if (len < 0) return new Uint8Array(0);
    const s = this.buf.subarray(this.off, this.off + len);
    this.off += len;
    return s;
  }

  skip(n: number): void {
    this.off += n;
  }
}

function parseBroker(addr: string): { host: string; port: number } {
  const i = addr.lastIndexOf(":");
  return { host: addr.slice(0, i), port: Number.parseInt(addr.slice(i + 1), 10) };
}

function encodeKafkaMessage(value: Uint8Array): Uint8Array {
  const inner = new KafkaBuf();
  inner.writeInt8(1);
  inner.writeInt8(0);
  inner.writeNullableBytes(null);
  inner.writeBytes(value);
  const payload = inner.toUint8Array();
  const crc = Bun.hash.crc32(payload) >>> 0;

  const message = new KafkaBuf();
  message.writeInt32(crc);
  message.writeRaw(payload);
  const messageBytes = message.toUint8Array();

  const set = new KafkaBuf();
  set.writeInt64(0);
  set.writeBytes(messageBytes);
  return set.toUint8Array();
}

let kafkaCorrelation = 1;

function buildRequest(apiKey: number, apiVersion: number, body: Uint8Array): Uint8Array {
  const hdr = new KafkaBuf();
  hdr.writeInt16(apiKey);
  hdr.writeInt16(apiVersion);
  hdr.writeInt32(kafkaCorrelation++);
  hdr.writeString("crowd-bun");
  hdr.writeRaw(body);
  const payload = hdr.toUint8Array();
  const frame = new KafkaBuf();
  frame.writeInt32(payload.length);
  frame.writeRaw(payload);
  return frame.toUint8Array();
}

async function kafkaRoundTrip(host: string, port: number, request: Uint8Array, timeoutMs = 8000): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const chunks: Uint8Array[] = [];
    let expected = -1;
    let settled = false;

    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        reject(new Error(`kafka timeout ${host}:${port}`));
      }
    }, timeoutMs);

    const finish = (err: Error | null, data?: Uint8Array) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (err) reject(err);
      else resolve(data!);
    };

    Bun.connect({
      hostname: host,
      port,
      socket: {
        open(sock) {
          sock.write(request);
        },
        data(_sock, data) {
          chunks.push(data);
          const buf = concatChunks(chunks);
          if (expected < 0 && buf.length >= 4) {
            expected = new DataView(buf.buffer, buf.byteOffset, buf.byteLength).getInt32(0, false) + 4;
          }
          if (expected > 0 && buf.length >= expected) {
            finish(null, buf.subarray(0, expected));
            _sock.end();
          }
        },
        close() {
          if (!settled) finish(new Error("kafka socket closed early"));
        },
        error(_sock, err) {
          finish(err instanceof Error ? err : new Error(String(err)));
        },
      },
    }).catch((err) => finish(err instanceof Error ? err : new Error(String(err))));
  });
}

interface BrokerMeta {
  id: number;
  host: string;
  port: number;
}

async function kafkaFetchLeader(brokers: string[], topic: string): Promise<{ host: string; port: number }> {
  const body = new KafkaBuf();
  body.writeInt32(1);
  body.writeString(topic);
  const req = buildRequest(3, 1, body.toUint8Array());

  for (const addr of brokers) {
    const { host, port } = parseBroker(addr);
    try {
      const resp = await kafkaRoundTrip(host, port, req);
      const r = new KafkaReader(resp.subarray(4));
      r.readInt32();
      const brokerCount = r.readInt32();
      const brokerMap = new Map<number, BrokerMeta>();
      for (let i = 0; i < brokerCount; i++) {
        const id = r.readInt32();
        const h = r.readString();
        const p = r.readInt32();
        r.readNullableString(); // rack (metadata v1)
        brokerMap.set(id, { id, host: h, port: p });
      }
      const topicCount = r.readInt32();
      for (let ti = 0; ti < topicCount; ti++) {
        const err = r.readInt16();
        if (err !== 0) {
          r.readString();
          continue;
        }
        const tname = r.readString();
        if (tname !== topic) continue;
        const partCount = r.readInt32();
        for (let pi = 0; pi < partCount; pi++) {
          const errCode = r.readInt16();
          const part = r.readInt32();
          const leader = r.readInt32();
          const replicaCount = r.readInt32();
          for (let ri = 0; ri < replicaCount; ri++) r.readInt32();
          const isrCount = r.readInt32();
          for (let ii = 0; ii < isrCount; ii++) r.readInt32();
          if (errCode === 0 && part === 0) {
            const b = brokerMap.get(leader);
            if (b) return { host: b.host, port: b.port };
          }
        }
      }
    } catch {
      continue;
    }
  }
  const first = parseBroker(brokers[0]!);
  return { host: first.host, port: first.port };
}

class KafkaProducer {
  private topic: string;
  private brokers: string[];
  private brokerBytes = new Map<string, number>();
  private leaderCache: { host: string; port: number } | null = null;

  constructor(brokers: string[], topic: string) {
    this.brokers = brokers;
    this.topic = topic;
    for (const b of brokers) this.brokerBytes.set(b, 0);
  }

  async publish(value: Uint8Array): Promise<void> {
    if (!this.leaderCache) {
      this.leaderCache = await kafkaFetchLeader(this.brokers, this.topic);
    }
    const recordSet = encodeKafkaMessage(value);
    const body = new KafkaBuf();
    body.writeInt32(-1);
    body.writeInt16(1);
    body.writeInt32(8000);
    body.writeInt32(1);
    body.writeString(this.topic);
    body.writeInt32(1);
    body.writeInt32(0);
    body.writeBytes(recordSet);
    const req = buildRequest(0, 3, body.toUint8Array());

    const leaderKey = `${this.leaderCache.host}:${this.leaderCache.port}`;
    try {
      await kafkaRoundTrip(this.leaderCache.host, this.leaderCache.port, req, 8000);
      for (const b of this.brokers) {
        if (b.includes(this.leaderCache.host)) {
          this.brokerBytes.set(b, (this.brokerBytes.get(b) ?? 0) + value.length);
        }
      }
      this.brokerBytes.set(leaderKey, (this.brokerBytes.get(leaderKey) ?? 0) + value.length);
    } catch (err) {
      this.leaderCache = null;
      throw err;
    }
  }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

type WsClient = ServerWebSocket<unknown>;

class Server {
  bridgeURL: string;
  httpPort: number;
  sensorID: number;
  displayTTLms: number;

  private live = new Map<string, Marker>();
  private seq = 0;
  private lastBridge = new Date(0);
  private clients = new Set<WsClient>();
  private expireTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private expireGen = new Map<string, number>();

  private kafkaEnabled: boolean;
  private kafkaProducer: KafkaProducer | null = null;
  private kafkaTopic: string;
  private kafkaCh: LidarSignal[] = [];
  private kafkaPeriodMs: number;
  private lastRoamPub = new Map<number, number>();
  private lastCrowdPub = 0;

  private bridgeChain: Promise<void> = Promise.resolve();

  constructor() {
    let portStr = env("HTTP_PORT", "1111");
    if (!portStr.startsWith(":")) portStr = ":" + portStr;
    this.httpPort = Number.parseInt(portStr.replace(":", ""), 10);

    let displaySec = envFloat("MARKER_DISPLAY_SEC", 0);
    if (displaySec <= 0) displaySec = envFloat("MARKER_TTL_SEC", 2);
    if (displaySec <= 0) displaySec = 2;

    this.bridgeURL = env("BRIDGE_WS", "ws://127.0.0.1:8765");
    this.sensorID = envInt("SENSOR_ID", 2);
    this.displayTTLms = displaySec * 1000;

    let periodSec = envFloat("KAFKA_PERIOD_SEC", 5);
    if (periodSec <= 0) periodSec = 5;
    this.kafkaPeriodMs = periodSec * 1000;

    this.kafkaEnabled = env("KAFKA_DISABLE", "0") !== "1";
    this.kafkaTopic = env("KAFKA_LIDAR_TOPIC", "lidar-topic");

    if (this.kafkaEnabled) {
      const brokers = kafkaBrokers();
      this.kafkaProducer = new KafkaProducer(brokers, this.kafkaTopic);
      log(`kafka → ${JSON.stringify(brokers)} topic=${this.kafkaTopic} eventId=${KAFKA_EVENT_ID} period=${this.kafkaPeriodMs}ms`);
    } else {
      log("kafka disabled (KAFKA_DISABLE=1)");
    }
    log(`marker display TTL=${this.displayTTLms}ms (delta live_update → live_remove)`);
  }

  private nextSeqLocked(): number {
    this.seq++;
    return this.seq;
  }

  private scheduleExpireLocked(id: string): void {
    const gen = (this.expireGen.get(id) ?? 0) + 1;
    this.expireGen.set(id, gen);
    const prev = this.expireTimers.get(id);
    if (prev) clearTimeout(prev);
    this.expireTimers.set(
      id,
      setTimeout(() => this.onMarkerExpire(id, gen), this.displayTTLms),
    );
  }

  private onMarkerExpire(id: string, gen: number): void {
    if (this.expireGen.get(id) !== gen) return;
    if (!this.live.has(id)) return;

    this.live.delete(id);
    const t = this.expireTimers.get(id);
    if (t) clearTimeout(t);
    this.expireTimers.delete(id);
    this.expireGen.delete(id);

    const now = new Date();
    const payload = {
      source: "crowd",
      timestamp: rfc3339Nano(now),
      seq: this.nextSeqLocked(),
      marker_event: "delta",
      map_version: 2,
      use_crowd_map: true,
      markers_ttl_ms: this.displayTTLms,
      live_remove: [id],
    };
    void this.broadcast(payload);
  }

  private buildSnapshotLocked(now: Date): Record<string, unknown> {
    this.seq++;
    const live: Marker[] = [];
    for (const m of this.live.values()) live.push(markerToJSON(m));
    return {
      source: "crowd",
      timestamp: rfc3339Nano(now),
      seq: this.seq,
      marker_event: "snapshot",
      map_version: 2,
      use_crowd_map: true,
      markers_ttl_ms: this.displayTTLms,
      live_markers: live,
      trail_markers: [] as Marker[],
      markers: live,
    };
  }

  latestSnapshot(): [Record<string, unknown>, boolean] {
    if (this.live.size === 0) return [{}, false];
    return [this.buildSnapshotLocked(new Date()), true];
  }

  private async broadcast(payload: Record<string, unknown>): Promise<void> {
    const msg = JSON.stringify(payload);
    for (const ws of [...this.clients]) {
      try {
        ws.send(msg);
      } catch {
        try {
          ws.close();
        } catch {
          /* ignore */
        }
        this.clients.delete(ws);
      }
    }
  }

  queueBridgeJSON(raw: string | ArrayBuffer): void {
    this.bridgeChain = this.bridgeChain.then(() => this.applyBridgeJSON(raw));
  }

  private async applyBridgeJSON(raw: string | ArrayBuffer): Promise<void> {
    let text: string;
    if (typeof raw === "string") text = raw;
    else text = new TextDecoder().decode(raw);

    let payload: JsonMap;
    try {
      payload = JSON.parse(text) as JsonMap;
    } catch {
      return;
    }

    const now = new Date();
    let sid = asInt(payload["sensor_id"]);
    if (sid <= 0) sid = this.sensorID;

    this.lastBridge = now;
    const kafkaOut: LidarSignal[] = [];

    const upsertMarker = (m: Marker): void => {
      this.live.set(m.marker_id, { ...m });
      this.scheduleExpireLocked(m.marker_id);
    };

    const ingestTrack = (obj: JsonMap): void => {
      const [x, okX] = asFloat(obj["x"]);
      const [y, okY] = asFloat(obj["y"]);
      if (!okX || !okY) return;

      const [z] = asFloat(obj["z"]);
      const oid = asInt(obj["object_id"]);
      let roamState = asString(obj["roam_state"]);
      const roaming = asBool(obj["is_roaming"]) || roamState === "ROAMING";
      if (roamState === "") roamState = roaming ? "ROAMING" : "NORMAL";

      const id = liveTrackID(sid, oid);
      upsertMarker({
        marker_id: id,
        sensor_id: sid,
        x,
        y,
        z,
        point_type: "track",
        object_id: oid,
        is_roaming: roaming,
        roam_state: roamState,
        is_crowding: false,
        is_crowd_suspicious: false,
        created_at: rfc3339Nano(now),
      });

      if (roaming) {
        const last = this.lastRoamPub.get(oid) ?? 0;
        if (last > 0 && now.getTime() - last < this.kafkaPeriodMs) return;
        this.lastRoamPub.set(oid, now.getTime());
        kafkaOut.push({
          eventId: KAFKA_EVENT_ID,
          type: "Roaming",
          startDatetime: nowKSTCompact(),
          objectId: oid,
          cameraIdx: KAFKA_CAMERA_IDX,
          image: KAFKA_IMAGE_URL,
        });
      }
    };

    const ingestCrowd = (obj: JsonMap): void => {
      const [x, okX] = asFloat(obj["x"]);
      const [y, okY] = asFloat(obj["y"]);
      if (!okX || !okY) return;

      let state = asString(obj["crowd_state"]);
      if (state === "") {
        if (asBool(obj["is_crowding"])) state = "CROWDING";
        else if (asBool(obj["is_crowd_suspicious"])) state = "SUSPICIOUS";
      }
      if (state !== "CROWDING" && state !== "SUSPICIOUS") return;

      const [z] = asFloat(obj["z"]);
      const [radius] = asFloat(obj["radius"]);
      const oid = asInt(obj["object_id"]);
      let occupantCount = asInt(obj["occupant_count"]);
      if (occupantCount <= 0) occupantCount = 1;

      const id = liveCrowdID(sid, oid);
      upsertMarker({
        marker_id: id,
        sensor_id: sid,
        x,
        y,
        z,
        point_type: "crowd",
        object_id: oid,
        crowd_state: state,
        is_crowding: state === "CROWDING",
        is_crowd_suspicious: state === "SUSPICIOUS",
        is_roaming: false,
        radius,
        occupant_count: occupantCount,
        created_at: rfc3339Nano(now),
      });

      if (this.lastCrowdPub > 0 && now.getTime() - this.lastCrowdPub < this.kafkaPeriodMs) return;
      this.lastCrowdPub = now.getTime();
      kafkaOut.unshift({
        eventId: KAFKA_EVENT_ID,
        type: "Crowd",
        startDatetime: nowKSTCompact(),
        locationLine: KAFKA_LOCATION_LINE,
        occupantCount,
        cameraIdx: KAFKA_CAMERA_IDX,
        image: KAFKA_IMAGE_URL,
        direction: crowdDirection(obj),
      });
    };

    const tracks = payload["tracks"];
    if (Array.isArray(tracks)) {
      for (const item of tracks) {
        if (item && typeof item === "object") ingestTrack(item as JsonMap);
      }
    }
    const crowds = payload["crowds"];
    if (Array.isArray(crowds)) {
      for (const item of crowds) {
        if (item && typeof item === "object") ingestCrowd(item as JsonMap);
      }
    }

    if (payload["x"] !== undefined) {
      const cs = asString(payload["crowd_state"]);
      if (cs !== "" || asBool(payload["is_crowding"]) || asBool(payload["is_crowd_suspicious"])) {
        ingestCrowd(payload);
      } else if (payload["object_id"] !== undefined) {
        ingestTrack(payload);
      }
    }

    const payloadOut = this.buildSnapshotLocked(now);
    void this.broadcast(payloadOut);
    await this.enqueueKafka(kafkaOut);
  }

  private async enqueueKafka(sigs: LidarSignal[]): Promise<void> {
    if (!this.kafkaEnabled || sigs.length === 0) return;
    for (const sig of sigs) {
      if (sig.type === "Crowd") {
        const deadline = Date.now() + 200;
        while (this.kafkaCh.length >= KAFKA_QUEUE_CAP) {
          if (Date.now() >= deadline) {
            log("kafka queue stalled, drop Crowd");
            break;
          }
          await sleep(1);
        }
        if (this.kafkaCh.length < KAFKA_QUEUE_CAP) {
          this.kafkaCh.push(sig);
          log("kafka queued Crowd");
        }
        continue;
      }
      if (this.kafkaCh.length >= KAFKA_QUEUE_CAP) {
        log(`kafka queue full, drop ${sig.type}`);
        continue;
      }
      this.kafkaCh.push(sig);
      log(`kafka queued ${sig.type} objectId=${sig.objectId}`);
    }
  }

  private async kafkaLoop(): Promise<void> {
    if (!this.kafkaEnabled || !this.kafkaProducer) return;
    while (true) {
      const sig = this.kafkaCh.shift();
      if (!sig) {
        await sleep(1);
        continue;
      }
      const body = new TextEncoder().encode(JSON.stringify(lidarSignalToJSON(sig)));
      try {
        await this.kafkaProducer.publish(body);
        log(`kafka → ${new TextDecoder().decode(body)}`);
      } catch (err) {
        log(`kafka enqueue fail: ${err} body=${new TextDecoder().decode(body)}`);
      }
    }
  }

  private handleHealth(): Response {
    return Response.json({
      status: "healthy",
      service: "crowd",
      live_markers: this.live.size,
      browser_clients: this.clients.size,
      bridge: this.bridgeURL,
      kafka: this.kafkaEnabled,
      last_bridge_at: rfc3339Nano(this.lastBridge),
      timestamp: rfc3339(new Date()),
    });
  }

  private handleCoordinates(req: Request): Response {
    if (req.method === "OPTIONS") {
      return new Response(null, {
        status: 200,
        headers: {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "GET, OPTIONS",
        },
      });
    }
    const [payload, ok] = this.latestSnapshot();
    const headers = {
      "Access-Control-Allow-Origin": "*",
      "Content-Type": "application/json",
    };
    if (!ok) {
      return Response.json({ error: "no coordinates" }, { status: 404, headers });
    }
    return Response.json(payload, { headers });
  }

  onBrowserOpen(ws: WsClient): void {
    this.clients.add(ws);
    log(`browser ws connected (${this.clients.size} clients)`);
    const [payload, ok] = this.latestSnapshot();
    if (ok) {
      try {
        ws.send(JSON.stringify(payload));
      } catch {
        /* ignore */
      }
    }
  }

  onBrowserClose(ws: WsClient): void {
    this.clients.delete(ws);
    log("browser ws closed");
  }

  private async bridgeLoop(): Promise<void> {
    while (true) {
      log(`dial bridge ${this.bridgeURL}`);
      try {
        const ws = new WebSocket(this.bridgeURL);
        await new Promise<void>((resolve, reject) => {
          const t = setTimeout(() => reject(new Error("handshake timeout")), 10000);
          ws.addEventListener("open", () => {
            clearTimeout(t);
            resolve();
          });
          ws.addEventListener("error", () => {
            clearTimeout(t);
            reject(new Error("ws error"));
          });
        });
        log("bridge connected");
        await new Promise<void>((resolve) => {
          ws.addEventListener("message", (ev) => {
            this.queueBridgeJSON(ev.data as string | ArrayBuffer);
          });
          ws.addEventListener("close", () => resolve());
          ws.addEventListener("error", () => resolve());
        });
        ws.close();
      } catch (err) {
        log(`bridge dial fail: ${err}`);
        await sleep(3000);
        continue;
      }
      await sleep(2000);
    }
  }

  start(): void {
    void this.kafkaLoop();
    void this.bridgeLoop();

    const self = this;

    Bun.serve({
      port: this.httpPort,
      hostname: "0.0.0.0",
      fetch(req, server) {
        const url = new URL(req.url);
        if (url.pathname === "/ws/coordinates") {
          if (server.upgrade(req)) return undefined as never;
          return new Response("upgrade failed", { status: 500 });
        }
        if (url.pathname === "/health") return self.handleHealth();
        if (url.pathname === "/coordinates") return self.handleCoordinates(req);
        return new Response("Not Found", { status: 404 });
      },
      websocket: {
        open(ws) {
          self.onBrowserOpen(ws);
        },
        message(_ws, _msg) {
          // Go: read loop only — consume incoming frames
        },
        close(ws) {
          self.onBrowserClose(ws);
        },
      },
    });

    log(`crowd.ts listening http://0.0.0.0:${this.httpPort}`);
    log(`  bridge  ← ${this.bridgeURL}`);
    log(`  front   → /ws/coordinates  /coordinates  /health (delta + snapshot)`);
    log(`  marker  → snapshot(이동) + live_remove delta after ${this.displayTTLms}ms`);
    log(`  kafka   → ${this.kafkaTopic} (eventId=${KAFKA_EVENT_ID} cameraIdx=${KAFKA_CAMERA_IDX})`);
  }
}

function kafkaBrokers(): string[] {
  const raw = env("KAFKA_BROKERS", "2.2.2.2:30000,2.2.2.2:30001,2.2.2.2:30002");
  return raw
    .split(",")
    .map((p) => p.trim())
    .filter((p) => p !== "");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

new Server().start();
```