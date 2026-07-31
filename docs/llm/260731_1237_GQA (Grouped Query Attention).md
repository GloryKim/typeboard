# GQA (Grouped Query Attention) 완전판

## 출처 : https://www.youtube.com/watch?v=vEKhXxSlclY

> **단일 합본 파일.** 영상 대본 + 스터디 본문 + Python 원문 + 보충 설명을 모두 이 파일에 담았다.  
> 재검토(2026-07-31): 목차에만 있던 **영상 대본(PART 00) 누락을 복구**하고,  
> Q/K/V·공유/α·품질 유지 이유·decode 흐름·코드 해설·타임라인 매핑을 **PART 09~11**로 보강했다.

## 목차

1. [한 줄 요약 · 출처 · 실행법](#part-0-한-줄-요약--출처--실행법)
2. [영상 대본 (원본)](#part-00-영상-대본-원본)
3. [01. 개요 및 로드맵](#part-01-개요-및-로드맵)
4. [02. Scaled Dot-Product Attention](#part-02-scaled-dot-product-attention)
5. [03. Multi-Head Attention](#part-03-multi-head-attention)
6. [04. GQA 개념과 설계](#part-04-gqa-개념과-설계)
7. [05. KV Cache](#part-05-kv-cache--왜-gqa가-이득인가)
8. [06. MHA · MQA · GQA 스펙트럼](#part-06-mha--mqa--gqa-스펙트럼)
9. [07. 중요점 체크리스트](#part-07-중요점-체크리스트)
10. [08. 파생 MLA](#part-08-파생--mla와-그-다음)
11. [코드: attention_variants.py](#part-code-a-attention_variantspy)
12. [코드: kv_cache_estimate.py](#part-code-b-kv_cache_estimatepy)
13. [코드: run_demo.py](#part-code-c-run_demopy)
14. [requirements.txt](#part-requirements)
15. [보충·심화 설명](#part-09-보충-심화-설명)
16. [코드 라인별 해설](#part-10-코드-라인별-해설)
17. [영상 타임라인 ↔ 본문 매핑](#part-11-영상-타임라인-매핑)

---

<a id="part-0-한-줄-요약--출처--실행법"></a>

## PART 0. 한 줄 요약 · 출처 · 실행법

> 출처 파일: `README.md`

# GQA (Grouped Query Attention) Study

영상 대본(`00_영상_대본.md`)을 근간으로, Attention → Multi-Head → **GQA** 흐름과  
파생 개념(**MQA, KV Cache, MLA**)까지 정리한 스터디 노트.

## 한 줄 요약

```text
Attention     : Q·K로 가중치 → V의 가중합
MHA           : head마다 다른 관점(서로 다른 Q/K/V)
GQA           : Q head는 많고, K/V head는 그룹으로 공유
                → 성능 ≈ MHA, KV cache는 그룹 수만큼 축소
MQA           : K/V head = 1 (GQA의 극단)
MLA (DeepSeek): K/V를 저차원 latent로 압축 캐시 (공유가 아닌 압축)
```

## 이 파일 안에서의 구성 (원래 분산 파일 ↔ PART)

| # | 원래 파일 | 이 완전판 PART |
|---|-----------|----------------|
| 00 | `00_영상_대본.md` | PART 00 |
| 01 | `01_개요_및_로드맵.md` | PART 01 |
| 02 | `02_Scaled_Dot_Product_Attention.md` | PART 02 |
| 03 | `03_Multi_Head_Attention.md` | PART 03 |
| 04 | `04_GQA_개념과_설계.md` | PART 04 |
| 05 | `05_KV_Cache와_왜_중요한가.md` | PART 05 |
| 06 | `06_MHA_MQA_GQA_스펙트럼.md` | PART 06 |
| 07 | `07_중요점_체크리스트.md` | PART 07 |
| 08 | `08_파생_MLA와_다음.md` | PART 08 |
| code | `attention_variants.py` 등 | PART CODE A~C |
| 보강 | (완전판 전용) | PART 09~11 |

## 코드 실행 (폴더의 `.py`와 동일 내용이 아래 PART CODE에 수록됨)

```bash
cd /Users/async/glory/research/260725_1215_GQA
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python code/run_demo.py
```

의존성: Python 3.9+, `numpy` (`requirements.txt`).

## 추천 경로 (이 파일 기준)

1. 개념: PART 01 → 02 → 03 → 04  
2. 왜 쓰나: PART 05 → 06 → 07  
3. 보충: PART 09 (특히 §9.2~9.4)  
4. MLA 예고: PART 08  
5. 손/코드: PART CODE + PART 10  
6. 영상 따라가기: PART 00 + PART 11

## 핵심 출처

| 주제 | 출처 |
|------|------|
| GQA 논문 | Ainslie et al., 2023 — [arXiv:2305.13245](https://arxiv.org/abs/2305.13245) |
| MQA | Shazeer, 2019 — Fast Transformer Decoding |
| Attention / MHA | Vaswani et al., 2017 — Attention Is All You Need |
| MLA | DeepSeek-V2 Technical Report |


---

<a id="part-00-영상-대본-원본"></a>

## PART 00. 영상 대본 (원본)

> 출처 파일: `00_영상_대본.md`  
> 타임스탬프가 포함된 자막 전문. 아래 스터디 본문의 근간이다.

```text
0:00
이번 영상에서는요
0:01
GQA,
0:02
Group query attention을 설명할 겁니다
0:04
GQA가 어떤 맥락에서 제안된 개념인지 설명을 위해서
0:07
attention이랑 Multihead attention도 같이 이야기합니다
0:11
그래서 이 영상을 쭉 보고나면
0:13
무슨 생각으로 GQA 개념을 떠올리게 됐는지
0:16
이해하실 수 있을 겁니다
어텐션의 기본 개념
0:18
attention의 개념부터 보면요
0:20
query와 key sequence로 내적을 통해서
0:23
attention score를 얻습니다
0:24
수식으로는 Q와 Kᵀ의 내적인데요
0:27
Q, query는 실수 tensor 길이 by dimension
0:31
K, key도 실수 tensor 길이 by dimension
0:34
그러면 attention 결과는,
0:35
어떻죠?
0:36
실수 tensor 길이 by 길이 입니다
0:38
조금만 더 자세히 보면
0:40
방금 attention score가 길이 by 길이라고 했죠?
0:43
즉, 이 화면에서는요
0:44
각 점들이 길이 by 길이로 늘어져있고
0:47
이 점들은 하나의 숫자라고
0:49
생각하시면 됩니다
0:50
그러면 점의 표현은 알았고,
0:52
하나만 더 자세히 볼까요?
0:54
네모로 표현한 sequence 샘플 하나
0:56
이 샘플 하나는
0:57
실수 값이 dₖ개 있는 벡터입니다
1:00
이 네모 하나는요
1:01
숫자들이 늘어져 있는데
1:03
몇 개?
1:03
dₖ개 늘어져있는 벡터입니다
1:06
그러면 attention score 계산도
1:08
자세히 한 샘플로 한 번 들여다볼게요
1:10
QKᵀ 내적은 모든 query, key 네모 쌍 경우에 대해
1:14
각각 이루어진다고 보시면 됩니다
1:16
아까 각 네모는 벡터라고 했죠?
1:19
여기서 key에는 ᵀ,
1:21
transpose가 들어가고
1:22
그 다음 내적을 한다는 말은요
1:24
벡터의 각 위치별 쌍끼리 곱해서
1:27
더한다는 의미입니다
1:28
여기 수식을 보면 길게 생겼지만요
1:30
이걸 잘 생각해보면
1:32
곱하고 더하고 곱하고 더하고 하니까
1:35
결국 숫자 하나입니다
1:37
그리고 query와 key vector는요
1:39
"두 벡터의 정보가 얼마나 관련되어있는지"
1:42
라는 숫자 하나를 만드는 정보들입니다
1:46
그 뒤에는 수치 안정성을 위해
1:48
고수같은 sqrt dₖ로 나누고요
1:51
가중합으로 만들기 위해 softmax를 합니다
1:54
그 다음에 value sequence와
1:56
다시 내적을 하는데요
1:57
이게 논문에서 볼 수 있는 그 attention 수식이죠?
2:00
잘 살펴보면
2:01
V의 앞까지는 계산이 복잡하긴 했지만요
2:04
가중치를 만드는 과정
2:05
그 뒤에는 그냥 value만 붙어서
2:07
결과적으로는 value의 가중치 합,
2:09
그 가중치 합은
2:10
Q, K의 내적으로 만들어지는
2:12
value의 가중치 합이
2:13
attention 모듈의 출력입니다
2:16
여기서 마지막 내적은요
2:18
이번엔 sequence 길이를 축으로
2:20
각 위치별로 곱한 다음
2:21
서로 더합니다
2:23
이 attention 출력 결과를
2:24
조금 더 풀어서 표현하면요
2:26
아까 말했듯이
2:27
점들은 하나의 숫자들이죠?
2:29
그리고 softmax 과정을 통해서
2:30
key 축에 대해서 합이 1로 맞춰져있습니다
2:34
그 다음 내적의 결과는요
2:35
방금 말했듯이
2:36
각 위치별로 곱한 다음
2:38
서로 더한 모습입니다
2:40
바로 이때 만들어진 attention 출력은
2:42
무슨 shape일까요?
2:43
이번에도 잘 보면 좀 길긴 하지만요
2:46
한 줄이 결국 하나의 네모,
2:48
dₖ개의 실수인 벡터입니다
2:50
그래서 attention 출력 shape는요
2:52
이 벡터가 L개 sequence로 구성돼서
2:55
L개의 dₖ 벡터입니다
멀티헤드 어텐션 설명
2:58
attention은 봤고,
3:00
GQA로 넘어가기 위해서요
3:01
그 사이 단계,
3:02
Multihead attention을 보겠습니다
3:04
지금 보고있는 건
3:05
아까 attention 출력, 그 중에서
3:07
한 줄만 나타낸 건데요
3:09
아까 말했듯이
3:10
네모는 벡터를 표현하고 있습니다
3:12
이 벡터는 각각 문장을 이루는
3:15
subword 정보를 내포하고 있습니다
3:17
그런데 가중치를 곱한다는 말은요
3:19
엄청 낮은 가중치인 위치에서는
3:21
해당 정보를 사실상 버리게 됩니다
3:24
하지만 글을 읽을 때처럼
3:26
sequence 분석을 할 때를 생각해보면요
3:28
이 sequence를 이해할 때는
3:29
정보의 한 측면으로만 판단하지는 않죠?
3:32
이렇게 여러 측면의 가중치를 고려할 수 있어야 합니다
3:36
바로 이 발상으로 Multihead attention이 동작합니다
3:39
벡터 하나를 한 측면의 정보로만 보지 않고요
3:42
여러 관점이 반영되어있다고 보려는 의도입니다
3:45
이렇게 나눠진 관점 하나하나가
3:47
head인데요
3:48
각 head별로 attention 가중치가 있으면,
3:50
어떻죠?
3:51
head별 관점 정보마다
3:53
서로 다른 위치의 샘플에 가중치를 둘 수 있습니다
3:56
지금 보는 화면처럼
3:57
multihead로 sequence를 분석했다고 하면요
3:59
어떤 관점에서는 meaning과 물음표에 초점을 두고,
4:03
또 어떤 관점에서는 meaning의 대상인 piui에 초점을 두고
4:07
또 어떤 관점에서는 is에 초점을 둬서
4:10
문장을 분석했다고 볼 수 있습니다
4:12
이 결과는 합쳐지더라도요
4:14
여전히 아까 attention 출력의 한 샘플처럼
4:16
하나의 vector,
4:18
dₖ개의 실수인데요
4:19
하지만 내재적으로는 어떻죠?
4:21
그냥 attention이랑은 좀 다른 구성입니다
4:24
각 head의 덩어리별로
4:25
다양한 맥락 관점을 가진 vector라고
4:27
해석할 수 있습니다
4:29
그런데요,
4:30
잘 생각해보면 좀 이상합니다
4:32
아까 attention 출력은 value의 가중치 합,
4:35
여기서 value는 나눠진다고 표현은 했지만요
4:38
보기에만 그렇고 실제로는
4:39
다 합쳐서 dₖ개 숫자 벡터 그대로입니다
4:43
이건 괜찮지만 문제는 가중치인데요
4:46
아까는 하나의 가중치였는데
4:47
갑자기 자연스럽게 여러 개 가중치가 있다고 설명했습니다
4:52
이 가중치는 조상님이 만들어줄까요?
4:54
아니죠?
4:54
잘 설계해서 만들어야합니다
4:56
어떻게 설계됐는지 볼게요
4:58
가중치를 만드는 재료인 query와 key로 돌아가겠습니다
5:01
처음에 그냥 attention에서는요
5:03
query와 key를 내적한다고 했습니다
5:06
그런데 multihead attention에서는요
5:08
이 query와 key도 head 수만큼 분리합니다
5:11
분리한 query, key를 각각 내적을 하면요
5:14
head 수만큼의 attention score를 얻을 수 있습니다
5:18
조금 더 자세히 볼게요
5:20
각 네모가 표현하는 벡터를
5:21
아까 value처럼 분리해서 생각합니다
5:24
이렇게 분리된 벡터들끼리 내적을 하면요
5:27
분리된 개수만큼,
5:28
즉 head 수만큼의 attention score를 얻습니다
5:31
이런 query, key 분리는
5:33
아까 말한 multihead attention의 의도랑도 맞죠?
5:36
빨간 head의 query와 key가
5:38
한 분석 관점의 관련도 가중치,
5:40
초록 head의 query key가
5:41
또 다른 분석 관점의 가중치
5:43
파란 head의 query key가
5:45
또 다른 분석 관점의 가중치
5:47
이렇게 서로 분리해서
5:48
별개 관점의 관련도 가중치를 만드니까요
5:51
결과적으로 아까 설명한 분리해둔 value랑
5:54
가중치합을 했을 때,
5:56
어떻죠?
5:56
head 개수 H개의 dₖ/H 개수의 실수 벡터가
6:01
H개 있으니까
6:02
그 shape는 dₖ 벡터랑 일치하죠?
6:05
하지만
6:05
내재적으로는 이 multihead attention이요
6:07
그냥 attention에 비해서 H개만큼 다양한 관점으로
6:11
attention을 진행한 셈입니다
6:13
그리고 전체 출력을 봤을 때는요
6:15
sequence를 H개의 맥락 관점에서 분석했다고
6:18
해석할 수 있습니다
6:20
지금까지 attention이랑
6:21
Multihead attention의 차이를 알아봤죠?
Group Query Attention(GQA)의 개념과 설계
6:25
이제 Group query attention 차례입니다
6:27
attention 고수들은 Multihead에서
6:29
한 번 더 건방진 상상을 했습니다
6:31
지금까지 봤던 흐름을 다시 떠올려보면요
6:34
multihead를 안 쓰면
6:35
단일 관점으로 sequence를 분석,
6:37
multihead를 쓰면
6:39
여러 관점으로 sequence를 분석한다고 했죠?
6:41
이 때 여러 관점을 본다는 걸
6:44
vector에서 head를 나눈다는 것으로 구현했습니다
6:47
그런데 여기서
6:47
고수들이 한 상상은요
6:49
관점이 여러 개인 건 그대로인데
6:51
' 이 각 관점마다도 여러 번 고려하는 것이 가능하다 '
6:54
라는 생각입니다
6:56
이게 무슨 말일까요?
6:57
한 번 볼게요
6:58
아까 multihead attention에서는요
7:00
query를 H개로 나누고
7:02
key랑 value도 똑같이 H개로 나눠서
7:05
같은 색 head끼리 짝지어서 내적했습니다
7:08
빨간 query는 빨간 key랑
7:10
초록 query는 초록 key랑
7:11
뭐 이렇게요
7:12
그런데 GQA에서는
7:14
이 개수를 서로 다르게 둡니다
7:16
지금 화면의 상황이면
7:18
query head는 4개인데
7:19
key value head는 2개로 둡니다
7:21
이러면 짝이 안 맞죠?
7:22
이때는요
7:23
query head 두 개가
7:24
하나의 key value head를 공유합니다
7:27
이걸 관점이라는 표현으로 다시 풀어보면요
7:30
multihead attention 이야기할 때
7:32
key value head 하나가
7:33
하나의 분석 관점이라고 했었죠?
7:35
그러면 query 두 개가
7:37
key value 하나를 공유한다는 말은요
7:39
같은 하나의 관점을 기준으로
7:41
이 관점의 특성을 두 번 들여다본다는 의미입니다
7:44
한 관점에서 한 번 보고 끝내는 게 아니라
7:47
같은 관점에서 query를 달리해서
7:49
여러 번 비교해보는 거죠?
7:51
GQA의 연구자들은 이런 상상을 한 겁니다
7:53
' 같은 관점에서 특성을 여러 번 분석하면
7:56
그것도 성능에 도움이 되지 않을까? '
7:58
이런 발상이 GQA의 의도입니다
8:01
그러면 성능은 어느 정도일 것 같나요?
8:03
직관적으로 생각해보면요
8:05
관점 자체는 2개밖에 없으니까
8:07
관점이 4개인 multihead attention보다는 못하고
8:09
관점이 1개인 그냥 attention보다는
8:12
좀 나을 것 같은 느낌도 들죠?
8:14
그러니까 둘 사이 어딘가의 성능일 거라고
8:17
예상하게 됩니다
GQA의 성능 및 마무리
8:19
그런데 막상 결과를 보면
8:20
조금 다릅니다
8:21
GQA 성능은요
8:23
multihead attention이랑 거의 비슷하다고 합니다
8:26
관점 수를 줄였는데도
8:28
성능은 별로 안 떨어진 거죠?
8:30
그런데 이 때 multihead attention에 비해서
8:32
이득이 하나 더 있는데요
8:33
key랑 value head 개수가 줄었으니까
8:35
계산 과정에서 key value의 크기가 작아집니다
8:39
그러면 어떻죠?
8:40
kv cache 크기가 작아져서요
8:42
메모리를 덜 쓰면서 돌릴 수 있습니다
8:44
그래서 성능은 multihead attention급으로 유지하면서
8:47
메모리는 아끼는,
8:48
유리한 모델 설계가 가능합니다
8:51
그런데 여기서 한 발 더 나가서
8:53
중국의 어텐션 장인들은요
8:55
GQA 말고도
8:56
MLA라는 방법을 만들었습니다
8:58
사실 그냥 어텐션 장인이라고 하면 별 거 아니어보이는데요,
9:01
앞에 중국인이 붙으면...
9:03
확실히 너무 무섭죠?
9:08
이 중국인 어텐션 장인들이 만든 MLA를
9:11
다음 영상에서 알아보겠습니다
```


---

<a id="part-01-개요-및-로드맵"></a>

## PART 01. 개요 및 로드맵

> 출처 파일: `01_개요_및_로드맵.md`

## 01. 개요 및 로드맵

> 근간: 영상 대본 — *“무슨 생각으로 GQA 개념을 떠올리게 됐는지”*  
> 보강: Ainslie et al. (2023) GQA 논문, MQA/KV cache 실무 맥락

---

### 1. 영상이 말하려는 한 줄기

```text
Attention
  └─ Q·K 관련도로 V를 가중합한다

Multi-Head Attention (MHA)
  └─ head를 나눠 “여러 관점”으로 가중합한다

Grouped Query Attention (GQA)
  └─ 관점(K/V head)은 줄이고,
     같은 관점을 query 여러 개로 여러 번 들여다본다
  └─ 성능은 MHA에 가깝고, KV 메모리는 줄어든다
```

영상은 **수식 암기보다 발상 순서**를 강조한다.

1. Attention이 뭔지  
2. 왜 Multi-head가 필요한지  
3. Multi-head에서 한 발 더 나간 상상이 GQA인지  
4. 직관(“관점 줄이면 성능도 줄겠지”)과 실험(“거의 안 줄어든다”)의 간극  
5. 실익은 **KV cache 축소**

---

### 2. 왜 지금 GQA를 알아야 하나

현대 LLM **추론(decode)** 의 병목은 종종 “행렬곱 FLOPs”가 아니라  
**과거 토큰의 Key/Value를 메모리에서 읽어오는 대역폭**이다.

| 단계 | 하는 일 | 메모리 이슈 |
|------|---------|-------------|
| Prefill | 프롬프트 전체 attention | 계산량 큼 |
| Decode | 토큰 1개씩 생성, 과거 KV 재사용 | **KV cache가 선형으로 성장** |

MHA는 head마다 K/V를 따로 캐시한다 → 컨텍스트·배치가 커지면 VRAM이 먼저 터진다.  
GQA는 **query head 수는 유지**하고 **K/V head만 줄여** 캐시를 줄인다.

대표 채택: Llama 2 70B, Llama 3 (예: 64 query heads / 8 KV heads), Mistral, Gemma 등.

---

### 3. 용어를 미리 고정

| 기호 | 의미 |
|------|------|
| \(L\) / \(T\) | sequence length (토큰 수) |
| \(d_k\) | key/query 한 벡터 차원 (영상: “네모 하나에 \(d_k\)개 실수”) |
| \(d_model\) | 모델 히든 차원 |
| \(H\) / \(n_q\) | **query** head 수 |
| \(G\) / \(n_{kv}\) | **key/value** head(그룹) 수 |
| group size | \(H / G\) — 한 KV head를 공유하는 query head 수 |

**스펙트럼 (논문 정의):**

```text
GQA-1  = MQA   (KV head 1개, 모든 Q가 공유)
GQA-G  = 일반 GQA
GQA-H  = MHA   (그룹 수 = query head 수 → 공유 없음)
```

---

### 4. 영상 직관 vs 논문 정의 (같이 보면 좋음)

| | 영상 비유 | 논문/구현 |
|--|-----------|-----------|
| MHA | head마다 “분석 관점” | head마다 독립 \(W^Q, W^K, W^V\) |
| GQA | 한 관점을 query 여러 번으로 재검토 | 여러 Q head가 **동일 K/V head**에 attend |
| 이득 | “메모리를 덜 씀” | KV cache 크기 \(\propto n_{kv}\) (query head 수와 무관하게 축소) |
| 성능 | MHA와 거의 비슷 | 논문: uptrained GQA ≈ MHA quality, speed ≈ MQA |

영상에서 “관점 = K/V head”로 말하는 부분은 **학습용 비유**로 매우 잘 맞는다.  
구현에서는 “관점”보다 **캐시에 몇 개의 K/V를 쌓느냐**가 핵심이다.

---

### 5. 읽기 순서

| 순서 | 문서 | 끝나면 할 수 있는 것 |
|------|------|----------------------|
| 1 | `02` Attention | \(QK^\top/\sqrt{d_k}\) → softmax → \(V\) shape 설명 |
| 2 | `03` MHA | head split / concat 의미 |
| 3 | `04` GQA | \(n_q=4, n_{kv}=2\) 짝짓기 |
| 4 | `05` KV cache | 왜 GQA가 “서빙”에서 필수인지 |
| 5 | `06`–`07` | MQA와의 위치, 체크리스트 |
| 6 | `08` | 영상 예고 MLA로 연결 |

코드는 어느 시점이든 `python code/run_demo.py`로 shape를 눈으로 확인.

---

### 6. 이 스터디에서 “성공”의 기준

다음을 **외우지 않고 설명**할 수 있으면 충분하다.

1. Attention 출력은 결국 **V의 가중합**이다.  
2. MHA는 그 가중합을 **여러 head(관점)** 로 한다.  
3. GQA는 Q는 head 많고 K/V는 **그룹 공유**한다.  
4. 그래서 **품질은 MHA급, KV 메모리는 \(H/G\)배 절약**에 가깝다.  
5. MLA는 “공유”가 아니라 **latent 압축**이다 (다음 단계).

---

<a id="part-02-scaled-dot-product-attention"></a>

## PART 02. Scaled Dot-Product Attention

> 출처 파일: `02_Scaled_Dot_Product_Attention.md`

## 02. Scaled Dot-Product Attention

> 영상 구간: “어텐션의 기본 개념” (~0:18–2:55)  
> 공식: Attention Is All You Need (Vaswani et al., 2017)

---

### 1. 한 줄

\[
\mathrm{Attention}(Q, K, V) = \mathrm{softmax}\!\left(\frac{QK^\top}{\sqrt{d_k}}\right) V
\]

- **앞부분** \(QK^\top / \sqrt{d_k}\) + softmax → **어디에 얼마나 볼지(가중치)**  
- **뒷부분** × \(V\) → **그 가중치로 Value를 섞은 결과**

영상 표현: *“V의 앞까지는 가중치를 만드는 과정, 그 뒤에는 value만 붙어서 가중합.”*

---

### 2. Tensor shape (영상과 동일)

입력 (배치 무시, 단일 sequence):

| | shape | 영상 표현 |
|--|-------|-----------|
| \(Q\) | \(L_q \times d_k\) | “길이 by dimension” |
| \(K\) | \(L_k \times d_k\) | 동일 |
| \(V\) | \(L_k \times d_v\) | 보통 \(d_v = d_k\) |
| \(QK^\top\) | \(L_q \times L_k\) | “길이 by 길이”, 각 점은 스칼라 score |
| softmax 후 | \(L_q \times L_k\) | **key 축으로 합=1** |
| 출력 | \(L_q \times d_v\) | “\(L\)개의 \(d_k\) 벡터” |

Self-attention이면 보통 \(L_q = L_k = L\), \(d_v = d_k\).

---

### 3. 한 칸의 score가 만들어지는 과정

영상: 네모 하나 = \(d_k\)개 실수로 된 벡터.

쿼리 위치 \(i\)의 벡터 \(q_i \in \mathbb{R}^{d_k}\),  
키 위치 \(j\)의 벡터 \(k_j \in \mathbb{R}^{d_k}\):

\[
\mathrm{score}_{ij} = q_i \cdot k_j = \sum_{t=1}^{d_k} q_{i,t}\, k_{j,t}
\]

행렬로는 \(QK^\top\)의 \((i,j)\) 성분.  
의미(영상): *“두 벡터의 정보가 얼마나 관련되어 있는지”를 숫자 하나로.*

---

### 4. \(\sqrt{d_k}\)로 나누는 이유

\(d_k\)가 크면 내적 분산이 커져 softmax가 **한쪽에 쏠리기(포화)** 쉽다.  
\(\sqrt{d_k}\)로 스케일하면 그래디언트·수치 안정성에 유리하다.

영상: *“수치 안정성을 위해 고수같은 \(\sqrt{d_k}\)로 나눈다.”*

---

### 5. Softmax → Value 가중합

행 \(i\) (한 쿼리)에 대해:

\[
\alpha_{ij} = \frac{\exp(\mathrm{score}_{ij}/\sqrt{d_k})}{\sum_{j'} \exp(\mathrm{score}_{ij'}/\sqrt{d_k})}
\quad,\quad
\sum_j \alpha_{ij} = 1
\]

출력의 \(i\)번째 벡터:

\[
o_i = \sum_{j=1}^{L_k} \alpha_{ij}\, v_j
\]

영상: *“sequence 길이를 축으로 각 위치별 곱한 다음 서로 더한다.”*  
→ 결과가 다시 \(d_k\)(또는 \(d_v\)) 차원 벡터.

**해석 포인트 (중요):**

- Attention은 “정보를 새로 창조”한다기보다,  
  **이미 있는 Value들을 관련도에 따라 섞는다.**
- Softmax 때문에 **낮은 가중치 위치의 정보는 사실상 버려진다**  
  → 다음 문서(MHA)로 이어지는 동기.

---

### 6. Causal mask (디코더 LLM에서)

생성 모델에서는 미래 토큰을 보면 안 되므로,  
\(j > i\)인 score에 \(-\infty\)를 넣어 softmax 후 0으로 만든다.

GQA/MHA와 무관한 **마스킹 규칙**이지만,  
KV cache 논의(문서 05)에서는 “과거 토큰의 K/V만 쌓인다”는 점과 맞물린다.

---

### 7. 미니 수치 예 (개념)

\(L=2\), \(d_k=2\):

```text
Q = [[1, 0],      K = [[1, 0],      V = [[10, 0],
     [0, 1]]           [0, 1]]           [ 0, 20]]

QK^T = [[1, 0],
        [0, 1]]

softmax(행) ≈ I 에 가까우면
출력 ≈ V  (자기 자신에만 attend)
```

실제로는 scale·다른 값으로 soft한 혼합이 된다.  
코드: `code/attention_variants.py`의 `scaled_dot_product_attention`.

---

### 8. 체크

- [ ] \(QK^\top\) shape가 \(L \times L\)인 이유  
- [ ] softmax가 **어느 축**으로 합 1인지 (보통 last dim = key)  
- [ ] 출력이 왜 \(V\)와 같은 feature 차원인지  
- [ ] “Attention = Value의 가중합”을 한 문장으로

---

<a id="part-03-multi-head-attention"></a>

## PART 03. Multi-Head Attention

> 출처 파일: `03_Multi_Head_Attention.md`

## 03. Multi-Head Attention (MHA)

> 영상 구간: “멀티헤드 어텐션 설명” (~2:58–6:23)

---

### 1. 동기 (영상의 핵심 문장)

단일 attention의 가중치는 **한 줄의 관련도**다.  
Softmax로 낮은 위치는 거의 0 → *“해당 정보를 사실상 버리게 된다.”*

그런데 문장(시퀀스)을 이해할 때는:

- 어떤 관점에서는 술어·물음표  
- 다른 관점에서는 목적어  
- 또 다른 관점에서는 be동사  

처럼 **여러 측면의 가중치**가 필요하다.

→ *“벡터 하나를 한 측면의 정보로만 보지 않고, 여러 관점이 반영되어 있다고 보려는 의도”* = **Multi-Head**.

---

### 2. Head란 무엇인가

\(d_{model}\) 차원을 \(H\)개로 쪼개 (또는 독립 투영으로)  
각 head \(h\)가 자신만의 \(Q_h, K_h, V_h\)로 attention을 수행한다.

표준 구현:

\[
\begin{aligned}
Q_h &= X W^Q_h,\quad
K_h = X W^K_h,\quad
V_h = X W^V_h \\
\mathrm{head}_h &= \mathrm{Attention}(Q_h, K_h, V_h) \\
\mathrm{MHA}(X) &= \mathrm{Concat}(\mathrm{head}_1,\ldots,\mathrm{head}_H)\, W^O
\end{aligned}
\]

관례: \(d_k = d_v = d_{model} / H\).

영상 shape 점검:

- head마다 출력: \(L \times (d_k)\) where \(d_k = d_{model}/H\)  
- concat → \(L \times d_{model}\)  
- *“H개의 \(d_k/H\)… 가 아니라 \(d_{model}/H\)”* — 영상 표현 \(d_k\)가 전체인지 head인지 혼동하기 쉬움.  
  **이 노트에서는** 전체 모델 차원을 \(d_{model}\), head당 \(d_k = d_{model}/H\)로 고정한다.

---

### 3. “가중치가 갑자기 여러 개”인 이유

영상이 지적하는 함정:

> Value를 색으로만 나눠 그린 것처럼 보여도,  
> **가중합 가중치가 head마다 다르려면 Q·K도 head마다 갈라져야 한다.**

맞다. 가중치는 조상이 만들어 주지 않는다.

```text
MHA:
  빨간 Q ⊗ 빨간 K → 빨간 α → 빨간 V
  초록 Q ⊗ 초록 K → 초록 α → 초록 V
  파란 Q ⊗ 파란 K → 파란 α → 파란 V
```

같은 입력 토큰이라도 head마다 **다른 위치**에 mass를 둘 수 있다.

---

### 4. 출력을 어떻게 읽나

표면 shape는 single-head attention과 같이 \(L \times d_{model}\)일 수 있다.  
하지만 내재적으로는:

```text
[ head1 블록 | head2 블록 | ... | headH 블록 ]
```

각 블록이 **서로 다른 맥락 관점**으로 섞인 부분공간이다.  
마지막 \(W^O\)가 이 부분공간들을 다시 mix한다.

---

### 5. 비용·메모리 (MHA의 부담 — 다음으로 이어짐)

학습/prefill에서 FLOPs는 head를 나눠도 (이상적으로) 비슷한 오더인 경우가 많다.  
**추론 decode**에서는 다르다:

매 생성 스텝마다 과거 모든 토큰의 **모든 head의 K, V**를 읽어야 한다.

\[
\text{KV cache per layer} \propto L \times H \times d_k \times 2
\]

(\(2\) = K와 V)

헤드 수 \(H\)가 커질수록 캐시가 커진다.  
→ GQA/MQA가 “머리를 줄이는” 대상은 주로 **이 K/V 쪽**.

---

### 6. 체크

- [ ] 왜 single attention만으로는 “한 측면”에 치우치기 쉬운가  
- [ ] Q/K/V를 head로 나누는 것과 “관점” 비유의 대응  
- [ ] concat 후 \(d_{model}\)이 유지되는 이유  
- [ ] MHA의 KV cache가 \(H\)에 비례한다는 점

---

<a id="part-04-gqa-개념과-설계"></a>

## PART 04. GQA 개념과 설계

> 출처 파일: `04_GQA_개념과_설계.md`

## 04. GQA — 개념과 설계

> 영상 구간: “Group Query Attention의 개념과 설계” (~6:25–8:17)  
> 논문: Ainslie et al., *GQA: Training Generalized Multi-Query Transformer Models from Multi-Head Checkpoints* (2023)  
> https://arxiv.org/abs/2305.13245

---

### 1. 영상의 “건방진 상상”

MHA까지 오면:

- multihead 없음 → 단일 관점  
- multihead → 여러 관점 (head = 관점)

여기서 한 발:

> *관점이 여러 개인 건 그대로인데,  
> **각 관점마다도 여러 번 고려하는 것이 가능하다.***

구현으로 옮기면:

```text
MHA:  Q head 수 = K head 수 = V head 수 = H
      (같은 색끼리만 짝)

GQA:  Q head 수 = H
      K/V head 수 = G  (G < H, 보통 H의 약수)
      → query 여러 개가 하나의 K/V head를 공유
```

영상 예시: **Q head 4, KV head 2**  
→ query 2개가 하나의 key/value head를 공유.

---

### 2. 비유로 다시 풀기 (영상)

- K/V head 하나 ≈ **하나의 분석 관점**  
- 그 관점에 Q head 두 개 ≈ **같은 관점을 다른 query로 두 번 들여다봄**

연구 의도(영상):

> *같은 관점에서 특성을 여러 번 분석하면 성능에 도움이 되지 않을까?*

직관적 예상(영상):

```text
성능:  single-attn  <  GQA(G=2)  <  MHA(H=4)
```

관점 수가 줄었으니 MHA보다 못하고, single보다는 나을 것 같다.

---

### 3. 결과가 직관과 다른 점 (영상 + 논문)

영상:

> *막상 결과는 GQA 성능이 multihead와 **거의 비슷**.  
> 관점 수를 줄였는데 성능은 별로 안 떨어진다.  
> 대신 KV 크기가 작아져 **메모리를 덜 쓴다.***

논문 주장 (요지):

- **Uptrained GQA**는 quality가 MHA에 가깝고  
- speed/효율은 MQA에 가깝다  
- MHA 체크포인트를 GQA로 변환할 때, 그룹 내 K/V head를 **mean-pool**한 뒤  
  원래 pretrain의 ~**5%** compute로 uptraining 가능

즉 GQA의 실무 포지션은:

```text
품질 ──────────────●────────── MHA
                   GQA
속도/메모리 ──●──────────────── MQA
              GQA
```

---

### 4. 수학적 정의

\(n_q = H\), \(n_{kv} = G\), \(H = G \cdot g\) (\(g\) = group size).

Query head \(h \in \{0,\ldots,H-1\}\)가 사용하는 KV head:

\[
\mathrm{kv\_index}(h) = \left\lfloor \frac{h}{g} \right\rfloor
\quad\text{또는}\quad
\left\lfloor \frac{h \cdot G}{H} \right\rfloor
\]

(구현에 따라 연속 블록 그룹핑이 일반적.)

각 query head \(h\):

\[
\mathrm{head}_h = \mathrm{Attention}\bigl(Q_h,\; K_{\mathrm{kv}(h)},\; V_{\mathrm{kv}(h)}\bigr)
\]

그 후 MHA와 같이 concat + \(W^O\).

**극단:**

| 설정 | 이름 |
|------|------|
| \(G = H\) | MHA |
| \(G = 1\) | MQA |
| \(1 < G < H\) | GQA |

---

### 5. Shape 예제 (영상 숫자)

\(L=8\), \(d_{model}=64\), \(H=4\), \(G=2\) → \(d_k = 16\), group size \(= 2\).

| 텐서 | shape |
|------|-------|
| \(Q\) | \((L, H, d_k) = (8,4,16)\) |
| \(K, V\) | \((L, G, d_k) = (8,2,16)\) |
| attention (head당) | \(Q_h\) vs 공유 \(K_g\) → weights \((8,8)\), out \((8,16)\) |
| concat | \((8, 64)\) |

KV cache (한 레이어, float16 가정은 문서 05):

- MHA: \(L \times H \times d_k \times 2\)  
- GQA: \(L \times G \times d_k \times 2\)  
- 비율: \(G/H = 1/2\) (이 예제에서 절반)

Llama 3 70B급 예: \(H=64\), \(G=8\) → **8배** 캐시 감소.

---

### 6. 구현 시 자주 하는 일

1. **K/V를 query head 수만큼 repeat/expand**  
   - `K`를 `(L, G, d_k)` → `(L, H, d_k)`로 각 그룹을 \(g\)번 복제  
   - 그다음 기존 MHA 커널처럼 계산  
2. 또는 flash-attn 등에서 **native GQA** 지원 (`num_kv_heads` 인자)

학습 시에는 \(W^K, W^V\)의 출력 차원만 \(G \cdot d_k\)로 줄이면 된다.

---

### 7. Uptraining (논문 포인트, 영상에는 약함)

이미 학습된 **MHA 모델**을 GQA로 바꿀 때:

1. query head를 \(G\)개 그룹으로 나눔  
2. 그룹 안 원래 K(및 V) head들을 **평균(mean pool)** → 새 KV head  
3. 짧은 추가 학습으로 품질 회복

처음부터 GQA로 pretrain하는 경우도 많음 (Llama 2 70B 등).

---

### 8. 체크

- [ ] \(n_q\)와 \(n_{kv}\)가 다를 때 짝짓기 규칙을 그림으로 그릴 수 있는가  
- [ ] GQA-1 / GQA-H가 각각 MQA / MHA인 이유  
- [ ] “성능≈MHA, 메모리↓”가 왜 서빙에서 결정적인지 → `05`  
- [ ] 직관(성능 중간)과 실험(거의 MHA)의 차이

---

<a id="part-05-kv-cache--왜-gqa가-이득인가"></a>

## PART 05. KV Cache — 왜 GQA가 이득인가

> 출처 파일: `05_KV_Cache와_왜_중요한가.md`

## 05. KV Cache — 왜 GQA가 “이득”인가

> 영상: *“kv cache 크기가 작아져서 메모리를 덜 쓰면서 돌릴 수 있다”* (~8:33–8:48)  
> 이 문서가 GQA를 **연구 아이디어**에서 **실무 필수 기법**으로 만드는 이유다.

---

### 1. Autoregressive decode에서 생기는 일

토큰을 하나씩 생성할 때, 매 스텝 \(t\)에서:

1. 새 토큰의 \(q_t, k_t, v_t\)만 새로 계산  
2. 과거 \(k_{1:t-1}, v_{1:t-1}\)는 **다시 안 만들고 재사용**  
3. \(q_t\)가 전체 \(K_{1:t}, V_{1:t}\)에 attend

재사용하는 저장소 = **KV cache**.

없으면 매 스텝마다 과거 전체를 다시 투영·저장해야 해서 느리다.  
있으면 계산은 싸지지만 **메모리가 시퀀스 길이에 비례해 증가**한다.

---

### 2. 캐시 크기 공식 (레이어당)

한 레이어, batch \(B\), 길이 \(L\), KV head \(n_{kv}\), head dim \(d_k\), dtype bytes \(b\):

\[
\mathrm{bytes} = B \times L \times n_{kv} \times d_k \times 2 \times b
\]

(\(2\) = Key + Value)

| 구조 | \(n_{kv}\) | 상대 캐시 |
|------|------------|-----------|
| MHA | \(H\) | \(1\times\) |
| GQA | \(G\) | \(G/H\) |
| MQA | \(1\) | \(1/H\) |

**Query head 수 \(H\)는 캐시 공식에 안 들어간다.**  
GQA가 Q를 많이 남겨도 캐시가 줄어드는 이유.

전체 모델이면 × `num_layers`. (일부 구조는 레이어 간 공유 등 변형 있음.)

---

### 3. 숫자 감 잡기

가정: \(B=1\), \(L=8192\), \(H=32\), \(d_k=128\), layers=32, fp16 (\(b=2\)).

| | \(n_{kv}\) | 대략 KV (전 레이어) |
|--|------------|---------------------|
| MHA | 32 | \(1 \times 8192 \times 32 \times 128 \times 2 \times 2 \times 32\) ≈ **4.0 GiB** |
| GQA (G=8) | 8 | ≈ **1.0 GiB** |
| MQA | 1 | ≈ **0.125 GiB** |

컨텍스트 128k, 배치>1이면 MHA는 금세 수십~수백 GB.  
서빙에서 “긴 컨텍스트 + 동시 요청”이 GQA를 기본값으로 만든 배경이다.

코드: `python code/kv_cache_estimate.py`

---

### 4. 메모리뿐 아니라 **대역폭**

Decode 한 스텝의 attention은 종종:

- 작은 \(q\)  
- 거대한 과거 \(K,V\)를 HBM에서 읽음  

→ **memory-bandwidth bound**.  
캐시가 \(G/H\)로 줄면 읽어야 할 바이트도 \(G/H\) → 토큰/초가 좋아지는 경우가 많다.

영상은 “메모리”만 말했지만, 실무 이득의 큰 축은 **대역폭·배치 크기·동시 세션 수**다.

---

### 5. Prefill vs Decode

| | 계산 | KV |
|--|------|-----|
| Prefill | 긴 프롬프트, 병렬 matmul | 캐시를 **채움** |
| Decode | 토큰당 소량 계산 | 캐시를 **읽고 덧붙임** |

GQA의 체감 이득은 대개 **Decode / 장기 서빙**에서 크다.  
학습 throughput만 보면 이야기가 다를 수 있다.

---

### 6. 영상 문장과 연결

> Key/Value head 개수가 줄었으니 계산 과정에서 key value 크기가 작아진다  
> → kv cache가 작아져 메모리를 덜 쓴다  
> → 성능은 MHA급 유지 + 메모리 절약

이 문서의 식으로 바꾸면:

```text
품질 ≈ MHA   (실험적으로 근접)
캐시 = Θ(n_kv)  ↓
```

---

### 7. 체크

- [ ] KV cache에 Q는 안 쌓이는 이유  
- [ ] 왜 \(n_q\)가 아니라 \(n_{kv}\)가 캐시를 결정하는지  
- [ ] GQA의 \(G/H\) 절약이 배치·컨텍스트에 곱해지는 구조  
- [ ] bandwidth bound decode와의 관계

---

<a id="part-06-mha--mqa--gqa-스펙트럼"></a>

## PART 06. MHA · MQA · GQA 스펙트럼

> 출처 파일: `06_MHA_MQA_GQA_스펙트럼.md`

## 06. MHA · MQA · GQA 스펙트럼

> GQA를 “단독 발명”이 아니라 **KV 공유의 연속체**로 보는 문서.

---

### 1. 한눈에

```text
n_kv = H  ────────────────────────  n_kv = 1
   MHA              GQA               MQA
 (공유 없음)    (부분 공유)        (전부 공유)

캐시: 큼 ◄────────────────────────► 작음
품질: 기준 ◄──────────────────────► (대체로) 약간↓
```

논문 표기:

- **GQA-H** = MHA  
- **GQA-1** = MQA  
- **GQA-G** = 중간

---

### 2. Multi-Query Attention (MQA) — Shazeer 2019

모든 query head가 **단 하나의** K/V head를 공유.

- 캐시 \(1/H\)  
- 디코드 매우 빠름  
- 다만 품질·학습 안정성에서 MHA 대비 하락이 보고되는 경우가 있음  
- PaLM, Falcon 등에서 사용 이력

GQA 논문의 동기 중 하나:  
*MQA만큼 공격적으로 줄이지 말고, 중간 지점을 두자.*

---

### 3. Grouped-Query Attention — Ainslie 2023

- \(1 < n_{kv} < H\)  
- Llama 계열 관례: \(H=32\)에 \(G=8\), 또는 \(H=64\)에 \(G=8\) 등  
- **품질 ≈ MHA**, **속도 ≈ MQA**에 가깝다는 것이 핵심 셀링 포인트

추가 기여: 기존 MHA 체크포인트 → mean-pool → **uptraining (~5% compute)**.

---

### 4. 비교 표

| | MHA | GQA | MQA |
|--|-----|-----|-----|
| Query heads | \(H\) | \(H\) | \(H\) |
| KV heads | \(H\) | \(G\) | \(1\) |
| KV cache | \(2H d_k L\) | \(2G d_k L\) | \(2 d_k L\) |
| 품질 (전형) | 기준 | ≈기준 | 다소↓ 가능 |
| 구현 복잡도 | 낮음 | 낮~중 | 낮음 |
| 대표 | GPT-3급 고전 Transformer | Llama 2/3, Mistral, Gemma | PaLM, 일부 Falcon |

(품질 행은 모델·데이터·튜닝에 따라 달라짐. DeepSeek-V2 ablation에서는 GQA가 MHA보다 손해인 설정도 보고됨 → MLA 동기.)

---

### 5. “관점” 비유를 스펙트럼에 얹기

영상 언어로:

| | 관점(K/V) 수 | 관점당 query 재검토 |
|--|--------------|---------------------|
| MHA | \(H\) | 1회 (1:1) |
| GQA | \(G\) | \(H/G\)회 |
| MQA | 1 | \(H\)회 |

GQA는 *관점 다양성*과 *관점 내 다중 질의*의 타협이다.

---

### 6. 설계 선택 가이드 (실무 감각)

| 상황 | 경향 |
|------|------|
| 연구·작은 모델, 품질 최우선 | MHA도 충분 |
| 긴 컨텍스트 서빙, 오픈 LLM | **GQA가 사실상 표준** |
| 극단적 메모리 절약, 품질 여유 | MQA 또는 MLA |
| 이미 MHA 체크포인트만 있음 | GQA uptraining 레시피 |

\(H\)가 커질수록 MQA의 “한 방에 \(H\)배 축소”는 더 공격적 →  
큰 모델일수록 **비율을 유지하는 GQA**가 논문이 강조하는 이유이기도 하다.

---

### 7. 체크

- [ ] GQA-1 / GQA-H 정의  
- [ ] MQA의 장단을 GQA와 한 문장씩  
- [ ] Llama식 64/8이 캐시를 몇 배로 줄이는지 (\(8\times\))

---

<a id="part-07-중요점-체크리스트"></a>

## PART 07. 중요점 체크리스트

> 출처 파일: `07_중요점_체크리스트.md`

## 07. 중요점 체크리스트

영상 + 논문 + 서빙 관점에서 **헷갈리기 쉬운 것 / 꼭 잡을 것**.

---

### A. 개념 (영상 근간)

1. **Attention 출력 = Value의 가중합**  
   가중치는 \(QK^\top\)에서 오고, 정보는 \(V\)에서 온다.

2. **Softmax는 key 축으로 합 1**  
   “어디에 주목할지”의 분포.

3. **MHA = 여러 가중합 분포**  
   한 분포로는 문장의 여러 측면을 동시에 살리기 어렵다.

4. **MHA에서 Q/K/V를 모두 head로 나누는 이유**  
   Value만 색칠해 나눠 그린다고 가중치가 늘지 않는다.  
   가중치를 만들려면 Q·K도 head별이어야 한다.

5. **GQA의 발상**  
   관점(K/V)은 줄이되, 같은 관점을 query 여러 개로 반복 조회.

6. **직관 ≠ 실험**  
   관점↓ → 성능↓일 것 같지만, GQA는 MHA에 가깝게 유지되는 경우가 많다.  
   그게 이 기법의 존재 이유다.

---

### B. 구현·수식

7. **\(n_q\)와 \(n_{kv}\)는 독립 하이퍼파라미터** (단 \(n_q \bmod n_{kv} = 0\)이 일반적).

8. **캐시 공식에 들어가는 건 \(n_{kv}\)**  
   Query를 64로 둬도 KV head가 8이면 캐시는 8 기준.

9. **GQA ≡ MHA when \(n_{kv}=n_q\)**  
   **GQA ≡ MQA when \(n_{kv}=1\)**.

10. **repeat_kv 트릭**  
    구현에서 K/V를 query head 수만큼 복제해 기존 MHA 커널을 재사용하는 경우가 많다.  
    (수학적으로 공유와 동일, 메모리상으로는 캐시에는 복제본을 안 둠.)

11. **\(d_k\) 표기 혼동**  
    논문/영상마다 “전체 차원” vs “head 차원”.  
    이 스터디: \(d_{model}\), \(d_k=d_{model}/n_q\).

---

### C. 시스템·서빙

12. **GQA의 본진은 decode KV memory/bandwidth**  
    “학습이 무조건 빨라진다”가 아님.

13. **컨텍스트·배치에 선형**  
    절약 배수 \(H/G\)가 길이와 배치에 곱해진다.

14. **품질-메모리 파레토**  
    MHA–GQA–MQA는 같은 축의 다른 점.  
    더 공격적인 다음 축이 **MLA(압축)**.

---

### D. 흔한 오해

| 오해 | 교정 |
|------|------|
| GQA는 attention 수식을 바꿨다 | 수식은 동일 SDPA. **K/V head 개수·공유만** 변경 |
| GQA는 head 수를 줄여 품질을 희생한다 | Q head는 유지. 희생하더라도 보통 **작은** 편 |
| KV cache에 Q도 넣는다 | 보통 **K/V만**. Q는 매 스텝 새로 |
| GQA = MLA | 공유(GQA) vs latent 압축(MLA) |
| 영상 속 “중국 장인 MLA”가 GQA의 세부 | **파생·후속** 기법 (문서 08) |

---

### E. 한 장 암기 카드

```text
SDPA : softmax(QK^T/√d) V
MHA  : head마다 자기 Q,K,V
GQA  : head마다 자기 Q / 그룹이 공유 K,V
이득 : KV cache × (n_kv / n_q)
목표 : MHA급 품질 + MQA급 효율에 근접
```

---

<a id="part-08-파생--mla와-그-다음"></a>

## PART 08. 파생 — MLA와 그 다음

> 출처 파일: `08_파생_MLA와_다음.md`

## 08. 파생 — MLA와 “그 다음”

> 영상 마무리: *GQA 말고 MLA… 다음 영상에서* (~8:51–9:11)  
> DeepSeek-V2의 **Multi-head Latent Attention (MLA)**

---

### 1. 같은 병목, 다른 질문

| | GQA / MQA | MLA |
|--|-----------|-----|
| 질문 | **몇 개의** K/V head를 둘까? (공유) | **무엇을** 캐시할까? (압축) |
| 수단 | head 수 \(n_{kv}\)↓ | 저차원 latent \(c\)를 캐시 후 필요 시 복원 |
| 복잡도 | 낮음 (현 LLM 표준) | 높음 (RoPE 흡수, absorb 등) |
| 대표 | Llama, Mistral | DeepSeek-V2/V3, R1 계열 |

영상에서 GQA를 이해한 뒤 MLA로 가는 이유는 명확하다.  
둘 다 **KV cache와의 전쟁**이고, GQA가 “공유”로 이긴 다음 수가 “압축”이다.

---

### 2. MLA 아이디어 (초간단)

표준: 토큰마다 multi-head K, V를 통째로 캐시.  
MLA: 토큰마다 **짧은 latent 벡터**만 캐시하고,  
attention에 쓸 K/V는 **저랭크 투영으로 그때그때 복원**(또는 행렬 흡수로 더 싸게).

결과적으로:

- 캐시 크기가 head 수에 **직접 비례하지 않음**  
- 보고된 설정에서 GQA보다 강한 압축 + 품질 유지/향상 ablation이 있음 (DeepSeek-V2)

세부 수식·RoPE 이슈는 별도 노트 주제.  
여기서는 **GQA 다음 노드**만 고정한다.

---

### 3. 진화 지도

```text
2017  Attention / MHA          (Vaswani)
2019  MQA                      (Shazeer) — KV 공유의 극단
2023  GQA                      (Ainslie) — 공유의 중간점 ← 이 스터디
2024  MLA                      (DeepSeek-V2) — 캐시 내용 자체를 압축
      (+ 기타: sliding window, sparse, quantization of KV, …)
```

---

### 4. GQA를 안 다음에 보면 좋은 질문

1. 내 모델의 \(n_q, n_{kv}, d_k, L, L_{layers}\)로 캐시가 몇 GB인가?  
2. Decode가 compute-bound인가 bandwidth-bound인가?  
3. MHA→GQA uptraining이 필요한가, from-scratch인가?  
4. GQA로도 부족하면 MLA·KV quant·window 중 무엇이 맞는가?

---

### 5. 참고

- DeepSeek-V2 Technical Report  
- Sebastian Raschka — MLA gallery  
- 영상 예고: MLA 회차 (이 폴더의 후속 스터디로 연결 가능)

---

### 6. 이 폴더에서의 마무리 문장

> Attention은 V의 가중합이고,  
> MHA는 그 가중합을 여러 관점으로 쪼개며,  
> GQA는 관점(K/V)을 공유해 **거의 같은 가중합 품질을 더 싼 캐시로** 사고,  
> MLA는 캐시에 넣는 표현 자체를 바꿔 **한 단계 더** 싸게 만든다.

---

<a id="part-code-a-attention_variantspy"></a>

## PART CODE A. attention_variants.py

> 출처 파일: `code/attention_variants.py`

```python
"""
Scaled Dot-Product / MHA / GQA / MQA — NumPy educational implementations.

Shapes use:
  B: batch, L: seq len, H: num query heads, G: num kv heads, D: head dim
  d_model = H * D
"""

from __future__ import annotations

import numpy as np


def softmax(x: np.ndarray, axis: int = -1) -> np.ndarray:
    x = x - np.max(x, axis=axis, keepdims=True)
    e = np.exp(x)
    return e / np.sum(e, axis=axis, keepdims=True)


def scaled_dot_product_attention(
    q: np.ndarray,
    k: np.ndarray,
    v: np.ndarray,
    *,
    causal: bool = False,
) -> tuple[np.ndarray, np.ndarray]:
    """
    q, k, v: (..., Lq, D) / (..., Lk, D) / (..., Lk, Dv)
    returns: out (..., Lq, Dv), weights (..., Lq, Lk)
    """
    d_k = q.shape[-1]
    scores = np.matmul(q, np.swapaxes(k, -1, -2)) / np.sqrt(d_k)
    if causal:
        Lq, Lk = scores.shape[-2], scores.shape[-1]
        mask = np.triu(np.ones((Lq, Lk), dtype=bool), k=1)
        scores = np.where(mask, -1e9, scores)
    weights = softmax(scores, axis=-1)
    out = np.matmul(weights, v)
    return out, weights


def _project(x: np.ndarray, w: np.ndarray) -> np.ndarray:
    """x: (B, L, d_in), w: (d_in, d_out) -> (B, L, d_out)"""
    return np.matmul(x, w)


def split_heads(x: np.ndarray, n_heads: int) -> np.ndarray:
    """(B, L, n_heads * D) -> (B, n_heads, L, D)"""
    B, L, _ = x.shape
    D = x.shape[-1] // n_heads
    return x.reshape(B, L, n_heads, D).transpose(0, 2, 1, 3)


def merge_heads(x: np.ndarray) -> np.ndarray:
    """(B, n_heads, L, D) -> (B, L, n_heads * D)"""
    B, H, L, D = x.shape
    return x.transpose(0, 2, 1, 3).reshape(B, L, H * D)


def repeat_kv(x: np.ndarray, n_rep: int) -> np.ndarray:
    """
    Expand KV heads to match query heads.
    x: (B, G, L, D) -> (B, G * n_rep, L, D)
    """
    if n_rep == 1:
        return x
    B, G, L, D = x.shape
    x = np.expand_dims(x, axis=2)  # (B, G, 1, L, D)
    x = np.repeat(x, n_rep, axis=2)  # (B, G, n_rep, L, D)
    return x.reshape(B, G * n_rep, L, D)


class MultiHeadAttention:
    """Classic MHA: n_kv_heads == n_heads."""

    def __init__(self, d_model: int, n_heads: int, rng: np.random.Generator):
        assert d_model % n_heads == 0
        self.d_model = d_model
        self.n_heads = n_heads
        self.d_head = d_model // n_heads
        scale = 0.02
        self.W_q = rng.normal(0, scale, (d_model, d_model))
        self.W_k = rng.normal(0, scale, (d_model, d_model))
        self.W_v = rng.normal(0, scale, (d_model, d_model))
        self.W_o = rng.normal(0, scale, (d_model, d_model))

    def __call__(self, x: np.ndarray, *, causal: bool = False) -> np.ndarray:
        q = split_heads(_project(x, self.W_q), self.n_heads)
        k = split_heads(_project(x, self.W_k), self.n_heads)
        v = split_heads(_project(x, self.W_v), self.n_heads)
        out, _ = scaled_dot_product_attention(q, k, v, causal=causal)
        return _project(merge_heads(out), self.W_o)


class GroupedQueryAttention:
    """
    GQA: n_heads query heads, n_kv_heads key/value heads.
    n_heads must be divisible by n_kv_heads.
    MQA when n_kv_heads == 1; MHA when n_kv_heads == n_heads.
    """

    def __init__(
        self,
        d_model: int,
        n_heads: int,
        n_kv_heads: int,
        rng: np.random.Generator,
    ):
        assert d_model % n_heads == 0
        assert n_heads % n_kv_heads == 0
        self.d_model = d_model
        self.n_heads = n_heads
        self.n_kv_heads = n_kv_heads
        self.d_head = d_model // n_heads
        self.n_rep = n_heads // n_kv_heads
        scale = 0.02
        self.W_q = rng.normal(0, scale, (d_model, n_heads * self.d_head))
        self.W_k = rng.normal(0, scale, (d_model, n_kv_heads * self.d_head))
        self.W_v = rng.normal(0, scale, (d_model, n_kv_heads * self.d_head))
        self.W_o = rng.normal(0, scale, (d_model, d_model))

    def __call__(self, x: np.ndarray, *, causal: bool = False) -> dict:
        q = split_heads(_project(x, self.W_q), self.n_heads)  # (B, H, L, D)
        k = split_heads(_project(x, self.W_k), self.n_kv_heads)  # (B, G, L, D)
        v = split_heads(_project(x, self.W_v), self.n_kv_heads)

        # Educational path: expand KV to H heads (same math as shared KV)
        k_exp = repeat_kv(k, self.n_rep)
        v_exp = repeat_kv(v, self.n_rep)
        out, weights = scaled_dot_product_attention(q, k_exp, v_exp, causal=causal)
        y = _project(merge_heads(out), self.W_o)

        return {
            "output": y,
            "weights": weights,  # (B, H, L, L) — note heads in same group share K/V but not α
            "q_shape": q.shape,
            "k_shape": k.shape,
            "v_shape": v.shape,
            "n_rep": self.n_rep,
        }


def kv_index_for_query_head(h: int, n_heads: int, n_kv_heads: int) -> int:
    """Which KV head does query head h attend to? (contiguous groups)"""
    group_size = n_heads // n_kv_heads
    return h // group_size


def demonstrate_grouping(n_heads: int = 4, n_kv_heads: int = 2) -> None:
    print(f"Grouping map: n_q={n_heads}, n_kv={n_kv_heads}")
    for h in range(n_heads):
        g = kv_index_for_query_head(h, n_heads, n_kv_heads)
        print(f"  query head {h}  →  kv head {g}")
```

---

<a id="part-code-b-kv_cache_estimatepy"></a>

## PART CODE B. kv_cache_estimate.py

> 출처 파일: `code/kv_cache_estimate.py`

```python
"""Estimate KV-cache bytes for MHA / GQA / MQA."""

from __future__ import annotations


def kv_cache_bytes(
    *,
    batch: int,
    seq_len: int,
    n_kv_heads: int,
    d_head: int,
    n_layers: int,
    bytes_per_elem: int = 2,  # fp16
) -> int:
    # K and V each: B * L * n_kv * d_head * bytes
    per_layer = batch * seq_len * n_kv_heads * d_head * 2 * bytes_per_elem
    return per_layer * n_layers


def human(n: int) -> str:
    for unit, div in (("GiB", 1024**3), ("MiB", 1024**2), ("KiB", 1024)):
        if n >= div:
            return f"{n / div:.2f} {unit}"
    return f"{n} B"


def compare(
    *,
    batch: int = 1,
    seq_len: int = 8192,
    n_q_heads: int = 32,
    d_head: int = 128,
    n_layers: int = 32,
    gqa_kv_heads: int = 8,
    bytes_per_elem: int = 2,
) -> None:
    configs = {
        "MHA": n_q_heads,
        f"GQA(G={gqa_kv_heads})": gqa_kv_heads,
        "MQA": 1,
    }
    print(
        f"B={batch}, L={seq_len}, n_q={n_q_heads}, d_head={d_head}, "
        f"layers={n_layers}, dtype={bytes_per_elem}B"
    )
    print("-" * 60)
    base = None
    for name, n_kv in configs.items():
        b = kv_cache_bytes(
            batch=batch,
            seq_len=seq_len,
            n_kv_heads=n_kv,
            d_head=d_head,
            n_layers=n_layers,
            bytes_per_elem=bytes_per_elem,
        )
        if base is None:
            base = b
        ratio = b / base
        print(f"{name:16s}  n_kv={n_kv:3d}  cache={human(b):>12s}  vs MHA={ratio:.4f}x")


if __name__ == "__main__":
    compare()
    print()
    compare(seq_len=128_000, gqa_kv_heads=8, n_q_heads=64)
```

---

<a id="part-code-c-run_demopy"></a>

## PART CODE C. run_demo.py

> 출처 파일: `code/run_demo.py`

```python
#!/usr/bin/env python3
"""Run educational demos for Attention / MHA / GQA."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))

from attention_variants import (
    GroupedQueryAttention,
    MultiHeadAttention,
    demonstrate_grouping,
    scaled_dot_product_attention,
)
from kv_cache_estimate import compare


def demo_sdpa() -> None:
    print("=" * 60)
    print("1) Scaled Dot-Product Attention (single head)")
    print("=" * 60)
    rng = np.random.default_rng(0)
    L, D = 4, 8
    q = rng.normal(size=(1, L, D))
    k = rng.normal(size=(1, L, D))
    v = rng.normal(size=(1, L, D))
    out, w = scaled_dot_product_attention(q, k, v)
    print(f"Q/K/V: {(L, D)}")
    print(f"weights (L×L) row sums: {w.sum(axis=-1).round(6)}")
    print(f"output shape: {out.shape}  # L × D  — Value의 가중합")
    print()


def demo_gqa_shapes() -> None:
    print("=" * 60)
    print("2) GQA shapes (영상 예: Q heads=4, KV heads=2)")
    print("=" * 60)
    demonstrate_grouping(4, 2)
    rng = np.random.default_rng(1)
    B, L, d_model = 2, 8, 64
    x = rng.normal(size=(B, L, d_model)).astype(np.float64)
    gqa = GroupedQueryAttention(d_model, n_heads=4, n_kv_heads=2, rng=rng)
    result = gqa(x, causal=True)
    print(f"input x:     {x.shape}")
    print(f"Q:           {result['q_shape']}  # (B, H, L, D)")
    print(f"K (shared):  {result['k_shape']}  # (B, G, L, D)  G < H")
    print(f"V (shared):  {result['v_shape']}")
    print(f"n_rep:       {result['n_rep']}  # 한 KV를 공유하는 Q head 수")
    print(f"output:      {result['output'].shape}")
    print(f"attn weights:{result['weights'].shape}  # head마다 α는 다를 수 있음")
    print()


def demo_spectrum() -> None:
    print("=" * 60)
    print("3) Same module: MHA / GQA / MQA via n_kv_heads")
    print("=" * 60)
    rng = np.random.default_rng(2)
    B, L, d_model, H = 1, 16, 64, 8
    x = rng.normal(size=(B, L, d_model))
    for name, g in [("MHA", H), ("GQA", 2), ("MQA", 1)]:
        m = GroupedQueryAttention(d_model, n_heads=H, n_kv_heads=g, rng=rng)
        y = m(x, causal=True)["output"]
        print(f"{name:4s}  n_kv={g}  out={y.shape}  ||y||={np.linalg.norm(y):.4f}")
    print()


def demo_mha_vs_gqa_param_kv() -> None:
    print("=" * 60)
    print("4) Projection width: K/V params shrink with n_kv")
    print("=" * 60)
    d_model, H = 4096, 32
    d_head = d_model // H
    for g in (32, 8, 1):
        q = d_model * (H * d_head)
        kv = 2 * d_model * (g * d_head)
        o = d_model * d_model
        print(
            f"n_kv={g:2d}  W_q={q/1e6:.2f}M  W_k+W_v={kv/1e6:.2f}M  "
            f"W_o={o/1e6:.2f}M  (cache ∝ {g})"
        )
    print()


def demo_kv_cache() -> None:
    print("=" * 60)
    print("5) KV cache estimate")
    print("=" * 60)
    compare(batch=1, seq_len=8192, n_q_heads=32, d_head=128, n_layers=32, gqa_kv_heads=8)
    print()


def main() -> None:
    demo_sdpa()
    demo_gqa_shapes()
    demo_spectrum()
    demo_mha_vs_gqa_param_kv()
    demo_kv_cache()
    # silence unused import warning style
    _ = MultiHeadAttention


if __name__ == "__main__":
    main()
```

---

<a id="part-requirements"></a>

## PART. requirements.txt

> 출처 파일: `requirements.txt`

```text
numpy>=1.24
```

---

## 부록. .gitignore

> 출처 파일: `.gitignore`

```text
.venv/
__pycache__/
*.pyc
.DS_Store
```

---

---

<a id="part-09-보충-심화-설명"></a>

## PART 09. 보충·심화 설명 (완전판 보강)

> 이 섹션은 분산 md에 없거나 짧게만 있던 내용을 **완전판 안에서** 추가로 풀어 쓴 것이다.  
> 영상 직관 + 논문/서빙 맥락을 한 번에 연결한다.

### 9.1 Q · K · V를 “역할”로 다시 보기

| 기호 | 비유 | 실제로 하는 일 |
|------|------|----------------|
| **Q (Query)** | “지금 내가 찾는 것” | 현재 위치(토큰)가 **어디에 주목할지**를 묻는 벡터 |
| **K (Key)** | “각 위치의 색인/태그” | 과거(또는 전체) 위치가 **무엇을 갖고 있는지**를 나타내는 벡터. Q와 내적되어 관련도 score가 됨 |
| **V (Value)** | “실제 내용물” | 가중치가 정해진 뒤 **섞여서 출력으로 나오는** 정보 |

핵심 한 문장:

> Attention은 **Q와 K로 ‘어디를 볼지’를 정하고**, **V로 ‘무엇을 가져올지’를 가져온다.**

그래서:

- Softmax 가중치 \(\alpha\)는 Q·K만으로 결정된다.  
- 출력 벡터의 “재료”는 V이다.  
- GQA가 줄이는 것은 캐시에 쌓이는 **K와 V**이지, 매 스텝 새로 만드는 **Q**가 아니다.

---

### 9.2 왜 “관점을 줄여도” 성능이 잘 유지되나?

영상 직관: 관점(K/V head) 수↓ → 성능↓일 것 같다.  
실험: GQA ≈ MHA에 가까운 경우가 많다.

가능한 이유 (직관적 설명):

1. **Head 중복(redundancy)**  
   MHA의 여러 head가 서로 비슷한 패턴을 attend하는 경우가 있다.  
   비슷한 K/V를 하나로 묶어 공유해도 정보 손실이 작을 수 있다.

2. **Query 쪽 다양성은 유지**  
   GQA는 \(n_q\)를 그대로 둔다.  
   같은 K/V를 보더라도 **서로 다른 Q**가 다른 \(\alpha\)를 만들 수 있다.  
   (같은 도서관 색인(K)을 보더라도, 질문(Q)이 다르면 집어 오는 책의책장(가중치)이 달라진다.)

3. **학습이 공유 구조에 적응**  
   from-scratch GQA이든 uptraining이든, 모델이 “공유된 K/V” 위에서 잘 동작하도록 파라미터를 맞춘다.

4. **병목이 품질이 아니라 서빙 메모리인 구간**  
   긴 컨텍스트 decode에서는 MHA의 여분 head가 주는 품질 이득보다,  
   KV 대역폭 절약이 전체 시스템 이득으로 더 크게 돌아올 수 있다.

주의: “항상 손해 0”은 아니다. DeepSeek-V2 ablation처럼 설정에 따라 GQA가 MHA보다 손해인 보고도 있다.  
그래도 Llama 계열에서 GQA가 표준이 된 이유는 **품질-효율 파레토가 매우 좋기 때문**이다.

---

### 9.3 같은 K/V를 공유해도 α는 다를 수 있다 (중요)

영상/코드에서 헷갈리기 쉬운 포인트.

```text
그룹 0:
  Q0 ──┐
       ├──► 같은 K0, V0
  Q1 ──┘

그룹 1:
  Q2 ──┐
       ├──► 같은 K1, V1
  Q3 ──┘
```

- \(K_0, V_0\)는 Q0과 Q1이 **공유**한다.  
- 그러나  
  \(\alpha^{(0)} = \mathrm{softmax}(Q_0 K_0^\top / \sqrt{d})\)  
  \(\alpha^{(1)} = \mathrm{softmax}(Q_1 K_0^\top / \sqrt{d})\)  
  이므로 **가중치 분포는 head마다 다를 수 있다.**  
- 출력도  
  \(O_0 = \alpha^{(0)} V_0\), \(O_1 = \alpha^{(1)} V_0\)  
  → 같은 Value 재료를 **다른 비율로** 섞는다.

코드의 `weights` shape가 `(B, H, L, L)`인 이유:  
공유는 K/V이고, attention map은 query head별이다.

---

### 9.4 짝짓기 그림 (영상 예: H=4, G=2)

```text
Query heads          KV heads
───────────          ────────
  Q0  ─────────────►  KV0
  Q1  ─────────────►  KV0     (그룹 0, n_rep=2)

  Q2  ─────────────►  KV1
  Q3  ─────────────►  KV1     (그룹 1, n_rep=2)
```

인덱스 규칙 (연속 블록):

\[
\mathrm{kv\_index}(h)=\left\lfloor\frac{h}{H/G}\right\rfloor
=\left\lfloor\frac{h}{2}\right\rfloor
\quad (H=4,G=2)
\]

| h (query) | kv_index |
|-----------|----------|
| 0 | 0 |
| 1 | 0 |
| 2 | 1 |
| 3 | 1 |

`repeat_kv`를 쓰면:

```text
K shape (B, G=2, L, D)
  → repeat n_rep=2
  → (B, H=4, L, D)   # [KV0, KV0, KV1, KV1] 순으로 복제
```

캐시에는 **복제본을 저장하지 않는다.**  
복제는 “기존 MHA 커널을 재사용하기 위한 계산용 펼치기”일 뿐이다.

---

### 9.5 Attention을 한 줄 계산으로 (손으로)

\(L=2\), \(d_k=2\), scale \(\sqrt{2}\approx 1.414\).

```text
q0 = [1, 0],  k0 = [1, 0], k1 = [0, 1]
v0 = [10, 0], v1 = [0, 20]
```

1. scores (scale 전):  
   \(q_0\cdot k_0=1\), \(q_0\cdot k_1=0\)
2. scale 후: \(1/1.414\approx0.707\), \(0\)
3. softmax:  
   \(\alpha \approx [0.67,\; 0.33]\) (대략)
4. 출력:  
   \(o_0 \approx 0.67\cdot[10,0] + 0.33\cdot[0,20] \approx [6.7,\; 6.6]\)

의미: q0은 k0과 더 닮았으므로 v0을 더 많이 가져온다.  
GQA든 MHA든 **이 한 head 안의 계산은 동일**하다. 달라지는 것은 “이 head의 K/V가 누구 것과 같은지”뿐이다.

---

### 9.6 학습 FLOPs vs 추론 메모리 — 무엇이 줄고 안 줄까

| 구간 | GQA가 줄이는 것 | 잘 안 줄거나 거의 같은 것 |
|------|-----------------|---------------------------|
| 학습 forward (짧은 seq) | \(W^K,W^V\) 파라미터·투영 폭 | Attention matmul은 \(n_q\) 기준이면 비슷 |
| Prefill (긴 프롬프트) | KV 저장량 | 계산은 여전히 큼 (다만 이후 decode에 이득) |
| Decode (토큰 생성) | **KV 읽기 대역폭·용량** ← 본진 | Q 투영, MLP 등 |

정리:

- “GQA = 학습이 무조건 빨라진다”는 과장.  
- “GQA = **긴 컨텍스트 서빙이 싸진다**”가 정확한 슬로건에 가깝다.

파라미터 측면 (본문 코드 demo 4와 동일 감각):

- \(W_Q\): \(d_{model}\times (n_q d_k)\) — GQA에서도 보통 유지  
- \(W_K, W_V\): \(d_{model}\times (n_{kv} d_k)\) — **\(n_{kv}/n_q\)로 축소**  
- \(W_O\): \(d_{model}\times d_{model}\) — 유지

즉 모델 가중치도 조금 줄지만, **체감 이득의 대부분은 KV cache**다.

---

### 9.7 Decode 한 스텝을 시간순으로

토큰 \(t\)를 생성할 때 (한 레이어, causal):

```text
1) 새 히든 x_t 입력
2) q_t, k_t, v_t 투영  (GQA면 k_t,v_t는 n_kv개 head분만)
3) 캐시에 k_t, v_t append
4) q_t 가 캐시된 K_{1:t}, V_{1:t} 에 attend
5) head concat + W_O
6) (이후 FFN, 다음 레이어…)
```

여기서 4번의 **읽기량**이 \(n_{kv}\)에 비례한다.  
\(n_q=64, n_{kv}=8\)이면 MHA 대비 캐시·읽기가 약 1/8.

---

### 9.8 Self-Attention vs Cross-Attention (한 줄)

이 문서·영상·Llama식 LLM의 GQA는 주로 **Self-Attention (디코더)** 이야기이다.

- Self: Q,K,V가 같은 시퀀스에서 옴  
- Cross (인코더-디코더): Q는 디코더, K/V는 인코더 출력

GQA의 “K/V head 공유” 아이디어는 cross에도 적용 가능하지만,  
KV cache 병목이 특히 큰 곳은 **오토리그레시브 디코더의 self-attn**이다.

---

### 9.9 Uptraining을 그림으로

```text
[기존 MHA 체크포인트]
   K heads: k0 k1 k2 k3 k4 k5 k6 k7   (예: H=8)
                 │
                 ▼  mean-pool per group (G=2 → 그룹당 4개)
[GQA 초기화]
   KV heads:  mean(k0..k3) , mean(k4..k7)
                 │
                 ▼  짧은 추가 학습 (~원 pretrain의 5% compute, 논문)
[GQA 모델]
   품질 회복 ≈ MHA, 캐시는 G=2 기준
```

처음부터 GQA로 pretrain하면 mean-pool 단계는 필요 없다 (Llama 2 70B 등).

---

### 9.10 품질이 “거의 비슷”하다는 말의 해석

논문/실무에서 말하는 “≈ MHA”는 보통:

- downstream 벤치 점수 차이가 작다  
- 사람이 체감하는 생성 품질 차이가 크지 않다  

를 뜻한다.  
**수학적으로 동치**라는 뜻이 아니다.  
표현력 상한은 MHA가 더 넓을 수 있으나, 데이터·스케일·서빙 제약 하에서 GQA가 충분히 좋은 점이다.

---

### 9.11 MLA를 GQA 다음에 읽는 최소 지도

```text
GQA/MQA:  “헤드를 몇 개 둘까?”     →  공유로 캐시 ↓
MLA:      “캐시에 무엇을 넣을까?” →  latent로 캐시 ↓↓
```

GQA를 이해했다면 MLA에서 확인할 질문:

1. 캐시 텐서의 shape가 \(n_{kv}\times d_k\)가 아니라 **latent dim**인가?  
2. 복원(또는 absorb) 비용이 decode 한 스텝에 얼마나 붙는가?  
3. RoPE를 latent와 어떻게 같이 다루는가? (구현 난이도의 핵심)

세부 수식은 후속 노트. 이 완전판에서는 **문제 의식의 연속성**만 고정한다.

---

### 9.12 한 페이지 총정리 (그림)

```text
입력 토큰 열 X
    │
    ├─► Q: n_q 개 head          (매 스텝 새로, 보통 캐시 X)
    ├─► K: n_kv 개 head ──┐
    └─► V: n_kv 개 head ──┼──► KV Cache (길이 L에 비례)
                          │
    Attention(Q, K, V) ◄──┘
    │
    Concat heads → W_O → 출력

설정:
  n_kv = n_q  → MHA
  1 < n_kv < n_q → GQA
  n_kv = 1  → MQA
```

---

<a id="part-10-코드-라인별-해설"></a>

## PART 10. 코드 라인별 해설

> 아래는 완전판에 수록된 Python을 **읽는 순서**로 해설한다.  
> (실행은 여전히 `code/run_demo.py` 또는 동일 로직을 복사해 사용.)

### 10.1 `scaled_dot_product_attention`

1. `scores = Q @ K^T / sqrt(d_k)` — 관련도 행렬  
2. `causal=True`이면 미래 위치(상삼각)를 \(-10^9\)로 막아 softmax≈0  
3. `softmax`는 마지막 축(key 축)  
4. `out = weights @ V` — Value 가중합  

반환의 `weights`는 디버깅·시각화용이다. 서빙에서는 보통 저장하지 않는다.

### 10.2 `split_heads` / `merge_heads`

- split: `(B,L,H*D) → (B,H,L,D)`  
  head를 배치 옆 차원으로 빼서, head별로 독립 matmul 하기 좋게 만듦.  
- merge: 반대로 concat.

영상에서 “네모를 head로 쪼갠다”와 대응.

### 10.3 `repeat_kv`

GQA의 교육용 핵심 트릭.

- 입력 `(B,G,L,D)`  
- 각 KV head를 `n_rep = H/G`번 복제 → `(B,H,L,D)`  
- 이후는 MHA와 동일한 SDPA 호출  

**주의:** 이 복제는 계산 그래프상의 펼치기일 뿐,  
실제 프로덕션에서는 flash-attention이 GQA를 native로 처리해  
불필요한 메모리 복사를 피하기도 한다.  
수학 결과는 같다.

### 10.4 `GroupedQueryAttention.__init__`

- `W_q`: 출력 폭 `H * d_head`  
- `W_k`, `W_v`: 출력 폭 `G * d_head` ← **여기가 MHA 대비 줄어든 부분**  
- `n_rep = H // G`

`n_kv_heads == H`이면 MHA, `== 1`이면 MQA가 **같은 클래스**로 표현된다.  
스펙트럼이 코드 한 줄(`n_kv_heads`)로 보인다는 점이 교육적으로 좋다.

### 10.5 `kv_cache_bytes`

공식 그대로:

\[
B \times L \times n_{kv} \times d_k \times 2 \times b \times n_{layers}
\]

`n_q_heads` 인자가 compare 출력에만 쓰이고, **바이트 계산에는 안 들어간다.**  
이것 자체가 “Q는 캐시 공식에 없다”는 사실을 코드로 증명한다.

### 10.6 `run_demo.py`가 보여주는 것

| 데모 | 배우는 것 |
|------|-----------|
| 1 SDPA | 행 합=1, 출력 shape = L×D |
| 2 GQA shapes | 영상 예 H=4,G=2의 실제 shape |
| 3 spectrum | 같은 모듈로 MHA/GQA/MQA |
| 4 param width | W_k+W_v가 n_kv에 비례해 줄음 |
| 5 cache estimate | GiB 단위 체감 |

---

<a id="part-11-영상-타임라인-매핑"></a>

## PART 11. 영상 타임라인 ↔ 본문 매핑

| 영상 시간 | 내용 | 이 완전판에서 |
|-----------|------|----------------|
| 0:00–0:16 | GQA를 왜 배우는지 예고 | PART 0, PART 01 |
| 0:18–2:55 | Attention 기본 (QKᵀ, √dₖ, softmax, V) | PART 02, §9.1, §9.5 |
| 2:58–6:23 | Multi-Head (관점, Q/K도 분리) | PART 03, §9.3 |
| 6:25–8:17 | GQA 발상·짝짓기·직관적 성능 예상 | PART 04, §9.2–9.4 |
| 8:19–8:48 | 실제 성능≈MHA + KV cache 이득 | PART 04§3, PART 05, §9.6–9.7 |
| 8:51–9:11 | MLA 예고 | PART 08, §9.11 |
| (전체 자막) | 원문 | PART 00 |

---

### 검토 메모 (빠진 것 → 반영)

1. **PART 00 영상 대본** — 목차에는 있었으나 본문에서 누락되어 있어 **전문을 재삽입**했다.  
2. Q/K/V 역할, 공유해도 α가 다른 이유, repeat_kv와 캐시의 차이 — 설명 보강.  
3. 왜 품질이 유지되는지, FLOPs vs 대역폭, decode 타임라인, uptraining 그림 — 추가.  
4. 코드 읽는 법, 영상 시간↔본문 표 — 추가.

## 끝

이 완전판에는 다음이 **전부** 포함되어 있습니다.

- README.md (PART 0)
- 00_영상_대본.md (PART 00) — 전문
- 01~08 스터디 md (PART 01~08)
- code/attention_variants.py, kv_cache_estimate.py, run_demo.py
- requirements.txt, .gitignore
- **PART 09~11 보충·심화·코드해설·타임라인 매핑** (완전판 전용 보강)
