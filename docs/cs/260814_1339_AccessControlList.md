# ACL (Access Control List) — 네트워크·방화벽 상세 정리

| 항목 | 내용 |
|------|------|
| 주제 | 방화벽·네트워크 관점의 **ACL(Access Control List)** |
| 목적 | ACL 정의, 동작 원리, 종류, 규칙 구조, 관련 용어, 실무 패턴을 한곳에 정리 |
| 범위 | 라우터/스위치 ACL, 방화벽 정책, 클라우드 NACL·Security Group, Linux 패킷 필터와의 관계 |
| 작성일 | 2026-08-13 |

---

## 1. ACL이란

**ACL(Access Control List, 접근 제어 목록)** 은 “누가 / 무엇을 / 어디로 / 어떤 프로토콜로 / 허용할지·거부할지”를 **규칙(rule)의 목록**으로 정의한 것이다.

네트워크·방화벽 맥락에서 ACL은 보통 **패킷(또는 세션)을 조건에 맞춰 검사한 뒤 permit(허용) 또는 deny(거부)** 하는 정책이다.

핵심 아이디어는 단순하다.

```text
패킷 도착
  → ACL 규칙을 위에서부터 순서대로 비교
    → 첫 번째 일치(match) 규칙의 동작(permit/deny)을 적용
      → 이후 규칙은 보지 않음 (first-match)
```

즉 ACL은 “허용/거부 규칙들의 **순서가 있는 리스트**”이다.

---

## 2. 왜 방화벽·네트워크에서 ACL이 중요한가

네트워크 경계와 내부 구간에서는 다음을 동시에 달성해야 한다.

| 목표 | ACL이 하는 일 |
|------|----------------|
| 경계 방어 | 외부→내부, 내부→외부 트래픽 중 허용할 것만 통과 |
| 최소 권한 | 필요한 통신만 열고 나머지는 차단 |
| 세그먼트 분리 | VLAN/서브넷 간 불필요한 횡이동(lateral movement) 차단 |
| 감사·추적 | “어떤 규칙으로 막혔는지/통과했는지” 근거 제공 |
| 장애 대응 | 공격·오동작 트래픽을 빠르게 차단 |

방화벽 제품·클라우드 보안 그룹·라우터·스위치·리눅스 패킷 필터는 UI·용어가 달라도, 내부적으로는 **ACL형 규칙 엔진**을 쓰는 경우가 많다.

---

## 3. 관련 개념과의 관계

### 3.1 ACL vs 방화벽(Firewall)

| 구분 | ACL | 방화벽 |
|------|-----|--------|
| 본질 | 허용/거부 **규칙 목록** | 규칙을 적용하는 **보안 시스템(제품/기능)** |
| 상태 추적 | 전통적 라우터 ACL은 **stateless**인 경우가 많음 | 현대 방화벽은 대개 **stateful** |
| 부가 기능 | 보통 L3/L4 필터 중심 | NAT, IPS/IDS, SSL inspection, 애플리케이션 제어 등 확장 |
| 표현 | “ACL을 건다” | “방화벽 정책을 건다” |

정리하면:

- ACL은 **정책 표현 방식**에 가깝고  
- 방화벽은 그 정책을 **집행하는 장치/서비스**에 가깝다.

실무에서는 방화벽 정책 한 줄이 ACL 한 엔트리와 거의 같은 의미로 쓰이기도 한다.

### 3.2 ACL vs Security Group / NACL (클라우드)

클라우드에서는 이름이 갈라진다.

| 용어 | 대표 환경 | 성격 |
|------|-----------|------|
| **Security Group** | AWS EC2 등 | 인스턴스(ENI)에 붙는 **stateful** 가상 방화벽 |
| **Network ACL (NACL)** | AWS VPC | 서브넷에 붙는 **stateless** 네트워크 ACL |
| **NSG** | Azure | Network Security Group (대체로 stateful에 가깝게 운용) |
| **방화벽 규칙 / 정책** | GCP, 온프레 방화벽 | 제품별로 ACL과 동일한 역할 |

“클라우드의 ACL”이라고 하면 보통 **NACL**을 가리키고,  
“인스턴스 방화벽”이라고 하면 **Security Group**을 가리키는 경우가 많다.

### 3.3 ACL vs iptables / nftables / pf

Linux·BSD의 패킷 필터도 ACL이다.

| 구현 | 비고 |
|------|------|
| iptables / ip6tables | Netfilter 기반 레거시 인터페이스 |
| nftables | 현대 Linux의 통합 패킷 필터 |
| pf (Packet Filter) | OpenBSD/FreeBSD 계열 |
| Windows Filtering Platform / 고급 방화벽 | Windows 호스트 방화벽 |

명령어와 체인이 달라도 “조건 매칭 → accept/drop” 구조는 ACL과 동일하다.

### 3.4 ACL vs RBAC / 파일 ACL

이름이 같아서 혼동하기 쉽다.

| 종류 | 대상 | 예 |
|------|------|----|
| **네트워크 ACL** | 패킷·플로우 | 소스 IP, 목적지 포트 허용/거부 |
| **파일시스템 ACL** | 파일·디렉터리 권한 | POSIX ACL, NTFS ACL |
| **RBAC** | 사용자·역할의 API/리소스 권한 | Kubernetes Role, IAM Role |

이 문서는 **네트워크·방화벽 ACL**만 다룬다.

---

## 4. ACL이 보는 OSI / 프로토콜 계층

실무 ACL은 주로 **L3·L4**를 본다. 차세대 방화벽은 L7까지 확장한다.

| 계층 | ACL에서 자주 쓰는 매칭 키 | 예시 |
|------|---------------------------|------|
| L2 | MAC, EtherType, VLAN ID | 스위치 포트 ACL, MAC ACL |
| L3 | 소스/목적지 IP, ICMP type/code | `10.0.0.0/8 → 192.168.1.10` |
| L4 | TCP/UDP 포트, 플래그 | `tcp/443`, `udp/53` |
| L7 | URL, Host, SNI, Application ID | `https://api.example.com`, `SSH` |

전통 라우터 ACL:

- Standard ACL: 주로 **소스 IP**  
- Extended ACL: **소스·목적지 IP + 프로토콜 + 포트**

현대 방화벽:

- 5-tuple + zone + user + app + time 등까지 확장

### 4.1 5-tuple (파이브 튜플)

네트워크 ACL·방화벽·플로우 분석에서 가장 기본이 되는 식별자이다.

```text
(프로토콜, 소스 IP, 소스 포트, 목적지 IP, 목적지 포트)
```

예:

```text
(tcp, 203.0.113.10, 53122, 10.0.1.5, 443)
```

동일 5-tuple이 일정 시간 유지되면 “한 세션/플로우”로 취급하는 경우가 많다.

---

## 5. ACL 규칙 구조

### 5.1 한 줄(엔트리)의 일반 형태

```text
[우선순위/번호] [동작] [프로토콜] [소스] [목적지] [포트/부가조건] [로깅 등]
```

예시(의사코드):

```text
10 permit tcp 10.0.1.0/24 any eq 443
20 deny  ip  10.0.1.0/24 10.0.2.0/24
30 permit ip any any
```

### 5.2 규칙 구성 요소

| 요소 | 의미 | 예시 |
|------|------|------|
| Sequence / Priority | 평가 순서 | 10, 20, 100 |
| Action | 허용/거부 | permit, deny, accept, drop, reject |
| Protocol | IP 상위 프로토콜 | ip, tcp, udp, icmp |
| Source | 출발지 | IP, CIDR, any, geo, 사용자 그룹 |
| Destination | 목적지 | IP, CIDR, any, FQDN(차세대) |
| Port / Service | L4 서비스 | 22, 80, 443, 1024-65535 |
| Direction | 방향 | inbound / outbound |
| State / Session | 상태 기반 여부 | new, established, related |
| Log | 기록 여부 | log, log-start |
| Time | 시간 조건 | business-hours only |
| Interface / Zone | 적용 위치 | eth0 in, trust→untrust |

### 5.3 Action의 차이

| Action | 의미 | 실무 메모 |
|--------|------|-----------|
| **permit / accept / allow** | 통과 허용 | 통과 후 다음 장비/프로세스 처리 |
| **deny / drop** | 조용히 폐기 | 상대방은 타임아웃을 경험하는 경우 많음 |
| **reject** | 거절을 알림 | TCP RST 또는 ICMP unreachable 등 |
| ** ent** | 로그 후 허용 | 감사·과도기 마이그레이션에 사용 |
| **drop + log** | 차단 + 기록 | 보안 모니터링에 중요 |

`drop`과 `reject`는 둘 다 “막는다”이지만, 상대방에게 피드백을 주는지가 다르다.

---

## 6. First-match와 Implicit Deny

### 6.1 First-match (최초 일치)

대부분의 네트워크 ACL은 **위에서 아래로** 검사하고, **처음 일치한 규칙만** 적용한다.

```text
1. permit tcp any host 10.0.0.5 eq 22
2. deny   tcp any host 10.0.0.5 eq 22
3. permit ip any any
```

위 예에서 SSH 트래픽은 1번에서 이미 허용되므로 2번은 절대 실행되지 않는다.  
→ **순서(order)가 정책의 일부**이다.

### 6.2 Implicit Deny (암묵적 거부)

많은 ACL 구현은 목록 끝에 **명시하지 않아도 “나머지 전부 deny”** 를 붙인다.

```text
명시 규칙들...
(암묵) deny ip any any
```

의미:

- “허용 목록에 없으면 차단” (allowlist / default-deny)
- 보안상 기본 자세로 자주 권장됨

반대로 일부 시스템(예: 특정 클라우드 SG의 기본값 차이, 스위치 포트 기본 동작 등)은 default-allow일 수 있으므로 **제품별 기본값을 반드시 확인**해야 한다.

### 6.3 규칙 배치 원칙

| 원칙 | 이유 |
|------|------|
| **구체적 규칙 → 일반 규칙** | `host` 규칙을 `any`보다 위에 |
| **deny 예외를 먼저** | 특정 차단이 광역 permit에 먹히지 않게 |
| **고빈도 규칙을 위쪽** | 성능(조기 매칭) |
| **shadowed rule 제거** | 아래 규칙이 위 규칙에 가려져 무의미해지는 것 방지 |

Shadowed rule 예:

```text
10 permit ip 10.0.0.0/8 any
20 deny  ip  10.1.1.5 any     ← 절대 도달 불가 (이미 10에서 허용)
```

---

## 7. Inbound / Outbound, Ingress / Egress

### 7.1 방향 용어

| 용어 | 의미 |
|------|------|
| **Inbound / Ingress** | 인터페이스·존·서브넷·호스트 **쪽으로 들어오는** 트래픽 |
| **Outbound / Egress** | 인터페이스·존·서브넷·호스트 **밖으로 나가는** 트래픽 |

중요: “들어온다/나간다”의 기준점은 **ACL이 붙은 객체**이다.

예:

- 라우터 `Gi0/0` inbound ACL → 그 인터페이스로 **들어오는** 패킷  
- 서브넷 NACL outbound → 서브넷에서 **떠나는** 패킷  
- 호스트 방화벽 outbound → 호스트에서 **외부로 나가는** 패킷  

### 7.2 Stateless에서의 왕복 문제

Stateless ACL은 **요청과 응답을 각각 규칙으로 열어줘야** 한다.

예: 내부 `10.0.1.10`이 외부 웹 `203.0.113.5:443`에 접속

1. Outbound 허용: `10.0.1.10 → 203.0.113.5 tcp/443`  
2. Inbound 허용(응답): `203.0.113.5 → 10.0.1.10` 그리고 **고포트(ephemeral port)**  

응답 포트는 보통 클라이언트가 쓰는 **임시 포트(1024~65535 등)** 이므로,  
stateless NACL에서는 ephemeral port 범위를 열어줘야 통신이 된다.

Stateful 방화벽/SG는 “나간 연결의 응답”을 자동으로 관련 세션으로 허용하는 경우가 많다.

---

## 8. Stateless vs Stateful

| 항목 | Stateless ACL | Stateful Firewall / SG |
|------|---------------|-------------------------|
| 세션 기억 | 없음 | 연결 상태 테이블 유지 |
| 규칙 | 왕방향 각각 필요 | 보통 한 방향만 열어도 응답 통과 |
| 성능/단순성 | 단순, 예측 가능 | 상태 테이블 관리 필요 |
| 대표 | 전통 라우터 ACL, AWS NACL | NGFW, AWS Security Group |
| 위장/스푸핑 대응 | 규칙 설계에 크게 의존 | 상태·시퀀스 검증에 유리한 편 |

### 8.1 연결 상태 용어

| 상태 | 의미 |
|------|------|
| **new** | 새 연결 시도 (예: TCP SYN) |
| **established** | 이미 맺어진 연결의 패킷 |
| **related** | 기존 연결과 연관 (FTP data 등) |
| **invalid** | 상태에 맞지 않는 패킷 |

iptables 예 (개념):

```text
allow established,related
allow new tcp dport 443
drop all
```

---

## 9. 전통 네트워크 장비에서의 ACL 분류

Cisco 계열 교육/실무에서 자주 쓰는 분류이다. 벤더마다 문법은 다르지만 개념은 통용된다.

### 9.1 Standard ACL

- 주로 **소스 IP**만 검사  
- 번호 예: 1–99, 1300–1999 (벤더/세대에 따라 다름)  
- 단순 필터, “이 출발지는 막는다/보낸다”에 적합  
- 보통 **목적지에 가까운 곳**보다, 정책 의도에 맞게 배치

### 9.2 Extended ACL

- **소스 + 목적지 + 프로토콜 + 포트** 검사  
- 번호 예: 100–199, 2000–2699  
- 실무의 대부분 세밀 제어는 Extended에 해당

### 9.3 Named ACL

- 숫자 대신 이름 사용 (`ACL-WEB-IN`)  
- 가독성·운영성 ↑  
- 엔트리 삽입/삭제가 번호 ACL보다 편한 경우가 많음

### 9.4 Numbered vs Named

| 구분 | 장점 | 단점 |
|------|------|------|
| Numbered | 짧고 전통적 | 의미 파악 어려움 |
| Named | 의도 표현 명확 | 이름 규칙 필요 |

### 9.5 적용 위치: 라우터 / 스위치 / 방화벽

| 위치 | 역할 |
|------|------|
| **라우터 ACL** | 서브넷 간 라우팅 경로에서 L3/L4 필터 |
| **스위치 VLAN ACL / Port ACL** | L2/L3 경계, 동일 VLAN 내 제한도 가능(구현 의존) |
| **방화벽 정책** | 존(zone) 기반, NAT·로깅·앱 제어와 결합 |
| **호스트 방화벽** | 최종 서버 OS에서 방어 (defense in depth) |

---

## 10. 매칭에 쓰는 네트워크 주소 표현

### 10.1 IP / CIDR

| 표현 | 의미 |
|------|------|
| `10.0.1.5` | 단일 호스트 |
| `10.0.1.0/24` | 10.0.1.0 ~ 10.0.1.255 |
| `0.0.0.0/0` 또는 `any` | 모든 IPv4 |
| `2001:db8::/32` | IPv6 프리픽스 |

### 10.2 Wildcard mask (시스코 ACL에서 자주 등장)

서브넷 마스크와 **비트가 반대**인 표현.

| 서브넷 마스크 | Wildcard | 의미 |
|---------------|----------|------|
| `255.255.255.0` | `0.0.0.255` | /24 |
| `255.255.255.255` | `0.0.0.0` | 호스트 하나 |
| `0.0.0.0` | `255.255.255.255` | any |

규칙: wildcard에서 `0` 비트는 “일치해야 함”, `1` 비트는 “아무거나”.

### 10.3 포트 표현

| 표현 | 의미 |
|------|------|
| `eq 443` | equal, 443만 |
| `neq 22` | not equal |
| `gt 1023` | greater than |
| `lt 1024` | less than |
| `range 1000 2000` |  inclusive 범위 |
| `any` | 모든 포트 |

잘 쓰는 서비스 포트 예:

| 포트 | 서비스 |
|------|--------|
| 22 | SSH |
| 53 | DNS |
| 80 | HTTP |
| 443 | HTTPS |
| 3389 | RDP |
| 3306 | MySQL |
| 5432 | PostgreSQL |
| 6379 | Redis |

---

## 11. Zone 기반 방화벽과 ACL

현대 방화벽은 인터페이스를 **보안 존(zone)** 으로 묶고, 존 사이 정책을 건다.

대표 존 예:

| Zone | 의미 |
|------|------|
| trust / internal | 내부망 |
| untrust / external | 인터넷 |
| dmz | 외부 공개 서버 구간 |
| management | 관리망 |
| guest | 게스트 Wi-Fi |

정책 예:

```text
trust → untrust : allow web (80/443)
untrust → dmz   : allow https to web-vip
untrust → trust : deny all
dmz → trust     : deny all (또는 최소 DB 포트만)
```

이 정책들도 결국 ACL 엔트리 집합이다.  
차이는 “IP 인터페이스 단위”가 아니라 **존 단위로 추상화**했다는 점이다.

---

## 12. 클라우드 ACL 심화 (AWS 기준으로 개념 정리)

벤더는 달라도 개념 대응이 가능하다.

### 12.1 Security Group (SG)

| 특성 | 내용 |
|------|------|
| 적용 대상 | ENI / 인스턴스 |
| 상태 | **Stateful** |
| 기본 자세 | 보통 inbound deny, outbound allow (기본값 확인 필요) |
| 규칙 의미 | “허용 목록” 성격이 강함 (명시 deny가 없거나 제한적인 제품도 있음) |
| 응답 트래픽 | 허용된 연결의 응답은 자동 허용되는 경우가 많음 |

### 12.2 Network ACL (NACL)

| 특성 | 내용 |
|------|------|
| 적용 대상 | **서브넷** |
| 상태 | **Stateless** |
| 규칙 | allow/deny 모두 존재, 번호 순 평가 |
| 왕복 | inbound/outbound를 각각 설계해야 함 |
| 용도 | 서브넷 수준 가드레일, 긴급 차단, 광역 제한 |

### 12.3 SG + NACL 함께 쓰는 이유

```text
Internet
   ↓
NACL (서브넷 가드레일, stateless)
   ↓
Security Group (인스턴스 방화벽, stateful)
   ↓
Host firewall / App auth
```

계층 방어(defense in depth):

1. 경계 방화벽/WAF  
2. NACL  
3. Security Group  
4. 호스트 방화벽  
5. 애플리케이션 인증·인가  

한 계층이 실수해도 다음 계층이 막는다.

### 12.4 실무에서 자주 하는 NACL 실수

1. outbound 응답 포트를 안 열어 **접속이 안 됨**  
2. ephemeral port 범위를 너무 좁게 열어 **간헐 실패**  
3. 규칙 번호 간격 없이 작성해 **삽입이 어려움** (10, 20, 30처럼 간격 권장)  
4. SG만 보고 NACL deny를 깜빡함  
5. “deny all”을 위에 넣어 아래 allow가 전부 무력화

---

## 13. 방화벽 ACL 설계 패턴

### 13.1 Default Deny + Allowlist

가장 기본적이고 권장되는 자세.

```text
필요한 것만 permit
마지막에 deny any any (명시 또는 암묵)
```

### 13.2 관리 접근 최소화

```text
permit tcp 관리자IP-셋 host 서버 eq 22
deny  tcp any host 서버 eq 22
```

SSH/RDP/DB 포트를 인터넷 `any`에 여는 것은 전형적인 사고 원인이다.

### 13.3 DMZ 패턴

```text
Internet → DMZ web : 443 only
DMZ web → Internal DB : 5432 only (web SG/IP만)
Internet → Internal : deny
DMZ → Management : deny
```

### 13.4 Anti-spoofing / Bogon 필터

경계에서 거짓 출발지 주소를 차단한다.

예:

- 인터넷 구간 inbound에서 소스 `10.0.0.0/8`, `192.168.0.0/16`, `172.16.0.0/12` 등 **사설 IP 유입 차단**  
- 내부에서 인터넷으로 나갈 때 소스가 회사 공인/할당 대역이 아니면 차단  

### 13.5 긴급 차단(Blackhole) 규칙

공격 IP·악성 C2 대역을 ACL 상단에 넣어 즉시 차단.

```text
5 deny ip host 198.51.100.66 any log
```

### 13.6 로깅 전략

| 대상 | 권장 |
|------|------|
| deny 규칙 | 가능하면 log (단, flood 주의) |
| 핵심 allow | 감사 필요 시 log |
| any any allow | 가급적 제거, 있으면 반드시 검토 |

고트래픽 deny에 log를 걸면 로그 폭발·성능 저하가 날 수 있으므로 샘플링·임계 기반 로깅을 검토한다.

---

## 14. TCP/UDP/ICMP와 ACL

### 14.1 TCP

- 연결형. SYN/SYN-ACK/ACK 핸드셰이크.  
- Stateful 장비는 SYN(new)만 정책으로 검사하고 established는 상태 테이블로 통과시키는 경우가 많음.  
- Stateless는 양쪽 포트를 모두 의식해야 함.

### 14.2 UDP

- 비연결형. “세션”이 명확하지 않음.  
- Stateful 방화벽도 UDP는 **타임아웃 기반 pseudo-session**으로 추적.  
- DNS, VPN(WireGuard 등), QUIC(UDP/443)에서 중요.

### 14.3 ICMP

- ping(echo request/reply), fragmentation needed, traceroute 관련 메시지 등.  
- 무분별 허용은 정보 노출·악용 여지, 무분별 차단은 Path MTU Discovery 실패 등 장애 가능.  
- “전부 차단”보다 **타입/코드 단위로 최소 허용**이 안전한 경우가 많다.

---

## 15. NAT와 ACL의 순서

실무 장애의 상당수는 “ACL이 잘못된 게 아니라 **NAT 전/후 주소로 규칙을 씀**”에서 발생한다.

일반적 질문:

- ACL은 **NAT 변환 전** 주소로 매칭하는가, **변환 후**인가?

답은 **장비·방향·벤더 구현에 따라 다르다.**

설계 시 체크리스트:

1. 해당 인터페이스/정책이 pre-NAT인지 post-NAT인지  
2. 로그에 찍히는 주소가 변환 전인지 후인지  
3. 포트 포워딩(DNAT) 후 destination이 VIP인지 real IP인지  

예:

```text
외부 사용자 → 공인 1.2.3.4:443 → (DNAT) → 내부 10.0.1.10:443
```

방화벽 규칙의 destination을 `1.2.3.4`로 써야 하는지 `10.0.1.10`으로 써야 하는지 제품 문서 확인이 필수이다.

---

## 16. 성능·운영 관점

### 16.1 규칙 수와 성능

- 규칙이 매우 많고 first-match가 늦게 일어나면 조회 비용 증가  
- 해시/트리 최적화, ASIC/TCAM을 쓰는 장비도 있음  
- TCAM 용량 초과 시 규칙 추가 실패·우회 성능 저하 가능

### 16.2 Object Group / Address Group

개별 IP를 반복하지 말고 그룹화한다.

```text
OBJ_ADMIN_IPS = { 203.0.113.10, 203.0.113.11 }
permit tcp OBJ_ADMIN_IPS host 10.0.0.5 eq 22
```

변경 시 그룹만 수정하면 정책 일관성이 유지된다.

### 16.3 변경 관리

| 항목 | 권장 |
|------|------|
| 변경 전 | 현재 세션/트래픽 hit count 확인 |
| 변경 | 유지보수 창, 롤백 계획 |
| 변경 후 | 실제 통신 테스트 + 로그 확인 |
| 문서화 | 규칙 번호, 목적, 티켓/담당자, 만료일 |

임시 개방(`any any`, 광역 포트)은 **만료일**을 반드시 넣는다.

---

## 17. 트러블슈팅 체크리스트

통신이 안 될 때 ACL 관점으로 보는 순서:

1. **경로 확인**: 라우팅·DNS·ARP/NDP 문제인가?  
2. **어느 장비에서 떨어지는가**: 클라이언트 → 중간 FW → 서버 호스트 FW  
3. **방향**: inbound 문제인가 outbound 문제인가  
4. **상태**: stateful이면 세션 테이블에 남는가  
5. **포트/프로토콜**: TCP인지 UDP인지, 실제 목적지 포트가 맞는지  
6. **주소**: NAT 전후 주소로 규칙을 잘못 쓰지 않았는지  
7. **순서**: 위쪽 deny/permit에 섀도잉되지 않았는지  
8. **hit count / log**: 해당 규칙이 실제로 매칭되는가  
9. **리턴 트래픽**: stateless면 응답 규칙이 있는가  
10. **시간/사용자/앱 조건**: 차세대 방화벽의 부가 매칭에 걸렸는가  

패킷 캡처(tcpdump/Wireshark)와 방화벽 세션 로그를 같이 보면 원인 분리가 빠르다.

---

## 18. 보안 모범 사례 요약

1. **Default deny**를 기본으로 한다.  
2. **최소 권한**: 호스트·포트·프로토콜을 필요한 만큼만.  
3. `0.0.0.0/0` 에 관리 포트(22/3389/DB)를 열지 않는다.  
4. 규칙 **순서를 설계**하고 shadowed rule을 제거한다.  
5. **계층 방어**: 경계 FW + NACL/SG + host FW.  
6. **변경은 티켓·문서·만료일**과 함께.  
7. deny와 중요 allow는 **로그**를 남긴다(폭주 주의).  
8. IPv6를 쓰면 **IPv6 ACL도 함께** 설계한다. (IPv4만 막고 IPv6 개방은 흔한 구멍)  
9. “잠시 any 오픈”은 사고의 시작점으로 취급한다.  
10. 주기적으로 **미사용 규칙 정리**(hit count 0 장기 규칙 검토).

---

## 19. 용어 Glossary (방화벽·네트워크 ACL)

| 용어 | 설명 |
|------|------|
| **ACL** | Access Control List. 허용/거부 규칙 목록 |
| **ACE** | Access Control Entry. ACL의 개별 한 줄 |
| **Packet Filtering** | 패킷 헤더 조건으로 통과/차단 |
| **Stateful Inspection** | 연결 상태를 추적하며 검사 |
| **Stateless** | 상태 없이 패킷 단위로만 검사 |
| **Implicit Deny** | 목록 끝의 암묵적 전부 |
| **First-match** | 최초 일치 규칙만 적용 |
| **5-tuple** | proto + srcIP + srcPort + dstIP + dstPort |
| **Well-known port** | 0–1023 등 잘 알려진 서비스 포트 |
| **Ephemeral port** | 클라이언트가 임시로 쓰는 고번호 포트 |
| **Ingress / Egress** | 유입 / 유출 |
| **East-West traffic** | 데이터센터·클라우드 내부 횡단 트래픽 |
| **North-South traffic** | 외부↔내부 경계 트래픽 |
| **Microsegmentation** | 워크로드 단위의 세밀한 동서 트래픽 통제 |
| **Zone** | 동일 신뢰 수준의 인터페이스 묶음 |
| **DMZ** | 외부 공개용 완충 구간 |
| **WAF** | Web Application Firewall (L7 HTTP 특화) |
| **NGFW** | Next-Generation Firewall (앱/사용자/위협 통합) |
| **Allowlist / Denylist** | 허용 목록 / 차단 목록 (구 whitelist/blacklist) |
| **Shadowed rule** | 상위 규칙에 가려 도달 불가능한 규칙 |
| **Hit count** | 규칙 매칭 횟수 |
| **TCAM** | 하드웨어 병렬 매칭 메모리 (장비 ACL 성능 핵심) |
| **Bogon** | 아직 할당되지 않거나 라우팅되면 안 되는 주소 대역 |
| **Spoofing** | 출발지 주소를 위조하는 행위 |
| **Connection tracking (conntrack)** | 리눅스 등에서 세션 상태 추적 |
| **Policy-based routing + ACL** | ACL로 관심 트래픽을 골라 별도 경로로 보내는 패턴 |
| **Control plane ACL / CoPP** | 장비 CPU로 가는 관리·프로토콜 패킷 보호 |
| **PACL / VACL / RACL** | 포트/VLAN/라우터 ACL (스위치 벤더 용어) |

---

## 20. 의사코드 예시 모음

### 20.1 웹 서버 서브넷 (개념)

```text
# inbound to web subnet
10 permit tcp any host 10.0.10.10 eq 443
20 permit tcp any host 10.0.10.10 eq 80
30 deny  ip  any 10.0.10.0/24 log

# outbound from web subnet (updates, DNS 등)
10 permit udp 10.0.10.0/24 host 10.0.0.53 eq 53
20 permit tcp 10.0.10.0/24 any eq 443
30 deny  ip  10.0.10.0/24 10.0.0.0/8 log   # 내부 횡이동 제한 예시
```

### 20.2 Stateful 호스트 방화벽 Minimal

```text
allow established,related
allow inbound tcp 443
allow outbound any          # 또는 더 축소
deny  all
```

### 20.3 관리 SSH 제한

```text
permit tcp 203.0.113.0/28 host 10.0.0.5 eq 22 log
deny  tcp any host 10.0.0.5 eq 22 log
```

---

## 21. ACL을 읽는 연습 (해석 방법)

규칙:

```text
100 deny  tcp 10.1.1.0/24 10.2.2.0/24 eq 3389
110 permit ip 10.1.1.0/24 10.2.2.0/24
120 deny  ip any any
```

해석:

1. `10.1.1.0/24` → `10.2.2.0/24` 의 **RDP(3389)만 차단**  
2. 그 외 동일 구간 IP 통신은 허용  
3. 나머지 모든 트래픽 차단  

이처럼 ACL은 “한 줄의 사전적 의미”와 “위→아래 순서의 종합 효과”를 함께 읽어야 한다.

---

## 22. 한 장 요약

```text
ACL = 순서가 있는 허용/거부 규칙 목록

평가: top-to-bottom, first-match
기본: 많은 시스템이 마지막에 implicit deny

전통 ACL: L3/L4, 종종 stateless
현대 방화벽: stateful + zone + app/user

클라우드:
  - Security Group ≈ stateful 인스턴스 방화벽
  - NACL ≈ stateless 서브넷 ACL

설계 핵심:
  1) default deny
  2) least privilege
  3) 올바른 방향(in/out)과 NAT 전후 주소
  4) 규칙 순서와 로깅
  5) 계층 방어
```

---

## 23. 참고로 같이 보면 좋은 주제

- TCP 3-way handshake와 stateful inspection  
- CIDR·서브넷팅·와일드카드 마스크  
- NAT (SNAT/DNAT)와 방화벽 정책 순서  
- AWS VPC 보안 (SG, NACL, Route, IGW/NAT GW)  
- iptables/nftables chain (INPUT/FORWARD/OUTPUT)  
- Zero Trust / Microsegmentation  
- WAF와 NGFW의 차이  

---

*이 문서는 방화벽·네트워크 운영에서 쓰는 ACL 개념을 벤더 비의존적으로 정리한 학습용 노트이다. 실제 장비를 다룰 때는 해당 벤더의 규칙 평가 순서, 기본값(implicit deny 여부), NAT와의 상관관계를 문서에서 재확인해야 한다.*
