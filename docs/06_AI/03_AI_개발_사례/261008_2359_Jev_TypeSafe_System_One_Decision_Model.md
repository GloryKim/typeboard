# Jev 사례로 본 생성형 AI와 판단 전용 모델의 역할 분리

## 요약

Jev는 샌프란시스코 스타트업 TypeSafe AI가 2026년 9월 15일 얼리 액세스로 공개한 AI 모델이다. 회사가 "System One"이라고 이름 붙인 새 범주의 첫 모델이다. LLM처럼 자연어를 이해하지만 문장을 쓰지 않는다. 프로그램이 상황(state)과 원하는 답의 형태(질문)를 보내면 선택지 하나, 점수, 또는 "예"일 확률만 돌려준다. 발표 수치는 응답 70~500ms, 입력 100만 토큰당 $0.042, 출력 토큰 무료이며, 같은 판단 작업에서 LLM보다 40~200배 빠르다고 한다. 공동 창업자 겸 CEO Diogo Almeida는 OpenAI에서 약 4년간 RLHF, InstructGPT, ChatGPT 작업을 하고 2024년에 나왔다.

Jev의 의미는 성능보다 구조에 있다. LLM에게 "JSON으로만 답하라"고 부탁하고, 파싱하고, 형식이 깨지면 다시 호출하는 코드가 필요 없다. 정의한 형태 밖의 답은 구조적으로 나올 수 없다. 공개 한 달 만에 CLI 도구, PostgreSQL 확장, MCP 연동, 오픈 레시피 Kev, 한국어 특화 오픈 모델까지 생태계가 생겼다. 다만 클라우드 전용이고, 한국어에서는 같은 답을 고르고도 신뢰도가 낮게 잡히며, 형식 보장이 정답 보장은 아니다. 결론적으로 Jev는 "LLM이 모든 걸 한다"에서 "생성은 LLM, 판단은 판단 전용 모델"로 역할을 나누는 흐름의 시작점이다.

## 1. Jev 개요와 이름의 의미

TypeSafe 공식 문서는 System One 모델을 "소프트웨어가 바로 쓸 수 있는 빠르고 구조화된 결정을 내리도록 만든 AI 모델"로 정의한다. 상태를 평가해 타입이 정해진 답과 확률을 돌려주며, 답장 작성, 코드 생성, 추론 과정 설명은 하지 않는다.

이름 두 개에 회사의 방향이 담겨 있다. System One은 대니얼 카너먼의 『생각에 관한 생각』에서 왔다. 시스템 1은 빠르고 직관적인 판단을, 시스템 2는 느리고 신중한 추론을 맡는다. 최근 LLM이 추론 토큰을 길게 늘어놓으며 시스템 2 쪽으로 달려가는 동안, TypeSafe는 반대편인 "1초 안에 내리는 판단"에만 집중했다. Jev는 경제학자 William Stanley Jevons에서 왔다. 증기기관 효율이 좋아지자 석탄 소비가 줄기는커녕 폭증했다는 제번스 역설을 남긴 사람이다. 지능의 비용이 한 자릿수씩 떨어질 때마다 그만큼 새로운 쓰임새가 열린다는 베팅이다.

> 출처: [TypeSafe AI 「System One」](https://docs.typesafe.ai/concepts/system-one), [Dale Seo 「Jev: 텍스트 대신 확률을 돌려주는 TypeSafe AI의 결정 모델」](https://daleseo.com/jev/), [Ruo Lee 「How Jev differs from an LLM」](https://blog.po24lio.com/en/jev-is-not-an-llm)

## 2. LLM과의 차이

| 항목 | 일반 LLM | Jev |
| --- | --- | --- |
| 출력 | 자유 문장(JSON 모드도 결국 텍스트) | 타입이 정해진 값: 선택지, 점수, 확률 |
| 형식 보장 | 프롬프트로 부탁, 깨지면 재시도 | 정의한 형태 밖의 답이 구조적으로 불가능 |
| 응답 시간 | 3초~329초 | 70ms~500ms |
| 입력 가격 | 100만 토큰당 $0.20~$10 | 100만 토큰당 $0.042 |
| 출력 가격 | 입력의 5배 안팎 | 무료 |
| 확률 | 보통 제공하지 않음 | 보정된 확률이 기본 |

보정된(calibrated) 확률이란 Jev가 0.9라고 답한 것들을 모아 보면 실제로 약 10개 중 9개가 "예"라는 뜻이다. 다만 이것은 여러 답을 모았을 때의 성질이고 개별 답이 맞는다는 보장은 아니다.

형식이 구조적으로 보장된다는 점은 신뢰 문제에서 의미가 크다. Ruo Lee는 채용 공고를 붙여 넣으면 맞는 프로젝트를 위로 올리는 기능을 포트폴리오 사이트에 만들면서 Jev를 택했다. LLM이 공고에 맞춰 프로젝트 설명을 고쳐 쓰면 문제가 되는데, 함께 검토하던 Claude Code의 표현대로 "Jev는 문장을 쓰지 않으니 경력 사실을 지어낼 수 없다."

## 3. 세 가지 질문 타입

Jev에 던지는 질문은 세 가지 타입 중 하나다. 어떤 타입을 고를지는 코드가 그 답을 어떻게 소비할지로 정하면 된다.

| 타입 | 묻는 것 | 답 | 코드에서 대응 |
| --- | --- | --- | --- |
| Choice | 여러 선택지 중 하나(최대 255개) | `choice`, 선택지별 `probabilities`, `confidence` | `switch` 분기 |
| Score | 순서 있는 단계(2~10개) 중 어디쯤 | 소수일 수 있는 `score`, 분포, `confidence` | 임계값 비교 |
| Noul | 예 또는 아니오 | "예"일 확률 `noul`(0~1) | `if` 조건 |

한 가지 함정이 있다. Noul의 0.5는 "중간 정도"가 아니라 "예인지 아니오인지 반반"이다. "이 지원자는 Python을 잘하나"처럼 정도를 묻는 질문에 Noul을 쓰면 값을 해석할 수 없다. 정도는 Score로 재고, Noul에는 "Python 경험이 있나"처럼 경계가 분명한 질문만 맡겨야 한다. Choice의 `confidence`와 Noul의 확률도 서로 다른 숫자이므로 같은 임계값으로 다루면 안 된다.

## 4. API와 설계 원칙

### 4.1 요청 형태

모든 요청은 `POST https://api.typesafe.ai/v1/systemone` 하나로 간다. `state`에는 판단 대상 맥락을 문자열, JSON 객체, 텍스트 배열로 넣고, `questions`에는 이름을 붙인 질문들을 넣는다. 질문과 합쳐 약 64k 토큰까지 받는다.

```json
{
  "model": "jev-latest",
  "state": {
    "ticket": {
      "subject": "Duplicate subscription charge",
      "message": "I was charged twice this month. Please refund the duplicate before Friday."
    },
    "customer_plan": "pro"
  },
  "questions": {
    "department": {
      "type": "choice",
      "instructions": "Which team should handle this ticket?",
      "criteria": {
        "billing": "Payments, invoices, and refunds",
        "technical": "Bugs, outages, integrations",
        "sales": "New purchases and upgrades",
        "other": "No listed department fits"
      }
    },
    "urgency": {
      "type": "score",
      "instructions": "How urgent is the request?",
      "criteria": ["No time constraint", "Can wait this week", "Needs attention within a day", "Immediate outage or harm"]
    },
    "refund_requested": {
      "type": "noul",
      "instructions": "Does the customer explicitly request a refund?"
    }
  }
}
```

SDK는 Python(`typesafe-sdk`), JavaScript(`@typesafe-ai/sdk`, Node.js 20 이상이며 Bun에서도 동작), Rust(`typesafe-jev`)가 있고 모두 `TYPESAFE_API_KEY` 환경 변수를 읽는다. 임계값을 튜닝했다면 `jev-latest` 대신 `jev-1.13.0`처럼 버전을 고정하는 것이 권장된다.

> 출처: [Jev Manual API Reference](https://jevmanual.com/manual/api/), [docs.rs typesafe-jev](https://docs.rs/typesafe-jev/latest/typesafe_jev/), [TypeSafe AI 「Noul」](https://docs.typesafe.ai/primitives/noul)

### 4.2 추측성 팬아웃

한 요청에 담긴 질문은 모두 같은 상태를 보고 병렬로 평가된다. 질문을 하나 더 붙여도 응답 시간은 거의 그대로이고 비용은 그 질문의 토큰만큼만 늘어난다. 그래서 공식 문서는 당장 필요한지 확실하지 않은 질문까지 한 번에 던져 두는 speculative fan-out을 권한다. 공식 쿡북에서는 위키백과 문서 하나에 질문 13개를 한 요청으로 묶었을 때 따로 호출할 때보다 12.2배 싸고 10배 빨랐다고 보고한다.

### 4.3 규칙은 코드에 둔다

Jev 문서와 실측 글이 공통으로 강조하는 원칙은 결정 규칙을 프롬프트가 아니라 코드와 저장소에 두라는 것이다.

```text
임계값 이상    : 자동 실행
임계값 미만    : 사람 또는 LLM으로 넘김
그 사이 구간   : 기록하고 주기적으로 임계값 재보정
```

## 5. 실측과 한계

### 5.1 응답 시간

Dale Seo가 해외에서 10회 호출한 결과는 최소 197ms, 중앙값 298ms, 최대 531ms였다. 발표 수치 범위 안이지만 아래쪽 끝에는 닿지 못했다. 대륙을 건너는 왕복 때문이고 연결을 맺는 데만 25~113ms가 들었다. 서울에서 호출하면 더 늘어난다고 보는 것이 안전하다. 그래도 질문 세 개를 300ms 안에 받는다면 사용자가 기다리는 화면 안에서 쓰기에 무리가 없다.

### 5.2 한국어: 틀리는 것이 아니라 신뢰도가 낮게 나온다

공식 문서는 영어가 주 학습 언어이고 한중일 문자 등 다른 언어는 처리는 되지만 성능이 영어와 같지 않다고만 밝힌다. Dale Seo가 같은 뜻의 고객 문의를 한국어와 영어로 만들어 비교한 결과는 다음과 같다.

| 문의 | 부서(한국어/영어) | 신뢰도(한국어/영어) | 불만 점수(한국어/영어) |
| --- | --- | --- | --- |
| 중복 결제 | billing / billing | 1.00 / 1.00 | 0.17 / 0.09 |
| 연동 실패 | technical / technical | 0.45 / 0.65 | 1.00 / 1.00 |
| 완곡한 불만 | technical / technical | 0.26 / 0.74 | 0.05 / 0.73 |

명확한 문의는 차이가 없지만 애매한 문의에서는 같은 답을 고르고도 한국어 쪽 신뢰도가 크게 낮았다. 감정 신호는 특히 약하게 읽어서, 영어 "I'm not happy with the fit"은 불만 0.73인데 한국어 "사이즈가 안 맞네요"는 0.05였다. 반면 긴급도를 묻는 Noul은 다섯 쌍 모두 차이가 0.01 안팎이었다. 영어 기준 임계값을 그대로 쓰면 맞게 분류한 한국어 건도 신뢰도 미달로 사람에게 넘어가 처리량이 떨어진다. 임계값은 언어별로 따로 잡고, 감정과 어조를 재는 Score는 한국어 표본으로 다시 보정해야 한다.

반대 사례도 있다. Ruo Lee가 한국어와 영어 채용 공고로 프로젝트 14개를 Score로 평가했을 때 순위 상관계수는 0.89~1.00으로 거의 같았다. 작업 종류에 따라 언어 격차가 다르다는 뜻이다.

### 5.3 그 밖의 한계

형식 보장은 정답 보장이 아니다. 선택지 밖의 답은 나오지 않지만 선택지 안에서 틀린 답은 나올 수 있다. 클라우드 API로만 제공되므로 데이터를 외부로 보낼 수 없는 환경에서는 쓸 수 없고, 아직 대기자 명단 단계다. 흥미로운 성질도 관찰됐다. 개수를 세게 하면 틀리기 한참 전부터 신뢰도를 끌어내리며 헤매고 있다는 신호를 보낸다. "틀릴 수 있다"를 감수하는 대신 "틀릴 것 같다고 미리 말해 준다"를 얻는 셈이다.

## 6. 생태계: 공개 한 달 만에 생긴 것들

### 6.1 개발 도구

jevtool은 "판단력 있는 유닉스 도구"를 표방한다. 의미로 코드를 찾는 jevgrep, 정규식으로 못 잡는 규칙을 CI에서 검사하는 jevlint, 로그의 심각도와 긴급도를 판정하고 같은 원인 줄을 묶는 jevtail, 설명한 기준으로 정렬하는 jevsort, 단어만 바뀌었는지 의미가 바뀌었는지 판정하는 jevdiff, Claude Code와 Cursor에 `judge` 도구를 주는 jev-mcp로 구성된다. David Zhang의 jevgrep은 코딩 에이전트에게 관련 파일을 짚어 주는 CLI로, SWE-bench 10개 중 같은 8개를 풀면서 비용을 약 30% 줄였다고 보고했다.

pg-jev는 PostgreSQL 14~17용 확장으로 SQL에서 자연어 조건을 쓰게 한다. `jev(row, condition, threshold)`는 `WHERE` 절의 불리언 조건으로, `jev_prob`는 확률로, `jev_choice`와 `jev_score`는 선택과 점수로 답한다. 배치 크기가 커질수록 정확도가 떨어져 기본값 20에서 100%, 40에서 92~98%였다고 한다.

> 출처: [jevtool](https://jevtool.dev/), [JevHub jevgrep](https://jevhub.io/builds/jevgrep), [pg-jev](https://aibriefing.imbch.dev/en/article/9835471)

### 6.2 오픈 레시피와 한국어 모델

Jared Palmer의 Kev는 System One 방식을 재현한 Apache-2.0 오픈 레시피이며 `/v1/systemone` 호환 서버를 제공한다. 이를 바탕으로 한국어 모델 두 개가 나왔다. Malkuth(4B/2B)는 GeekNews Show GN으로 소개된 다국어·한국어 분류 특화 오픈 웨이트 모델로, Kev 체크포인트를 post-training했다. 다키클라우드의 K-Decision(`kd-4b-ko-v0`)은 Qwen3.5-4B-Base에 LoRA rank 16과 포인터 헤드를 얹어 Kev 레시피를 재현했고, 한국어 법령과 행정 문서, 온프레미스 운영을 목표로 한다.

Malkuth 개발자가 공개 벤치마크 29개 묶음(약 12만 9천 문항)으로 비교한 결과는 다음과 같다.

| 모델 | 정확도(held-out) | Brier 점수(낮을수록 좋음) |
| --- | --- | --- |
| Jev API | 0.754 | 0.355 |
| Malkuth-4B | 0.724 | 0.385 |
| Malkuth-2B | 0.703 | 0.412 |
| Kev-9B | 0.700 | 0.407 |
| Kev-4B | 0.686 | 0.407 |

Jev가 여전히 가장 높지만 4B 오픈 모델이 3%p 차이까지 따라왔다. 모든 모델이 과신하는 경향이 있었고 보정 편차는 Jev가 0.066~0.088로 가장 좁았다. Malkuth는 2B가 4B보다 보정이 더 잘 되어 있었다.

> 출처: [Show GN Malkuth](https://it.peoplentools.com/show-gn-malkuth-%EB%8B%A4%EA%B5%AD%EC%96%B4-%ED%95%9C%EA%B5%AD%EC%96%B4-%EB%B6%84%EB%A5%98-%ED%8A%B9%ED%99%94-jev-like-system-one-model-4b-2b-324425.html), [jevlist Malkuth](https://jevlist.ai/projects/malkuth), [다키클라우드 「Jev 한국판을 만든 이유」](https://thakicloud.com/tech-blog/ko/research/k-decision-jev-korean-comparison/)

## 7. 어디에 맞고 어디에 맞지 않는가

```text
잘 맞는 곳 : 결과가 분기·필터·정렬·임계값으로 바로 이어지는 판단
             고객 문의 분류, 로그 심각도 판정, 콘텐츠 검수(개인정보 포함 여부),
             채용 공고와 이력서 매칭, CI의 비정규식 규칙 검사, DB 행의 자연어 필터링,
             학습 데이터셋 품질 필터링, 코딩 에이전트의 파일 탐색 보조
안 맞는 곳 : 답장 작성, 요약, 코드 생성, 이유 설명 — 여전히 LLM의 일
```

실제로는 두 종류의 모델을 조합하는 그림이 자연스럽다. 판단 전용 모델이 먼저 빠르고 싸게 분류하고, 신뢰도가 낮은 건만 LLM이나 사람에게 넘기는 방식이다.

## 8. 판단 모델 도입 점검표

| 영역 | 점검 항목 |
| --- | --- |
| 작업 적합성 | 결과가 생성이 아니라 분기·필터·정렬·임계값으로 소비되는가? |
| 질문 타입 | 정도를 묻는 질문에 Noul이 아니라 Score를 쓰고 있는가? |
| 규칙 위치 | 임계값과 결정 규칙이 프롬프트가 아니라 코드와 저장소에 있는가? |
| 언어 보정 | 한국어 등 비영어 데이터에 대해 언어별 임계값을 따로 잡았는가? |
| 버전 고정 | 임계값을 튜닝했다면 모델 버전을 고정했는가? |
| 요청 묶기 | 같은 상태에 대한 질문을 한 요청으로 묶어 비용과 지연을 줄였는가? |
| 지연 예산 | 국내에서 호출할 때의 실제 왕복 시간을 측정했는가? |
| 데이터 경계 | 외부 전송이 불가능한 데이터라면 온프레미스 오픈 모델을 검토했는가? |
| 에스컬레이션 | 신뢰도 미달 건을 사람이나 LLM에게 넘기는 경로가 있는가? |
| 정답 검증 | 형식 보장과 별개로 표본 데이터로 정확도를 측정하고 있는가? |

## 결론

Jev는 "LLM에게 JSON으로만 답하라고 부탁하는" 관행에 대한 구조적인 답이다. 문장을 쓰지 않는 모델은 형식이 깨질 수 없고, 사실을 지어낼 수도 없으며, 판단마다 보정된 확률을 붙여 코드가 임계값으로 결정을 내리게 한다. 빠르고 싸다는 장점은 판단을 넣을 수 있는 곳을 넓힌다. 이름처럼 판단의 비용이 떨어지면 판단의 수요는 폭발할 수 있다. 그러나 형식 보장은 정답 보장이 아니고, 한국어에서는 신뢰도가 낮게 잡히며, 클라우드 전용이라는 제약도 분명하다. 공개 한 달 만에 오픈 레시피와 한국어 온프레미스 모델이 3%p 차이까지 따라온 것은 이 범주 자체가 빠르게 일반화될 것임을 보여준다. AI를 소프트웨어에 넣는 방식은 하나의 거대한 생성 모델에서, 생성과 판단을 나누어 각각에 맞는 모델을 쓰는 쪽으로 옮겨 가고 있다.

> 출처: [TypeSafe AI 문서](https://docs.typesafe.ai/concepts/system-one), [Dale Seo 블로그](https://daleseo.com/jev/), [준이아빠블로그 「Jev AI 모델: 선택지와 확률, 신뢰도 해석과 분류 연습」](https://www.digitalmarketer.co.kr/insights/jev-typesafe-system-one-model), [Promptating 「Meet 'Jev'」](https://promptating.com/en/articles/2026-09-24-6/)
