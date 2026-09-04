# Cross-platform target triples (OS + CPU)

빌드·배포 시 자주 보는 **플랫폼 식별자**다.  
형식은 대개 `OS-아키텍처` 또는 `OS-libc`이며, Bun·Node·Go·Rust·Tauri 바이너리 이름에도 비슷하게 붙는다.

관련 노트: [`260802_2343_go+rs+bun_Vendor_cross_build.md`](./260802_2343_go+rs+bun_Vendor_cross_build.md), [`260802_2248_go_vs_rust_crossbuild.md`](./260802_2248_go_vs_rust_crossbuild.md)

---

## 표로 한눈에

| 식별자 | OS | CPU | 대표 기기 / 환경 |
| --- | --- | --- | --- |
| `darwin-arm64` | macOS | Apple Silicon (ARM64) | M1 / M2 / M3 / M4 Mac |
| `darwin-x64` | macOS | Intel (x86_64) | 구형 Intel Mac |
| `Linux-x64` | Linux | x86_64 (amd64) | 일반 서버·데스크톱 PC |
| `Linux-arm64` | Linux | ARM64 (aarch64) | Graviton, Raspberry Pi 64bit, 일부 서버 |
| `Linux-musl` | Linux | (보통 x64/arm64) + **musl** libc | Alpine Linux, 정적에 가까운 배포 |
| `win32-x64` | Windows | x86_64 | 대부분의 Windows PC |

> 표기 대소문자(`Linux` vs `linux`)는 도구마다 다르다. 의미는 같다.

---

## 각 항목 의미

### `darwin-arm64`

- **darwin** = Apple이 쓰는 macOS 커널/플랫폼 이름 (Unix 계열)
- **arm64** = Apple Silicon CPU (`aarch64`와 동일 계열)
- 현재 Mac 개발 머신에서 가장 흔한 타깃

### `darwin-x64`

- macOS + **Intel** `x86_64`
- Rosetta로 arm64 바이너리를 돌릴 수는 있지만, 네이티브 배급은 별도 `darwin-x64` 빌드가 필요할 수 있음
- 신규 Mac은 arm64이므로 점차 비중이 줄어듦

### `Linux-x64`

- Linux 커널 + **x86_64** CPU
- 클라우드 VM·온프레미스 서버의 기본 타깃인 경우가 많음
- libc는 보통 **glibc** (Ubuntu, Debian, RHEL 등)

### `Linux-arm64`

- Linux + **ARM64**
- AWS Graviton, Oracle Ampere, 일부 임베디드/보드
- x64와 **바이너리 호환되지 않음** → 별도 크로스빌드 또는 해당 아키텍처에서 빌드

### `Linux-musl`

- OS는 Linux인데, C 표준 라이브러리가 **glibc가 아니라 musl**
- Alpine Linux 컨테이너에서 자주 필요
- glibc용 `Linux-x64` 바이너리를 Alpine에 그대로 넣으면 **동적 링크 오류**가 날 수 있음 → musl용으로 따로 빌드
- 도구에 따라 `linux-x64-musl`, `linux-arm64-musl`처럼 **CPU까지 합쳐** 표기하기도 함

### `win32-x64`

- **win32** = Windows API/플랫폼 통칭 (이름은 32지만, `x64`면 64비트 Windows)
- **x64** = Intel/AMD 64비트
- Windows용 `.exe` / `.msi` / `.zip` 배포에 해당
- ARM Windows(`win32-arm64`)는 별도 타깃

---

## 왜 나눠야 하나?

1. **CPU ISA가 다르면** 기계어가 다름 (`x64` ≠ `arm64`)
2. **OS ABI·시스템 콜이 다르면** 바이너리가 안 돌아감 (`darwin` ≠ `linux` ≠ `win32`)
3. **libc가 다르면** (glibc vs musl) 같은 Linux라도 동적 링크가 깨질 수 있음

그래서 릴리스는 보통 이렇게 여러 개를 만든다.

```text
app-darwin-arm64
app-darwin-x64
app-linux-x64
app-linux-arm64
app-linux-x64-musl   # 필요 시
app-win32-x64.exe
```

---

## 내 머신 확인 (참고)

```bash
# OS + arch (uname)
uname -s    # Darwin / Linux
uname -m    # arm64 / x86_64 / aarch64

# Node / Bun 식 표기
node -p "process.platform + '-' + process.arch"
# 예: darwin-arm64, linux-x64, win32-x64
```

| `process.platform` | 의미 |
| --- | --- |
| `darwin` | macOS |
| `linux` | Linux |
| `win32` | Windows |

| `process.arch` | 의미 |
| --- | --- |
| `arm64` | ARM 64비트 |
| `x64` | Intel/AMD 64비트 |
| `ia32` | 32비트 x86 (레거시) |
