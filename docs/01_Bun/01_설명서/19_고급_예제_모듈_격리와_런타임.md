# 19. 고급 예제: 모듈 격리와 런타임

이 장은 일반 웹 서버·CLI 예제보다 한 단계 아래의 런타임 기능을 다룹니다. 이 기능들은 강력하지만, 용도와 한계를 함께 이해해야 합니다.

| 예제 | 파일 | 실행 |
|---|---|---|
| 46. 모듈 그래프 격리 | `examples/46-module-graph/main.ts` | `bun examples/46-module-graph/main.ts` |

## 예제 46. 모듈 그래프 격리

`Bun.ModuleGraph`는 같은 모듈 파일을 한 프로세스 안에서 여러 번 읽되, 각 인스턴스의 모듈 전역 상태·타이머·I/O를 분리합니다. 플러그인 호스트, 테넌트별로 상태가 달라야 하는 신뢰된 앱 인스턴스, 테스트 격리에 쓸 수 있습니다. **보안 샌드박스는 아닙니다.**

```ts
// app.ts
let visits = 0;

export function visit(name: string) {
  visits++;
  return { name, visits, isMain: import.meta.main };
}
```

```ts
// main.ts
type App = typeof import("./app");
const appPath = `${import.meta.dir}/app.ts`;
const first = new Bun.ModuleGraph();
const second = new Bun.ModuleGraph();

try {
  const firstApp = await first.import<App>(appPath);
  const secondApp = await second.import<App>(appPath);

  console.log(first.run(() => firstApp.visit("A")));  // visits: 1
  console.log(first.run(() => firstApp.visit("A")));  // visits: 2
  console.log(second.run(() => secondApp.visit("B"))); // visits: 1
} finally {
  first.dispose();
  second.dispose();
}
```

| 메서드 | 역할 |
|---|---|
| `graph.import(파일)` | 해당 그래프의 독립적인 모듈 인스턴스를 읽음 |
| `graph.run(함수)` | 함수와 그 함수가 시작하는 비동기 작업을 그래프 소유로 실행 |
| `graph.dispose()` | 그래프가 연 서버·타이머·소켓·Worker 등을 정리하고 모듈 레지스트리를 버림 |

`dispose()` 뒤에 남은 객체를 계속 쓰면 안 됩니다. 특히 그래프는 권한을 제한하지 않으므로, 신뢰할 수 없는 사용자 코드를 실행하는 보안 경계로 사용하면 안 됩니다. 그런 용도에는 별도 프로세스·컨테이너·권한 분리가 필요합니다.

## 고급 API를 다루는 원칙

- 외부 인프라 API(`Bun.s3`, `Bun.redis`, PostgreSQL·MySQL용 `Bun.SQL`)는 실제 계정·서버와 권한을 마련한 뒤 전용 예제로 실행합니다.
- OS 상호작용 API(`Bun.secrets`, `Bun.openInEditor`, `Bun.Image.fromClipboard`)는 사용자의 키체인·편집기·클립보드 상태를 바꾸거나 접근하므로 자동 예제로 실행하지 않습니다.
- 엔진 제어 API(`Bun.unsafe`, `Bun.shrink`, `bun:jsc` 저수준 기능)는 일반 서비스 예제로는 부적절해 이 설명서의 고급 예제 범위에서 제외했습니다. 안전한 사용 기준을 확인한 뒤 차후 업데이트할 예정입니다.
