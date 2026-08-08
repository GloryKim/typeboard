# Kimi K3 완전 정복 — LLM 구조 발전과정으로 읽는 오픈 프론티어 모델

> **문서 성격**: Moonshot AI의 **Kimi K3**(2026.07.17 공개)를 아키텍처·학습·포스트트레이닝·인프라·벤치마크까지 상세 정리한 기술 심층 보고서.
> **1차 근거**: Kimi K3 Tech Report(arXiv **2607.24653**), Kimi Linear 논문(arXiv **2510.26692**), Moonshot 공식 저장소/블로그, 그리고 노토랩 세미나 슬라이드([노토랩X수도리무브] Kimi K3 이해하기).
> **팩트체크 표기**: 본문 수치·구조는 Tech Report 및 공식 저장소 원문에서 교차 확인했습니다. 세미나 슬라이드에만 등장하고 원문에서 확인되지 않은 항목은 별도로 표시합니다.

---

## 0. 핵심 요약 (TL;DR)

Kimi K3는 **"두 개의 스케일링 축(사전학습 규모 + 테스트타임 추론)"을 동시에 프론티어까지 밀어붙인** 오픈-웨이트 네이티브 멀티모달 MoE 모델이다.

| 항목 | 값 |
|---|---|
| 공개일 | 2026.07.17 |
| 총 파라미터 | **2.8T** (2.78T) |
| 활성 파라미터 | **104B** (104.2B) |
| 구조 | Mixture-of-Experts (MoE) |
| 레이어 수 | **93** |
| 컨텍스트 길이 | **1,048,576 토큰 (1M)** |
| 어텐션 | **Hybrid**: KDA 69개 + Gated MLA 24개 (블록당 3:1) |
| MoE | **Stable LatentMoE** — 896 라우팅 전문가 중 **16개 활성** (+ 공유 전문가 2) |
| 활성함수 | **SiTU-GLU** (Sigmoid Tanh Unit GLU) |
| 위치 인코딩 | **NoPE** (No Positional Encoding) |
| 옵티마이저 | **Per-Head Muon** |
| 비전 인코더 | **MoonViT-V2** (0.4B, 27층, from-scratch) |
| 양자화 | **MXFP4 가중치 / MXFP8 활성값** (QAT) |
| 스케일링 효율 | Kimi K2 대비 약 **2.5×** 개선 |
| 위상 | Claude Fable 5 · GPT-5.6 Sol에는 다소 뒤지나, **평가된 다른 오픈/프로프라이어터리 모델은 대체로 상회**. **세계 최초 오픈 3T급 모델**. |

**한 줄 요지**: K3의 진짜 기여는 새 알고리즘 하나가 아니라, **긴 컨텍스트를 감당하는 어텐션(KDA), 깊이 방향 정보 흐름(AttnRes), 극단적 희소 MoE(Stable LatentMoE)**를 안정적으로 결합해 **정보 흐름을 토큰·깊이·너비 세 방향으로 동시에 스케일링**한 데 있다.

---

## 1. 배경: Kimi 계보와 문제의식

### 1-1. Kimi 라인업 타임라인

| 시점 | 모델 | 키워드 |
|---|---|---|
| 2025.07 | Kimi K2 | Open Agentic Intelligence |
| 2026.01 | Kimi K2.5 | Visual Agentic Intelligence |
| 2026.04 | Kimi K2.6 | Advancing Open-Source Coding |
| **2026.07** | **Kimi K3** | **Open Frontier Intelligence** |

### 1-2. 왜 K3인가 — 두 개의 스케일링 축

- 전통적 스케일링 = **사전학습**(더 큰 모델 · 더 많은 데이터).
- 최근의 두 번째 축 = **테스트타임 컴퓨팅**(추론/에이전트 강화학습): OpenAI o-시리즈, Anthropic extended-thinking, DeepSeek-R1, Kimi K1.5 등.
- 문제: 오픈소스 생태계는 두 번째 축은 빠르게 따라잡았지만 **첫 번째 축(파라미터 규모)은 대부분 1T급에 정체**되어, 강한 프로프라이어터리 모델과의 격차가 벌어질 위험.
- **K3의 선택**: 두 축을 **동시에** 프론티어로 — 사전학습을 **3T급**으로 키우면서, 강화학습·추론 노력·장기 상호작용을 **1M 컨텍스트**에서 스케일링.

---

## 2. 아키텍처 심층 분석

K3 아키텍처는 **정보 흐름을 세 방향으로 스케일링**하도록 설계됐다.

| 방향(dimension) | 담당 컴포넌트 | 목적 |
|---|---|---|
| **시퀀스(토큰)** | Hybrid Attention (KDA + Gated MLA) | 긴 컨텍스트 토큰 혼합 |
| **깊이(레이어)** | Attention Residuals (AttnRes) | 레이어 간 선택적 정보 검색 |
| **너비(채널)** | Stable LatentMoE | 희소 채널 혼합 |
| (입력) | MoonViT-V2 | 네이티브 비전 |

> 블록 구성: **각 블록 = KDA×3 + Gated MLA×1**, 각 어텐션 레이어 뒤에 Stable LatentMoE FFN이 붙는다. 백본 맨 끝에는 Gated MLA를 하나 더 두어 **마지막 레이어는 항상 전역 어텐션**이 되도록 한다.

### 2-1. 어텐션 발전 방향 (KDA를 이해하기 위한 배경)

세미나 슬라이드가 잘 정리한 "왜 이렇게까지 왔는가"의 흐름:

1. **기본(Full) 어텐션**은 새 토큰마다 모든 토큰과의 관계 + Softmax를 계산 → **시퀀스 길이의 제곱(O(n²))**에 비례 → Long Context에서 계산·메모리가 폭발.
2. **압축/범위 축소**로 오버헤드를 줄이자 → **Linear Attention / Recurrent State(Mamba 계열)**, **Sliding-Window Attention**. 중간 상태를 메모리(State)로 압축.
3. **압축하면 정보가 충돌·희석된다** → **DeltaNet / Gated DeltaNet**으로 중간중간 상태를 "수정(기존 성분 제거 후 추가)".
4. **그래도 압축으로 잃는 전역 정보를 어떻게 보완?** → **평소엔 Linear로 처리하다가 중간중간 전역 Attention을 섞는 하이브리드**. (Kimi K3, Qwen 3.5, Nemotron 3 등 최신 흐름)

### 2-2. KDA (Kimi Delta Attention) — 채널별 감쇠 게이트

KDA는 **delta-rule recurrence를 채널별(channel-wise) forget gate로 확장**한 선형 어텐션 모듈이다. (Gated DeltaNet이 head 단위의 거친 forget gate를 쓰는 반면, KDA는 **각 feature 차원마다 독립적인 감쇠율**을 둔다 — GLA 스타일의 미세한 게이팅.)

**핵심 recurrence** (단일 head, 은닉상태 `S_t ∈ ℝ^{d_k×d_v}`):

$$\mathbf{S}_t = (\mathbf{I} - \beta_t \mathbf{k}_t \mathbf{k}_t^\top)\,\mathrm{Diag}(\boldsymbol{\alpha}_t)\,\mathbf{S}_{t-1} + \beta_t \mathbf{k}_t \mathbf{v}_t^\top, \qquad \tilde{\mathbf{o}}_t = \mathbf{S}_t^\top \mathbf{q}_t$$

- `α_t ∈ (0,1)^{d_k}`: **채널별 1-step 유지(retention) 계수** (감쇠 게이트)
- `β_t ∈ (0,1)`: delta-rule **쓰기 강도**
- q/k/v는 `ShortConv → Swish`를 거치고, q·k는 추가로 `L2Norm`.

#### K3가 Kimi Linear 대비 바꾼 3가지 (하드웨어 효율/안정성)

1. **Lower-bounded decay (하한 감쇠)**: Kimi Linear의 unbounded `-Softplus` 대신 **scaled sigmoid**로 log-decay에 하한을 둔다.
   - `g_t = g_min · Sigmoid(e^{A_h} z_t)`, `α_t = exp(g_t)`, **`g_min = -5` 고정**.
   - 효과: 유지계수 `α > e^{-5} ≈ 6.7e-3`, 16-토큰 타일의 누적 log-decay가 `(-80, 0)` 범위 → 역수 rescaling이 `e^{80}` 미만으로 **BF16 동적 범위 내**. 덕분에 **대각/비대각 타일 모두 Tensor Core dense matmul**로 처리 가능 → Kimi Linear에서 병목이던 대각 타일의 position-pair 계산을 제거.
2. **Full-rank output gate**: 저차원(low-rank) 출력 게이트를 **입력 의존 full-rank 투영**으로 교체.
   - `y_t = W_o [ Sigmoid(W_g x_t) ⊙ RMSNorm(õ_t) ]`
3. **Chunkwise 병렬 형태**: 청크 간 recurrent, 청크 내 parallel. rank-1 변환들을 dense 표현으로 압축(UT transform), Diagonal-Plus-Low-Rank(DPLR) 전이행렬의 전용 변형으로 계산량 절감.

> **직관**: DeltaNet이 "누적만 하고 지우지 못하는" 문제를 delta-rule로 해결하고, Gated DeltaNet이 "문맥 경계에서 전역 감쇠"를 추가했다면, **KDA는 감쇠를 채널별로 세분화**해 유한한 RNN 메모리를 훨씬 정밀하게 제어한다.

#### 예시 코드 — KDA recurrence (교육용 NumPy 의사구현)

```python
import numpy as np

def kda_step(S, q, k, v, alpha, beta):
    """
    KDA 단일 스텝 (Eq. 1). 교육용 참조 구현 (실제 커널은 chunkwise + Tensor Core).
      S     : (d_k, d_v) recurrent state
      q,k   : (d_k,)   L2Norm(Swish(ShortConv(Wx)))
      v     : (d_v,)
      alpha : (d_k,)   채널별 retention (0,1)  = exp(g_min * sigmoid(e^A z))
      beta  : scalar   delta-rule write strength (0,1)
    """
    S = S * alpha[:, None]                    # 채널별 감쇠 Diag(alpha) S_{t-1}
    S = S - beta * np.outer(k, (k @ S))       # (I - beta k k^T) 로 기존 성분 제거
    S = S + beta * np.outer(k, v)             # 새 정보 delta 추가
    o = S.T @ q                               # õ_t = S^T q
    return S, o

def kda_sequence(Q, K, V, Alpha, Beta):
    T, d_k = Q.shape
    d_v = V.shape[1]
    S = np.zeros((d_k, d_v))
    outs = []
    for t in range(T):
        S, o = kda_step(S, Q[t], K[t], V[t], Alpha[t], Beta[t])
        outs.append(o)
    return np.stack(outs)  # (T, d_v)
```

### 2-3. Gated MLA — MLA(DeepSeek-V2) + Gated Attention(Qwen3-Next) + NoPE

전역 상호작용을 담당하는 3:1의 "1" 계층.

- **MLA (Multi-head Latent Attention)**: DeepSeek-V2([arXiv:2405.04434](https://arxiv.org/abs/2405.04434))가 도입. **KV를 저차원 latent 벡터 `c_t = W_c x_t`로 압축 저장**하고, 어텐션 시 up-projection으로 복원 → **KV 캐시 footprint 대폭 감소**하면서 전역 토큰-토큰 어텐션 유지. K2/K2.5도 채택.
- **Gated Attention**: MLA 출력에 **입력 의존 채널별 full-rank sigmoid 게이트**를 곱한다.
  - `y_t = W_o [ Sigmoid(W_g x_t) ⊙ õ_t ]`
  - 목적: **attention sink 회피** + 채널별 선택적 통과. (Qwen3-Next의 Gated Attention 아이디어 계열, [arXiv:2505.06708](https://arxiv.org/abs/2505.06708))
- **NoPE 적용**: K2/K2.5와 달리 **모든 MLA 레이어에 명시적 위치 인코딩을 쓰지 않는다.** 위치·최근성 정보는 사이사이 KDA가 담당, MLA는 순수 전역 콘텐츠 상호작용을 담당. → **컨텍스트 길이 확장 시 RoPE 주파수 재조정이나 YaRN이 불필요**.
- 학습 시 flash attention의 편향된 라운딩 오차를 막기 위해 **어텐션 출력을 FP32로 유지**.

### 2-4. Attention Residuals (AttnRes) — 깊이 방향의 "어텐션"

**문제의식**: 표준 residual(`h ← h + Attn(Norm(h))`)은 모든 이전 레이어 정보를 **하나의 상태로 균일 누적** → RNN이 시간축에서 겪던 것과 같은 **깊이 방향 병목**.

**아이디어**: 시퀀스에서 recurrence를 어텐션으로 바꿨듯, **깊이(depth)에도 어텐션을 적용**. 각 레이어가 **이전 모든 레이어 출력에 대해 학습된 가중치(softmax)**로 선택적으로 접근한다.

**Full AttnRes** — 레이어 `l`마다 학습형 pseudo-query `q_l = w_l`:

$$\alpha_{i\to l} = \frac{\phi(q_l, k_i)}{\sum_{j=0}^{l-1}\phi(q_l, k_j)}, \quad h_l = \sum_{i=0}^{l-1}\alpha_{i\to l}\, v_i, \quad \phi(q,k)=\exp(q^\top \mathrm{RMSNorm}(k))$$

- `RMSNorm`이 출력 크기가 큰 레이어의 과도한 지배를 방지.
- **Block AttnRes**: `L`개 레이어를 `N`개 블록으로 나눠 블록 단위 표현에만 전역 어텐션 → 메모리/통신 오버헤드를 `O(Ld) → O(Nd)`로 축소.
- **K3 설정**: `N ≈ 8`이면 대부분 이득 회복. K3는 **12-레이어 크기의 8블록**(임베딩 포함 시 9블록)으로 분할.

**효과**: 깊이에 따른 출력 크기가 더 일정, gradient 분포가 더 균일, downstream 전 영역 성능 개선.

#### 예시 코드 — Attention Residual (깊이 방향 softmax) 의사구현

```python
import torch, torch.nn.functional as F

def attn_residual(layer_outputs, w_l):
    """
    layer_outputs: list of (B, D)  = [embedding, f_1(h_1), ..., f_{l-1}(h_{l-1})]
    w_l          : (D,)  현재 레이어의 학습형 pseudo-query
    반환         : (B, D)  깊이 방향 가중합 h_l
    """
    K = torch.stack([F.rms_norm(k, (k.shape[-1],)) for k in layer_outputs], dim=1)  # (B, L, D)
    scores = torch.einsum("d,bld->bl", w_l, K)          # q^T RMSNorm(k)
    alpha  = torch.softmax(scores, dim=-1)              # 깊이 방향 softmax
    V = torch.stack(layer_outputs, dim=1)               # (B, L, D)
    return torch.einsum("bl,bld->bd", alpha, V)         # sum_i alpha_i v_i
```

### 2-5. Stable LatentMoE — 극단적 희소 MoE의 안정화

**MoE 발전 흐름** (슬라이드 정리):

| 세대 | 아이디어 | 대표 |
|---|---|---|
| 2021 | 토큰마다 최적 전문가 MLP 호출 | Switch Transformers ([2101.03961](https://arxiv.org/abs/2101.03961)) |
| 2024 | 더 sparse하게 (fine-grained + shared expert) | DeepSeekMoE ([2401.06066](https://arxiv.org/abs/2401.06066)) |
| 2025 | Expert 신호를 **latent로 압축** | LatentMoE (NVIDIA 계열) |
| **2026** | 여기에 **안정화** 장치 추가 | **StableLatentMoE (Kimi K3)** |

**LatentMoE의 핵심**: 모델 전체 너비 `d`와 **라우팅 전문가 너비 `ℓ`를 분리**. 공유 전문가는 full-width, 라우팅 전문가는 **압축 latent 공간(`ℓ`)에서 동작** → 전문가 수를 크게 늘려도 통신·가중치 트래픽이 억제된다. K3는 이 덕에 **896 전문가 / 토큰당 16 활성**(sparsity 56)까지 확장.

**두 가지 실패 모드와 K3의 3대 안정화 장치**:

- 실패① 라우팅 경로가 `W↓ → 전문가 FFN → W↑`로 **연쇄 행렬곱** → 2.8T 규모에서 **내부 활성값 폭발**.
- 실패② 약 10³개 전문가의 부하 균형이 기존 auxiliary-loss-free bias 업데이트의 안정 영역을 벗어남.

| 장치 | 내용 |
|---|---|
| **Normalized LatentMoE** | 전문가 집계 후 up-projection 전에 **RMSNorm** 삽입 → 스케일 변동 둔감화 (검증 loss·downstream 개선) |
| **SiTU-GLU** | SwiGLU의 unbounded 곱셈항을 **scaled tanh(soft-cap)**로 제한 (`β1=4`, `β2=25`) → 활성값 폭발/저정밀 오버플로 억제 |
| **Quantile Balancing (QB)** | 각 전문가의 라우터 점수 **quantile**에서 목표 부하에 맞는 bias를 직접 설정 → 고정스텝 heuristic 및 민감한 하이퍼파라미터 제거 |

**SiTU-GLU 정의**:

$$\mathrm{SiTU\text{-}GLU}(x) = \left[\beta_1 \tanh\!\left(\tfrac{W_g x}{\beta_1}\right)\odot \mathrm{Sigmoid}(W_g x)\right]\odot\left[\beta_2 \tanh\!\left(\tfrac{W_u x}{\beta_2}\right)\right]$$

원점 근처에서는 SwiGLU와 거의 같고, 큰 양수 입력에서 `|f(x)| ≤ β1·β2 = 100`으로 **유계**.

#### 예시 코드 — SiTU-GLU (PyTorch)

```python
import torch, torch.nn as nn

class SiTUGLU(nn.Module):
    def __init__(self, d_in, d_hidden, beta1=4.0, beta2=25.0):
        super().__init__()
        self.Wg = nn.Linear(d_in, d_hidden, bias=False)
        self.Wu = nn.Linear(d_in, d_hidden, bias=False)
        self.b1, self.b2 = beta1, beta2

    def forward(self, x):
        g = self.Wg(x)
        u = self.Wu(x)
        gate = self.b1 * torch.tanh(g / self.b1) * torch.sigmoid(g)  # soft-capped Swish
        up   = self.b2 * torch.tanh(u / self.b2)                      # soft-capped linear
        return gate * up   # |output| <= beta1*beta2 = 100 로 유계
```

### 2-6. NoPE — 위치 인코딩의 진화와 K3의 선택

| 세대 | 방식 | 컨텍스트 | 한계 |
|---|---|---|---|
| 1세대 | Sinusoid 벡터 더하기 | ~4k | 짧음 |
| 2세대 | **RoPE** (어텐션 단계에서 회전, [2104.09864](https://arxiv.org/abs/2104.09864)) | ~32k | Position OOD |
| 2.5세대 | **YaRN** (Position Interpolation + Bandwise Scaling + Attention Temperature, [2309.00071](https://arxiv.org/abs/2309.00071)) | 32k~256k, 일부 1M | 재튜닝 필요 |
| 3세대 | **NoPE** (No Positional Encoding) | — | — |

- **NoPE의 전제**: LLM 학습이 기본적으로 위치 정보를 (인과적 마스킹·게이팅 등을 통해) 이미 담고 있으므로, 명시적 PE가 없어도 된다는 접근. 실무적으로는 RoPE+NoPE 혼합 또는 NoPE-only로 접근([2501.18795](https://arxiv.org/abs/2501.18795)).
- **K3의 실제 구현**: 위치 정보를 **KDA의 recurrent 게이팅·감쇠로 암시적으로 인코딩**. 덕분에 **PE 수정 없이 1M 토큰으로 직접 외삽(extrapolate)** 가능 — RoPE rescaling이나 YaRN 보간이 불필요.

### 2-7. Per-Head Muon 옵티마이저

- K2가 도입한 **Muon**을 어텐션 투영에 대해 **head 단위 변형**으로 정제.
- 전체 Q/K/V 행렬을 한 덩어리로 Newton–Schulz 직교화하면 gradient 스케일이 큰 head가 공유 업데이트 방향을 지배 → **head별로 분리 직교화**해 head 간 업데이트 스케일 균등화.
- 효과: head 간 학습 동역학이 균형적, 대규모에서 안정성↑, 옵티마이저 오버헤드도 소폭 감소.

### 2-8. 네이티브 비전 — MoonViT-V2

- 텍스트·이미지·비디오를 **하나의 공유 백본**이 단일 컨텍스트에서 처리(사후 정렬 단계 없음).
- **MoonViT-V2**: 27층 ViT, 약 **0.4B(401M)** 파라미터, patch size 14, head 12. **from-scratch로 next-token prediction 학습** (SigLIP 등 대조학습 사전학습 인코더 미사용) → 학습 안정성↑(gradient norm 낮고 spike 적음), 대조학습 초기화가 대규모 멀티모달에 필수가 아님을 시사.
- 이미지/비디오 파라미터 공유(공간 intra-frame + 시간 inter-frame 어텐션), 투영 전 **2×2 pixel-shuffle 다운샘플링**으로 비주얼 토큰 1/4 축소 → 최대 3584×3584 입력을 1M 컨텍스트 안에서 감당.

### 2-9. Kimi K2 → K3 아키텍처 비교 (Tech Report Table 1)

| 항목 | Kimi K2 | Kimi K3 | 변화 |
|---|---|---|---|
| 레이어 수 | 61 | **93** | ↑52% |
| 총 파라미터 | 1.04T | **2.78T** | ↑167% |
| 활성 파라미터 | 32.6B | **104.2B** | ↑220% |
| 은닉 차원 | 7,168 | 7,168 | = |
| Latent MoE 차원 | – | **3,584 (0.5×)** | 신규 |
| 전문가당 MoE 은닉차원 | 2,048 | **3,072** | ↑50% |
| 라우팅 전문가 | 384 | **896** | ↑133% |
| 토큰당 활성 전문가 | 8 | **16** | ↑100% |
| 공유 전문가 | 1 | **2** | ↑100% |
| 어텐션 head | 64 | **96** | ↑50% |
| 학습 컨텍스트 | 128K | **1M** | 8× |
| 어텐션 방식 | MLA | **Hybrid KDA–MLA** | – |
| 활성함수 | SwiGLU | **SiTU-GLU** | – |
| 어텐션 구성 | 61 MLA | **69 KDA + 24 MLA** | – |
| 어휘 크기 | 160K | 160K | = |

---

## 3. 사전학습(Pre-Training)

- **데이터**: Web Text · Code · Mathematics · Knowledge 4개 텍스트 도메인 + 대규모 비전 코퍼스(캡션, interleaved 문서, OCR, perception, video, visual coding). 규칙 기반 + 분류기 품질 점수 + 중복제거, 지식·수학은 K2의 rephrasing 레시피로 재작성.
- **스케일링 법칙**: batch size, LR, tokens-per-parameter(TPP), model shape를 재튜닝. 결과적으로 **K2 대비 약 2.5× 스케일링 효율**. LR 스케줄은 **cosine decay**가 WSD보다 우수(각자 최적 하이퍼파라미터 하에서 비교).
- **학습 레시피**: Per-Head Muon + K2의 weight-clipping, QB로 MoE 부하 균형, cosine LR + 1% linear warmup, weight decay 0.1.
- **롱컨텍스트 4단계 커리큘럼**:
  - 사전학습: **8K → 64K**
  - 쿨다운: **256K → 1M**
  - 비용이 큰 롱시퀀스 계산을 전체 예산의 일부에 집중. 자연 롱문서/비디오는 정제·업샘플, **여러 문서·서브태스크를 순열·연결**해 1M 전 구간을 참조해야 풀리는 합성 롱컨텍스트 태스크 생성(어텐션이 로컬 패턴으로 퇴화하는 것을 방지).
- **위치 인코딩**: NoPE → PE 수정 없이 1M로 외삽.

---

## 4. 포스트트레이닝(Post-Training)

3단계 파이프라인: **SFT → RL(도메인×노력 전문가) → MOPD(통합)**.

### 4-1. SFT
- 고품질 cold-start 정책 확립. 복잡한 에이전틱 태스크 커버리지 확대, 이전 Kimi 도메인 특화 모델로 궤적 합성 + 다단계 검증 + 사람 개입 주석. **XTML 기반 chat template**(eXtensible Token Markup Language)로 직렬화. SFT 단계부터 **QAT(MXFP4/MXFP8)** 시작.

### 4-2. RL — 9개 전문가(3 도메인 × 3 노력)
- 도메인: **(i) general**(경험·비전·추론·faithfulness·검색·지식노동), **(ii) general agents**(장기 어시스턴트·deep research·문단 작문), **(iii) coding agents**(SWE·코딩·커널·웹개발).
- 각 도메인 × **추론 노력 {low, high, max}** = **총 9개 전문가**.
- **알고리즘**: 동기식 RL의 **partial rollout** 확장 — `N` 프롬프트 × `K` 완성 중 비율 `λ`가 끝나면 최적화 진행(straggler 회피), 일시정지 궤적은 다음 iteration에서 재개(샌드박스 인프라). per-token 정규화로 극단적 off-policy(데이터 staleness) 안정 처리.
- **추론 노력 RL**: 문제별 토큰 예산 `b0(x)` 대비 `τ·b0(x)` 초과 시 보상 -1로 과잉 사고 억제. `τ`를 커리큘럼으로 줄여 max→high→low 전문가 도출.
- **Agentic GRM**(생성 보상 모델): 비검증 태스크에 토너먼트식 이진 비교, rubric 생성→채점→scorepad. verbosity 예산으로 장황함 보상해킹 억제.

### 4-3. MOPD — Multi-Teacher On-Policy Distillation
- 9개 전문가를 **단일 모델로 통합**. 도메인 `d`·노력 `e`마다 해당 teacher가 지도.
- **on-policy** 핵심: teacher의 데이터가 아니라 **student 자신의 롤아웃(중간 출력)에 대해** teacher와의 per-token log-비율을 dense reward로 사용.

$$r^{d}_{\mathrm{opd}}(y_t\mid e,x,y_{<t}) = \mathrm{clip}\!\left(\mathrm{sg}\!\left(\log\frac{\pi^{(d,e)}_{\text{teacher}}(y_t\mid x,y_{<t})}{\pi_\theta(y_t\mid e,x,y_{<t})}\right), -R_{\max}, R_{\max}\right)$$

> 이 개념은 Thinking Machines Lab의 [On-Policy Distillation](https://thinkingmachines.ai/blog/on-policy-distillation/)과 궤를 같이한다. 슬라이드는 그 계보를 "MiMo → DeepSeek V4 → Kimi K3"로 소개.

### 4-4. 배포 지향 학습
- **MXFP4 QAT**: MoE 전문가 가중치를 MXFP4, 활성값 MXFP8로 양자화(비전문가 모듈은 고정밀 유지). SFT·RL 전 구간 QAT로 train–inference mismatch 제거.
- **Draft 모델(EAGLE-3)**: 사전학습된 **MTP(멀티 토큰 예측) 레이어**를 EAGLE-3 스타일 draft로 파인튜닝(7-step unroll), low/mid/high 특징(1st·4th·최종 AttnRes 블록 출력) 융합. acceptance rate 자체를 직접 최적화하는 **LK loss** 사용 → 추측 디코딩(speculative decoding) 가속.

### 4-5. RL 태스크 합성과 에이전틱 환경

K3의 RL 효과는 **풍부하고 다양하며 검증 가능한(verifiable) 환경**에 크게 의존한다. 이를 위해 다양한 화이트박스 환경과 태스크 합성 패러다임을 설계했다.

- **통합 화이트박스 RL 환경(Unified White-Box RL Environment)**: 에이전트 하니스(harness)를 **구성 가능·조합 가능한 모듈 집합**(툴 인터페이스, 시스템 프롬프트, 컨텍스트 관리 전략, skills, memories, subagents 등)으로 표현. 설정만 바꿔 **Kimi Code, Claude Code, Codex, OpenClaw, Hermes** 같은 주류 하니스를 즉석에서 인스턴스화하거나 새 하니스를 구성한다. → 특정 하니스 관습에 과적합되지 않고 **cross-scaffold 일반화**.
- **검증 가능한 에이전틱 문제**: 다단계 정보 검색(계획→증거 수집→검증 가능한 답), 전문 실무(투자은행·데이터 분석·법률: 요청 분해→샌드박스 툴 조작→수십~수백 스텝 산출물), **다단계 시각 추론**(Python 인터프리터 샌드박스에서 이미지 crop/zoom/변환·계산·중간결과 검증을 반복, 생성 이미지까지 새 관측으로 수용).
- **커널 최적화 태스크**: 단일 연산자~fused mega-kernel까지. **CUDA, Triton, CuTe DSL, Gluon, ThunderKittens, TileLang** 등 다양한 GPU 프로그래밍 + **BF16/FP8/FP4** 커버. 보상은 정확성 + 성능(전문가 구현 매칭 시 0.5, roofline 근접 시 1로 증가). **reward-hacking 탐지 시스템**(CUDA graph replay, 입력 캐싱, 정밀도 축소 등 페널티).
- **개인 비서(Personal Assistant) 태스크**: **Gmail·Notion·Slack·Canvas의 현실적 모의 구현**. 며칠에 걸친 지속·진화 환경에서 앱 간 상호의존 이벤트 수십 개, 단일 롤아웃이 **수천 툴콜·수백만 토큰**에 이를 수 있음.
- **자율 실행 태스크(Autonomous Execution Tasks, AET)**: 초기 상태·제약 목표·툴 액션 공간·실행 예산·**독립 검증기(verifier)**만 주고 참조 궤적 없이 **분해·툴 선택·계획·오류복구·종료를 자율 수행**. 보상은 자기보고가 아닌 **검증기의 최종 상태 평가** 기반. 공개 검증기(진단 피드백)와 숨은 검증기(held-out 평가)를 페어링해 reward hacking 억제.
- **웹 개발 태스크**: 한 줄 설명~여러 문단 스펙 입력, 산출물은 웹사이트·게임·3D/WebGL·시각화·SVG·풀스택 앱. 컨테이너 샌드박스 + 다양한 scaffold. 보상 = 결정적 체크(빌드/동작/구조·픽셀 유사도) + 모델 심사. 빌드 실패/에러/가짜 구현 시 보상 0.

### 4-6. 지식 그래프 기반 태스크 합성 (Knowledge-Graph-Guided Task Synthesis)

포스트트레이닝 태스크의 **품질·다양성**을 좌우하는 소스 자료를 확보하기 위해, **자기 진화하는 계층적 지식 그래프**를 구축한다.

- **에이전트 주도 구축**: coarse seed 노드에서 시작해 각 노드에 에이전트를 배정, 웹 검색으로 개념을 조사하며 **DAG(방향성 비순환 그래프)**로 재귀 확장. 중복 최소화를 위해 기존 노드 재사용, 간선은 항상 **coarse → fine** 방향. 개념이 충분히 원자적(atomic)이면 확장 중단.
- **자료 검색 & 합성**: 원하는 도메인/태스크 분포를 노려 여러 granularity의 노드를 샘플링 → 조상 노드 맥락과 결합해 웹 쿼리 구성 → 실제 자료를 모아 합성 에이전트가 다양한 유형의 학습 태스크 생성.

---

## 5. 인프라 (2.8T·1M 규모를 감당하는 시스템)

K3는 **하이브리드 KDA 어텐션 · 3T급 희소 멀티모달 · 밀리언 토큰 에이전틱**이라는, 한 모델에 좀처럼 함께 오지 않는 세 시스템 난제를 동시에 안는다. 인프라는 모델 생애주기 전반에 걸쳐 공동설계됐다.

### 5-1. KDA 알고리즘–시스템 공동설계

KDA는 softmax 어텐션의 커지는 KV 캐시를 **고정 크기 recurrent state `S ∈ ℝ^{d_k×d_v}`**로 대체한다(전송·재사용이 저렴). 단, 직렬 업데이트가 병렬 실행에 불리 → regime별 전용 커널로 해결.

- **FlashKDA (chunkwise 커널)**: 학습·prefill용. CUTLASS 기반으로 **청크 내 계산과 청크 간 상태 전파를 오버랩**, Triton 참조 구현 대비 크게 빠름. `flash-linear-attention` 백엔드로 자동 디스패치.
- **Intra-device Context Parallelism**: 초장문 prefill에서 시퀀스를 **단일 rank의 SM들에 분할**해 세그먼트 전이를 병렬 평가 후 정확히 합성(디바이스 간 통신 0).
- **KDA Context Parallelism (KCP)**: 디바이스 간. KDA는 token 의존 전이행렬 `M_t = (I − β_t k_t k_tᵀ)Diag(α_t)`를 incoming state에 곱하므로 단순 합산(vanilla linear attn 방식)으로는 부정확. KCP는 각 세그먼트 효과를 **① 누적 전이 `M` ② 0에서 생성된 로컬 상태 `S̃`** 두 국소량으로 분해 → **고정 크기 all-gather + prefix scan**으로 선형 compute 스케일링. (DeltaNet CP 기반, FLA PR #691)

### 5-2. 3T급 사전학습 인프라

- 병렬화 조합: **PP(virtual stage) + EP + ZeRO-1 DP + Pipeline ZeRO-2 gradient sharding + CP**. MoE all-to-all(dispatch/combine)을 계산과 오버랩.
- **MoonEP** ([github.com/MoonshotAI/MoonEP](https://github.com/MoonshotAI/MoonEP)) — **완벽한 부하 균형** EP:
  - **동적 redundant expert**: forward에서 라우터 출력으로 계획·prefetch, backward에서 로컬 reduce 후 home rank로 환원.
  - **경계 증명**: rank당 최대 `E/R`개의 redundant expert면 균형 계획이 항상 존재(사실상 tight). → 학습이 중단되지 않음(ECHO/UltraEP의 cap 튜닝·중단 문제 해소).
  - **zero-copy 통신**: fused permute/unpermute로 토큰을 원격 rank의 expert-grouped 위치로 직접 전송, 버퍼는 고정 `S×K`.
  - **static shape / sync-free**: 매 rank가 정확히 `S×K` 토큰 → 레이어별 host 동기화 제거.
- **메모리 효율 학습**: 통합 activation manager(recompute/quantize/offload를 텐서 단위 정책으로 조합), block-wise FP8 activation, SonicMoE식 gradient 재작성, **Block AttnRes companion 최적화**(경계 레이어에서 블록 표현 1회 생성·공유), PP rank 간 activation 원격 오프로드(Mooncake Transfer Engine), **P2P 기반 Muon 직교화**(full-parameter 버퍼 제거).
- **멀티모달 인코더 최적화**: 큰 이미지·긴 비디오에 **Dynamic CP**(patch 차원 분할 + gather-KV), ViT 계산을 **PP 파이프라인 버블에 은닉**(DEP 확장) → 비전 인코더 실효 오버헤드 거의 제거.

### 5-3. 1M 에이전틱 RL 인프라

- **co-located RL** + **partial rollout**로 1M-컨텍스트 실험을 수백 GPU 내로 유지, tail latency 완화.
- **External KV Cache Pool**: 재사용 가능한 idle prefix를 GPU에서 evict될 때만 **CPU DRAM 외부 풀에 write-back**, 재사용 전 prefetch. KDA state는 대응 MLA KV 블록과 lifecycle을 맞춰 함께 오프로드. 학습 state는 iteration 후 NVMe로 오프로드해 DRAM 확보.
- **Rollout auto-throttling**: active/queued request 수, KV 캐시 사용률 등 런타임 신호로 동시성 동적 제어(초반 활용↑, 후반 preemption 방지).
- **Gradient-buffer 재사용**: reference 등 forward-only 비정책 모델 가중치를 정책모델 FP32 gradient 버퍼 슬롯에 스트리밍(추가 할당 없이).

### 5-4. 샌드박스 인프라 — AgentENV

[AgentENV](https://github.com/kvcache-ai/AgentENV)는 **Firecracker microVM** 기반 에이전틱 전용 샌드박스.

- **고충실도 격리**: 컨테이너로는 불가능한 수준(디스크 마운트·컨테이너·VM 실행 허용)의 격리·현실성. (컨테이너 런타임에서 겪던 kernel panic·deadlock 회피)
- **유연한 라이프사이클**: 증분 체크포인트(dirty page만 저장) → **checkpoint 133ms / resume 49ms**. Pause&Resume(추론 대기 중 자원 0 — 샌드박스 수명의 최대 98%가 대기), Fork(부작용 없는 보상 판정용), Snapshot(오류 복구).
- **고효율·고밀도**: OverlayBD + 커스텀 ublk + P2P 전송으로 **sub-second 대량 기동**, copy-on-write로 **메모리 overcommit 최대 6.5×**.
- **규모(팩트)**: K3 학습·평가 전 과정에서 **총 51,219,741개 샌드박스 / 1,505,678개 이미지** 생성.

### 5-5. 추론·온라인 서빙

- **KDA-aware Prefix Cache**: 고정 크기 KDA state를 MLA KV와 **같은 paged pool**에 패킹, 두 캐시를 공동 관리. hash granularity(512토큰)와 물리 블록(1024–6144토큰)을 **분리**해, 하이브리드 모델도 full-attention 수준의 **임의 512-토큰 경계 prefix 재사용** 달성.
- **전용 커널**: KDA 디코딩(MTP 추측 디코딩 시 rollback을 위해 projected input만 캐시 후 on-chip 재구성 — ReplaySSM식), Block AttnRes(inter-block side-stream 오버랩 + intra-block fusion), Stable LatentMoE(latent down-proj + router GEMM 융합, WarpDecode식 token-centric MoE 디코딩).
- **Fleet 스케줄링**: **cache-aware affinity**(세션을 prefix 캐시 보유 클러스터로 라우팅, consistent hashing으로 primary/secondary 이중화), **budget-based admission control**(요청 클래스별 예산 분리 → 롱컨텍스트 버스트가 짧은 요청 TTFT를 망치지 않게).

---

## 6. 벤치마크 (Tech Report Table 2 발췌)

**평가 조건**: Kimi K3는 reasoning effort=max, temperature=1.0. 비교군: Claude Fable 5, GPT-5.6 Sol, Claude Opus 4.8, GPT-5.5(xhigh), GLM-5.2(오픈).

**총평**: K3는 **가장 강한 프로프라이어터리(Claude Fable 5 · GPT-5.6 Sol)에는 전체적으로 다소 뒤지지만**, 평가 스위트의 **다른 오픈/프로프라이어터리 모델은 대체로 상회**. 여러 에이전틱 벤치에서 SOTA.

| 영역 | 벤치마크 | Kimi K3 | Claude Fable 5 | GPT-5.6 Sol |
|---|---|---:|---:|---:|
| 추론·지식 | GPQA Diamond | 93.5 | 92.6 | 94.1 |
| 추론·지식 | AA-LCR | **74.7** | 70.0 | 73.7 |
| 추론·지식 | HLE-Full (no/with tools) | 43.5 / 56.0 | 53.3 / 63.0 | 44.5 / 58.0 |
| 코딩 | ProgramBench | **77.8** | 76.8 | 77.6 |
| 코딩 | Terminal-Bench 2.1 | 88.3 | 88.0 | 88.8 |
| 코딩 | SWE-Marathon | **42.0** | 35.0 | 39.0 |
| 코딩 | FrontierSWE | 81.2 | 86.6 | 71.3 |
| 에이전틱 | BrowseComp | **91.2** | 88.0 | 90.4 |
| 에이전틱 | DeepSearchQA (F1) | **95.0** | 94.2 | - |
| 에이전틱 | MCPMark-Verified | **94.5** | 87.4 | 92.9 |
| 에이전틱 | τ³-Banking | **33.4** | 26.8 | 33.0 |
| 에이전틱 | Harvey Lab-AA | **94.6** | 93.6 | 87.2 |
| 비전 | OmniDocBench | **91.1** | 89.8 | 85.8 |
| 비전 | Video-MME (w/ sub) | **90.0** | - | 89.5 |

> **약점(솔직한 한계)**: 연구급 추론에서 여전히 격차 — HLE-Full·CritPt(23.4)는 Claude Fable 5·GPT-5.6 Sol·GPT-5.5에 뒤진다. Elo 기반 지식노동(GDPval-AA v2 3위, AA-Briefcase 2위)과 일부 컴퓨터-유즈(OSWorld 2.0, SaaS-Bench)도 리드하지 못함.

---

## 7. 추론 비용 (가격 정책)

세미나 슬라이드 6("추론 비용 — OpenRouter")에 대응하는 부분. **오픈-웨이트로 자체 서빙도 가능**하지만, 관리형 API 가격은 아래와 같다. (2026년 7~8월 공개 기준, 세율 별도)

| 항목 | 가격 (USD / 1M tokens) | 비고 |
|---|---:|---|
| 입력 (cache **miss**) | **$3.00** | 처음 보내는/캐시 불가 컨텍스트 |
| 입력 (cache **hit**) | **$0.30** | 동일·유사 컨텍스트 재사용 시 **90% 절감** |
| 출력 | **$15.00** | 추론 트레이스 포함 (입력 대비 5×) |
| 컨텍스트 | 1,048,576 토큰 | 공개 카드상 **long-context 추가요금 없음(flat)** |
| 최대 출력 | ~16K 토큰 | |

**직접 API vs OpenRouter (중요한 함정)**:

| | 직접 Moonshot API (`kimi-k3`) | OpenRouter (`moonshotai/kimi-k3`) |
|---|---|---|
| 가격대 | $0.30 / $3 / $15 | 공개 리스팅상 **$3 / $15** |
| 프롬프트 캐싱 할인 | **적용됨** | **미적용** (모든 입력 $3 과금) |
| 장점 | 캐시 워크로드에서 20~40% 저렴 | 단일 키로 다중 모델·모델 fallback |
| 유의 | 계정·빌링 설정 | K3가 몰릴 때 capacity/429 |

> 즉, **캐시 적중률이 높은 워크로드(긴 프로젝트 컨텍스트 반복 조회 등)는 직접 API가 유리**하고, OpenRouter는 프롬프트 캐싱 할인을 통과시키지 않아 캐시 가능한 워크로드에서 가장 비싼 접근이 될 수 있다. 블렌디드 비용 예: 캐시 90% 적중 시 직접 API ≈ $2.1/1M vs OpenRouter ≈ $3.75/1M.
> **주의**: 가격은 수시로 변동하므로 실제 사용 전 [OpenRouter](https://openrouter.ai/moonshotai/kimi-k3) 및 Moonshot 공식 가격 페이지에서 재확인 필요.

---

## 8. 사용 예시 (개념 코드)

> 아래는 오픈-웨이트(`moonshotai/Kimi-K3`) 사용 흐름의 **개념 예시**입니다. 실제 실행에는 매우 큰 GPU 메모리와 최신 라이브러리 버전이 필요하며, 정확한 API·모델 ID·서빙 옵션은 [공식 저장소](https://github.com/MoonshotAI/Kimi-K3)와 HuggingFace 카드에서 확인하세요.

### 8-1. Transformers (텍스트 추론)

```python
from transformers import AutoModelForCausalLM, AutoTokenizer
import torch

model_id = "moonshotai/Kimi-K3"
tok = AutoTokenizer.from_pretrained(model_id, trust_remote_code=True)
model = AutoModelForCausalLM.from_pretrained(
    model_id, torch_dtype=torch.bfloat16, device_map="auto", trust_remote_code=True,
)

messages = [
    {"role": "system", "content": "You are Kimi, a helpful assistant."},
    {"role": "user", "content": "KDA가 기존 Gated DeltaNet과 다른 점을 3가지로 요약해줘."},
]
inputs = tok.apply_chat_template(messages, add_generation_prompt=True, return_tensors="pt").to(model.device)
out = model.generate(inputs, max_new_tokens=512, temperature=1.0, top_p=0.95)
print(tok.decode(out[0][inputs.shape[-1]:], skip_special_tokens=True))
```

### 8-2. vLLM 서빙 (1M 컨텍스트 · 추측 디코딩 개념)

```bash
# 개념 예시 — 실제 플래그/버전은 공식 문서 확인
vllm serve moonshotai/Kimi-K3 \
  --tensor-parallel-size 8 \
  --max-model-len 1048576 \
  --quantization mxfp4 \
  --speculative-model eagle3 \
  --trust-remote-code
```

### 8-3. 하이브리드 어텐션 블록 구성 (구조 이해용 스켈레톤)

```python
import torch.nn as nn

class KimiK3Block(nn.Module):
    """블록 = KDA×3 + Gated MLA×1, 각 어텐션 뒤에 Stable LatentMoE FFN."""
    def __init__(self, d):
        super().__init__()
        self.attn = nn.ModuleList([KDA(d), KDA(d), KDA(d), GatedMLA(d)])  # 3:1
        self.ffn  = nn.ModuleList([StableLatentMoE(d) for _ in range(4)])

    def forward(self, h, layer_cache):
        for attn, ffn in zip(self.attn, self.ffn):
            h = attn_residual_merge(h, attn, layer_cache)   # AttnRes: 깊이 방향 선택적 검색
            h = h + ffn(h)                                  # 희소 채널 혼합
        return h
```

---

## 9. 정리 — Kimi K3가 말하는 "LLM 구조 발전 방향"

1. **어텐션**: Full(O(n²)) → Linear/Recurrent → DeltaNet/Gated DeltaNet → **하이브리드(KDA 3 : 전역 MLA 1)**. 위치정보는 **NoPE + KDA 게이팅**으로 암시.
2. **깊이**: 균일 residual 누적 → **AttnRes로 레이어 간 어텐션**.
3. **너비(MoE)**: Switch → DeepSeekMoE → LatentMoE → **StableLatentMoE(896/16, RMSNorm + SiTU-GLU + Quantile Balancing)**.
4. **학습/배포**: Per-Head Muon, cosine decay, 4단계 롱컨텍스트 커리큘럼, 9-expert RL → **MOPD 통합**, **MXFP4 QAT + EAGLE-3** 서빙.
5. **의의**: 오픈 진영이 정체됐던 **사전학습 규모 축을 3T급으로** 끌어올리면서, **정보 흐름을 토큰·깊이·너비 세 방향으로 동시에 스케일링**해 K2 대비 약 2.5× 효율을 달성. "세계 최초 오픈 3T급" 네이티브 멀티모달 모델.

---

## 부록 A. 레퍼런스 (팩트체크 결과)

| 자료 | 링크 | 확인 |
|---|---|---|
| Kimi K3 Tech Report — *Open Frontier Intelligence* | https://arxiv.org/abs/2607.24653 | ✅ 원문 교차확인 |
| Kimi K3 공식 저장소 | https://github.com/MoonshotAI/Kimi-K3 | ✅ (Model Summary 표 확인) |
| Kimi K3 공식 블로그 | https://www.kimi.com/blog/kimi-k3 | ✅ |
| Kimi Linear (KDA 원 논문) | https://arxiv.org/abs/2510.26692 | ✅ 원문 교차확인 |
| Kimi Linear 저장소 | https://github.com/MoonshotAI/Kimi-Linear | ✅ |
| DeepSeek-V2 (MLA) | https://arxiv.org/abs/2405.04434 | ✅ 널리 알려진 실 논문 |
| Gated Delta Networks | https://arxiv.org/abs/2412.06464 | ✅ 실 논문 (KDA의 기반) |
| YaRN | https://arxiv.org/abs/2309.00071 | ✅ 실 논문 |
| On-Policy Distillation (Thinking Machines Lab) | https://thinkingmachines.ai/blog/on-policy-distillation/ | ✅ 실 블로그 |
| Gated Attention for LLMs (Attention-Sink-Free) | https://arxiv.org/abs/2505.06708 | ⚠️ 슬라이드 인용, ID 확인 권장 |
| Switch Transformers | https://arxiv.org/abs/2101.03961 | ✅ 실 논문 |
| DeepSeekMoE | https://arxiv.org/abs/2401.06066 | ✅ 실 논문 |
| RoFormer (RoPE) | https://arxiv.org/abs/2104.09864 | ✅ 실 논문 |
| RoPE to NoPE and Back Again | https://arxiv.org/abs/2501.18795 | ⚠️ 슬라이드 인용, ID 확인 권장 |
| LatentMoE | (슬라이드: arXiv 2601.18089) | ⚠️ 슬라이드 인용, 정식 ID 확인 권장 |
| MoonEP (인프라) | https://github.com/MoonshotAI/MoonEP | 테크리포트 각주 |
| AgentENV (샌드박스) | https://github.com/kvcache-ai/AgentENV | 테크리포트 각주 |
| Attention Residuals 공식 구현 | https://github.com/MoonshotAI/Attention-Residuals | 슬라이드 |
| Kimi K3 가격 (OpenRouter) | https://openrouter.ai/moonshotai/kimi-k3 | ✅ 라이브 가격 확인 |
| Kimi K3 가격 (공식/캐시 할인) | https://www.kimi.com/resources/kimi-k3-pricing | ✅ cache-hit $0.30 확인 |

## 부록 B. 핵심 하이퍼파라미터 요약

| 항목 | 값 |
|---|---|
| KDA lower-bound `g_min` | −5 |
| SiTU-GLU soft-cap | `β1=4`(gate), `β2=25`(up) → `|f|≤100` |
| AttnRes 블록 | 12-레이어 × 8블록 (임베딩 포함 9) |
| 공유 전문가 수 | 2 |
| 라우팅 전문가 / 활성 | 896 / 16 (sparsity 56) |
| Latent MoE 차원 | 3,584 (은닉 7,168의 0.5×) |
| 전문가당 MoE 은닉차원 | 3,072 |
| 어텐션 head | 96 |
| 어휘 | 160K |
| 컨텍스트 | 1,048,576 |
| LR 스케줄 | cosine, 1% warmup, weight decay 0.1 |
| 양자화 | MXFP4(가중치) / MXFP8(활성) QAT |

> **팩트체크 주의**: 본 보고서의 수치는 arXiv 2607.24653 및 공식 저장소 기준입니다. 세미나 슬라이드에만 등장하는 일부 arXiv ID(2601.18089, 2505.06708, 2501.18795)는 원문에서 직접 확인되지 않았으므로 인용 시 정식 식별자를 재확인하시기 바랍니다.
