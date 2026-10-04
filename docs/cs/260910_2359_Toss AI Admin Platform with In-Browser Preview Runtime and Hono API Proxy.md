# 토스 AI 어드민 사례로 본 브라우저 내 프리뷰 런타임과 Hono 기반 정책 프록시 설계

## 요약

토스 인터널(Internal) 조직은 프롬프트 몇 번으로 사내 어드민을 만드는 AI 어드민 플랫폼을 운영하고 있다. 사내 공개 후 약 반년 동안 만들어진 프로젝트는 약 440개, 페이지는 약 2,400개이며, 실제 라이브 중인 어드민은 약 120개다. 이 내용은 2026년 8월 25일 웨비나 「AI 시대, 토스 FE는 어떻게 일할까」의 'AI 시대 어드민' 세션(부제: 브라우저 안에 개발 환경 만들기)에서 발표됐다.

설계의 핵심은 어드민을 두 층으로 나눈 것이다. 데이터 처리와 컴플라이언스는 플랫폼이 정책으로 강제한다. 등록된 API를 서버가 프록시하면서 마스킹, 암호화, 접근 로그(증적)를 자동으로 적용한다. 화면은 AI가 API 스키마와 사내 UI 패턴을 참고해 React 코드로 생성한다. 가장 어려웠던 문제는 AI가 만든 코드를 사용자 브라우저 안에서 바로 실행해 보여주는 프리뷰 환경이었다. 서버 데브 서버 방식과 Sandpack 방식은 실패했고, 가상 파일 시스템과 esbuild-wasm, 패키지 셋 해시, 임포트 맵을 조합한 자체 런타임을 만들어 첫 화면 표시 시간을 47초에서 1.3초로 줄였다.

이 리포트는 1~7장에서 영상의 내용을 순서대로 빠짐없이 정리한다. 8장에서는 영상의 서버 측 정책 프록시와 패키지 셋 서비스를 Hono로 구현하는 설계를 제시한다. 영상에는 서버 프레임워크 이름이 나오지 않으므로, 8장은 토스가 공개한 사실이 아니라 영상의 구조를 Hono로 옮긴 참고 설계다.

## 1. 발표 개요와 확인된 사실

| 항목 | 내용 |
| --- | --- |
| 영상 | 토스 챌린저스 「이제 토스는 AI '딸깍'으로 만듭니다」 (12분 7초, 2026년 9월 10일 게시) |
| 원 행사 | 2026년 8월 25일 웨비나 「AI 시대, 토스 FE는 어떻게 일할까」 중 'AI 시대 어드민' 세션 편집본 |
| 발표자 | 토스 인터널 조직에서 어드민 제품을 만드는 프론트엔드 엔지니어 |
| 부제 | 브라우저 안에 개발 환경 만들기 |
| 주제 | AI로 어드민을 만드는 과정에서, 생성된 어드민을 사용자 브라우저 안에서 보여주기 위한 프론트엔드 개발 환경 구축 |

발표는 플랫폼 소개, 데모, 성과, 설계 원칙, 프리뷰 런타임 구축기, 회고, 질의응답 순서로 진행됐다.

> 출처: [토스 챌린저스 「이제 토스는 AI '딸깍'으로 만듭니다」](https://www.youtube.com/watch?v=tcGKZANuUVE), [웨비나 전체 영상](https://youtube.com/live/xDVbTlFfu30)

## 2. 문제 정의: 어드민 하나를 만들 때 반복되는 일

### 2.1 일반적인 서비스 구축 과정

프론트엔드 관점에서 서비스 하나를 만들려면 다음 과정을 거친다.

```text
프로젝트 스캐폴딩
  → npm 패키지 의존성 추가
  → 빌드 도구 선택 (주로 Vite)
  → 인증 연동
  → 배포 방식 결정
  → 코드 작성 → 빌드 → 배포
  (+ 서버 측 API 준비)
```

### 2.2 어드민에 추가로 필요한 것

어드민은 내부 데이터를 다루므로 일반 서비스보다 요구사항이 많다.

- 보안 관련 요구사항
- 개인정보 조회 통제
- 로그와 증적(감사 기록) 보관
- 다운로드 파일 암호화
- 마스킹 정책

### 2.3 페인 포인트와 배경

토스는 비즈니스가 빠르게, 많이 진행되기 때문에 이를 관리할 어드민 수요가 꾸준히 생긴다. 하지만 비즈니스 속도와 확장 속도만큼 프론트엔드 리소스가 있는 것은 아니다. FE 인력이 없는 팀은 어드민을 아예 만들지 못하거나, 서버 개발자가 직접 만들어 개별적으로 쓰는 형태였다.

여기서 두 가지 문제가 생겼다.

1. 어드민을 만들 때마다 2.1과 2.2의 과정을 처음부터 반복한다.
2. 어드민을 개별로 만들면 보안 도메인 지식과 규칙을 모든 어드민에 고르게 적용하기 어렵다.

그래서 팀은 이 문제를 개별 어드민이 아니라 플랫폼으로 풀기로 했고, 그 결과가 AI로 어드민을 만드는 제품이다.

## 3. 데모: 프롬프트에서 어드민까지

데모에서 사용자가 프롬프트를 입력하면 "화면을 만들고 있어요"라는 상태가 표시되고, 옆에 생성 중인 어드민의 프리뷰가 빠르게 나타난다. 흐름은 다음과 같다.

```text
사용자 프롬프트 입력
  → 에이전트가 사용자 질문(User Question) 기능으로 어떤 어드민을 만들지 되묻기
  → 사용자 답변
  → 에이전트가 계획 수립(Planning)
  → 코드 작성(Code Writing)
  → 수 분 내 어드민 완성, 브라우저 프리뷰로 즉시 확인
```

단순히 화면만 몇 분 만에 생기는 것이 아니다. 그 이전 단계로 사용할 API를 등록하고, 어떤 API를 쓸지 확인하는 정책적 절차가 제품에 녹아 있다. 이 과정도 프롬프트 창에서 에이전트와 대화(티키타카)하며 진행된다.

중앙에서 통제하는 플랫폼으로서의 컴플라이언스는 주로 데이터를 다루는 API 쪽에 적용되어 있다. 데이터를 어떻게 다뤄야 하는지는 정책으로 정의했고, 이 정책은 유관 부서와 소통하면서 함께 잡아갔다.

## 4. 성과

| 지표 | 수치 |
| --- | --- |
| 기간 | 사내 공개 후 약 반년 |
| 생성된 프로젝트 | 약 440개 |
| 생성된 페이지 | 약 2,400개 |
| 실제 라이브 중인 어드민 | 약 120개 |

진행자가 "440개를 진짜 다 쓰나"라고 묻자, 발표자는 라이브 기준 약 120개라고 답했다. 진행자는 이 수치만으로도 많다고 평가했다.

## 5. 설계 원칙: 정책은 플랫폼이, 화면은 AI가

팀은 어드민을 제품으로 볼 때 역할을 두 가지로 나눴다.

### 5.1 데이터와 컴플라이언스: 플랫폼 정책

어드민이 데이터를 다루는 방식과 컴플라이언스 통제는 플랫폼이 정책으로 정해 준다.

- 사용자는 사용할 API를 플랫폼에 등록한다.
- 어드민 화면은 원본 API를 직접 부르지 않고, 플랫폼 서버가 그 API를 프록시한다.
- 프록시 계층에서 정책이 자동으로 적용된다.
- 마스킹, 조회 시 암호화 같은 보호 기능이 자동으로 제공된다.
- 모든 호출이 프록시를 거치므로 접근 로그와 증적이 자동으로 남는다.

### 5.2 데이터를 보여주는 화면: AI 코드 생성

화면은 프론트엔드 엔지니어링 관점의 코드 생성 문제로 따로 다룬다.

- AI가 등록된 API의 스키마를 참고해 React 코드를 만든다.
- 자주 쓰이는 패턴을 미리 넣어 두었다. 테이블, 필터, 상세 페이지를 어떻게 구성하면 되는지 AI에게 알려주고, AI는 이를 따라 화면을 만든다.

```text
            ┌──────────────────────────────┐
  브라우저   │ AI가 생성한 React 어드민 화면 │
            └──────────────┬───────────────┘
                           │ 등록된 API만 호출
            ┌──────────────▼───────────────┐
  플랫폼    │ 정책 프록시                    │
            │ 인증 · 마스킹 · 암호화 · 증적  │
            └──────────────┬───────────────┘
                           │
            ┌──────────────▼───────────────┐
  내부 시스템│ 원본 서비스 API               │
            └──────────────────────────────┘
```

이렇게 나누면 AI가 어떤 화면 코드를 만들더라도 데이터 보호 규칙은 서버 측에서 일관되게 강제된다. 개별 어드민마다 보안 규칙을 구현해야 했던 문제를 구조로 해결한 것이다.

## 6. 가장 어려웠던 문제: 브라우저 안의 개발 환경

프론트엔드 개발자는 로컬 개발 환경에서 코드를 실행해 프리뷰를 확인하며 개발한다. 그런데 AI가 만든 코드는 사용자가 로컬 환경 없이 바로 확인할 수 있어야 하므로, 개발 환경 자체를 브라우저 안으로 가져와야 했다.

### 6.1 시도 1: 서버에서 Next.js 데브 서버 실행

서버에 Next.js 데브 서버 프로세스를 하나 띄우고, 각 사용자의 iframe이 그 프로세스에 연결되는 방식이었다. 개념 검증(POC)으로는 충분했지만 격리 문제가 있었다. 사용자마다 서로 다른 페이지를 쓰고 있어도, 한 페이지에서 에러 오버레이가 발생하면 데브 서버 전체로 전파됐다.

### 6.2 시도 2: Sandpack

격리를 위해 실행 환경을 사용자별로 분리된 브라우저 쪽으로 옮기기로 했고, 첫 선택은 CodeSandbox의 Sandpack이었다. Sandpack은 공식 문서에서 코드를 고치면 바로 UI에 반영되는 예제처럼, iframe 위에 가상 실행 환경을 만들어 프리뷰를 제공하는 라이브러리다.

문제는 속도와 사내 패키지였다.

- 첫 화면이 보이기까지 47초가 걸렸다.
- 느린 이유는 브라우저 런타임에 진입하는 시점부터 패키지를 다운로드하기 때문이다.
- 패키지를 퍼블릭 레지스트리에서 받는 구조라 사내 패키지를 주입하기 어려웠다.

### 6.3 시도 3: 직접 구축한 프리뷰 런타임

두 번의 실패 후 팀은 프리뷰 런타임을 직접 만들었다. 진행자의 표현대로 사실상 Sandpack을 직접 만든 셈이다. 핵심은 세 가지다.

1. 사용자 브라우저에서 실시간으로 바뀌는 코드를 빌드하는 환경
2. 패키지를 브라우저 런타임에서 내려받지 않고 미리 준비해 두는 패키지 시스템
3. 두 결과물을 합쳐 실행하고 화면에 보여주는 실행 계층

#### 가상 파일 시스템과 esbuild-wasm

브라우저에는 파일 시스템이 없으므로 경로를 키로 하는 가상 파일 시스템을 만들었고, 이를 레이어별로 구성했다. 번들러는 esbuild를 WebAssembly로 빌드해 브라우저에서 실행할 수 있게 만든 esbuild-wasm을 썼다. 가상 파일 시스템을 esbuild 플러그인으로 꽂아, esbuild가 import 구문을 해석해 파일을 가져올 때 플러그인이 가상 파일 시스템과 연결해 준다. esbuild는 이렇게 모은 파일을 브라우저에서 바로 실행할 수 있는 ESM 번들로 만든다.

```text
가상 파일 시스템 (경로 → 소스, 레이어 구성)
  → esbuild-wasm 플러그인이 import 해석·파일 로드
  → 브라우저용 ESM 번들 생성
```

#### 패키지 시스템과 패키지 셋 해시

처음에는 고정된 패키지 조합을 썼기 때문에 문제가 없었다. 그러나 사용자가 프로젝트마다 다른 패키지를 쓰고 싶어 하면서 조합이 동적으로 바뀌었고, 이를 중앙에서 하나로 관리하기 어려워졌다.

먼저 패키지를 각각 따로 번들링해 봤다. 그러자 싱글톤으로 유지되어야 하는 패키지 조합에서 싱글톤이 깨지는 문제가 생겼다. 예를 들어 React 인스턴스가 둘로 나뉘면 훅이 동작하지 않는 식의 문제다. 그래서 패키지 조합 전체를 함께 다뤄야 한다고 판단했고, **패키지 셋 해시(package set hash)** 개념을 도입해 전체 조합을 하나의 의미 단위로 묶었다.

```text
package.json의 의존성 엔트리 목록 + yarn.lock 해시
  → 하나로 해싱 → 패키지 셋 해시
  → 해시를 키로 S3에 업로드
```

패키지 셋을 만들고 쓰는 흐름은 다음과 같다.

```text
[서버: 패키지 셋 생성]
워크스페이스 생성
  → package.json 기반 yarn install로 의존성 설치
  → Vite로 번들링
  → 임포트 맵(import map) 생성
  → S3 업로드

[브라우저: 패키지 셋 사용]
임포트 맵 조회
  → 어떤 캐시, 어떤 패키지 조합을 쓸지 확인
  → esbuild-wasm 번들과 함께 실행
```

#### 실행과 화면 반영: 트랜잭션 커밋처럼

마지막으로 사용자 코드 번들과 패키지 셋을 합쳐 화면에 보여준다. 팀이 신경 쓴 부분은 조합이 성공했을 때만 화면에 반영되도록 한 것이다. 빌드나 실행이 실패하면 이전 화면을 유지하고, 성공하면 한 번에 교체한다. 데이터베이스의 트랜잭션 커밋처럼 동작하게 만든 것이다.

#### 결과

| 방식 | 첫 화면 표시 시간 |
| --- | --- |
| Sandpack | 47초 |
| 자체 프리뷰 런타임 | 1.3초 |

## 7. 회고와 질의응답

### 7.1 엔지니어가 하는 일

발표자는 돌아보면 엔지니어의 일이 다음과 같았다고 정리했다.

- 제품의 요구에 맞게 경계를 설정한다.
- 빠르게 변해야 할 부분과 느리게 변할 부분을 구분한다.
- 그 사이에 인터페이스를 정의한다.
- 코드가 안전하고 빠르게 실행될 수 있는 구조를 만든다.

또한 자신이 직접 구현한 부분은 거의 없고, 이미 잘 쓰이는 라이브러리와 도구(esbuild-wasm, Vite, yarn, S3, 임포트 맵 등)를 조합해 시스템을 만들었다고 말했다. 도구를 잘 고르고 조합하는 것 역시 엔지니어의 일이라는 것이다.

### 7.2 맺음말: 개인의 10x에서 조직의 10x로

발표자는 조직의 문제를 잘 구조화해 AI 제품으로 만들면 조직 전체의 10x를 만들 수 있다고 말했다. 토스도 이런 시도를 많이 하고 있다고 덧붙였다. 진행자는 개인의 10x를 넘어 조직 전체가 영향을 받는다는 점에서 AI 시대 어드민에 어울리는 문구라고 평가했다.

### 7.3 질의응답

- **질문**: 브라우저에 파일 시스템이 없어 가상 파일 시스템을 만들었다고 했는데, 실제 디스크에 파일이 써지는가, 아니면 런타임이 끝나면 사라지는가?
- **답변**: 메모리에서만 쓰는 가상 파일 시스템이다. 실제 코드는 사용자의 편집에 따라 S3에 업데이트된다.

## 8. Hono로 구현하는 정책 프록시와 패키지 셋 서비스

영상은 서버 측 구현 기술을 밝히지 않았다. 이 장은 5장의 정책 프록시와 6장의 패키지 셋·프로젝트 저장 서버를 Hono로 구현하는 참고 설계다. Hono는 Web 표준 `Request`/`Response` 위에서 동작하는 경량 TypeScript 웹 프레임워크로, Bun·Node.js·Deno·Cloudflare Workers에서 같은 코드로 실행된다. 미들웨어 체인으로 인증·정책·증적을 순서대로 끼울 수 있어 정책 프록시 구조와 잘 맞는다.

### 8.1 서버 역할 분리

```text
Hono 서버
├── /proxy/:apiId/*                    정책 프록시 (인증 → 정책 로드 → 증적 → 원본 호출 → 마스킹)
├── /apis/:apiId/schema                AI 코드 생성용 API 스키마 제공
├── /package-sets/:hash/import-map.json 패키지 셋 임포트 맵 (S3/CDN 위치 안내)
└── /projects/:id/files                사용자 코드 저장 (S3)
```

### 8.2 정책 프록시

미들웨어 순서가 곧 정책 적용 순서다. 인증 실패나 미등록 API는 원본 호출 전에 차단되고, 증적 미들웨어는 `next()` 이후 응답 상태까지 기록한다.

```ts
import { Hono } from 'hono'
import { createMiddleware } from 'hono/factory'
import { HTTPException } from 'hono/http-exception'

type User = { id: string; roles: string[] }
type Policy = { upstream: string; allowRoles: string[]; maskFields: string[] }
type Env = { Variables: { user: User; policy: Policy } }

declare function verifySession(token: string | undefined): Promise<User | null>
declare function getPolicy(apiId: string): Promise<Policy | null>
declare function writeAuditLog(entry: Record<string, unknown>): Promise<void>

const auth = createMiddleware<Env>(async (c, next) => {
  const user = await verifySession(c.req.header('authorization'))
  if (!user) throw new HTTPException(401)
  c.set('user', user)
  await next()
})

const loadPolicy = createMiddleware<Env>(async (c, next) => {
  const policy = await getPolicy(c.req.param('apiId') ?? '')
  if (!policy) throw new HTTPException(404, { message: 'unregistered api' })
  if (!policy.allowRoles.some((r) => c.get('user').roles.includes(r))) {
    throw new HTTPException(403)
  }
  c.set('policy', policy)
  await next()
})

const audit = createMiddleware<Env>(async (c, next) => {
  const startedAt = Date.now()
  await next()
  await writeAuditLog({
    userId: c.get('user').id,
    apiId: c.req.param('apiId'),
    method: c.req.method,
    path: c.req.path,
    status: c.res.status,
    elapsedMs: Date.now() - startedAt,
  })
})

function mask(value: unknown, fields: string[]): unknown {
  if (Array.isArray(value)) return value.map((v) => mask(v, fields))
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([k, v]) =>
        fields.includes(k) ? [k, '****'] : [k, mask(v, fields)],
      ),
    )
  }
  return value
}

const app = new Hono<Env>()

app.all('/proxy/:apiId/*', auth, loadPolicy, audit, async (c) => {
  const { upstream, maskFields } = c.get('policy')
  const subPath = c.req.path.slice(`/proxy/${c.req.param('apiId')}`.length)
  const url = new URL(subPath + new URL(c.req.url).search, upstream)

  const res = await fetch(url, {
    method: c.req.method,
    headers: { 'content-type': 'application/json', 'x-admin-user': c.get('user').id },
    body: ['GET', 'HEAD'].includes(c.req.method) ? undefined : await c.req.arrayBuffer(),
  })

  return new Response(JSON.stringify(mask(await res.json(), maskFields)), {
    status: res.status,
    headers: { 'content-type': 'application/json' },
  })
})

export default app
```

Bun에서는 `export default app`만으로 `Bun.serve`가 `app.fetch`를 핸들러로 사용한다. 다운로드 파일 암호화는 같은 구조에서 응답 본문을 암호화 스트림으로 감싸는 미들웨어로 추가할 수 있고, 조회 사유 입력 같은 추가 정책도 `loadPolicy` 뒤에 미들웨어로 끼우면 된다.

### 8.3 AI에게 줄 API 스키마

AI가 화면 코드를 만들려면 등록된 API의 요청·응답 형태를 알아야 한다. 등록 시점에 OpenAPI 스키마를 저장해 두고 그대로 내려주면, 에이전트는 이 스키마와 테이블·필터·상세 페이지 패턴 문서를 함께 참고해 React 코드를 생성한다. 마스킹 대상 필드도 스키마에 표시해 두면 AI가 화면에서 해당 필드를 마스킹된 값으로 가정하고 코드를 짤 수 있다.

```ts
declare function getSchema(apiId: string): Promise<object | null>

app.get('/apis/:apiId/schema', auth, async (c) => {
  const schema = await getSchema(c.req.param('apiId'))
  if (!schema) return c.notFound()
  return c.json(schema)
})
```

### 8.4 패키지 셋 해시와 임포트 맵

패키지 셋 해시는 의존성 목록과 `yarn.lock`을 함께 해싱해 만든다. 같은 조합이면 항상 같은 키가 나오므로 S3에 한 번 만든 번들을 모든 프로젝트가 재사용하고, 내용이 바뀌지 않으므로 장기 캐시를 걸 수 있다.

```ts
async function packageSetHash(dependencies: Record<string, string>, lockfile: string) {
  const hasher = new Bun.CryptoHasher('sha256')
  hasher.update(JSON.stringify(Object.entries(dependencies).sort()))
  hasher.update(lockfile)
  return hasher.digest('hex')
}

const CDN = 'https://cdn.example.internal/package-sets'

app.get('/package-sets/:hash/import-map.json', (c) => {
  c.header('cache-control', 'public, max-age=31536000, immutable')
  return c.redirect(`${CDN}/${c.req.param('hash')}/import-map.json`, 302)
})
```

### 8.5 브라우저 측 빌드와 커밋형 반영

브라우저에서는 메모리 가상 파일 시스템을 esbuild-wasm 플러그인으로 연결한다. 상대 경로 import는 가상 파일 시스템에서 읽고, 패키지 import는 `external`로 남겨 임포트 맵이 패키지 셋 번들로 연결하게 한다. React 같은 싱글톤 패키지가 패키지 셋 안에서 한 번만 로드되므로 싱글톤이 깨지지 않는다.

```ts
import * as esbuild from 'esbuild-wasm'

await esbuild.initialize({ wasmURL: '/esbuild.wasm' })

const files = new Map<string, string>()
const EXTENSIONS = ['', '.tsx', '.ts', '.jsx', '.js', '/index.tsx', '/index.ts']

function resolveInVfs(path: string): string | undefined {
  return EXTENSIONS.map((ext) => path + ext).find((p) => files.has(p))
}

const vfsPlugin: esbuild.Plugin = {
  name: 'vfs',
  setup(build) {
    build.onResolve({ filter: /^\.{0,2}\// }, (args) => {
      const base = args.importer ? `file://${args.importer}` : 'file:///'
      const path = resolveInVfs(new URL(args.path, base).pathname)
      return path ? { path, namespace: 'vfs' } : { errors: [{ text: `not found: ${args.path}` }] }
    })
    build.onResolve({ filter: /^[^./]/ }, (args) => ({ path: args.path, external: true }))
    build.onLoad({ filter: /.*/, namespace: 'vfs' }, (args) => ({
      contents: files.get(args.path),
      loader: 'tsx',
    }))
  },
}

async function rebuild(iframe: HTMLIFrameElement, importMap: object) {
  const result = await esbuild.build({
    entryPoints: ['/src/main.tsx'],
    bundle: true,
    format: 'esm',
    jsx: 'automatic',
    write: false,
    plugins: [vfsPlugin],
  }).catch(() => null)

  if (!result) return

  const code = URL.createObjectURL(new Blob([result.outputFiles[0].text], { type: 'text/javascript' }))
  iframe.srcdoc = `<!doctype html>
<script type="importmap">${JSON.stringify(importMap)}</script>
<div id="root"></div>
<script type="module" src="${code}"></script>`
}
```

`rebuild`는 빌드가 실패하면 아무것도 바꾸지 않고 반환하므로 이전 프리뷰가 그대로 남는다. 성공했을 때만 iframe 내용을 교체하는 것이 6.3에서 말한 트랜잭션 커밋형 반영이다. 실제 운영에서는 런타임 에러까지 확인한 뒤 교체하도록 숨겨진 iframe에서 먼저 실행하고 전환하는 방식을 더할 수 있다.

## 9. 운영 점검표

| 영역 | 점검 항목 |
| --- | --- |
| 정책 강제 | 화면 코드가 원본 API를 직접 부르지 못하고 반드시 정책 프록시를 거치는가? |
| API 등록 | 사용할 API의 등록·승인 절차와 정책(권한, 마스킹 필드)이 유관 부서와 합의되어 있는가? |
| 데이터 보호 | 마스킹, 조회 암호화, 다운로드 파일 암호화가 프록시 계층에서 자동 적용되는가? |
| 증적 | 모든 호출의 사용자·API·시간·결과가 자동으로 기록되는가? |
| 코드 생성 | AI가 API 스키마와 사내 UI 패턴(테이블, 필터, 상세)을 참고하도록 제공되는가? |
| 격리 | 한 사용자의 프리뷰 오류가 다른 사용자에게 전파되지 않는가? |
| 패키지 | 패키지 조합을 셋 단위로 해싱해 싱글톤 깨짐을 막고, 사내 패키지를 포함할 수 있는가? |
| 성능 | 패키지를 런타임에 내려받지 않고 미리 빌드·캐시해 첫 화면 시간을 관리하는가? |
| 반영 | 빌드·실행이 성공했을 때만 프리뷰를 교체하는가? |
| 저장 | 가상 파일 시스템은 메모리에만 두고, 원본 코드는 사용자 편집에 따라 S3에 저장하는가? |

## 결론

토스 AI 어드민의 핵심은 AI가 화면을 빨리 만든다는 점보다, AI가 무엇을 만들든 안전하도록 경계를 먼저 설계했다는 점에 있다. 데이터와 컴플라이언스는 플랫폼의 정책 프록시가 강제하고, AI는 그 위에서 API 스키마와 사내 패턴을 따라 화면만 만든다. 이 구조 덕분에 FE 리소스가 없는 팀도 반년 만에 수백 개의 어드민을 만들 수 있었다. 브라우저 안의 개발 환경은 두 번의 실패 끝에 가상 파일 시스템, esbuild-wasm, 패키지 셋 해시, 임포트 맵, 커밋형 반영을 조합해 47초를 1.3초로 줄였다. 발표자가 강조했듯 엔지니어의 일은 모든 것을 직접 짜는 것이 아니라 경계와 인터페이스를 정하고 검증된 도구를 조합하는 것이다. 서버 측을 Hono로 구현하면 인증·정책·증적을 미들웨어 체인으로 표현할 수 있어, 이 경계를 코드 구조로 그대로 옮길 수 있다.

> 출처: [토스 챌린저스 「이제 토스는 AI '딸깍'으로 만듭니다」](https://www.youtube.com/watch?v=tcGKZANuUVE), [웨비나 「AI 시대, 토스 FE는 어떻게 일할까」 전체 영상](https://youtube.com/live/xDVbTlFfu30), [Hono 공식 문서](https://hono.dev/docs/)
