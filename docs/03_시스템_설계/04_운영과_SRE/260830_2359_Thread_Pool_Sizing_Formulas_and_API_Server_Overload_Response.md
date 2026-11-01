# 자바 API 서버의 스레드 풀 크기 산정과 과부하 대응: 서버 스레드 풀 관리와 오토 스케일링

## 요약

본 보고서는 리눅스 환경에서 동작하는 자바 API 서버를 대상으로 두 가지 문제를 다룬다. 첫째, 스레드 풀의 크기를 정하는 공학적 기준이 있는가. 둘째, API 서버에 과부하가 발생했을 때 어떻게 대응해야 하는가. 두 번째 문제에는 데이터베이스 부하가 없고, 병목은 WAS(웹 애플리케이션 서버)에만 있으며, 서비스 계층은 출력(print)만 수행하고, 외부 시스템 연계는 고려하지 않는다는 전제가 붙는다.

스레드 풀은 스레드 생성 비용과 컨텍스트 스위칭 비용을 줄이기 위해 사용한다. 크기 산정에는 브라이언 게츠의 공식(코어 수 × 목표 CPU 사용률 × (1 + 대기 시간 / 연산 시간))과 리틀의 법칙을 쓸 수 있으나, 이는 초깃값을 정하는 도구일 뿐이며 최종값은 계측과 부하 테스트로 확정해야 한다.

주어진 전제는 I/O 대기가 없는 CPU 바운드 작업을 의미한다. 이 경우 논블로킹 I/O 전환이나 스레드 증설은 처리량을 높이지 못하고, 서버 한 대의 최적 스레드 수는 코어 수에 수렴한다. 따라서 대응은 두 축으로 정리된다. 서버 내부에서는 **스레드 풀을 관리**해 서버 한 대가 무너지지 않고 낼 수 있는 최대 처리량을 확보하고, 서버 외부에서는 **오토 스케일링**으로 서버 대수를 부하에 맞춘다. 여기에 더해 `System.out.println`의 내부 동기화로 인한 병목을 비동기 로깅으로 제거하고, CPU 사용률·지연·락 경합·GC·큐 길이를 계측해 병목의 실체를 확인해야 한다.

## 1. 문제 정의

### 1.1 대상 환경

| 항목 | 내용 |
| --- | --- |
| 운영체제 | 리눅스 |
| 런타임 | JVM 기반 자바 애플리케이션 (Spring Boot, 내장 Tomcat 가정) |
| 구조 | 클라이언트 → 로드 밸런서 → WAS(API 서버) → (데이터베이스) |
| 문제 1 | 스레드 풀 크기를 정하는 공학적 공식이 존재하는가 |
| 문제 2 | API 서버 과부하 시 대응 방법은 무엇인가 |

### 1.2 문제 2의 전제 조건

- 데이터베이스 부하와 데이터베이스 I/O는 없다.
- 병목은 WAS에만 존재한다.
- 서비스 계층은 출력(print)만 수행한다.
- 외부 시스템 연계에 따른 부담은 고려하지 않는다.

이 전제는 단순해 보이지만, 일반적인 웹 서비스 튜닝 방법(스레드 증설, 비동기 I/O 전환)이 모두 무력해지는 조건이다. 문제의 핵심은 이 전제를 올바르게 해석하는 데 있다(6장).

### 1.3 예제 애플리케이션

본 보고서의 예제는 다음과 같은 CPU 바운드 API를 기준으로 한다. 요청마다 일정량의 연산을 수행하고 로그를 한 줄 남기며, 데이터베이스나 외부 호출은 없다.

```java
package com.example.api;

public final class Work {
    private Work() {}

    public static long run(int rounds) {
        long acc = 0;
        for (int i = 0; i < rounds; i++) {
            acc = acc * 31 + Long.hashCode(acc ^ i);
        }
        return acc;
    }
}
```

```java
package com.example.api;

import java.util.Map;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

@RestController
class ComputeController {
    private static final Logger log = LoggerFactory.getLogger(ComputeController.class);

    @GetMapping("/compute")
    Map<String, Object> compute(@RequestParam(defaultValue = "200000") int rounds) {
        long result = Work.run(rounds);
        log.info("computed rounds={} result={}", rounds, result);
        return Map.of("rounds", rounds, "result", result);
    }
}
```

## 2. 기초 개념: 프로세스, 스레드, CPU

### 2.1 리눅스의 스레드와 JVM

리눅스 커널은 스레드를 별도의 개념으로 두지 않고, 자원을 공유하는 태스크인 경량 프로세스(LWP, Lightweight Process)로 다룬다. 실무에서는 스레드라고 불러도 무방하며, 윈도우 계열은 스레드라는 용어를 명시적으로 사용한다.

자바 백엔드는 JVM 프로세스 위에서 동작하며, JVM은 유저 모드 애플리케이션 프로세스다. 자바 스레드(플랫폼 스레드)는 리눅스에서 경량 프로세스와 1:1로 매핑되지만, 자바 코드 수준에서는 매핑 방식과 무관하게 스레드로 다룬다.

### 2.2 스레드는 실행 단위이며, 실행은 CPU 연산이다

스레드는 실행의 단위다. 실행은 곧 연산이고, 연산을 수행하는 장치는 CPU다. 따라서 스레드 수에 관한 모든 논의는 CPU 코어라는 하드웨어 자원과 직접 연결된다.

### 2.3 프로세스와 스레드의 관계

| 구분 | 프로세스 | 스레드 |
| --- | --- | --- |
| 역할 | 행위(주로 I/O)의 주체, 권한과 자원 할당의 단위 | 프로세스 내부의 실행 단위 |
| 가상 메모리 | 프로세스마다 독립된 가상 메모리 공간 | 소속 프로세스의 공간을 공유 |
| 권한 | 파일·장치 파일 접근 권한이 프로세스 단위로 부여 | 프로세스의 권한을 공유 |
| 자원 | 열린 파일, 소켓 등을 소유 | 프로세스의 자원을 공유 |

유저 모드 프로세스가 장치와 통신하든, 소켓으로 통신하든, 디스크에 쓰든 그 대상은 파일의 형태로 추상화된다. 즉 I/O는 모두 파일 입출력이며, 그 주체는 프로세스다. 스레드는 이 프로세스 안에서 자원을 공유하며 나뉘어 실행된다.

## 3. 스레드 풀의 필요성

### 3.1 코어보다 스레드가 많을 때의 동작

스레드는 CPU 코어를 나눠 쓴다. 코어가 8개인데 실행 가능한 스레드가 100개라면, 의자 8개에 100명이 앉으려는 것과 같다. 동시에 실제로 연산하는 스레드는 8개뿐이며, 나머지 92개는 실행 가능 상태이면서도 CPU를 배정받기 위해 대기한다.

### 3.2 컨텍스트 스위칭 비용

CPU를 쓰는 상태와 기다리는 상태가 계속 바뀌는 과정에서 컨텍스트 스위칭이 발생한다.

```text
현재 스레드의 컨텍스트(레지스터 값 등) 저장
  → 다음 스레드의 저장된 컨텍스트 복원
  → 다음 스레드 실행 재개
```

스위칭은 CPU를 소비한다. CPU를 쓰는 스레드를 관리하기 위해 다시 CPU를 써야 하는 구조이며, 캐시와 TLB가 오염되는 간접 비용도 따른다. 따라서 스위칭은 최소화해야 한다.

### 3.3 스레드 풀이 줄이는 비용

스레드 생성과 소멸 자체도 연산 자원을 소비한다. 스레드 풀은 스레드를 미리 만들어 두고, 작업이 끝나도 소멸시키지 않고 대기 상태로 유지했다가 재사용한다. 대기 방식에도 차이가 있어, 완전히 잠드는 방식보다 깨어날 때 비용이 적은 대기 방식(예: 윈도우의 alertable wait)이 존재한다.

| 비용 | 스레드를 매번 생성할 때 | 스레드 풀을 쓸 때 |
| --- | --- | --- |
| 생성·소멸 | 요청마다 발생 | 기동 시 1회 |
| 컨텍스트 스위칭 | 스레드 수가 무제한으로 늘어 급증 | 풀 크기로 상한 제어 |
| 메모리 | 스레드마다 스택(기본 수백 KB~1MB) 무제한 증가 | 풀 크기로 상한 제어 |

## 4. 스레드 풀 크기 산정

### 4.1 브라이언 게츠의 공식

『Java Concurrency in Practice』(Brian Goetz 외)는 스레드 풀 크기를 다음과 같이 제시한다.

```text
스레드 수 = CPU 코어 수 × 목표 CPU 사용률 × (1 + 대기 시간 / 연산 시간)
```

- 스레드 수가 코어 수와 같으면 스위칭이 거의 일어나지 않아 이론적으로 가장 효율적이다. 그러나 실제 작업에는 대기가 섞이므로 그대로 적용하기 어렵다.
- 가장 비효율적인 상황은 스레드가 CPU 배정을 기다리는 자리를 차지하면서도 실제로는 외부 I/O를 기다리느라 연산하지 않는 경우다. SSD 같은 디스크 I/O든 네트워크 통신이든, 응답을 기다리는 동안 그 스레드는 CPU를 쓰지 않는다.
- 따라서 핵심 변수는 대기 시간과 연산 시간의 비율이다. 대기가 길수록 코어 하나에 더 많은 스레드를 배정해야 CPU가 놀지 않는다.

**계산 예시**: 데이터베이스 응답 대기가 평균 90ms, 응답을 받은 뒤 연산이 10ms라면 코어당 약 10개가 적정하다.

```text
코어 1개 × 사용률 1.0 × (1 + 90 / 10) = 10
```

같은 책은 순수 연산 작업에는 코어 수 + 1개를 권한다. 페이지 폴트 등으로 잠시 멈추는 스레드가 생겨도 CPU를 놀리지 않기 위해서다.

### 4.2 I/O 바운드와 CPU 바운드

| 구분 | 특징 | 적정 스레드 수 | 개선 방향 |
| --- | --- | --- | --- |
| I/O 바운드 | 외부 I/O 대기가 길다 | 코어 수보다 훨씬 많음 | 논블로킹 I/O, 가상 스레드로 대기 비용 제거 |
| CPU 바운드 | 대기가 거의 없고 연산이 대부분 | 코어 수 ~ 코어 수 + 1 | 연산 최적화, 서버 대수 확장 |

I/O 바운드 작업은 블로킹 I/O를 논블로킹 I/O로 바꾸는 것만으로도 성능이 크게 개선된다. 반면 CPU 바운드 작업에는 이런 방법이 효과가 없다.

### 4.3 리틀의 법칙

리틀의 법칙(Little's Law)은 안정 상태의 시스템에서 동시에 처리 중인 요청 수를 다음과 같이 구한다.

```text
동시 처리 중인 요청 수(L) = 초당 도착 요청 수(λ) × 평균 처리 시간(W)
```

초당 500건이 들어오고 평균 처리 시간이 200ms라면 동시 처리 중인 요청은 평균 100건이다. 요청당 스레드 하나를 쓰는 구조라면 100개 안팎의 동시 처리 능력이 필요하다.

```text
L = 500 req/s × 0.2 s = 100
```

CPU 바운드라면 W가 곧 연산 시간이므로 서버 한 대의 처리량 상한도 구할 수 있다. 코어 8개, 요청당 연산 20ms라면 이론적 최대 처리량은 다음과 같다.

```text
최대 처리량 = 코어 수 / 요청당 연산 시간 = 8 / 0.02 s = 400 req/s
```

이 상한을 넘는 유입은 스레드를 아무리 늘려도 처리할 수 없다. 이것이 9장의 서버 대수 확장이 필요한 근거다.

### 4.4 공식보다 중요한 것: 계측

공식은 누구나 찾을 수 있다. 실제로 어려운 것은 공식에 넣을 값, 즉 연산 시간과 대기 시간을 측정하는 일이다. 코어 수는 하드웨어 사양으로 바로 알 수 있지만, 대기와 연산의 비율은 코드 구간을 프로파일링해야 얻을 수 있다.

다음 유틸리티는 특정 작업의 벽시계 시간과 스레드 CPU 시간을 함께 측정해 대기·연산 비율을 구한다.

```java
package com.example.profiling;

import java.lang.management.ManagementFactory;
import java.lang.management.ThreadMXBean;
import java.util.concurrent.Callable;

public final class WaitComputeProfiler {
    private static final ThreadMXBean MX = ManagementFactory.getThreadMXBean();

    private WaitComputeProfiler() {}

    public record Sample(long wallNanos, long cpuNanos) {
        public long waitNanos() {
            return Math.max(0, wallNanos - cpuNanos);
        }

        public double waitComputeRatio() {
            return cpuNanos == 0 ? Double.POSITIVE_INFINITY : (double) waitNanos() / cpuNanos;
        }
    }

    public static <T> T measure(String name, Callable<T> task) throws Exception {
        long wallStart = System.nanoTime();
        long cpuStart = MX.getCurrentThreadCpuTime();
        try {
            return task.call();
        } finally {
            Sample s = new Sample(System.nanoTime() - wallStart, MX.getCurrentThreadCpuTime() - cpuStart);
            System.err.printf("%s wall=%.1fms cpu=%.1fms wait=%.1fms W/C=%.2f%n",
                    name, s.wallNanos() / 1e6, s.cpuNanos() / 1e6, s.waitNanos() / 1e6, s.waitComputeRatio());
        }
    }

    public static int recommendedPoolSize(double waitComputeRatio, double targetUtilization) {
        int cores = Runtime.getRuntime().availableProcessors();
        return Math.max(1, (int) Math.ceil(cores * targetUtilization * (1 + waitComputeRatio)));
    }
}
```

측정된 대기 시간에는 I/O 대기뿐 아니라 CPU를 배정받지 못해 기다린 시간도 포함된다. 따라서 측정은 부하가 낮은 상태에서 수행해야 순수한 I/O 대기 비율에 가깝다. 운영 환경에서는 async-profiler 같은 샘플링 프로파일러의 CPU 모드와 벽시계(wall-clock) 모드 결과를 비교하는 방법이 더 정확하다.

## 5. 검증: 공식은 초깃값, 최종값은 부하 테스트

### 5.1 공식의 한계

공식은 알려진 공식일 뿐이며 실제 시스템에서 그대로 들어맞는 경우는 드물다. 원인을 바로 알 수 없는 요인(락 경합, GC, 캐시 효과, 커널 스케줄링 등)이 항상 존재하기 때문이다. 따라서 공식으로 초깃값을 정하고, 부하 테스트로 최종값을 확정한다. 실제 사용자 트래픽으로 검증하는 것이 이상적이지만 불가능하므로, 사전에 부하 테스트를 수행한다.

### 5.2 스레드 수에 따른 처리량 실험

다음 프로그램은 스레드 수를 바꿔 가며 같은 양의 작업을 처리하고 처리량을 비교한다. 첫 번째 인자로 작업당 I/O 대기 시간(ms)을 주면 I/O 바운드 작업을 흉내 낸다.

```java
package com.example.bench;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

public class PoolSizeBenchmark {
    static long work(int rounds, long ioMillis) throws InterruptedException {
        if (ioMillis > 0) {
            Thread.sleep(ioMillis);
        }
        long acc = 0;
        for (int i = 0; i < rounds; i++) {
            acc = acc * 31 + Long.hashCode(acc ^ i);
        }
        return acc;
    }

    public static void main(String[] args) throws Exception {
        long ioMillis = args.length > 0 ? Long.parseLong(args[0]) : 0;
        int cores = Runtime.getRuntime().availableProcessors();
        int tasks = 2_000;
        int rounds = 2_000_000;
        int[] sizes = {1, cores, cores * 2, cores * 8, cores * 32};

        for (int warmup = 0; warmup < 200; warmup++) {
            work(rounds, 0);
        }

        for (int size : sizes) {
            ExecutorService pool = Executors.newFixedThreadPool(size);
            List<Future<Long>> futures = new ArrayList<>(tasks);
            long start = System.nanoTime();
            for (int i = 0; i < tasks; i++) {
                futures.add(pool.submit(() -> work(rounds, ioMillis)));
            }
            long sink = 0;
            for (Future<Long> f : futures) {
                sink += f.get();
            }
            double seconds = (System.nanoTime() - start) / 1e9;
            System.out.printf("io=%dms threads=%4d time=%6.2fs throughput=%7.0f tasks/s (sink=%d)%n",
                    ioMillis, size, seconds, tasks / seconds, sink);
            pool.shutdown();
        }
    }
}
```

```powershell
java PoolSizeBenchmark.java 0
java PoolSizeBenchmark.java 20
```

예상되는 경향은 다음과 같다.

| 조건 | 스레드 1 → 코어 수 | 코어 수 → 코어 수 × 32 |
| --- | --- | --- |
| `io=0` (CPU 바운드) | 처리량이 코어 수에 비례해 증가 | 처리량이 정체되거나 소폭 감소 |
| `io=20` (I/O 바운드) | 처리량 증가 | 대기 비율에 맞는 지점까지 처리량이 계속 증가 |

이 실험은 4.2의 결론, 즉 CPU 바운드 작업에서는 코어 수를 넘는 스레드가 처리량에 기여하지 못한다는 점을 수치로 확인시켜 준다.

### 5.3 부하 생성 도구와 시나리오 설계

부하 테스트에는 nGrinder, Gatling 같은 부하 생성 도구를 사용한다. 도구 사용보다 어려운 것은 부하를 정의하는 일이다. 실제 사용 패턴과 다른 부하를 만들면 불필요한 병목을 측정하게 되고, 그 결과로 튜닝하면 실제 환경에서는 효과가 없다. 실제 트래픽의 요청 비율, 요청 크기, 동시 사용자 증가 형태를 최대한 반영해야 한다.

다음은 Gatling Java DSL로 작성한 시나리오다. 초당 10명에서 500명까지 5분간 도착률을 늘리고, P99 지연과 실패율을 판정 기준으로 둔다.

```java
package simulations;

import static io.gatling.javaapi.core.CoreDsl.*;
import static io.gatling.javaapi.http.HttpDsl.*;

import io.gatling.javaapi.core.ScenarioBuilder;
import io.gatling.javaapi.core.Simulation;
import io.gatling.javaapi.http.HttpProtocolBuilder;
import java.time.Duration;

public class ComputeSimulation extends Simulation {
    HttpProtocolBuilder httpProtocol = http
            .baseUrl("http://localhost:8080")
            .acceptHeader("application/json");

    ScenarioBuilder scenario = scenario("compute")
            .exec(http("GET /compute")
                    .get("/compute?rounds=200000")
                    .check(status().is(200)));

    {
        setUp(scenario.injectOpen(
                        rampUsersPerSec(10).to(500).during(Duration.ofMinutes(5)),
                        constantUsersPerSec(500).during(Duration.ofMinutes(5))))
                .protocols(httpProtocol)
                .assertions(
                        global().responseTime().percentile(99.0).lt(500),
                        global().failedRequests().percent().lt(1.0));
    }
}
```

`injectOpen`은 응답 속도와 무관하게 정해진 도착률로 요청을 보내는 개방형 모델이다. 실제 인터넷 트래픽에 가까우며, 서버가 느려질 때 대기열이 쌓이는 현상을 그대로 드러낸다.

### 5.4 부하 테스트에서 측정할 지표

| 지표 | 확인 내용 |
| --- | --- |
| TPS | 부하 증가에 따라 처리량이 어디서 정체되는가 |
| 지연 시간 백분위(P95, P99) | 처리량 정체 지점 이후 상위 지연이 얼마나 급증하는가 |
| CPU 사용률 | 처리량이 정체될 때 CPU가 포화되었는가 |
| 큐 대기열 길이 | 작업 큐나 연결 대기열이 어디까지 차는가 |
| 스레드 상태 | 풀의 스레드가 `RUNNABLE`, `BLOCKED`, `WAITING` 중 어느 상태에 머무는가 |
| 오류율 | 타임아웃, 503 등 실패 응답이 언제부터 발생하는가 |

스레드 수를 바꿔 가며 테스트하고, TPS가 더 이상 오르지 않으면서 P99 지연이 나빠지기 시작하는 지점의 직전을 적정값으로 정한다. 다만 부하 생성 도구를 쓰더라도 실제 환경에 가깝게 재현하는 일은 쉽지 않으므로, 운영 지표로 지속적으로 재검증해야 한다.

## 6. 전제 조건의 해석

### 6.1 "출력만 한다"의 의미 1: `println`은 동기화되어 있다

서비스 계층의 출력은 `System.out.println` 같은 메서드 호출을 뜻한다. `PrintStream.println`은 내부에서 락을 잡고 출력하므로, 여러 스레드가 동시에 호출하면 한 번에 한 스레드만 출력 구간을 실행한다. 메시지 한 줄을 찍는 것만으로도 스레드 간 동기화가 발생하며, 스레드가 많을수록 락 경합으로 성능이 떨어질 수 있다.

다음 실험은 출력을 버리는 스트림에 여러 스레드가 동시에 `println`을 호출할 때 처리량이 스레드 수에 비례해 늘지 않음을 보여준다.

```java
package com.example.bench;

import java.io.OutputStream;
import java.io.PrintStream;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public class PrintlnContention {
    public static void main(String[] args) throws Exception {
        PrintStream out = new PrintStream(OutputStream.nullOutputStream());
        int cores = Runtime.getRuntime().availableProcessors();
        int callsPerThread = 2_000_000;

        for (int threads : new int[] {1, cores, cores * 4}) {
            ExecutorService pool = Executors.newFixedThreadPool(threads);
            CountDownLatch done = new CountDownLatch(threads);
            long start = System.nanoTime();
            for (int t = 0; t < threads; t++) {
                pool.execute(() -> {
                    for (int i = 0; i < callsPerThread; i++) {
                        out.println("request handled");
                    }
                    done.countDown();
                });
            }
            done.await();
            double seconds = (System.nanoTime() - start) / 1e9;
            System.err.printf("threads=%3d  %.0f println/s%n", threads, (double) threads * callsPerThread / seconds);
            pool.shutdown();
        }
    }
}
```

스레드를 코어 수만큼 늘려도 초당 출력 횟수가 비례해 늘지 않는다면, 출력 구간이 직렬화되고 있다는 뜻이다. 실제 콘솔이나 파일로 출력하면 I/O 시간이 락을 잡은 채로 더해지므로 경합은 더 심해진다.

### 6.2 "출력만 한다"의 의미 2: I/O 대기가 없다

전제의 핵심은 I/O 대기가 없다는 점이다. 이로부터 다음 결론이 나온다.

| 흔한 대응 | 전제하에서의 효과 | 이유 |
| --- | --- | --- |
| 논블로킹 I/O 전환 (WebFlux 등) | 없음 | 기다릴 I/O가 없으므로 블로킹과 논블로킹의 차이가 사라진다 |
| 스레드 수 증설 | 없음 또는 악화 | CPU 바운드이므로 코어 수를 넘는 스레드는 스위칭 비용만 늘린다 |
| 가상 스레드 도입 | 없음 | 가상 스레드도 연산은 결국 코어 수만큼만 동시에 수행된다 |

결국 서버 한 대의 최적 스레드 수는 코어 수에 수렴하는 단순한 구조이며, 남는 문제는 6.1의 동기화 병목과 서버 한 대의 처리량 상한이다. 이 두 문제가 7장 이후의 두 가지 대응 축으로 이어진다.

## 7. 대응 전략: 두 가지 축

### 7.1 전체 처리량의 구성

서비스 전체의 처리 능력은 다음과 같이 분해된다.

```text
전체 처리량 = 서버 1대의 안정 처리량 × 서버 대수
              └── 축 1: 스레드 풀 관리 ──┘   └ 축 2: 오토 스케일링 ┘
```

| 축 | 범위 | 목표 |
| --- | --- | --- |
| 축 1: 서버 스레드 풀 관리 | 서버 한 대 내부 | 스위칭과 경합 없이 서버 한 대의 최대 처리량을 내고, 과부하 시에도 그 처리량을 유지 |
| 축 2: 오토 스케일링 | 서버 외부 | 부하에 맞춰 서버 대수를 자동으로 늘리고 줄임 |

### 7.2 한 축만으로 부족한 이유

| 상황 | 결과 |
| --- | --- |
| 스레드 풀만 조정 | CPU 바운드 작업의 서버당 처리량 상한은 코어 수로 정해진다(4.3). 상한을 넘는 유입은 처리할 수 없다. |
| 오토 스케일링만 적용 | 과도한 스레드로 인한 스위칭 낭비, 무제한 큐로 인한 지연 누적, `println` 경합 같은 비효율이 서버 대수만큼 복제된다. 같은 부하에 더 많은 서버가 필요해 비용이 증가한다. |
| 둘 다 없음 | 요청이 무제한 큐에 쌓여 응답 시간이 계속 증가하고, 타임아웃과 재시도 폭주, 메모리 부족으로 서버가 연쇄적으로 중단된다. |

또한 두 축은 동작 시간이 다르다. 오토 스케일링은 새 서버가 투입되기까지 수십 초에서 수 분이 걸리며, 그동안 기존 서버를 보호하는 것은 스레드 풀의 크기 제한, 큐 제한, 거부 정책이다. 축 1은 순간을 버티고, 축 2는 용량을 늘린다.

## 8. 축 1: 서버 스레드 풀 관리

### 8.1 자바 API 서버 안의 대기열과 풀

Spring Boot 내장 Tomcat에서 요청은 다음 단계를 거친다.

```text
클라이언트 연결
  → OS TCP 백로그          server.tomcat.accept-count      (기본 100)
  → Tomcat 연결 수용        server.tomcat.max-connections   (기본 8192, NIO)
  → Tomcat 워커 스레드 풀   server.tomcat.threads.max       (기본 200)
                            server.tomcat.threads.min-spare (기본 10)
  → 애플리케이션 전용 Executor (선택)
```

| 설정 | 의미 | 과소 설정 시 | 과대 설정 시 |
| --- | --- | --- | --- |
| `threads.max` | 동시에 요청을 처리하는 워커 스레드 최대 수 | CPU가 남는데 요청이 대기 | 스위칭·메모리 증가, 지연 증가 |
| `threads.min-spare` | 항상 유지하는 최소 스레드 수 | 급증 시 스레드 생성 비용 발생 | 유휴 메모리 낭비 |
| `max-connections` | Tomcat이 동시에 유지하는 연결 수 | 연결 거부가 일찍 발생 | 처리 불가능한 연결까지 받아 대기만 길어짐 |
| `accept-count` | 연결 수용 한도 초과 후 OS 백로그 대기 수 | 연결 거부가 일찍 발생 | 클라이언트가 오래 기다리다 타임아웃 |

### 8.2 CPU 바운드 서버의 설정

기본값 `threads.max=200`은 I/O 대기가 많은 일반 웹 서비스를 가정한 값이다. 4코어 서버에서 CPU 바운드 요청을 200개 스레드로 처리하면 코어당 50개 스레드가 경쟁하게 되어, 처리량은 늘지 않고 스위칭 비용과 지연만 증가한다.

```yaml
server:
  port: 8080
  shutdown: graceful
  tomcat:
    threads:
      max: 8
      min-spare: 8
    accept-count: 100
    max-connections: 1000
    connection-timeout: 5s
    mbeanregistry:
      enabled: true

spring:
  lifecycle:
    timeout-per-shutdown-phase: 30s

management:
  endpoints:
    web:
      exposure:
        include: health,metrics,prometheus
  endpoint:
    health:
      probes:
        enabled: true
  metrics:
    distribution:
      percentiles-histogram:
        http.server.requests: true
```

위 설정은 4코어 서버 기준 예시다. Tomcat 워커는 순수 연산 외에 요청 파싱과 응답 쓰기도 수행하므로, 코어 수에서 코어 수의 2배 사이에서 시작해 5장의 부하 테스트로 확정한다. `mbeanregistry.enabled`는 Tomcat 스레드 지표 수집에, `percentiles-histogram`은 P99 지연 계산에 필요하다.

### 8.3 공식을 반영한 애플리케이션 스레드 풀

연산 작업을 Tomcat 워커와 분리된 전용 풀에서 실행하면 크기와 큐를 독립적으로 제어할 수 있다.

```java
package com.example.api;

import io.micrometer.core.instrument.MeterRegistry;
import io.micrometer.core.instrument.Tags;
import io.micrometer.core.instrument.binder.jvm.ExecutorServiceMetrics;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

@Configuration
class ExecutorConfig {

    @Bean(destroyMethod = "shutdown")
    ThreadPoolExecutor computePool(MeterRegistry registry) {
        int cores = Runtime.getRuntime().availableProcessors();
        int size = cores + 1;

        ThreadPoolExecutor pool = new ThreadPoolExecutor(
                size, size,
                0L, TimeUnit.MILLISECONDS,
                new ArrayBlockingQueue<>(size * 25),
                Thread.ofPlatform().name("compute-", 0).factory(),
                new ThreadPoolExecutor.AbortPolicy());

        new ExecutorServiceMetrics(pool, "compute", Tags.empty()).bindTo(registry);
        return pool;
    }

    @Bean(destroyMethod = "shutdown")
    ThreadPoolExecutor ioPool(MeterRegistry registry) {
        double waitMs = 90;
        double computeMs = 10;
        int size = (int) Math.ceil(Runtime.getRuntime().availableProcessors() * 0.8 * (1 + waitMs / computeMs));

        ThreadPoolExecutor pool = new ThreadPoolExecutor(
                size, size,
                0L, TimeUnit.MILLISECONDS,
                new ArrayBlockingQueue<>(1_000),
                Thread.ofPlatform().name("io-", 0).factory(),
                new ThreadPoolExecutor.CallerRunsPolicy());

        new ExecutorServiceMetrics(pool, "io", Tags.empty()).bindTo(registry);
        return pool;
    }
}
```

`Thread.ofPlatform()`은 Java 21 API다. 하위 버전에서는 `ThreadFactory`를 직접 구현해 스레드 이름을 지정한다. 스레드 이름은 스레드 덤프 분석(10장)에서 어느 풀의 스레드인지 식별하는 데 쓰인다.

```java
package com.example.api;

import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ThreadPoolExecutor;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

@RestController
class AsyncComputeController {
    private final ThreadPoolExecutor computePool;

    AsyncComputeController(@Qualifier("computePool") ThreadPoolExecutor computePool) {
        this.computePool = computePool;
    }

    @GetMapping("/compute-async")
    CompletableFuture<Map<String, Object>> compute(@RequestParam(defaultValue = "200000") int rounds) {
        return CompletableFuture.supplyAsync(
                () -> Map.<String, Object>of("rounds", rounds, "result", Work.run(rounds)),
                computePool);
    }
}
```

### 8.4 `ThreadPoolExecutor`의 동작 순서와 무제한 큐의 함정

```text
작업 도착
  → 실행 중 스레드 < corePoolSize           : 새 스레드 생성
  → 큐에 여유 있음                          : 큐에 적재
  → 큐가 가득 참, 스레드 < maximumPoolSize  : 추가 스레드 생성
  → 큐와 스레드 모두 한도 도달              : 거부 정책 실행
```

큐를 무제한(`new LinkedBlockingQueue<>()`)으로 만들면 큐가 가득 차는 일이 없으므로 스레드가 `maximumPoolSize`까지 늘어나지 않는다. 최대 스레드 수를 설정해도 실제로는 core 수만큼만 동작하면서 큐에 요청이 무한히 쌓이는 흔한 오류다. `Executors.newFixedThreadPool`도 내부적으로 무제한 큐를 사용하므로 서버 코드에서는 큐 크기를 명시한 `ThreadPoolExecutor`를 직접 생성하는 것이 안전하다. Tomcat의 워커 풀은 이 문제를 피하기 위해 큐에 넣기 전에 `maxThreads`까지 스레드를 먼저 늘리는 전용 큐를 사용한다.

### 8.5 백프레셔와 빠른 실패

과부하 시 가장 나쁜 동작은 모든 요청을 받아 두고 모두 늦게 실패하는 것이다. 큐 크기를 제한하고, 초과 요청은 즉시 거절하는 편이 시스템 전체에 유리하다.

| 거부 정책 | 동작 | 용도 |
| --- | --- | --- |
| `AbortPolicy` | `RejectedExecutionException` 발생 | 예외를 503 Service Unavailable과 `Retry-After`로 변환 |
| `CallerRunsPolicy` | 제출한 스레드가 직접 실행 | 제출 속도를 늦추는 자연스러운 백프레셔 |
| `DiscardPolicy` | 작업을 조용히 폐기 | 유실되어도 되는 작업 |
| `DiscardOldestPolicy` | 가장 오래된 작업을 폐기하고 새 작업 적재 | 최신 값만 의미 있는 작업 |

전용 풀에서 거부된 요청을 503으로 응답하는 예외 처리기는 다음과 같다.

```java
package com.example.api;

import java.util.Map;
import java.util.concurrent.RejectedExecutionException;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

@RestControllerAdvice
class OverloadExceptionHandler {

    @ExceptionHandler(RejectedExecutionException.class)
    ResponseEntity<Map<String, String>> handleRejected() {
        return ResponseEntity.status(HttpStatus.SERVICE_UNAVAILABLE)
                .header(HttpHeaders.RETRY_AFTER, "1")
                .body(Map.of("error", "server overloaded"));
    }
}
```

Tomcat 워커 단계에서 동시 처리 수 자체를 제한하려면 필터로 진입을 통제한다. 아래 필터는 동시에 처리 중인 요청이 한도를 넘으면 큐에 쌓지 않고 즉시 503을 반환한다. 헬스 체크 경로는 제외해, 과부하 상태에서도 로드 밸런서가 서버를 비정상으로 오판하지 않게 한다.

```java
package com.example.api;

import io.micrometer.core.instrument.Counter;
import io.micrometer.core.instrument.MeterRegistry;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import java.io.IOException;
import java.util.concurrent.Semaphore;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.core.Ordered;
import org.springframework.core.annotation.Order;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

@Component
@Order(Ordered.HIGHEST_PRECEDENCE)
class ConcurrencyLimitFilter extends OncePerRequestFilter {
    private final Semaphore permits;
    private final Counter rejected;

    ConcurrencyLimitFilter(@Value("${app.max-in-flight:16}") int maxInFlight, MeterRegistry registry) {
        this.permits = new Semaphore(maxInFlight);
        this.rejected = registry.counter("app.requests.rejected");
        registry.gauge("app.requests.in_flight", permits, p -> maxInFlight - p.availablePermits());
    }

    @Override
    protected boolean shouldNotFilter(HttpServletRequest request) {
        return request.getRequestURI().startsWith("/actuator");
    }

    @Override
    protected void doFilterInternal(HttpServletRequest request, HttpServletResponse response, FilterChain chain)
            throws ServletException, IOException {
        if (!permits.tryAcquire()) {
            rejected.increment();
            response.setStatus(HttpStatus.SERVICE_UNAVAILABLE.value());
            response.setHeader(HttpHeaders.RETRY_AFTER, "1");
            return;
        }
        try {
            chain.doFilter(request, response);
        } finally {
            permits.release();
        }
    }
}
```

빠르게 503을 반환하면 클라이언트는 재시도하거나 다른 서버로 넘어가고, 서버는 처리 가능한 요청에 CPU를 집중한다. 10장에서 보듯 오토 스케일링이 서버를 늘리는 동안 시스템을 보호하는 장치가 바로 이 제한과 거부다. 클라이언트 쪽에서는 `Retry-After`를 존중하고 지수 백오프와 지터를 적용해야 재시도 폭주를 막을 수 있다.

### 8.6 격리(벌크헤드)

성격이 다른 작업이 하나의 풀을 공유하면 느린 작업이 스레드를 모두 점유해 빠른 작업까지 멈춘다. 선박의 격벽처럼 작업 종류별로 풀을 나누면 한 종류의 장애가 다른 종류로 번지지 않는다.

```java
int cores = Runtime.getRuntime().availableProcessors();

ThreadPoolExecutor apiPool = new ThreadPoolExecutor(
        cores, cores, 0L, TimeUnit.MILLISECONDS,
        new ArrayBlockingQueue<>(200),
        Thread.ofPlatform().name("api-", 0).factory(),
        new ThreadPoolExecutor.AbortPolicy());

ThreadPoolExecutor reportPool = new ThreadPoolExecutor(
        2, 2, 0L, TimeUnit.MILLISECONDS,
        new ArrayBlockingQueue<>(20),
        Thread.ofPlatform().name("report-", 0).factory(),
        new ThreadPoolExecutor.AbortPolicy());
```

무거운 리포트 생성 요청이 몰려도 `reportPool`의 2개 스레드와 20개 큐만 소진되고, 일반 API를 처리하는 `apiPool`은 영향을 받지 않는다.

### 8.7 실행 중 크기 조정

`ThreadPoolExecutor`는 재시작 없이 크기를 변경할 수 있다. JDK 9부터는 core가 max보다 커지는 순간 예외가 발생하므로, 늘릴 때는 max를 먼저, 줄일 때는 core를 먼저 변경한다.

```java
static void resize(ThreadPoolExecutor pool, int size) {
    if (size > pool.getMaximumPoolSize()) {
        pool.setMaximumPoolSize(size);
        pool.setCorePoolSize(size);
    } else {
        pool.setCorePoolSize(size);
        pool.setMaximumPoolSize(size);
    }
}
```

운영 중 조정은 부하 테스트 결과를 반영하거나 장애 대응 시 임시로 쓰는 수단이다. 서버 한 대 안에서 스레드를 늘리는 것은 I/O 바운드일 때만 효과가 있으며, CPU 바운드 과부하에서 늘려야 하는 것은 스레드가 아니라 서버 대수다.

### 8.8 비동기 로깅

6.1의 `println` 병목은 로깅 프레임워크의 비동기 출력으로 제거한다. 요청 스레드는 메모리 큐에 로그 이벤트만 넣고, 실제 기록은 별도 스레드가 수행한다.

```java
// 변경 전: 요청 스레드가 PrintStream 락을 잡고 출력
System.out.println("computed rounds=" + rounds + " result=" + result);

// 변경 후: SLF4J 로거 호출, 실제 기록은 비동기 어펜더가 처리
log.info("computed rounds={} result={}", rounds, result);
```

```xml
<configuration>
  <appender name="FILE" class="ch.qos.logback.core.rolling.RollingFileAppender">
    <file>logs/app.log</file>
    <rollingPolicy class="ch.qos.logback.core.rolling.TimeBasedRollingPolicy">
      <fileNamePattern>logs/app.%d{yyyy-MM-dd}.log</fileNamePattern>
      <maxHistory>14</maxHistory>
    </rollingPolicy>
    <encoder>
      <pattern>%d %level [%thread] %logger - %msg%n</pattern>
    </encoder>
  </appender>

  <appender name="ASYNC" class="ch.qos.logback.classic.AsyncAppender">
    <queueSize>8192</queueSize>
    <discardingThreshold>0</discardingThreshold>
    <neverBlock>true</neverBlock>
    <appender-ref ref="FILE" />
  </appender>

  <root level="INFO">
    <appender-ref ref="ASYNC" />
  </root>
</configuration>
```

`neverBlock`을 켜면 큐가 가득 찼을 때 요청 스레드를 막지 않는 대신 로그가 유실될 수 있다. 감사 로그처럼 유실되면 안 되는 로그는 별도 어펜더와 경로로 분리한다. `{}` 자리표시자를 쓰면 로그 레벨이 꺼져 있을 때 문자열 결합 비용도 발생하지 않는다.

### 8.9 가상 스레드의 적용 범위

Java 21의 가상 스레드(Spring Boot 3.2 이상에서 `spring.threads.virtual.enabled=true`)는 I/O 대기 중인 스레드가 OS 스레드를 점유하지 않게 해 I/O 바운드 서버의 동시 처리량을 크게 늘린다.

```yaml
spring:
  threads:
    virtual:
      enabled: true
```

그러나 본 보고서의 전제처럼 I/O 대기가 없는 CPU 바운드 작업에서는 연산이 결국 코어 수만큼만 동시에 수행되므로 처리량이 늘지 않는다. 또한 JDK 21~23에서는 `synchronized` 구간 안에서 가상 스레드가 OS 스레드에 고정(pinning)되는 문제가 있었고, JDK 24(JEP 491)에서 해소되었다. `println`처럼 동기화된 구간이 많은 코드라면 JDK 버전도 함께 확인해야 한다.

### 8.10 스레드 풀 모니터링

Spring Boot Actuator와 Micrometer로 풀 상태를 지표화하고 경보를 설정한다.

| 지표 | Prometheus 이름 | 의미 | 경보 기준 예시 |
| --- | --- | --- | --- |
| `tomcat.threads.busy` | `tomcat_threads_busy_threads` | 바쁜 워커 스레드 수 | 최대치 대비 90% 이상 지속 |
| `tomcat.threads.config.max` | `tomcat_threads_config_max_threads` | 워커 스레드 최대치 | 포화도 계산의 분모 |
| `executor.active` | `executor_active_threads` | 전용 풀의 실행 중 작업 수 | 풀 크기에 상시 고정 |
| `executor.queued` | `executor_queued_tasks` | 전용 풀의 대기 작업 수 | 지속적 증가 추세 |
| `app.requests.rejected` | `app_requests_rejected_total` | 필터에서 거절한 요청 수 | 0보다 큰 증가율 지속 |

```yaml
groups:
  - name: api-thread-pool
    rules:
      - alert: TomcatThreadsSaturated
        expr: max by (instance) (tomcat_threads_busy_threads / tomcat_threads_config_max_threads) > 0.9
        for: 5m
      - alert: ComputeQueueGrowing
        expr: max by (instance) (executor_queued_tasks{name="compute"}) > 100
        for: 2m
      - alert: RequestsRejected
        expr: sum(rate(app_requests_rejected_total[5m])) > 0
        for: 5m
      - alert: HighP99Latency
        expr: histogram_quantile(0.99, sum by (le) (rate(http_server_requests_seconds_bucket[5m]))) > 0.5
        for: 5m
```

## 9. 축 2: 오토 스케일링

### 9.1 기본 구조: 로드 밸런서 뒤의 수평 확장

일반적인 서비스는 웹 프론트엔드와 백엔드로 구성되며, AWS 기준 구조는 다음과 같다.

```text
클라이언트
  → ALB (Application Load Balancer: 리버스 프록시, SSL 종료)
      ├─ EC2: WAS(API 서버) A
      ├─ EC2: WAS(API 서버) B
      └─ EC2: WAS(API 서버) C
  → 데이터베이스 (별도 계층)
```

- ALB는 리버스 프록시 구조이며, ALB 자체의 가용성은 AWS가 보장한다.
- ALB 뒤의 EC2 인스턴스에서 WAS가 API 서버로 동작한다.
- SSL 종료(termination)는 ALB에서 수행해 WAS의 암복호화 부담을 덜어 준다.
- WAS를 여러 대 배치하면 요청이 A, B, C에 분산된다.
- 데이터베이스는 별도 계층으로 분리한다.

| 방식 | 의미 | 한계 |
| --- | --- | --- |
| 스케일 업 | 대수는 유지하고 서버 사양(코어, 메모리)을 높인다 | 사양 상한이 있고, 교체 시 중단이 따르며, 단일 장애점이 남는다 |
| 스케일 아웃 | 비슷한 사양의 서버를 여러 대로 늘려 부하를 분산한다 | 서버가 무상태여야 하고 로드 밸런서가 필요하다 |

전제상 스레드 증설과 논블로킹 전환은 효과가 없으므로, 서버 한 대의 처리량 상한을 넘는 부하에 대한 정답은 스케일 아웃이다. 오토 스케일링은 이 스케일 아웃을 지표에 따라 자동으로 수행하고, 부하가 줄면 다시 축소(scale in)하는 운영 방식이다. 서버를 미리 넉넉히 띄우면 평소 비용이 낭비되고, 적게 띄우면 피크에 장애가 나므로 대수를 부하에 맞추는 장치가 필요하다.

### 9.2 AWS 구성 요소

```text
ALB (리스너, SSL 종료)
  → 대상 그룹 (헬스 체크, 등록 해제 지연)
      → EC2 Auto Scaling 그룹 (최소/희망/최대 용량, 시작 템플릿, 다중 AZ)
          → 스케일링 정책 (대상 추적, 단계, 예약, 예측)
          → CloudWatch 지표 (CPU 사용률, 대상당 요청 수 등)
```

| 요소 | 역할 |
| --- | --- |
| 시작 템플릿 | 새 인스턴스의 AMI, 인스턴스 유형, 사용자 데이터 정의 |
| 최소/희망/최대 용량 | 최소는 가용성, 최대는 비용 상한 |
| 헬스 체크 | ALB 헬스 체크로 응답하지 않는 인스턴스를 자동 교체 |
| 등록 해제 지연 | 축소 시 진행 중 요청이 끝날 때까지 대기 (기본 300초) |
| 웜 풀 | 미리 초기화한 인스턴스로 확장 시간 단축 |

### 9.3 스케일링 정책

| 정책 | 동작 | 적합한 경우 |
| --- | --- | --- |
| 대상 추적 | 지표를 목표값(예: CPU 60%)으로 유지하도록 자동 조절 | 기본 선택지 |
| 단계 | 경보 초과 정도에 따라 추가 대수를 차등 지정 | 급격한 증가에 공격적으로 대응 |
| 예약 | 정해진 시각에 용량 변경 | 출근 시간, 이벤트 오픈 등 예측 가능한 피크 |
| 예측 | 과거 패턴으로 미리 확장 | 일 단위 주기가 뚜렷한 트래픽 |

**지표 선택**: 본 보고서의 전제는 CPU 바운드이므로 CPU 사용률이 부하를 잘 대표한다. 반면 I/O 바운드 서비스는 CPU가 낮은데도 지연이 커질 수 있으므로 `ALBRequestCountPerTarget`(대상당 요청 수), 지연 시간, 큐 길이가 더 정확하다. 스케일링 지표는 실제 병목을 대표해야 한다.

### 9.4 Terraform 예시

다음은 ALB 대상 그룹, Auto Scaling 그룹, CPU 60% 대상 추적 정책을 정의한 Terraform 구성이다. VPC, 서브넷, ALB 리스너, 시작 템플릿은 별도로 정의되어 있다고 가정한다.

```hcl
resource "aws_lb_target_group" "api" {
  name                 = "api-tg"
  port                 = 8080
  protocol             = "HTTP"
  vpc_id               = var.vpc_id
  deregistration_delay = 30

  health_check {
    path                = "/actuator/health/readiness"
    interval            = 10
    healthy_threshold   = 2
    unhealthy_threshold = 3
    matcher             = "200"
  }
}

resource "aws_autoscaling_group" "api" {
  name                      = "api-asg"
  min_size                  = 3
  max_size                  = 15
  desired_capacity          = 3
  vpc_zone_identifier       = var.private_subnet_ids
  target_group_arns         = [aws_lb_target_group.api.arn]
  health_check_type         = "ELB"
  health_check_grace_period = 120

  launch_template {
    id      = aws_launch_template.api.id
    version = "$Latest"
  }
}

resource "aws_autoscaling_policy" "cpu_target" {
  name                   = "cpu-60"
  autoscaling_group_name = aws_autoscaling_group.api.name
  policy_type            = "TargetTrackingScaling"

  target_tracking_configuration {
    predefined_metric_specification {
      predefined_metric_type = "ASGAverageCPUUtilization"
    }
    target_value = 60
  }
}

resource "aws_autoscaling_schedule" "morning_peak" {
  scheduled_action_name  = "morning-peak"
  autoscaling_group_name = aws_autoscaling_group.api.name
  recurrence             = "50 8 * * MON-FRI"
  time_zone              = "Asia/Seoul"
  min_size               = 6
  max_size               = 15
  desired_capacity       = 6
}
```

`deregistration_delay`는 요청 처리 시간이 짧은 API에 맞춰 기본 300초에서 30초로 줄였다. 이 값은 그레이스풀 셧다운 제한 시간(`timeout-per-shutdown-phase: 30s`)과 맞춰야 한다. 예약 정책은 평일 오전 피크 10분 전에 최소 대수를 6대로 올려 확장 지연을 피한다.

### 9.5 목표 사용률을 50~60%로 두는 이유

새 서버가 서비스에 투입되기까지는 다음 단계가 필요하다.

```text
경보 발생 → 인스턴스 시작 → OS 부팅 → JVM 기동 → 애플리케이션 초기화
  → 헬스 체크 통과 → ALB 트래픽 수신 → JIT 워밍업 완료
```

이 시간 동안 기존 서버가 증가한 부하를 버텨야 하므로 목표 사용률을 낮게 잡아 여유분(headroom)을 확보한다. 목표가 60%라면 트래픽이 갑자기 1.5배가 되어도 기존 서버가 약 90%로 버티는 동안 새 서버가 투입된다. 확장 시간 자체를 줄이는 방법은 다음과 같다.

- 예약·예측 스케일링으로 피크 전에 미리 확장
- 웜 풀이나 사전 구성된 AMI로 부팅·설치 시간 단축
- AppCDS(클래스 데이터 공유), CRaC, GraalVM 네이티브 이미지로 JVM 기동과 워밍업 시간 단축

### 9.6 필요 서버 대수 산정

부하 테스트로 서버 한 대가 목표 CPU에서 낼 수 있는 처리량을 구하면 필요 대수를 계산할 수 있다. 4.3의 예시(코어 8개, 요청당 연산 20ms, 이론 최대 400 req/s)를 적용하면 다음과 같다.

```text
서버 1대 목표 처리량 = 400 req/s × 목표 사용률 0.6 = 240 req/s
피크 2,000 req/s 처리에 필요한 대수 = ceil(2,000 / 240) = 9대
3개 AZ 중 1개 장애 시에도 9대 이상 유지 = AZ당 5대 × 3 = 15대
```

```java
static int requiredInstances(double peakRps, double perInstanceMaxRps, double targetUtilization,
                             int availabilityZones) {
    int base = (int) Math.ceil(peakRps / (perInstanceMaxRps * targetUtilization));
    int perZone = (int) Math.ceil((double) base / (availabilityZones - 1));
    return perZone * availabilityZones;
}

// requiredInstances(2_000, 400, 0.6, 3) == 15
```

이 값이 Auto Scaling 그룹의 최대 용량을 정하는 근거가 되며, 최대 용량은 비용 폭주를 막는 안전장치이기도 하다.

### 9.7 축소와 그레이스풀 셧다운

확장보다 축소가 더 위험하다. 처리 중인 요청을 가진 서버를 즉시 종료하면 해당 요청은 실패한다.

```text
축소 결정
  → ALB 대상 그룹에서 등록 해제 (새 요청 중단)
  → 등록 해제 지연 동안 진행 중 요청 완료 대기
  → 애플리케이션 그레이스풀 셧다운 (스레드 풀의 잔여 작업 처리)
  → 인스턴스 종료
```

8.2의 `server.shutdown: graceful` 설정은 종료 신호를 받으면 새 요청을 거절하고 진행 중 요청을 마친 뒤 종료하게 한다. 전용 스레드 풀도 종료 시 남은 작업을 처리하도록 정리한다.

```java
@PreDestroy
void drainComputePool() throws InterruptedException {
    computePool.shutdown();
    if (!computePool.awaitTermination(25, TimeUnit.SECONDS)) {
        computePool.shutdownNow();
    }
}
```

또한 축소는 천천히 수행해야 한다. 트래픽이 잠시 줄었다고 즉시 축소하면 다시 확장하는 진동(flapping)이 발생한다. 확장은 빠르게, 축소는 안정화 기간을 두고 천천히 하는 것이 원칙이다.

### 9.8 전제 조건: 무상태 서버

서버가 언제든 추가되고 제거될 수 있어야 하므로 서버는 로컬 상태를 가지면 안 된다.

| 상태 | 문제 | 해결 |
| --- | --- | --- |
| 세션 | 다른 서버로 요청이 가면 인증이 풀림 | 토큰 기반 인증, Redis 등 외부 세션 저장소 |
| 로컬 파일 | 서버 제거 시 파일 유실 | S3 같은 객체 저장소 |
| 로컬 캐시 | 서버마다 값 불일치 | 짧은 TTL 또는 Redis 같은 공유 캐시 |
| 스케줄러 | 서버 수만큼 배치 중복 실행 | 분산 락 또는 별도 배치 서버 |

ALB의 고정 세션(sticky session)으로 우회할 수 있으나, 부하가 고르게 분산되지 않고 축소 시 세션이 끊기므로 근본적인 해결책이 아니다.

### 9.9 Kubernetes에서의 오토 스케일링

컨테이너 환경에서는 HPA(Horizontal Pod Autoscaler)가 파드 수를 조절한다. HPA의 기본 계산식은 다음과 같다.

```text
희망 파드 수 = ceil(현재 파드 수 × 현재 지표값 / 목표 지표값)
예: 파드 4개, CPU 평균 90%, 목표 60% → ceil(4 × 90 / 60) = 6개
```

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: api
  template:
    metadata:
      labels:
        app: api
    spec:
      terminationGracePeriodSeconds: 45
      containers:
        - name: api
          image: registry.example.com/api:1.0.0
          ports:
            - containerPort: 8080
          env:
            - name: JAVA_TOOL_OPTIONS
              value: "-XX:MaxRAMPercentage=75"
          resources:
            requests:
              cpu: "2"
              memory: 2Gi
            limits:
              cpu: "2"
              memory: 2Gi
          readinessProbe:
            httpGet:
              path: /actuator/health/readiness
              port: 8080
            periodSeconds: 5
          livenessProbe:
            httpGet:
              path: /actuator/health/liveness
              port: 8080
            periodSeconds: 10
          lifecycle:
            preStop:
              exec:
                command: ["sh", "-c", "sleep 10"]
---
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: api
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: api
  minReplicas: 3
  maxReplicas: 30
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 60
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
        - type: Percent
          value: 100
          periodSeconds: 15
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Percent
          value: 10
          periodSeconds: 60
```

| 설정 | 의미 |
| --- | --- |
| `resources.requests.cpu` | CPU 사용률 기준 HPA의 분모. 설정하지 않으면 HPA가 동작하지 않는다 |
| `limits.cpu: "2"` | JVM의 `availableProcessors()`가 2를 반환하게 되어 스레드 풀 크기 계산의 기준이 된다 |
| `readinessProbe` | 기동 완료 전 파드에 트래픽이 가지 않게 한다 |
| `preStop` `sleep 10` | 서비스 엔드포인트에서 제거가 전파될 시간을 벌어 종료 중 요청 유실을 막는다 |
| `terminationGracePeriodSeconds` | `preStop` 10초 + 그레이스풀 셧다운 30초보다 길게 설정 |
| `behavior.scaleUp` | 15초마다 최대 2배까지 빠르게 확장 |
| `behavior.scaleDown` | 5분 안정화 후 1분에 10%씩 천천히 축소 |

파드가 늘어 노드 자원이 부족해지면 Cluster Autoscaler나 Karpenter가 노드를 추가한다.

### 9.10 네트워크 수준 확장과 오케스트레이션

과부하 상황에서는 네트워크 계층의 상태도 확인해야 한다. 유입량이 애플리케이션 로드 밸런서의 처리 범위를 넘거나 연결 수 자체가 병목이라면, 애플리케이션 로드 밸런싱과 함께 NLB(Network Load Balancer) 같은 네트워크 수준 로드 밸런싱과 네트워크 입출력의 확장 방안을 검토한다. 이는 주로 인프라 계층에서 다루는 영역이다.

클라우드 환경에서는 Docker 이미지 기반 배포, Kubernetes 같은 오케스트레이션, 배포 전략(롤링, 블루-그린), 설정과 비밀 값 관리 등 운영 방식 전반의 설계가 오토 스케일링과 함께 따라온다.

### 9.11 오토 스케일링으로 해결되지 않는 병목

모든 서버가 공유하는 자원이 병목이면 서버를 늘려도 효과가 없으며, 오히려 공유 자원에 대한 압력이 커진다.

- 데이터베이스 커넥션 수와 쿼리 처리량 (본 보고서의 전제에서는 제외)
- 외부 API의 호출 한도
- 분산 락, 단일 리더처럼 하나만 존재하는 자원
- 서버 내부의 동기화 병목 (`println` 등): 대수를 늘려도 서버당 효율은 그대로다

따라서 확장 전에 11장의 진단으로 병목이 실제로 서버 CPU인지 확인해야 한다.

## 10. 두 축의 연동

### 10.1 트래픽 급증 시 동작 순서

```text
t0  트래픽 급증
t1  [축 1] 워커 스레드 포화, 큐 증가
      → 제한된 큐와 거부 정책으로 초과분은 503으로 빠르게 실패
      → 서버는 정상 처리량을 유지
t2  [축 2] CPU 평균이 목표 60% 초과 → 확장 결정
t3  새 서버 기동, JVM 초기화, 헬스 체크 통과
t4  로드 밸런서가 새 서버로 트래픽 분산
t5  [축 1] 서버당 부하 감소, 큐 해소, 503 비율 0으로 복귀
t6  트래픽 감소 → 안정화 기간 후 [축 2] 축소
      → 등록 해제 지연 + [축 1] 그레이스풀 셧다운으로 잔여 작업 처리
```

축 1이 없으면 t1~t4 사이에 기존 서버가 무제한 큐와 과도한 스레드로 무너지고, 새 서버가 투입되어도 이미 쌓인 요청과 재시도 폭주를 감당하지 못한다. 축 2가 없으면 t5에 도달하지 못한다. 두 축을 함께 설계해야 과부하가 일부 요청의 일시적 거절로 끝나고 장애로 번지지 않는다.

### 10.2 설정값의 연결

- 컨테이너에서 JVM의 `Runtime.getRuntime().availableProcessors()`는 CPU `limits`를 반영한다(JDK 10 이상, 8u191 이상). 스레드 풀 크기를 이 값으로 계산하면 파드 크기를 바꿔도 풀 크기가 자동으로 따라간다.
- 오토 스케일링의 목표 CPU 사용률은 부하 테스트에서 찾은 스레드 풀 적정점의 CPU 사용률보다 낮게 설정해야, 풀이 포화되기 전에 확장이 시작된다.
- 로드 밸런서의 등록 해제 지연, 애플리케이션의 그레이스풀 셧다운 시간, 전용 풀의 `awaitTermination` 시간, Kubernetes의 `terminationGracePeriodSeconds`는 바깥쪽이 안쪽보다 길도록 맞춘다.

```text
terminationGracePeriodSeconds (45s)
  > preStop (10s) + timeout-per-shutdown-phase (30s)
  > awaitTermination (25s)
```

## 11. 원인 진단과 가시성

### 11.1 병목의 실체 확인

모든 대응은 병목이 실제로 CPU인지 확인한 뒤에 적용해야 한다.

| 관찰 | 의심 원인 |
| --- | --- |
| CPU 사용률 높음, 지연 증가 | 진짜 CPU 병목 → 스케일 아웃, 연산 최적화 |
| CPU 사용률 낮음, 지연 큼 | 락 경합(`println` 등 동기화 구간), 스레드가 락을 얻기 위해 경쟁 |
| 주기적인 지연 급증 | 가비지 컬렉션 일시 정지 |
| 큐 길이 지속 증가 | 유입량이 처리 능력 초과, 또는 처리 스레드 정체 |
| 비자발적 컨텍스트 스위칭 급증 | 스레드 수가 코어 수에 비해 과다 |

요청을 받아들이는 단계에 큐 시스템을 두었다면 큐에 요청이 얼마나 쌓이는지 반드시 모니터링한다. 예상하지 못한 원인이 있을 수 있으므로 지표, 로그, 스레드 덤프를 함께 볼 수 있는 가시성(observability)을 확보하는 것이 중요하다.

### 11.2 리눅스와 JDK 도구

| 확인 대상 | 명령 | 확인 값 |
| --- | --- | --- |
| 스레드별 CPU 사용 | `top -H -p <pid>` | CPU를 많이 쓰는 경량 프로세스(스레드) |
| 컨텍스트 스위칭 | `pidstat -w -p <pid> 1` | 자발적(`cswch/s`)·비자발적(`nvcswch/s`) 스위칭 횟수 |
| CPU 대기열 | `vmstat 1` | 실행 대기 스레드 수(`r`), 초당 스위칭(`cs`) |
| 스레드 상태·락 경합 | `jcmd <pid> Thread.print` | `BLOCKED` 상태 스레드와 대기 중인 락 |
| GC | `jstat -gcutil <pid> 1000` | GC 횟수와 누적 시간 |
| CPU·락 프로파일 | async-profiler (`-e cpu`, `-e lock`) | 연산과 락 대기가 집중된 코드 경로 |

### 11.3 CPU를 많이 쓰는 스레드의 코드 위치 찾기

`top -H`가 보여주는 스레드 ID를 16진수로 바꾸면 스레드 덤프의 `nid` 값과 대응된다.

```bash
#!/usr/bin/env bash
set -euo pipefail

PID=$(pgrep -f 'app.jar' | head -n 1)

top -H -b -n 1 -p "$PID" | sed -n '8,17p'

TID=$(top -H -b -n 1 -p "$PID" | awk 'NR > 7 { print $1; exit }')
NID=$(printf '0x%x' "$TID")
echo "busiest thread tid=$TID nid=$NID"

jcmd "$PID" Thread.print | grep -A 20 "nid=$NID "
```

스레드 이름이 `compute-3`처럼 출력되면 8.3의 전용 풀 스레드임을 알 수 있다. 같은 락을 기다리는 `BLOCKED` 스레드가 다수 보이면 6.1의 동기화 병목을, 비자발적 스위칭이 급증하면 스레드 과다를 의심한다.

### 11.4 락 경합 확인

스레드 덤프에서 `println` 경합은 다음과 같은 형태로 나타난다. 다수의 요청 스레드가 같은 `PrintStream` 객체의 락을 기다린다.

```text
"http-nio-8080-exec-17" #63 daemon prio=5 os_prio=0 tid=... nid=0x2f1a waiting for monitor entry
   java.lang.Thread.State: BLOCKED (on object monitor)
        at java.io.PrintStream.println(PrintStream.java:...)
        - waiting to lock <0x00000000c0a1b2c8> (a java.io.PrintStream)
        at com.example.api.ComputeController.compute(ComputeController.java:...)
```

같은 객체 주소(`<0x...>`)를 기다리는 스레드가 많을수록 해당 구간이 직렬화되어 있다는 뜻이다. JDK 버전에 따라 모니터 대신 내부 락(`ReentrantLock`)을 쓰는 경우 `WAITING (parking)` 상태로 나타날 수 있다.

## 12. 면접 답변 구성

### 12.1 답변 구조

```text
1. 스레드 풀을 쓰는 이유: 생성 비용과 컨텍스트 스위칭 비용 최소화
2. 크기 산정: 코어 수 × 목표 사용률 × (1 + 대기/연산), 리틀의 법칙 → 초깃값
3. 확정 방법: 프로파일링으로 대기·연산 시간 계측, nGrinder·Gatling 부하 테스트
   (TPS, P99, CPU 사용률, 큐 길이, 스레드 상태)
4. 전제 해석: I/O 대기 없음 = CPU 바운드
   → 논블로킹 전환·스레드 증설 효과 없음, 최적 스레드 수 ≈ 코어 수
5. 대응 축 1 (서버 스레드 풀 관리): 워커 스레드를 코어 수 근처로 조정, 큐 제한과 503 빠른 실패,
   println 동기화 병목 → 비동기 로깅
6. 대응 축 2 (오토 스케일링): 로드 밸런서 뒤 수평 확장, CPU 목표 추적 정책, 무상태 서버,
   그레이스풀 셧다운, 컨테이너 오케스트레이션
7. 검증: CPU 낮은데 지연 크면 락 경합·GC·큐 적체 확인, 가시성 확보
```

### 12.2 답변 예시

```text
"이 문제의 답은 크게 두 가지라고 생각합니다.

첫째, 서버 안에서는 스레드 풀을 관리하겠습니다.
전제상 DB와 외부 I/O가 없으므로 CPU 바운드 작업이고, 스레드를 늘리거나
논블로킹으로 바꿔도 효과가 없습니다. 그래서 Tomcat 워커 스레드를 기본 200개가 아니라
코어 수 근처로 줄여 컨텍스트 스위칭을 없애고, 큐 크기를 제한해 넘치는 요청은 503으로
빠르게 거절해 서버가 무너지지 않게 하겠습니다. println은 내부적으로 동기화되어 있어
병목이 되므로 비동기 로깅으로 바꾸고, 적정 스레드 수는 nGrinder나 Gatling 부하 테스트로
TPS와 P99가 꺾이는 지점을 찾아 확정하겠습니다.

둘째, 서버 밖에서는 오토 스케일링을 적용하겠습니다.
CPU 바운드라 서버 한 대의 처리량 상한은 코어 수로 정해지므로, ALB 뒤에
Auto Scaling 그룹(또는 Kubernetes HPA)을 두고 CPU 60% 대상 추적 정책으로
수평 확장하겠습니다. 서버는 무상태로 만들고, 축소할 때는 등록 해제 지연과
그레이스풀 셧다운으로 진행 중인 요청을 보호하겠습니다.

마지막으로 실제 병목이 CPU인지 CPU 사용률, 지연, 락 경합, GC, 큐 길이를
모니터링해서 확인하겠습니다."
```

## 13. 점검표

| 영역 | 점검 항목 |
| --- | --- |
| 작업 성격 | 대상 작업이 I/O 바운드인지 CPU 바운드인지 계측으로 구분했는가? |
| 초깃값 | 대기·연산 시간 비율과 코어 수로 스레드 풀 초깃값을 계산했는가? |
| 워커 스레드 | Tomcat `threads.max`를 기본값 200이 아니라 작업 성격과 코어 수에 맞게 조정했는가? |
| 큐 | 작업 큐 크기와 거부 정책을 명시해 무제한 적체를 막았는가? |
| 빠른 실패 | 과부하 시 초과 요청을 503과 `Retry-After`로 빠르게 거절하는가? |
| 격리 | 성격이 다른 작업을 별도 풀로 나눠 서로 영향을 주지 않게 했는가? |
| 부하 테스트 | 실제 환경에 가까운 시나리오로 TPS·P99·CPU·큐·스레드 상태를 측정했는가? |
| 동기화 | 요청 경로에 `println` 같은 동기화 구간이나 공유 락이 있는가? |
| 로깅 | 로그 출력이 비동기로 처리되어 요청 스레드를 막지 않는가? |
| 확장 | 로드 밸런서 뒤에서 수평 확장이 가능한 구조인가? |
| 오토 스케일링 | 실제 병목을 대표하는 지표와 여유 있는 목표값(50~60%)으로 정책을 설정했는가? |
| 용량 | 부하 테스트 결과로 최소·최대 대수를 계산하고 AZ 장애까지 고려했는가? |
| 무상태 | 세션·파일·캐시·스케줄러가 서버 로컬에 묶여 있지 않은가? |
| 축소 | 등록 해제 지연과 그레이스풀 셧다운으로 진행 중 요청을 보호하는가? |
| 진단 | CPU 사용률, 지연, 락 경합, GC, 큐 길이를 함께 볼 수 있는 가시성이 있는가? |

## 결론

스레드 풀 크기 문제의 핵심은 공식을 암기하는 것이 아니라, 스레드가 CPU를 나눠 쓰는 실행 단위이고 컨텍스트 스위칭이 CPU를 소비하는 비용이라는 점을 이해하는 데 있다. 게츠의 공식과 리틀의 법칙은 대기 시간과 연산 시간의 비율로 초깃값을 제시하지만, 최종값은 프로파일링과 부하 테스트로 확정해야 한다.

과부하 문제의 전제(데이터베이스 I/O 없음, 출력만 수행)는 I/O 대기가 없는 CPU 바운드 작업을 의미한다. 이 경우 논블로킹 전환이나 스레드 증설은 효과가 없으며, 대응은 두 축으로 정리된다. 서버 스레드 풀 관리는 워커 스레드를 코어 수에 맞추고, 큐를 제한하고, 초과 요청을 빠르게 거절하고, 동기화된 출력을 비동기 로깅으로 바꿔 서버 한 대가 무너지지 않고 낼 수 있는 최대 처리량을 지킨다. 오토 스케일링은 그 서버의 대수를 부하에 맞춰 늘리고 줄인다. 스레드 풀의 제한과 빠른 실패가 확장되는 동안의 시간을 벌고, 오토 스케일링이 서버 한 대의 상한을 넘는 부하를 받아 낸다. 그리고 두 축 모두 CPU 사용률, 지연, 락 경합, GC, 큐 길이의 계측으로 병목을 확인한 뒤에 적용해야 한다.

## 참고 자료

- 널널한 개발자 TV, 「면접질문: 백엔드 개발자 스레드풀 관련 공식과 API서버 과부하 대응방법」, 2026. <https://www.youtube.com/watch?v=s92izC3LVv0>
- Brian Goetz 외, 『Java Concurrency in Practice』, Addison-Wesley, 2006.
- [Spring Boot Reference: Embedded Web Servers](https://docs.spring.io/spring-boot/how-to/webserver.html)
- [Amazon EC2 Auto Scaling: Target tracking scaling policies](https://docs.aws.amazon.com/autoscaling/ec2/userguide/as-scaling-target-tracking.html)
- [Kubernetes: Horizontal Pod Autoscaling](https://kubernetes.io/docs/tasks/run-application/horizontal-pod-autoscale/)
- [Gatling Documentation](https://docs.gatling.io/)
