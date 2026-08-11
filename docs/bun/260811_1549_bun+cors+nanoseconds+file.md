# Bun 기반 웹 서버-클라이언트 통신 및 데이터 처리 기술 보고서

## 1. 개요 (Overview)

본 보고서는 JavaScript/TypeScript 고성능 런타임인 **Bun**의 내장 API만을 활용하여 구축된 HTTP 웹 서버(`server.ts`)와 클라이언트(`client.ts`)의 양방향 데이터 통신 메커니즘을 상세히 분석합니다.

외부 3rd-party 패키지(Express, Axios, fs-extra 등)에 의존하지 않고, Bun 런타임 표준 내장 함수만으로 다음 3가지 핵심 요소를 완벽히 구현하는 방법을 다룹니다:
1. **CORS (Cross-Origin Resource Sharing) 양방향 완벽 대응**
2. **`Bun.file()` 및 `Bun.write()`를 활용한 데이터 누적(Cumulative) JSON 저장**
3. **`Bun.nanoseconds()`를 이용한 나노초 단위 정밀 통신 속도(RTT) 측정**

---

## 2. 전체 시스템 아키텍처 (System Architecture)

```
[ Client Environment ]                              [ Server Environment ]
+-------------------------+                        +-------------------------+
|      client.ts          |                        |        server.ts        |
|                         |                        |                         |
|  1. Bun.nanoseconds()   | -- POST /data Request ->| 1. CORS Preflight /     |
|     (시작 시간 기록)     |    (JSON Payload)        |    Main Handling        |
|                         |                        |                         |
|  2. fetch() 전송         |                        | 2. Bun.file().text()    |
|     (mode: "cors")      |                        |    (기존 JSON 로드)      |
|                         |                        |                         |
|                         |                        | 3. Array.push() &       |
|                         |                        |    Bun.write()          |
|                         |                        |    (누적 파일 저장)      |
|                         |                        |                         |
|  3. Bun.nanoseconds()   | <- JSON Response ------| 4. Response.json()      |
|     (종료 시간 기록)     |    (status, saved...)   |    (CORS Header 포함)   |
|                         |                        |                         |
|  4. RTT 속도 계산        |                        +-------------------------+
+-------------------------+                                     |
                                                                v
                                                    [ received_data.json ]
```

---

## 3. 서버 측 구현 상세 분석 (`server.ts`)

### 3.1 전체 소스 코드
```typescript
const server = Bun.serve({
  port: 3000,
  hostname: "0.0.0.0",

  async fetch(req) {
    const url = new URL(req.url);

    // 1. CORS 완전 허용 헤더 정의
    const corsHeaders = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type, Authorization",
    };

    // 2. Preflight (OPTIONS) 요청 사전 처리
    if (req.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }

    // 3. POST /data 요청 처리
    if (req.method === "POST" && url.pathname === "/data") {
      try {
        const body = await req.json();

        // 4. 기존 JSON 파일 읽기 및 데이터 누적
        const filePath = "./received_data.json";
        const file = Bun.file(filePath);

        let dataList: any[] = [];
        if (await file.exists()) {
          try {
            const existingText = await file.text();
            dataList = JSON.parse(existingText);
            if (!Array.isArray(dataList)) dataList = [];
          } catch {
            dataList = [];
          }
        }

        // 수신 타임스탬프 추가 후 배열에 병합
        const newRecord = { timestamp: new Date().toISOString(), ...body };
        dataList.push(newRecord);

        // 5. JSON 파일 저장
        await Bun.write(filePath, JSON.stringify(dataList, null, 2));

        console.log(`\n[데이터 누적 저장 완료] 총 ${dataList.length}개 기록됨`);

        return Response.json(
          { status: "success", count: dataList.length, saved: newRecord },
          { headers: corsHeaders }
        );
      } catch (error) {
        return Response.json(
          { status: "error", message: "잘못된 JSON 형식입니다." },
          { status: 400, headers: corsHeaders }
        );
      }
    }

    return new Response("Not Found", { status: 404, headers: corsHeaders });
  },
});

console.log(`서버 실행 중: http://localhost:${server.port}`);
```

### 3.2 핵심 기능별 세부 구현 원리

#### A. 바인딩 및 네트워크 수신 설정 (`hostname: "0.0.0.0"`)
* `hostname`을 기본값(`127.0.0.1`) 대신 `"0.0.0.0"`으로 지정하여 서버 내부 인터페이스뿐만 아니라, 동일 서브넷 LAN 환경 및 외부 IP/라우터로부터 유입되는 모든 네트워크 요청을 대기하도록 설정합니다.

#### B. CORS (Cross-Origin Resource Sharing) 핸들링
* **Preflight (OPTIONS) 처리:** 웹 브라우저나 일부 HTTP 클라이언트는 `Content-Type: application/json` 형태의 POST 요청을 보낼 때 실제 요청 전에 `OPTIONS` 메소드로 서버의 허용 여부를 점검합니다. 서버는 `200 OK` 혹은 `204 No Content`와 함께 CORS 헤더를 즉시 반환하도록 응답합니다.
* **CORS Response Headers:**
  * `Access-Control-Allow-Origin: *` : 모든 도메인/IP에서의 접근 허용.
  * `Access-Control-Allow-Methods` : GET, POST, OPTIONS 등 허용할 HTTP 메서드 명시.
  * `Access-Control-Allow-Headers` : 헤더에 `Content-Type` 등이 포함될 수 있도록 허용.

#### C. `Bun.file()` & `Bun.write()`를 통한 JSON 누적 저장
* **`Bun.file(path)`:** 해당 경로의 파일 핸들러(Blob)를 생성합니다.
* **`file.exists()` & `file.text()`:** 파일 존재 유무를 비동기로 확인하고, 파일이 존재할 경우 파일 텍스트 전체를 비동기로 읽어옵니다.
* **파싱 및 예외 안전 장치:** 읽어온 JSON이 배열 형식이 아니거나 파일이 손상된 경우 빈 배열(`[]`)로 자동 초기화하여 서버 다운을 방지합니다.
* **타임스탬프 결합:** `new Date().toISOString()`을 수신 객체에 추가하여 데이터 수신 시점을 보존합니다.
* **`Bun.write(path, data)`:** 파이프라인 최적화가 적용된 Bun의 내장 파일 쓰기 API로, `JSON.stringify(dataList, null, 2)` 형식으로 재기록합니다.

---

## 4. 클라이언트 측 구현 상세 분석 (`client.ts`)

### 4.1 전체 소스 코드
```typescript
async function sendData() {
  const payload = {
    userId: 101,
    name: "박민수",
    message: "Bun 내장 함수로 전송하는 데이터입니다.",
  };

  try {
    // 1. 통신 시작 시각 기록 (나노초)
    const startTime = Bun.nanoseconds();

    // 2. HTTP POST 요청 전송
    const response = await fetch("http://localhost:3000/data", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      mode: "cors", // CORS 모드 명시
      body: JSON.stringify(payload),
    });

    const result = await response.json();

    // 3. 통신 완료 시각 기록 및 RTT 계산
    const endTime = Bun.nanoseconds();
    const durationNs = endTime - startTime;
    const durationMs = durationNs / 1_000_000;

    console.log("[클라이언트 수신]");
    console.log(result);
    console.log(`\n⏱️ 통신 속도 (RTT): ${durationMs.toFixed(3)} ms (${durationNs.toLocaleString()} ns)`);
  } catch (error) {
    console.error("통신 오류:", error);
  }
}

sendData();
```

### 4.2 핵심 기능별 세부 구현 원리

#### A. `Bun.nanoseconds()` 기반 왕복 지연 시간 (RTT) 측정
* **작동 원리:** `Bun.nanoseconds()`는 단조 시계(Monotonic Clock)를 기반으로 고정밀 나노초($10^{-9}$초) 정수 값을 반환합니다. OS의 시스템 시간 변경(NTP 동기화 등)에 영향을 받지 않아 지연 시간 측정이 매우 정확합니다.
* **시간 단위 변환 공식:**
  $$	ext{durationMs} = rac{	ext{endTime} - 	ext{startTime}}{1,000,000}$$
* **결과 표시:** `toFixed(3)`을 이용해 소수점 3자리(마이크로초 단위까지 포함) 밀리초 형식으로 직관적으로 출력합니다.

#### B. `fetch` API와 CORS 설정
* Web Standard인 `fetch` 함수를 사용하여 서버로 JSON 바이트 스트림을 전송합니다.
* `mode: "cors"` 속성을 명시하여 크로스 오리진 요청 시 브라우저 정책 준수 및 헤더 검증을 명확히 수행합니다.

---

## 5. 이종 네트워크/서브넷 환경 구축 가이드

클라이언트와 서버의 서브넷 마스크나 IP 대역이 완전히 다를 경우 아래 사항을 추가 적용해야 합니다.

| 구성 항목 | 동일 PC (Local) | 동일 LAN (다른 서브넷) | 외부 인터넷 망 (공인 IP) |
| :--- | :--- | :--- | :--- |
| **Server Binding** | `127.0.0.1` 또는 `0.0.0.0` | `0.0.0.0` | `0.0.0.0` |
| **Client URL** | `http://localhost:3000/data` | `http://<서버내부IP>:3000/data` | `http://<서버공인IP>:3000/data` |
| **네트워크 요구사항** | 없음 | L3 라우팅 테이블 연결 | 포트 포워딩 (`3000` 포트) |
| **방화벽 설정** | 인바운드 영향 없음 | 3000번 포트 허용 필수 | 3000번 포트 허용 필수 |

---

## 6. 결론 및 요약

1. **내장 기능 기반 개발 극대화**: Bun 런타임의 `Bun.serve()`, `Bun.file()`, `Bun.write()`, `Bun.nanoseconds()` 내장 API만을 활용하여 외부 의존성 없이 가볍고 빠른 서버-클라이언트 아키텍처 구축이 가능합니다.
2. **안전한 데이터 관리**: 서버는 incoming 요청을 차곡차곡 배열로 파싱 및 덮어쓰기 형태로 저장을 지속하므로, 데이터 손실 없는 JSON 이벤트 로그 누적 시스템을 쉽게 완성할 수 있습니다.
3. **CORS 및 성능 측정 측정 표준화**: HTTP Preflight 대응 및 고정밀 나노초 타이머 조합으로 프로덕션 환경에서도 신뢰성 높은 네트워크 모니터링 체계를 확보할 수 있습니다.
