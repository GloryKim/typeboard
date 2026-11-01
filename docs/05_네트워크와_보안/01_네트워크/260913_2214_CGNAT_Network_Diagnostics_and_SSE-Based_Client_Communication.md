# CGNAT 환경의 네트워크 진단과 SSE 기반 클라이언트 통신

영상출처 : https://www.youtube.com/watch?v=MAJ2oexlexI&t=54s

## 요약

CGNAT(Carrier-Grade NAT)은 여러 가입자가 하나의 공인 IPv4 주소를 공유하도록 ISP가 수행하는 주소 변환이다. 사용자의 공유기 뒤에 ISP의 NAT 장비가 한 단계 더 존재하므로, 외부에서 사용자의 장치로 직접 연결하거나 포트 포워딩을 설정하기 어렵다. 이 구조 자체는 해킹의 증거가 아니다.

CGNAT 클라이언트에 서버가 데이터를 보내야 한다면, 서버가 클라이언트의 사설 주소로 접속하는 방식은 사용할 수 없다. 클라이언트가 먼저 공인 서버에 장시간 연결을 만들고, 서버가 그 연결을 통해 데이터를 내려보내는 구조가 필요하다. 웹 환경에서는 SSE(Server-Sent Events)가 이 목적에 적합한 단방향(server → client) 기술이다.

## 1. 주소 대역을 해석하는 방법

다음 주소는 사용 목적이 서로 다르므로, 주소만 보고 공격 여부를 판단해서는 안 된다.

| 대역 | 의미 | 일반적인 관찰 위치 |
| --- | --- | --- |
| `192.168.0.0/16`, `10.0.0.0/8`, `172.16.0.0/12` | RFC 1918 사설 주소 | 가정·사무실 내부 인터페이스 |
| `100.64.0.0/10` | RFC 6598 Shared Address Space | ISP CGNAT 내부 구간, 일부 VPN 오버레이 |
| `169.254.0.0/16` | IPv4 link-local(APIPA) | DHCP 실패 등으로 자동 설정된 주소 |
| `224.0.0.0/4` | IPv4 멀티캐스트 | IPTV·라우팅·검색 프로토콜 등 |

`100.64.0.0/10`의 범위는 `100.64.0.0`부터 `100.127.255.255`까지다. 이는 RFC 1918 사설 주소가 아니라 통신사업자용 공유 주소 공간이다. ISP가 이 대역을 WAN 인터페이스에 할당하면 CGNAT일 가능성이 높지만, Tailscale 같은 VPN 오버레이도 같은 대역을 사용할 수 있으므로 인터페이스와 경로를 함께 확인해야 한다.

공인 IP 조회 사이트가 보여 주는 주소는 대개 ISP의 NAT 게이트웨이 주소다. 여러 가입자가 이를 공유하므로 해당 주소만으로 특정 가정, 장치 또는 사용자를 식별할 수 없다. IP 지리정보 데이터베이스 역시 사업자 등록 지역이나 대표 좌표를 제공할 뿐, 실제 사용자의 물리적 위치를 확정하지 않는다.

## 2. CGNAT의 동작과 제약

일반적인 가정용 NAT에서는 공유기가 내부 사설 주소와 하나의 공인 주소 사이를 변환한다. CGNAT 환경에서는 다음과 같은 이중 변환이 추가된다.

```text
클라이언트(192.168.x.x)
  → 가정용 공유기 NAT
  → ISP CGNAT(100.64.0.0/10)
  → 공인 IPv4
  → 인터넷 서버
```

CGNAT 장비는 연결별로 변환 테이블을 유지한다. 내부 클라이언트가 먼저 외부로 연결을 만들면 응답 패킷은 해당 테이블을 통해 돌아오지만, 외부 서버가 사전 연결 없이 내부 클라이언트로 접속할 포트 매핑은 존재하지 않는다. 따라서 다음 기능은 제한되거나 별도 중계가 필요하다.

- 외부에서 내부 서버로 직접 접속하는 포트 포워딩
- P2P·일부 온라인 게임의 직접 연결
- 외부에서 시작하는 원격 관리 접속

지연시간 증가는 NAT 단계 자체보다 회선 품질·경로·릴레이 사용 여부의 영향을 크게 받는다. CGNAT이라고 해서 항상 게임 지연이나 패킷 손실이 발생하는 것은 아니다.

## 3. 외부에서 보이는 이상 IP의 진단 절차

1. 장치의 WAN 주소와 인터넷에서 확인한 공인 주소를 비교한다.
2. WAN 주소가 `100.64.0.0/10`인지 확인하고, VPN 클라이언트가 해당 주소를 만든 것은 아닌지 점검한다.
3. ISP에 CGNAT 사용 여부와 공인 IPv4 또는 브리지 모드 제공 여부를 문의한다.
4. 공유기 관리자 계정, 펌웨어, 원격 관리 설정, 포트 포워딩, DNS 설정을 확인한다.
5. 의심스러운 프로세스·로그인·아웃바운드 연결은 증적을 보존한 뒤 보안 도구로 검사한다.

알 수 없는 IP가 로그에 보인다는 사실만으로 침해를 결론 내릴 수 없다. 실제 침해 판단에는 인증 로그, 프로세스 실행 기록, 방화벽 흐름 로그, 파일 변경 이력 등 여러 증거가 필요하다. 감염이 의심될 때는 무조건 포맷하기보다 네트워크 격리와 증적 보전을 먼저 고려하고, 계정·공유기 자격 증명은 신뢰할 수 있는 장치에서 교체한다.

## 4. SSE의 개념과 CGNAT 클라이언트로의 송신

SSE는 브라우저가 HTTP 연결을 열어 둔 상태에서 서버가 `text/event-stream` 형식의 이벤트를 지속적으로 전송하는 표준 API다. 연결 방향은 서버에서 클라이언트로의 단방향이며, 클라이언트의 요청 자체는 일반적인 아웃바운드 HTTPS 연결이므로 CGNAT 뒤에서도 동작한다.

### 4.1 권장 토폴로지

```text
CGNAT 클라이언트 --(HTTPS 요청 및 장기 연결)--> 공인 SSE 서버
CGNAT 클라이언트 <--(동일 TCP 연결의 응답 스트림)-- 공인 SSE 서버
```

서버는 클라이언트의 `192.168.x.x` 또는 `100.64.x.x` 주소로 접속하지 않는다. 클라이언트가 `/events`에 연결한 뒤 서버가 해당 연결 객체를 보관하고, 이벤트가 발생하면 그 스트림에 기록한다. 연결이 끊기면 클라이언트가 재연결한다.

### 4.2 최소 구현 예시

서버:

```js
const clients = new Set();

Bun.serve({
  port: 3000,
  fetch(req) {
    const url = new URL(req.url);
    if (url.pathname !== "/events") return new Response("Not Found", { status: 404 });

    const stream = new ReadableStream({
      start(controller) {
        const encoder = new TextEncoder();
        const send = (event, data, id) => {
          controller.enqueue(encoder.encode(
            `${id ? `id: ${id}\\n` : ""}event: ${event}\\ndata: ${JSON.stringify(data)}\\n\\n`
          ));
        };
        const client = { controller, send };
        clients.add(client);
        send("ready", { connected: true });
        const heartbeat = setInterval(() => send("ping", {}), 25_000);
        req.signal.addEventListener("abort", () => {
          clearInterval(heartbeat);
          clients.delete(client);
          try { controller.close(); } catch {}
        });
      },
    });

    return new Response(stream, {
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache, no-transform",
        "Connection": "keep-alive",
      },
    });
  },
});

function broadcast(data, id) {
  for (const client of clients) client.send("update", data, id);
}
```

클라이언트:

```js
const events = new EventSource("https://example.com/events");
events.addEventListener("update", event => {
  const payload = JSON.parse(event.data);
  // 서버가 보낸 데이터 처리
});
events.onerror = () => {
  // EventSource가 기본 재연결을 수행한다.
};
```

실제 서비스에서는 인증 토큰 또는 쿠키, 테넌트별 연결 권한, 연결 수 제한을 적용한다. 이벤트에 `id:`를 부여하면 브라우저가 재연결 시 `Last-Event-ID`를 보내므로, 서버는 최근 이벤트를 재전송하거나 현재 상태를 다시 내려 누락을 보완할 수 있다.

### 4.3 프록시와 운영 고려사항

- 리버스 프록시의 버퍼링을 끄고, 유휴 타임아웃을 SSE보다 길게 설정한다.
- 주기적인 heartbeat를 보내 NAT 매핑과 중간 프록시 연결이 유지되도록 한다.
- 이벤트 스트림은 단방향이므로 클라이언트의 명령은 일반 `fetch` POST 또는 WebSocket으로 별도 처리한다.
- 서버 재시작·멀티 인스턴스 환경에서는 Redis 등 pub/sub 또는 메시지 브로커로 이벤트를 fan-out한다.
- CGNAT 포트 고갈과 모바일 네트워크 변경에 대비해 재연결 지수 백오프와 중복 이벤트 처리를 구현한다.

## 5. SSE로 해결되지 않는 경우

파일 공유, 양방향 저지연 통신, 직접 P2P 연결이 필요하면 SSE만으로 충분하지 않다. 이때는 WebSocket(역시 클라이언트가 먼저 연결), 관리형 릴레이, VPN 오버레이, IPv6, TURN 서버 또는 역방향 터널을 검토한다. CGNAT을 우회한다고 해서 외부에 장치를 무방비로 노출해서는 안 되며, 인증·암호화·접근 제어를 반드시 유지해야 한다.

## 결론

CGNAT에서 관찰되는 `100.64.0.0/10` 주소와 여러 사용자가 공인 IP를 공유하는 현상은 정상적인 네트워크 구성일 수 있다. 주소만으로 해킹을 판단하지 말고, WAN 주소·VPN·공유기 설정·로그를 함께 확인해야 한다. CGNAT 클라이언트로 서버 데이터를 전달해야 한다면 클라이언트가 공인 서버에 먼저 연결하는 역방향 연결 모델을 사용하고, 웹 브라우저 기반 단방향 알림에는 SSE와 heartbeat·재연결·`Last-Event-ID`를 조합하는 것이 실용적인 선택이다.
