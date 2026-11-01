# 22. 고급 예제: OS 통합

| 예제 | 파일 | 실행 |
|---|---|---|
| 52. 클립보드와 편집기 | `examples/52-os-integration.ts` | 플래그를 붙여 실행 |

## 예제 52. 클립보드와 편집기

이 예제는 사용자 환경에 접근하므로 아무 플래그 없이 실행하면 설명만 보여 줍니다.

```bash
# 클립보드 이미지의 크기·형식 확인
bun examples/52-os-integration.ts --clipboard

# 현재 예제 파일을 $VISUAL 또는 $EDITOR의 편집기로 열기
bun examples/52-os-integration.ts --editor
```

| API | 역할 | 주의점 |
|---|---|---|
| `Bun.Image.hasClipboardImage()` | 이미지가 있는지 빠르게 확인 | Linux에서는 항상 false일 수 있음 |
| `Bun.Image.fromClipboard()` | 시스템 클립보드에서 이미지 읽기 | 사용자가 복사한 민감한 이미지를 로그·업로드하지 않음 |
| `image.metadata()` | 변환하지 않고 크기·형식 읽기 | 큰 이미지를 불필요하게 인코딩하지 않음 |
| `Bun.openInEditor()` | `$VISUAL` 또는 `$EDITOR`로 파일 열기 | GUI 창을 여므로 명시적 사용자 동작 뒤에만 호출 |

클립보드 접근은 macOS·Windows에서 지원됩니다. Linux에서는 `wl-paste`나 `xclip`으로 바이트를 읽어 `new Bun.Image(바이트)`로 넘기는 방식을 사용하세요.

```ts
// examples/52-os-integration.ts
const flags = new Set(Bun.argv.slice(2));

if (!flags.size) {
  console.log("--clipboard: 시스템 클립보드 이미지의 정보 읽기");
  console.log("--editor: 현재 예제 파일을 기본 편집기로 열기");
  process.exit(0);
}

if (flags.has("--clipboard")) {
  if (!Bun.Image.hasClipboardImage()) {
    console.log("1) 클립보드에 이미지가 없습니다.");
  } else {
    const image = Bun.Image.fromClipboard();
    if (!image) throw new Error("클립보드 이미지를 읽지 못했습니다.");
    const meta = await image.metadata();
    console.log("1) 클립보드 이미지:", `${meta.width}×${meta.height}`, meta.format);
  }
}

if (flags.has("--editor")) {
  Bun.openInEditor(import.meta.path, { line: 1 });
  console.log("2) 편집기 열기 요청을 보냈습니다. $VISUAL 또는 $EDITOR 설정을 사용합니다.");
}
```
