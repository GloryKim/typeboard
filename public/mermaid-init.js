// Turns ```mermaid code blocks into diagrams in the browser.
// The page loads this (and the mermaid library, a Topcoat asset) only when it contains a diagram,
// since mermaid is far larger than the rest of the site's scripts.

const blocks = [...document.querySelectorAll(".markdown pre > code.language-mermaid")].map((code) => {
  const figure = document.createElement("figure");
  figure.className = "mermaid-diagram";
  const pre = code.parentElement;
  pre.replaceWith(figure);
  // Kept so a failed diagram can fall back to showing its source.
  figure.append(pre);
  pre.hidden = true;
  return { figure, pre, source: code.textContent ?? "" };
});

const scheme = matchMedia("(prefers-color-scheme: dark)");
let renders = 0;

async function renderAll() {
  const { mermaid } = window;
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: scheme.matches ? "dark" : "default",
    fontFamily: getComputedStyle(document.body).fontFamily,
    // The default 200px wraps API names like Bun.password.hash() mid-word.
    flowchart: { wrappingWidth: 280 },
    gantt: { leftPadding: 110, barHeight: 24, fontSize: 13, sectionFontSize: 13 },
  });
  for (const { figure, pre, source } of blocks) {
    try {
      const { svg } = await mermaid.render(`mermaid-${renders++}`, source);
      figure.innerHTML = svg;
      figure.append(pre);
      figure.classList.remove("mermaid-error");
    } catch (err) {
      console.error("[mermaid]", err);
      figure.querySelector("svg")?.remove();
      figure.classList.add("mermaid-error");
      pre.hidden = false;
    }
  }
}

if (blocks.length && window.mermaid) {
  renderAll();
  scheme.addEventListener("change", renderAll);
}
