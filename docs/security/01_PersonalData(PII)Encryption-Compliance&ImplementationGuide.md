# 개인정보 암호화 보안 기준 및 구현 가이드

> **문서 성격**: 개인정보를 저장·전송하는 서비스(1인 개발/소규모 포함)가 반드시 준수해야 하는 **법적 암호화 의무**와 **실무 구현 기준**을 정리한 보안 보고서.
> **주의**: 법령·고시는 지속적으로 개정됩니다. 실제 적용 전 반드시 [국가법령정보센터](https://www.law.go.kr/)와 개인정보보호위원회 최신 고시·안내서 원문을 재확인하십시오. 본 문서의 법적 서술은 참고용이며 법률 자문을 대체하지 않습니다.

---

## 0. 핵심 요약 (TL;DR)

| 구분 | 결론 |
|---|---|
| 필수 암호화 대상 | **고유식별정보 4종**(주민등록번호, 여권번호, 운전면허번호, 외국인등록번호) + **신용카드번호, 계좌번호, 생체인식정보** = **총 7종** |
| 비밀번호 | **복호화 불가능한 단방향 해싱** (SHA-256 지양, **bcrypt / Argon2 / scrypt / PBKDF2** 권장) |
| 이름·이메일·전화번호 | 필수 대상 아님(권고). 단 접근통제·부분 암호화 시 사고 발생 시 **책임 경감** 사유 |
| 전송 구간 | **HTTPS/TLS 필수** (`http://` → `https://` 확인) |
| 암호화 키 | **코드 하드코딩 금지**, 환경변수/시크릿 매니저로 분리 |
| 최선의 방어 | **애초에 저장하지 않기**(데이터 최소화) — 카드/계좌는 PG사 위임, 주민번호는 본인확인기관(CI/DI) 활용 |

---

## 1. 문서 개요

- **목적**: 회원가입/로그인 등 개인정보를 다루는 애플리케이션이 **개인정보의 안전성 확보조치 기준(개인정보보호위원회 고시)** 상 암호화 의무를 충족하도록, 법적 기준과 구현 방법을 제공.
- **적용 범위**: 개인정보를 **저장** 또는 **전송**하는 모든 온라인 서비스. **서비스 규모와 무관**하며, "AI(바이브 코딩)가 만들어 줬다"는 사실은 면책 사유가 되지 않는다.
- **근거 기준**: *개인정보의 안전성 확보조치 기준 안내서(2025년 11월판)* 및 그 기준 고시.
  - 안내서가 기준으로 삼은 고시는 개정되어도(예: 이후 개정판) **암호화 관련 조항의 실질 내용은 동일**하므로 본 문서 기준을 최신으로 간주. 다만 시행 전 현행 원문 재확인 필수.

---

## 2. 법적 책임 (미준수 시)

안전조치 의무를 위반하거나 개인정보가 유출될 경우 다음 책임이 **일부 또는 전부** 부과될 수 있다.

| 책임 유형 | 내용 |
|---|---|
| 과태료 | 안전조치 의무 위반 시 **3천만 원 이하** |
| 과징금 | 개인정보 유출 시 부과 대상 가능 |
| 손해배상 | 피해 발생 시 배상 책임. **고의·중과실 시 최대 5배(징벌적)** |
| 통지·신고 의무 | 일정 요건의 유출 발생 시 정보주체 통지 및 신고 **의무** |
| 시정명령·공표 | 사안이 중대할 경우 시정명령 및 처분사실 공표 |

> 암호화는 **외부 해킹 자체를 막는 수단이 아니라**, 침입·탈취가 발생했을 때 **개인정보 노출 피해를 최소화**하는 최후의 방어선이다. 탈취되어도 암호화되어 있으면 실질 피해가 줄고, 법적 판단에서도 **감경 요소**로 고려된다.

### 실제 처분 사례 (참고)

| 사례 | 시점 | 개요 | 처분 |
|---|---|---|---|
| 락앤락 | 2026.07 | 서버 보안 취약점으로 내부망 침입, 약 **130만 명** 개인정보 유출. 고유식별정보 등 **미암호화 보관** 확인 | 과징금 약 **5억 300만 원** + 과태료 540만 원 |
| 법무법인 로고스 | 2025.11 | 외부 해킹으로 소송자료 유출. 주민번호·계좌번호·비밀번호 등 **미암호화 저장** 적발 | 과징금 약 **5억 2,300만 원** + 과태료 600만 원 |

> 위 금액은 "미암호화" 하나 때문만이 아니라, 취약점 미조치·관리자 계정 비밀번호 관리 부실·탐지/대응 미흡 등 **종합적 요소**가 반영된 결과다.

---

## 3. 필수 암호화 대상 (법정 7종)

**제7조(개인정보의 암호화)** 상 저장·전송 시 반드시 암호화해야 하는 정보:

**고유식별정보 4종**
1. 주민등록번호
2. 여권번호
3. 운전면허번호
4. 외국인등록번호

**기타 중요정보 3종**
5. 신용카드번호
6. 계좌번호
7. 생체인식정보

> 위 7종 중 **하나라도** 저장 또는 전송한다면, **안전한 암호 알고리즘**으로 반드시 암호화해야 한다.

### 필수 대상이 아닌 개인정보 (이름·이메일·전화번호·주소 등)

- 법적 **필수 암호화 의무 대상은 아님**(권고 수준).
- 안내서 표현: *"암호화 대상 이외의 개인정보(성명·휴대전화번호·주소 등)도 개인정보 처리 환경 및 유출 시 위험을 고려하여 **암호화하여 저장할 수 있다**."* → 의무가 아닌 **선택/권고**.
- 다만 인터넷 전송 시, 업무용 PC 저장 시에는 **별도 기준**이 적용될 수 있으므로 개별 확인 필요.
- 실무적으로 접근통제·부분 암호화 등을 적용하면 사고 시 **책임 경감** 요인이 된다.

---

## 4. 암호화 vs 해싱 (개념 구분)

민감정보 저장 방식은 크게 두 가지다.

| 구분 | 방향성 | 복원 | 용도 |
|---|---|---|---|
| **암호화(Encryption)** | 양방향 (평문 ↔ 암호문) | 키로 **복호화 가능** | 주민번호·계좌번호처럼 **다시 원문이 필요한** 정보 |
| **해싱(Hashing)** | 단방향 (평문 → 해시) | **복호화 불가** | 비밀번호처럼 **관리자도 원문을 알면 안 되는** 정보 |

- 주민번호/계좌번호 → 조회·검증 시 원문 복원이 필요 → **암호화**.
- 비밀번호 → 관리자조차 알아선 안 됨 → **단방향 해싱**.

---

## 5. 대상 구분: 이용자 vs 이용자가 아닌 정보주체

안내서는 정보주체를 두 유형으로 구분하며, **적용 기준이 다르다.**

| 구분 | 정의 | 기준 |
|---|---|---|
| **이용자** | 온라인 서비스를 이용하는 고객 (대부분의 웹/앱 서비스가 여기에 해당, **바이브 코딩의 99%**) | **저장 위치 불문** 모두 암호화 대상. 대상 범위가 넓음(고유식별정보 + **신용카드번호·계좌번호** 포함) |
| **이용자가 아닌 정보주체** | 내부 직원, 오프라인 고객 등 | 저장 위치(인터넷 구간/내부망 등)에 따라 개별 판단. 기준은 **고유식별정보** 중심 |

- **결론**: 일반적인 온라인 서비스 개발자는 **"이용자" 기준(더 엄격)**을 필수로 적용해야 한다.
- 임직원/오프라인 사업을 함께 다루게 되면 "이용자가 아닌 정보주체" 기준을 추가 검토.

---

## 6. 전송 구간 암호화 (HTTPS/TLS)

DB 저장뿐 아니라 **네트워크 전송 구간**도 암호화 의무 대상이다. 웹 통신은 **HTTPS 필수**.

- 브라우저 → 서버로 개인정보가 이동하는 구간을 TLS로 보호.
- 배포 플랫폼(호스팅) 대부분이 기본 제공하나, **주소가 `https://`로 시작하는지 반드시 확인**.

### 6-1. HTTP → HTTPS 리다이렉트 & HSTS (Express 예시)

```javascript
// app.ts — 운영 환경에서 HTTPS 강제 + HSTS
import express from "express";

const app = express();

// 프록시(로드밸런서) 뒤에 있을 때 X-Forwarded-Proto 신뢰
app.set("trust proxy", 1);

app.use((req, res, next) => {
  if (process.env.NODE_ENV === "production" && !req.secure) {
    return res.redirect(308, `https://${req.headers.host}${req.originalUrl}`);
  }
  next();
});

// HSTS: 브라우저가 이후 항상 HTTPS로만 접속하도록 강제 (2년)
app.use((req, res, next) => {
  res.setHeader(
    "Strict-Transport-Security",
    "max-age=63072000; includeSubDomains; preload"
  );
  next();
});
```

### 6-2. Nginx 리버스 프록시 예시

```nginx
server {
    listen 80;
    server_name example.com;
    return 308 https://$host$request_uri;   # HTTP는 무조건 HTTPS로
}

server {
    listen 443 ssl http2;
    server_name example.com;

    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;     # 구버전 TLS 비활성화
    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains" always;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Host $host;
    }
}
```

---

## 7. 실무 권장 사항 및 구현 예시

### 7-1. 비밀번호는 안전한 단방향 해싱 (SHA-256 지양)

- **SHA-256의 문제**: 무결성 검증용으로 설계되어 **연산이 매우 빠름**. 이 속도가 비밀번호 저장에서는 치명적 약점 → 탈취 시 **레인보우 테이블·무차별 대입(brute-force)**으로 빠르게 매칭 시도 가능.
- **권장**: 의도적으로 느리고(work factor 조절) 솔트가 내장된 **bcrypt, Argon2, scrypt, PBKDF2**.
  - (참고: SHA-256이 안내서의 "안전한 암호 알고리즘 예시"에 포함되어 있어 **법 위반은 아니지만**, 비밀번호 해싱 용도로는 실무상 지양.)

#### bcrypt (Node.js)

```javascript
import bcrypt from "bcrypt";

const SALT_ROUNDS = 12; // cost factor. 하드웨어 성능에 맞춰 조정(높을수록 느리고 안전)

export async function hashPassword(plain: string): Promise<string> {
  return bcrypt.hash(plain, SALT_ROUNDS); // salt는 결과 문자열에 내장됨
}

export async function verifyPassword(plain: string, hash: string): Promise<boolean> {
  return bcrypt.compare(plain, hash); // 타이밍 안전 비교
}
```

#### Argon2id (Node.js) — 현재 가장 권장되는 방식

```javascript
import argon2 from "argon2";

export async function hashPassword(plain: string): Promise<string> {
  return argon2.hash(plain, {
    type: argon2.argon2id,
    memoryCost: 19456, // 19 MiB
    timeCost: 2,
    parallelism: 1,
  });
}

export async function verifyPassword(hash: string, plain: string): Promise<boolean> {
  return argon2.verify(hash, plain);
}
```

#### Argon2id (Python)

```python
from argon2 import PasswordHasher
from argon2.exceptions import VerifyMismatchError

ph = PasswordHasher()  # 안전한 기본 파라미터 제공

def hash_password(plain: str) -> str:
    return ph.hash(plain)

def verify_password(stored_hash: str, plain: str) -> bool:
    try:
        return ph.verify(stored_hash, plain)
    except VerifyMismatchError:
        return False
```

> **금지 패턴 (절대 사용 금지)**
> ```javascript
> // ❌ 평문 저장
> user.password = req.body.password;
> // ❌ 빠른 해시 + 솔트 없음 (레인보우 테이블에 취약)
> user.password = crypto.createHash("sha256").update(req.body.password).digest("hex");
> ```

### 7-2. 양방향 암호화가 필요한 정보 (주민번호·계좌번호 등)

복원이 필요한 필수 대상은 **AES-256-GCM** 같은 인증 암호(AEAD)를 사용한다. GCM은 기밀성 + 무결성(변조 탐지)을 동시에 제공한다.

#### AES-256-GCM (Node.js `crypto`)

```javascript
import crypto from "crypto";

const ALGO = "aes-256-gcm";
// 32바이트(256비트) 키. 반드시 환경변수/시크릿 매니저에서 로드 (7-3 참고)
const KEY = Buffer.from(process.env.DATA_ENC_KEY_BASE64!, "base64");

export function encrypt(plain: string): string {
  const iv = crypto.randomBytes(12); // GCM 권장 IV 길이 96비트, 매번 새로 생성
  const cipher = crypto.createCipheriv(ALGO, KEY, iv);
  const enc = Buffer.concat([cipher.update(plain, "utf8"), cipher.final()]);
  const tag = cipher.getAuthTag();
  // iv:tag:ciphertext 를 base64로 묶어 저장
  return [iv.toString("base64"), tag.toString("base64"), enc.toString("base64")].join(":");
}

export function decrypt(payload: string): string {
  const [ivB64, tagB64, dataB64] = payload.split(":");
  const decipher = crypto.createDecipheriv(ALGO, KEY, Buffer.from(ivB64, "base64"));
  decipher.setAuthTag(Buffer.from(tagB64, "base64"));
  const dec = Buffer.concat([
    decipher.update(Buffer.from(dataB64, "base64")),
    decipher.final(), // 태그 검증 실패 시 여기서 예외 → 변조 탐지
  ]);
  return dec.toString("utf8");
}
```

#### AES-256-GCM (Python `cryptography`)

```python
import os, base64
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

KEY = base64.b64decode(os.environ["DATA_ENC_KEY_BASE64"])  # 32바이트

def encrypt(plain: str) -> str:
    aes = AESGCM(KEY)
    iv = os.urandom(12)
    ct = aes.encrypt(iv, plain.encode(), None)  # 태그가 ct 뒤에 포함됨
    return base64.b64encode(iv + ct).decode()

def decrypt(payload: str) -> str:
    raw = base64.b64decode(payload)
    iv, ct = raw[:12], raw[12:]
    return AESGCM(KEY).decrypt(iv, ct, None).decode()
```

> **결정적 암호화가 필요한 경우(검색·중복확인용)**: 랜덤 IV 방식은 같은 평문도 매번 다른 암호문이 되어 `WHERE ssn = ?` 검색이 불가능하다. 이럴 땐 **HMAC 기반 blind index**(검색용 키로 별도 HMAC 컬럼 생성)를 함께 두고, 실제 값은 GCM으로 저장하는 패턴을 쓴다.

### 7-3. 암호화 키는 코드에서 분리 (하드코딩 금지)

하드코딩된 키는 소스 유출·권한 없는 내부자 열람·Git 공개 시 **그대로 노출**되어 모든 암호화가 무력화된다.

```javascript
// ❌ 하드코딩 — 소스가 유출되면 키도 함께 유출
const KEY = "a1b2c3d4e5f6...";

// ✅ 환경변수/시크릿 매니저에서 로드
const KEY = Buffer.from(process.env.DATA_ENC_KEY_BASE64!, "base64");
if (KEY.length !== 32) throw new Error("DATA_ENC_KEY_BASE64 must be 32 bytes");
```

- 키 저장 위치 우선순위: **클라우드 시크릿 매니저(AWS Secrets Manager, GCP Secret Manager, Vault 등) > 배포 환경변수 > `.env` 파일**.
- `.env`는 반드시 `.gitignore`에 포함:

```gitignore
.env
.env.*
!.env.example
*.pem
*.key
```

- `.env.example`에는 **값 없이 키 이름만** 남겨 협업자에게 필요한 변수를 알린다:

```bash
# .env.example
DATA_ENC_KEY_BASE64=
DATABASE_URL=
SESSION_SECRET=
```

- **키 로테이션**을 고려해 암호문에 키 버전을 함께 저장(`v1:iv:tag:ct`)하면 무중단 교체가 쉽다.

### 7-4. 최선의 방어 = 저장하지 않기 (데이터 최소화)

가장 안전한 방법은 **애초에 민감정보를 저장하지 않는 것**이다. "어떻게 암호화할까" 이전에 "**꼭 저장해야 하는가**"를 먼저 검토한다.

| 정보 | 권장 처리 |
|---|---|
| 카드번호·계좌번호 | **PG사에 위임**. 결제 연동 시 PG사가 보관하고, 서비스는 조회/빌링키만 사용 → 우리 DB에 저장하지 않음. 법적 의무 주체도 PG사가 됨 |
| 주민등록번호 | **본인확인기관** 활용. **CI/DI** 등 대체 식별값만 저장. (주민번호는 규제가 매우 까다로워 원칙적으로 저장 회피) |
| 이름·이메일·전화번호 | 서비스에 꼭 필요한 최소 항목만 수집, 목적 달성 후 파기 |

```text
[데이터 최소화 의사결정 흐름]
이 정보를 저장해야 하는가?
  └ 아니오 → 저장하지 않음 (최선)
  └ 예 → 외부(PG/본인확인기관)에 위임 가능한가?
          └ 예 → 위임 (빌링키/CI·DI만 보관)
          └ 아니오 → 필수 7종인가?
                     └ 예 → AES-256-GCM 암호화 + 키 분리
                     └ 아니오 → 접근통제 + (권장)부분 암호화
```

---

## 8. 회원 테이블 설계 예시 (종합)

```sql
CREATE TABLE users (
    id              BIGSERIAL PRIMARY KEY,
    email           VARCHAR(255) NOT NULL UNIQUE,     -- 필수 대상 아님(권고)
    password_hash   VARCHAR(255) NOT NULL,            -- bcrypt/argon2 결과 (단방향)
    name            VARCHAR(100),                     -- 필수 대상 아님(권고)
    phone           VARCHAR(50),                      -- 필수 대상 아님(권고)

    -- 필수 암호화 대상(저장이 불가피할 때만): AES-256-GCM 암호문 저장
    ssn_enc         TEXT,                             -- 주민등록번호(가급적 저장 회피)
    ssn_index       CHAR(64),                         -- (선택) 검색용 HMAC blind index
    account_enc     TEXT,                             -- 계좌번호

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

```javascript
// 회원가입 핸들러 (개념 예시)
app.post("/signup", async (req, res) => {
  const { email, password, name, phone } = req.body;

  const passwordHash = await hashPassword(password);   // 단방향 해싱
  // 카드/계좌/주민번호는 가능하면 받지 않음. 불가피하면 encrypt() 후 저장.

  await db.users.insert({
    email,
    password_hash: passwordHash,
    name,
    phone,
  });

  res.status(201).json({ ok: true });
});
```

---

## 9. 배포 전 5가지 자가 진단 체크리스트

배포 전 아래 5가지를 반드시 점검한다.

- [ ] **1. 비밀번호가 DB에 평문으로 저장되고 있지 않은가?**
- [ ] **2. 비밀번호를 복호화 가능한 방식으로 저장하고 있지 않은가?** (반드시 단방향 해싱)
- [ ] **3. 어떤 개인정보를 저장하는지 파악하고, 그중 법정 필수 암호화 7종을 식별하고 있는가?**
- [ ] **4. 암호화 키/API 키가 코드에 하드코딩되어 있지 않은가?**
- [ ] **5. 개인정보가 암호화되지 않은 통신(HTTP)으로 전송되고 있지 않은가?** (HTTPS 필수)

> 모르는 항목이 있으면 이 체크리스트 텍스트를 그대로 AI에게 주고 *"내 프로젝트에서 이 부분을 점검해줘"*라고 요청하는 것도 좋은 방법이다.

---

## 10. AI(바이브 코딩) 활용 시 규칙

AI는 **동작하는 코드**는 잘 만들지만, **적법하고 안전한 서비스**인지는 최종적으로 사람이 확인하고 책임진다.

1. "회원가입/로그인 되게 해줘" 같은 막연한 요청 대신, **보안 요구사항과 법적 기준을 구체적으로 명시**한다.
2. 프롬프트 예시:

```text
회원가입/로그인 기능을 구현해줘. 다음 보안 요구사항을 반드시 지켜:
- 비밀번호는 argon2id로 단방향 해싱 (평문/복호화 가능 방식 금지)
- 주민번호/계좌번호/카드번호 등 법정 필수 암호화 대상은 저장을 피하고,
  불가피하면 AES-256-GCM으로 암호화하고 키는 환경변수에서 로드
- 모든 개인정보 전송 구간은 HTTPS 강제
- 암호화 키/시크릿은 코드에 하드코딩하지 말고 .env로 분리(.gitignore 포함)
- 근거: 개인정보의 안전성 확보조치 기준 제7조(개인정보의 암호화)
```

3. 이런 규칙은 **스킬 파일/룰 파일**(예: `AGENTS.md`, `.cursor/rules/`)에 넣어 개인정보를 다룰 때 항상 참조하도록 한다.
4. **법·고시는 계속 개정**되므로 AI의 기억에만 의존하지 말고, 현행 원문을 함께 제공한다:

```text
다음은 현재 시행 중인 개인정보 안전성 확보조치 기준 제7조 원문이야.
네 사전 지식보다 이 원문을 우선해서 판단해줘.
<원문 붙여넣기>
```

---

## 11. 마무리

- 개인정보를 다루는 서비스라면 **규모와 무관**하게 암호화 의무를 준수해야 하며, **AI 생성**은 면책 사유가 아니다.
- 요약:
  - 비밀번호 → **안전한 단방향 해싱(bcrypt/Argon2)**
  - 필수 7종 → **안전한 알고리즘으로 저장·전송 암호화(AES-256-GCM)** + **키 분리**
  - 카드·계좌 → **PG사 위임**, 주민번호 → **본인확인기관(CI/DI)**
  - 전송 구간 → **HTTPS 필수**
  - 최선의 방어 → **불필요한 개인정보는 애초에 저장하지 않기**

---

## 부록. 참고

- 국가법령정보센터: https://www.law.go.kr/
- 개인정보보호위원회: https://www.pipc.go.kr/
- 안내서 기준: *개인정보의 안전성 확보조치 기준 안내서(2025년 11월판)* — 시행 전 현행 고시 원문 재확인 권장.
- OWASP Password Storage Cheat Sheet, OWASP Transport Layer Security Cheat Sheet (해싱·TLS 실무 기준 참고).

> 본 문서는 개발 실무 참고 자료이며, 구체적 사안에 대한 법적 판단은 전문가의 자문을 받으시기 바랍니다.
