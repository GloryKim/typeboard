//! Page helpers (copy button, search keyboard, TOC highlight).

pub const JS: &str = r#"// Small page helpers the Topcoat runtime doesn't cover. The mobile menu toggle and the live search
// results are Topcoat runtime code (signals and a shard in src/app.rs); search-word highlighting
// (?hl=) is done on the server.

// Copy button on code blocks
for (const pre of document.querySelectorAll(".markdown pre")) {
  if (pre.querySelector("code.language-mermaid")) continue;
  const button = document.createElement("button");
  button.className = "copy-button";
  button.type = "button";
  button.textContent = "복사";
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(pre.querySelector("code")?.innerText ?? pre.innerText);
      button.textContent = "복사됨";
    } catch {
      button.textContent = "복사 실패";
    }
    setTimeout(() => (button.textContent = "복사"), 1500);
  });
  pre.appendChild(button);
}

// Search box: ↑/↓/Enter to pick a live result, Esc or a click outside to close, "/" or Ctrl/⌘+K to focus.
// The panel element is re-rendered by the runtime, so it is looked up each time.
const searchForm = document.querySelector(".search");
const searchInput = searchForm?.querySelector("input");
if (searchForm && searchInput) {
  const panel = () => searchForm.querySelector(".search-panel");
  const links = () => [...(panel()?.querySelectorAll("a.search-hit") ?? [])];
  let active = -1;

  const close = () => {
    const p = panel();
    if (p) p.hidden = true;
    active = -1;
  };
  const select = (index) => {
    const items = links();
    if (!items.length) return;
    active = (index + items.length) % items.length;
    items.forEach((a, i) => a.classList.toggle("active", i === active));
    items[active].scrollIntoView({ block: "nearest" });
  };

  // A new set of results resets the keyboard selection.
  new MutationObserver(() => (active = -1)).observe(searchForm, { childList: true, subtree: true });

  searchInput.addEventListener("focus", () => {
    const p = panel();
    if (p && searchInput.value.trim() && p.childElementCount) p.hidden = false;
  });
  searchInput.addEventListener("keydown", (e) => {
    const p = panel();
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (p?.hidden && searchInput.value.trim() && p.childElementCount) p.hidden = false;
      select(active + (e.key === "ArrowDown" ? 1 : -1));
    } else if (e.key === "Enter" && active >= 0 && p && !p.hidden) {
      e.preventDefault();
      location.href = links()[active].href;
    } else if (e.key === "Escape") {
      close();
      searchInput.blur();
    }
  });
  document.addEventListener("click", (e) => {
    if (!searchForm.contains(e.target)) close();
  });
  document.addEventListener("keydown", (e) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName ?? "");
    if ((e.key === "/" && !typing) || (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey))) {
      e.preventDefault();
      searchInput.focus();
      searchInput.select();
    }
  });
}

// Highlight the heading currently in view in the page table of contents
const tocLinks = new Map(
  [...document.querySelectorAll(".toc a")].map((a) => [decodeURIComponent(a.hash.slice(1)), a]),
);
if (tocLinks.size) {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        for (const a of tocLinks.values()) a.classList.remove("active");
        tocLinks.get(entry.target.id)?.classList.add("active");
      }
    },
    { rootMargin: "-64px 0px -70% 0px" },
  );
  for (const id of tocLinks.keys()) {
    const heading = document.getElementById(id);
    if (heading) observer.observe(heading);
  }
}
"#;
