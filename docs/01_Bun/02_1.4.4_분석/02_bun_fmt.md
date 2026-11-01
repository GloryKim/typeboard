# 2. bun fmt, 내장 포매터

1.4.4에서 가장 눈에 띄는 새 명령은 `bun fmt`입니다. Prettier와 같은 결과를 목표로 한 포매터이고, JavaScript·TypeScript 프린터는 Oxc 포매터를 옮긴 것입니다. Oxc 포매터는 Biome 포매터를 옮긴 것이라, 계보는 Prettier → Biome → Oxc → Bun입니다.

```bash
bun fmt
```

```text
src/index.ts
src/styles.css
Formatted 2 files, 2 unchanged [5.59ms]
```

`bun format`은 이 명령이 아닙니다. `package.json`에 `format` 스크립트가 있으면 그 스크립트를 실행합니다. 내장 포매터를 쓰려면 철자가 `fmt`여야 합니다.

## 다루는 언어

JavaScript, TypeScript, JSX, Flow, CSS, SCSS, Less, HTML, Vue, Svelte, Angular, Lightning Web Components, MJML, JSON, JSONC, JSON5, YAML, TOML, Markdown, MDX, GraphQL, Handlebars를 포맷합니다. HTML 안의 스크립트, Markdown 코드 블록, JavaScript의 `css`...`` 템플릿처럼 파일 안에 들어간 다른 언어도 같이 다룹니다.

`*.min.js`, `*.min.css`, 잠금 파일은 경로를 직접 넘기지 않으면 건너뜁니다. Prettier 설정이 없으면 TOML도 포맷합니다.

## 속도와 일치율

공식 노트의 비교입니다. Prettier 저장소 2,616파일을 그 프로젝트 설정으로 `--check`하면 `bun fmt`는 59.7ms·최대 48MB, Prettier 3.9.10은 17.1초·459MB입니다. vitest, date-fns, monkeytype, h3에서는 Prettier 3.9.9보다 234배에서 425배 빠르다고 합니다.

oxfmt 0.74.0과 각 프로젝트 설정으로 비교한 결과는 이렇습니다.

| 프로젝트 | 파일 수 | bun fmt | oxfmt 0.74.0 |
|---|---:|---:|---:|
| RSSHub | 7,121 | 97.3ms | 298.5ms |
| redocly-cli | 3,397 | 62.4ms | 217.5ms |
| vite | 2,537 | 65.2ms | 406.7ms |
| fumadocs | 2,452 | 44.6ms | 1,395ms |

같은 파일을 보고, 둘 다 종료 코드 0이며 트리를 바꾸지 않은 실행만 비교했습니다. 코어 하나를 묶으면 배율은 줄어듭니다. RSSHub는 1.9배, redocly-cli는 1.7배, fumadocs는 21배입니다. 빠른 이유는 결과가 달라서가 아니라, 같은 일을 더 짧은 시간에 하기 때문입니다.

Prettier 3.9.9 포맷 테스트 9,731개 중 9,648개(99.1%)가 바이트 단위로 같습니다. JavaScript·JSX·TypeScript·JSON·Vue·Angular·Handlebars는 사실상 전부 같습니다. YAML이 95.9%로 가장 낮고, 차이 83개 중 64개는 YAML 블록 스칼라 끝의 빈 줄을 Prettier가 지우는 한 가지 경우입니다. 그 빈 줄은 문자열 값의 일부라, Bun은 값을 바꾸지 않으려고 남깁니다.

oxfmt 설정이 있으면 oxfmt 0.72 픽스처 1,498개 중 1,343개(89.7%)가 같습니다. 공개 저장소 330곳에서 각 프로젝트의 포맷 명령과 비교하면, oxfmt 0.72와는 240번 중 227번, Prettier와는 74번 중 57번이 모든 파일이 같았습니다. Prettier 쪽 불일치는 대부분 오래된 Prettier, 플러그인, Markdown에서 나옵니다. Next.js는 26,334파일이 같았지만 Babel만 파싱하는 제안 문법 파일 하나 때문에 불일치로 집계됩니다.

## 검사만 하기, 커밋에 붙이기

```bash
bun fmt --check
```

고칠 파일이 있으면 이름을 찍고 종료 코드 1로 끝납니다. 파일은 쓰지 않습니다.

```bash
bun fmt --staged
bun fmt --changed
bun fmt --changed=origin/dev
```

`--staged`는 커밋하려는 파일만 고치고 다시 스테이징합니다. `lint-staged` 없이 `.git/hooks/pre-commit`에 다음만 두면 됩니다.

```sh
#!/bin/sh
bun fmt --staged
```

`--changed`는 기본 브랜치와 다른 파일만 고칩니다. ref를 주면 그 커밋과 비교합니다. `sortImports = true`이면 같은 모듈의 import를 합치고 `{ }` 안 이름을 정렬합니다.

```ts
// 전
import { b, a } from "./local";
import { b } from "./local";

// 후
import { a, b } from "./local";
```

## 기존 설정을 그대로 읽는다

다음을 읽습니다.

- `.prettierrc*`, `prettier.config.*`, `package.json`의 `"prettier"`, `overrides`, `.editorconfig`, `.prettierignore`
- `.oxfmtrc.json`, `.oxfmtrc.jsonc`, `oxfmt.config.ts` 또는 `.mts`
- Prettier·oxfmt와 같은 CLI 플래그 (`--print-width 100`, `--no-semi`)
- `// prettier-ignore`

`package.json` 스크립트의 `prettier --write .`를 `bun fmt`로 바꾸면 됩니다. import 정렬은 내장이라 `@trivago/prettier-plugin-sort-imports`, `@ianvs/prettier-plugin-sort-imports`, `prettier-plugin-organize-imports`는 `plugins`에 남겨 둬도 됩니다.

Astro처럼 Bun이 직접 포맷하지 않는 언어는 프로젝트에 설치된 Prettier로 넘깁니다. 지원하지 않는 플러그인(예: `prettier-plugin-jsdoc`)이 설정에 있으면, 그 플러그인 이름을 오류로 찍고 그 플러그인이 적용되는 파일은 건너뛴 뒤 종료 코드 2로 끝냅니다. 전체 포맷이 성공한 것처럼 보이지 않습니다.

`bun fmt --doctor`는 Bun이 읽은 설정과, 이제 지워도 되는 패키지를 보여 줍니다.

## bunfig.toml

Prettier 옵션 이름을 그대로 `[fmt]`에 둡니다. `bun fmt --init`이 이 섹션을 만듭니다.

```toml
[fmt]
printWidth = 100
semi = false
sortImports = true
sortTailwindcss = true
sortPackageJson = true
jsdoc = true

[[fmt.overrides]]
files = ["*.test.ts"]
options = { printWidth = 120 }
```

`sortImports`, `sortTailwindcss`, `sortPackageJson`, `jsdoc`는 기본이 꺼져 있고, Prettier나 oxfmt 설정이 없어도 동작합니다. `sortImports`는 import를 묶고, `{ }` 안의 이름을 정렬하고, 같은 모듈에서 온 중복 import를 합칩니다.

모노레포에서는 작업 디렉터리에서 위로 올라가며 `[fmt]`가 있는 가장 가까운 `bunfig.toml`을 씁니다.

## 플러그인 세 개는 내장

`prettier-plugin-tailwindcss`, `prettier-plugin-packagejson`, `prettier-plugin-svelte`가 하는 일을 내장합니다. `plugins`에 남겨 둬도 되고, 그 플러그인 옵션도 받습니다.

Tailwind 클래스는 프로젝트에 설치된 `tailwindcss` 3 또는 4의 순서로 정렬합니다. 설치되어 있지 않으면 클래스를 건드리지 않습니다. Tailwind 4.3.3과 3.4.19에서 플러그인 0.8.1과 비교하면 JS/TS 파일 13,952개 중 13,654개(97.9%), HTML·Vue·Markdown·CSS 계열 2,176개 중 2,148개(98.7%)가 같았습니다. Tailwind 3 프로젝트 15개는 전부 같았습니다.

차이의 약 절반은 의도적입니다. `` className={`header${open ? " active" : ""}`} ``에서 플러그인은 `" active"`의 앞 공백을 지워 클래스 이름이 `headeractive`가 됩니다. `bun fmt`는 공백을 남겨 클래스 두 개로 유지합니다.

`package.json`은 플러그인과 같은 키 순서만 맞추고 값은 그대로 둡니다. 실제 파일 13,670개 중 13,644개(99.8%)가 Prettier 3.9.9와 플러그인 출력과 바이트 단위로 같습니다. 20개는 Bun이 값을 보존해서 다르고, 6개는 플러그인이 예외를 던집니다.

`.svelte`는 기본으로 포맷하며 `prettier-plugin-svelte` 4.1.1과 맞춥니다. Prettier 설정이 있으면 그 설정이 플러그인을 나열할 때만 포맷합니다. 끄려면 `[fmt]`에 `svelte = false`를 두거나 `--no-svelte`를 붙입니다.

## Prettier와 일부러 다른 곳

Prettier 3.9.9가 코드의 뜻을 바꾸는 입력은 Bun이 원래 뜻을 유지합니다.

| 입력 | Prettier 3.9.9 | bun fmt |
|---|---|---|
| CSS `font-family: Font\ Awesome` | `Font\Awesome`. `\A`가 16진 이스케이프가 됨 | 유지 |
| CSS 선택자 `x \|b` | `x\|b`. 자손 선택자가 네임스페이스 선택자가 됨 | 유지 |
| Handlebars의 특정 공백·주석 조합 | 주석이 다음 줄까지 삼킴 | 유지 |

`bun fmt --debug-check`는 포맷한 뒤 다시 파싱해서 뜻이 바뀌면 실패합니다. Prettier의 `--debug-check`와 같은 역할입니다.

## 도입할 때

설정 파일을 새로 만들기 전에 `bun fmt --check`로 차이만 보는 편이 안전합니다. 기존 Prettier 설정이 있으면 대부분 그대로 먹히고, Tailwind 클래스 문자열을 이어 붙이는 코드와 YAML 블록 스칼라 끝의 빈 줄에서 차이가 납니다. CI에서는 `--check`의 종료 코드 1을 실패로 보면 됩니다. 지원하지 않는 플러그인이 있으면 종료 코드가 2입니다.
