# Deno 팀의 Cloudflare 합류로 본 JS 런타임의 사업성과 프로그래밍 모델 경쟁

## 요약

2026년 10월 9일 Node.js와 Deno를 만든 Ryan Dahl은 Deno 팀 전체가 Cloudflare에 합류한다고 발표했다. Deno 런타임은 1년간 월별 버그·보안 패치만 낸 뒤 Deno 팀의 개발이 종료되고, 호스팅 서비스 Deno Deploy는 6개월 뒤 문을 닫는다. 패키지 레지스트리 JSR과 Rust용 V8 바인딩 `rusty_v8`은 계속 운영된다. Cloudflare 블로그에는 "Acquisitions" 태그가 붙었고 거래 조건은 공개되지 않았다. Deno는 Sequoia 주도 시리즈 A를 포함해 총 2,600만 달러를 투자받은 회사였다.

팀의 새 목표는 Deno가 최근 공개한 celld와 Cloudflare의 workerd를 합쳐, Workers와 Durable Objects를 자기 서버에서도 쉽게 운영하게 만드는 것이다. Ryan Dahl은 런타임 자체보다 확장성이 내장된 프로그래밍 모델이 더 중요하다고 판단했다. 커뮤니티는 "인수가 아니라 팀만 채용하고 제품은 접은 것"이라는 비판과 "자체 호스팅 가능한 Workers가 락인 비판에 대한 실질적 답"이라는 기대로 갈렸다. 결론적으로 이 사건은 무료 오픈소스 런타임이 독립 사업으로 버티기 어렵다는 점, 그리고 경쟁의 단위가 런타임에서 프로그래밍 모델로 옮겨 가고 있다는 점을 보여준다.

## 1. 발표 내용과 확인된 사실

발표는 Deno 블로그와 Cloudflare 블로그에 동시에 올라왔다. 제품별 일정은 다음과 같다.

| 제품 | 발표 내용 | 2026-10-09 기준 예상 시점 |
| --- | --- | --- |
| Deno 런타임 | 1년간 월별 릴리스(버그 수정·보안 업데이트) 후 Deno 팀 개발 종료. MIT 오픈소스 유지 | 약 2027년 10월 |
| Deno Deploy | 6개월 운영 후 종료, 유료 고객은 Cloudflare Workers 이전 지원 | 약 2027년 4월 |
| JSR | 계속 운영, 인프라를 Cloudflare로 이전 | 종료 일정 없음 |
| rusty_v8 | 지원 계속, workerd 통합 추진 | 종료 일정 없음 |
| celld | workerd로 코드와 설계 병합 | 종료 일정 없음 |
| Fresh, Deno KV, Subhosting | 발표에서 언급 없음 | 미정 |

Hacker News에서 가장 많이 나온 불만은 런타임 종료라는 가장 중요한 내용이 글 맨 아래에 묻혀 있었다는 점이다. Cloudflare 쪽 게시물에는 런타임 종료 언급이 아예 없었다. 커뮤니티에서는 "opendeno"로 포크하자는 제안도 나왔지만 아직 실체는 없다.

> 출처: [Deno Blog 「Deno is joining Cloudflare」](https://deno.com/blog/cloudflare), [Cloudflare Blog 「Deno is joining Cloudflare」](https://blog.cloudflare.com/deno-joins-cloudflare/), [TechCrunch](https://techcrunch.com/2026/10/10/cloudflare-acquires-deno-to-improve-its-workers-programming-model/), [SignalStack 정리](https://www.signalstack.news/blog/deno-joins-cloudflare-runtime-deploy-jsr-timeline/)

## 2. 합류의 논리: 런타임에서 프로그래밍 모델로

Ryan Dahl은 예전 글 "JavaScript Containers"에서 컴퓨팅, 스토리지, 통신이 함께 동작해 앱마다 인프라를 조립할 필요가 없는 환경을 목표로 제시했다. Deno 블로그 원문은 그 목표를 따라온 과정을 세 단계로 설명한다.

```text
Deno (런타임)       : 모듈 배포, 보안 권한, 통합 툴체인, 단일 실행 파일, Node 호환
  → Deno Deploy    : "개발자 경험 밑에 인프라 복잡성이 여전히 많이 남아 있었다"
  → celld          : "확장성을 앱마다 조립하는 인프라가 아니라 프로그래밍 모델에 내장"
  → Cloudflare 합류 : Workers / Durable Objects 팀과 합쳐서 계속
```

그는 "합류한다는 건 어디에 노력을 쏟을지 고른다는 뜻"이라며, 별도 런타임과 호스팅 서비스를 계속 개발하는 대신 공유 플랫폼에 집중하기로 했다고 밝혔다. 특히 AI 에이전트를 강조했다. Durable Objects는 저렴한 서버리스 실행, 영속 상태, WebSocket, 고수준 JavaScript 인터페이스를 함께 제공하므로 에이전트 하네스에 잘 맞는다는 것이다.

### 2.1 Node.js의 IRC 서버에서 Durable Objects까지

Ryan Dahl이 처음 Node.js를 소개할 때 쓴 예제는 약 500줄짜리 IRC 서버였다. 비동기 I/O로 많은 연결을 간단하게 처리했지만 서버 한 대, 스레드 하나 구조라 사용자가 늘면 여러 서버로 쪼개는 작업이 필요했다. 현대 채팅 서비스는 메시지 기록, 계정, 데이터 저장까지 필요하므로 네트워크 연결을 처리하는 런타임만으로는 문제가 해결되지 않는다. Durable Objects는 그 쪼개기를 프로그래밍 모델 안에 넣은 것이다.

## 3. 핵심 기술: Durable Objects, workerd, celld

### 3.1 Durable Objects

Durable Object는 이름으로 찾아갈 수 있고 자기 SQLite 데이터베이스를 가진 작은 서버다. 객체 하나는 단일 스레드로 실행되므로 같은 객체에 들어온 두 요청이 동시에 실행되지 않는다. 스토리지 접근은 동기식이라 중간에 다른 요청이 끼어들지 못한다. 락 없이도 데이터가 일관되게 유지되는 이유다.

```text
채팅 서비스 예시
  채널 #general ──▶ DO("general") ── SQLite(메시지 기록) + WebSocket 연결들
  채널 #random  ──▶ DO("random")  ── SQLite + WebSocket 연결들
  사용자 alice  ──▶ DO("alice")   ── SQLite(계정 정보)
```

채널마다, 사용자마다, 문서마다, 에이전트마다 객체 하나를 두면 애플리케이션은 처음부터 자연스럽게 샤딩된다. 같은 추상화 위에 큐, 키/값 저장소, 실행 상태를 보존하는 워크플로, Git 저장소 같은 서비스도 만들 수 있다.

### 3.2 workerd의 한계

workerd는 Cloudflare가 실제 서비스에 쓰는 Workers 런타임을 오픈소스로 공개한 것이다. Shopify 같은 대형 고객이 "런타임이 오픈소스가 아니면 채택할 수 없다"고 요구해서 공개됐다. 그러나 workerd의 Durable Objects는 단일 인스턴스에서만 동작한다. 로컬 테스트는 되지만 여러 서버로 확장할 수 없다. Cloudflare 내부의 분산 구현은 전 세계 수백 개 위치와 여러 내부 서비스를 전제로 설계되어 자체 호스팅에 그대로 쓰기 어렵다.

### 3.3 celld

celld는 Deno 팀이 최근 공개한 자체 호스팅용 분산 Durable Objects 데몬이다. Rust로 작성한 단일 실행 파일이며, 머신마다 프로세스 하나(node)를 띄우고 같은 버킷을 공유하는 node들이 하나의 fleet이 된다.

| 항목 | 내용 |
| --- | --- |
| 실행 | 각 node가 V8을 내장하고 기존 `wrangler.json` 기반 Workers 번들을 그대로 실행 |
| 지원 범위 | Workers, Durable Objects, KV, Queues, D1, R2, Workflows, Cron Triggers, 정적 자산 |
| 저장소 | S3 호환·GCS·Azure Blob 버킷 하나가 배포본, cell 상태, 소유권 기록을 모두 보관 |
| 조율 | 합의 서비스·멤버십 프로토콜·장애 감지기 없음. 버킷 조건부 쓰기로 "cell 하나는 동시에 node 하나만 소유" 보장 |
| 내구성 | 버킷 또는 다른 node 디스크에 저장이 확인된 뒤에만 쓰기 응답(RPO=0) |
| 확장 | 같은 버킷을 가리키는 node를 하나 더 띄우면 끝 |

운영자가 관리할 것은 celld 인스턴스 여러 개와 객체 스토리지 버킷 하나뿐이다. 자체 호스팅인데 S3에 의존한다는 점이 역설적이지만, S3는 사실상 표준 프로토콜이고 직접 운영할 수도 있어서 타당하다는 평가가 많다.

> 출처: [GitHub denoland/celld](https://github.com/denoland/celld), [celld docs](https://celld.dev/docs/), [celld guarantees](https://celld.dev/docs/guarantees)

### 3.4 통합 이후의 그림

계획은 celld의 "버킷으로 조율하고 저장하는 workerd 인스턴스 묶음" 모델을 workerd에 가져오는 것이다. 별도 런타임을 키우지 않으니 개발 역량이 분산되지 않고, Cloudflare 네트워크와 자체 호스팅이 정확히 같은 API를 갖게 된다. Ryan Dahl과 Bert Belder가 이 작업을 이끌고, Workers 책임자 Kenton Varda는 이를 이용해 집에서 Cloudflare OS를 호스팅하겠다고 했다.

## 4. Cloudflare의 입장: 락인이 아니라 탈출구

Kenton Varda는 "Workers가 다른 클라우드와 다르게 동작하는 건 락인을 위해서"라는 통념에 반박했다. 실행 구조, 리소스 바인딩, Durable Objects는 클라우드 앱을 더 효율적이고 단순하게 만들기 위한 설계라는 것이다. 오픈소스로 탈출구를 주는 것이 오히려 좋은 사업이며, 실제로 workerd를 이용해 Cloudflare 밖으로 이전한 고객도 있고 이전할 수 있다는 선택권이 채택을 돕는다고 설명했다.

다만 자체 호스팅에 필요한 주변 서비스와 도구는 충분히 만들지 못했다고 인정했다. "자체 호스팅 런타임의 개발자 경험은 우리가 끝내 풀지 못한 부분인데, Deno 팀은 그걸 안다." Ryan Dahl에 대해서는 "서버에서 JavaScript를 쓴다는 아이디어를 만든 사람이며, 그가 없었다면 Workers도 없었을 것"이라고 평가했다.

## 5. 영향을 받는 쪽

### 5.1 Deno Deploy 사용자

가장 급한 쪽이다. 6개월 안에 옮겨야 하고 공식 지원 경로는 Cloudflare Workers뿐이다. 런타임 API가 다르고 Durable Objects와 Cloudflare 과금 모델에 맞춰야 하므로 이름만 바꾸는 이전이 아니다. 대안은 지원받는 이전 대신 벤더 의존이 커지는 Workers, 또는 지금 일이 더 많지만 특정 벤더에 덜 묶이는 Node와 일반 컨테이너 조합이다.

### 5.2 Deno 런타임 사용자

1년은 패치가 나오므로 당장 급하지는 않지만 마이그레이션 일정은 잡아야 한다. Node는 2020년보다 훨씬 나아져 네이티브 TypeScript 타입 제거와 권한 플래그를 갖췄다. `package.json` 없이 URL로 패키지를 가져오던 셸 스크립트 용도는 `zx --install`이나 Python PEP 723이 대안으로 거론된다. JSR은 계속 운영되고 다른 런타임에서도 쓸 수 있어 영향이 적다.

### 5.3 하류 프로젝트: yt-dlp

yt-dlp는 YouTube의 JavaScript 챌린지를 풀기 위해 외부 JS 런타임이 필수이고, 파일·네트워크 접근 없이 제한된 권한으로 코드를 돌릴 수 있다는 이유로 Deno를 기본·권장 런타임으로 쓴다. 발표 당일 "지원 종료 시 Deno의 우선순위를 내리자"는 이슈가 열렸다. 대안은 Node(22 이상), QuickJS, QuickJS-ng이며, Bun은 이미 deprecated 상태이고 지원 범위가 1.2.11~1.3.14라서 Rust로 재작성된 Bun 1.4는 대상이 아니다.

> 출처: [yt-dlp#17799](https://github.com/yt-dlp/yt-dlp/issues/17799), [yt-dlp/ejs](https://github.com/yt-dlp/ejs/), [yt-dlp Wiki EJS](https://github.com/yt-dlp/yt-dlp/wiki/ejs)

## 6. 커뮤니티 반응

### 6.1 "Deno는 언제 죽었나"

가장 뜨거운 논쟁은 원인에 관한 것이었다. 한쪽은 npm 호환성을 선택한 날 Deno가 죽었다고 본다. 단순하던 기능 범위가 비대해지고 "Node를 근본부터 다시 만든다"는 목표를 포기했다는 것이다. 반대쪽은 방향 전환을 강제한 것이 VC가 아니라 npm 호환을 앞세운 Bun의 확산이라고 반박한다. Node만 상대할 때는 천천히 개선해서 이기는 전략이 가능했지만, Node와 더 매끄럽게 붙는 경쟁자가 등장하면서 달라졌다는 것이다. "충분히 쓸 만한 기존 강자가 버티면 대체재는 결국 몰락한다", "Deno는 애초에 사업이 되어선 안 됐다"는 의견도 나왔다. 그래도 초기 Deno가 훌륭했던 덕에 Node가 기능을 따라 했고 더 나은 Node를 쓰게 됐다는 평가에는 대체로 동의했다.

### 6.2 "인수"라는 표현과 Cloudflare의 의도

"Deno가 Cloudflare에 합류하는 게 아니라 Deno는 문을 닫고 팀이 합류하는 것", "인재 인수로 Deno 개발 사실상 종료가 정확한 제목"이라는 비판이 많았다. 의도에 대해서는 celld 모델이 workerd에 들어가면 Workers가 진짜로 자체 호스팅 가능해진다는 기대와, 현재 Durable Objects의 대안이 없는 상황에서 celld라는 잠재적 경쟁자를 조기에 없앤 것 아니냐는 의심이 공존했다.

### 6.3 개발 도구 업계의 통합 흐름

HN 댓글에서는 최근 인수 목록이 정리됐다. Bun과 Stainless는 Anthropic, Astral(uv)과 Warp는 OpenAI, Cursor는 SpaceX, Astro·VoidZero(Vite)·Deno는 Cloudflare, NuxtLabs와 Svelte는 Vercel, Remix와 Tailwind는 Shopify로 갔다. "예전 스타트업의 회수 전략은 Yahoo에 파는 것이었는데 이제는 AI 기업에 파는 것"이라는 말이 이 흐름을 요약한다.

## 7. JS 런타임 지형 변화

| 런타임 | 소유·운영 | 엔진 | 2026-10 상태 |
| --- | --- | --- | --- |
| Node.js | OpenJS 재단 | V8 | 사실상 표준. 네이티브 TS 타입 제거, 권한 플래그 도입 |
| Bun | Anthropic (2025-12 인수) | JavaScriptCore | 1.4에서 Rust 재작성, AOT 컴파일 실험 중 |
| Deno | Cloudflare (팀 합류) | V8 | 유지보수 모드, 약 2027-10 개발 종료 |
| workerd | Cloudflare | V8 | celld 병합으로 자체 호스팅 강화 예정 |

Node를 다시 만들겠다던 두 도전자 중 Deno는 퇴장하고 Bun은 AI 기업의 지원을 받아 더 공격적으로 가고 있다. Deno의 유산인 권한 모델, 기본 TypeScript 지원, 단일 실행 파일, URL import는 이미 Node와 Bun에 흡수됐다.

## 8. 런타임 선택 점검표

| 영역 | 점검 항목 |
| --- | --- |
| 지속성 | 런타임을 운영하는 주체의 사업 모델과 재정 기반을 확인했는가? |
| 이전 비용 | 런타임 고유 API(`Deno.*`, `Bun.*`)에 얼마나 깊이 묶여 있는가? |
| 표준 준수 | 웹 표준 API(`fetch`, `Request`, `Response`) 위주로 작성해 런타임 교체 비용을 낮췄는가? |
| 호스팅 의존 | 런타임과 호스팅 서비스가 같은 회사에 묶여 있지 않은가? |
| 대체 경로 | 지원 종료 시 옮겨 갈 런타임과 예상 작업량을 파악하고 있는가? |
| 하류 영향 | 내가 의존하는 도구(예: yt-dlp)가 특정 런타임을 기본값으로 쓰고 있지 않은가? |
| 상태 관리 | 앱의 상태와 확장 방식이 특정 플랫폼(Durable Objects 등)의 프로그래밍 모델에 묶여 있는가? |

## 결론

Deno의 Cloudflare 합류는 기술적 완성도와 프로젝트의 지속성이 별개의 문제라는 점을 보여준다. 무료 오픈소스 런타임만으로는 수익을 내기 어렵고, 충분히 쓸 만한 Node가 버티는 한 대체재의 사업 모델은 호스팅밖에 없었다. 그 호스팅마저 거대 플랫폼과 경쟁해야 했다. 동시에 이 사건은 경쟁의 단위가 바뀌고 있음을 보여준다. Ryan Dahl은 V8 위의 실행기보다 상태, 통신, 확장을 내장한 프로그래밍 모델이 더 중요하다고 판단했고, AI 에이전트마다 상태를 가진 작은 서버가 필요하다는 요구는 그 판단을 뒷받침한다. 런타임을 고르는 쪽에서 얻을 교훈은 분명하다. 웹 표준 위에 코드를 쌓고 런타임 고유 API와 특정 호스팅에 깊이 묶이지 않아야, 이런 일이 생겨도 이전 비용을 감당할 수 있다.

> 출처: [GeekNews 「Deno 팀, Cloudflare 합류 - 런타임 개발은 1년 뒤 종료」](https://news.hada.io/topic?id=35073), [DEV 「Deno Has One Year Left. Node Won Without a Fight.」](https://dev.to/alanwest/deno-has-one-year-left-node-won-without-a-fight-4jfp), [DEV 「Deno Is Dead in 12 Months」](https://dev.to/ashraf_chowdury09/deno-is-dead-in-12-months-what-cloudflares-acquisition-means-for-your-stack-1e3p)
