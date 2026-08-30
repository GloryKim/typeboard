import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon, type ISearchOptions } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

const CATPPUCCIN_MOCHA = {
  background: "#1e1e2e",
  foreground: "#cdd6f4",
  cursor: "#f5e0dc",
  cursorAccent: "#1e1e2e",
  selectionBackground: "#585b7088",
  selectionForeground: "#cdd6f4",
  black: "#45475a",
  red: "#f38ba8",
  green: "#a6e3a1",
  yellow: "#f9e2af",
  blue: "#89b4fa",
  magenta: "#f5c2e7",
  cyan: "#94e2d5",
  white: "#bac2de",
  brightBlack: "#585b70",
  brightRed: "#f38ba8",
  brightGreen: "#a6e3a1",
  brightYellow: "#f9e2af",
  brightBlue: "#89b4fa",
  brightMagenta: "#f5c2e7",
  brightCyan: "#94e2d5",
  brightWhite: "#a6adc8",
};

const SEARCH_DECORATIONS: NonNullable<ISearchOptions["decorations"]> = {
  matchBackground: "#45475a",
  matchBorder: "#585b70",
  matchOverviewRuler: "#a6adc8",
  activeMatchBackground: "#89b4fa",
  activeMatchBorder: "#b4befe",
  activeMatchColorOverviewRuler: "#89b4fa",
};

type Session = {
  id: number;
  ptyId: number | null;
  title: string;
  host: HTMLElement;
  tabBtn: HTMLButtonElement;
  titleEl: HTMLElement;
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
};

function toBytes(message: unknown): Uint8Array {
  if (message instanceof Uint8Array) {
    return message;
  }
  if (message instanceof ArrayBuffer) {
    return new Uint8Array(message);
  }
  if (Array.isArray(message)) {
    return Uint8Array.from(message as number[]);
  }
  if (typeof message === "string") {
    return new TextEncoder().encode(message);
  }
  return new Uint8Array();
}

const FONT_KEY = "typeboard.fontSize";
const FONT_DEFAULT = 13;
const FONT_MIN = 9;
const FONT_MAX = 28;

function loadFontSize(): number {
  const raw = Number(localStorage.getItem(FONT_KEY));
  if (!Number.isFinite(raw)) {
    return FONT_DEFAULT;
  }
  return Math.min(FONT_MAX, Math.max(FONT_MIN, Math.round(raw)));
}

function saveFontSize(size: number): void {
  localStorage.setItem(FONT_KEY, String(size));
}

function applyChromeScale(fontSize: number): void {
  const scale = Math.min(1.35, Math.max(0.9, fontSize / FONT_DEFAULT));
  document.documentElement.style.setProperty("--ui-scale", String(scale));
}

function createTerminal(host: HTMLElement, fontSize: number): {
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
} {
  const term = new Terminal({
    cursorBlink: true,
    cursorStyle: "bar",
    fontFamily:
      'Menlo, Monaco, "SF Mono", "JetBrains Mono", "Cascadia Code", Consolas, monospace',
    fontSize,
    lineHeight: 1.2,
    theme: CATPPUCCIN_MOCHA,
    allowProposedApi: true,
    macOptionIsMeta: true,
    scrollback: 10_000,
    convertEol: false,
    overviewRulerWidth: 12,
  });

  const fit = new FitAddon();
  const search = new SearchAddon();
  term.loadAddon(fit);
  term.loadAddon(search);
  term.loadAddon(
    new WebLinksAddon((_event, uri) => {
      void openUrl(uri);
    }),
  );
  term.open(host);

  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => webgl.dispose());
    term.loadAddon(webgl);
  } catch {
    // Canvas renderer is the fallback.
  }

  return { term, fit, search };
}

function setupFindBar(getActive: () => Session | null): {
  bind: (session: Session) => void;
  open: () => void;
} {
  const bar = document.querySelector<HTMLElement>("#find-bar");
  const input = document.querySelector<HTMLInputElement>("#find-input");
  const countEl = document.querySelector<HTMLElement>("#find-count");
  const caseEl = document.querySelector<HTMLInputElement>("#find-case");
  const prevBtn = document.querySelector<HTMLButtonElement>("#find-prev");
  const nextBtn = document.querySelector<HTMLButtonElement>("#find-next");
  const closeBtn = document.querySelector<HTMLButtonElement>("#find-close");
  if (!bar || !input || !countEl || !caseEl || !prevBtn || !nextBtn || !closeBtn) {
    return { bind: () => undefined, open: () => undefined };
  }

  const options = (): ISearchOptions => ({
    caseSensitive: caseEl.checked,
    incremental: false,
    decorations: SEARCH_DECORATIONS,
  });

  const run = (direction: "next" | "prev", incremental: boolean): void => {
    const session = getActive();
    if (!session) {
      return;
    }
    const query = input.value;
    if (!query) {
      session.search.clearDecorations();
      countEl.textContent = "";
      countEl.classList.remove("none");
      return;
    }
    const opts = { ...options(), incremental };
    if (direction === "prev") {
      session.search.findPrevious(query, opts);
    } else {
      session.search.findNext(query, opts);
    }
  };

  const open = (): void => {
    const session = getActive();
    if (!session) {
      return;
    }
    const selected = session.term.hasSelection() ? session.term.getSelection() : "";
    if (selected && !selected.includes("\n")) {
      input.value = selected;
    }
    bar.hidden = false;
    input.focus();
    input.select();
    run("next", true);
  };

  const close = (): void => {
    if (bar.hidden) {
      return;
    }
    bar.hidden = true;
    getActive()?.search.clearDecorations();
    countEl.textContent = "";
    countEl.classList.remove("none");
    getActive()?.term.focus();
  };

  const bind = (session: Session): void => {
    session.search.onDidChangeResults(({ resultIndex, resultCount }) => {
      if (getActive()?.id !== session.id) {
        return;
      }
      if (!input.value) {
        countEl.textContent = "";
        countEl.classList.remove("none");
        return;
      }
      if (resultCount === 0) {
        countEl.textContent = "No results";
        countEl.classList.add("none");
        return;
      }
      countEl.classList.remove("none");
      if (resultIndex === -1) {
        countEl.textContent = `${resultCount}+`;
        return;
      }
      countEl.textContent = `${resultIndex + 1} / ${resultCount}`;
    });
  };

  input.addEventListener("input", () => run("next", true));
  caseEl.addEventListener("change", () => run("next", false));
  prevBtn.addEventListener("click", () => run("prev", false));
  nextBtn.addEventListener("click", () => run("next", false));
  closeBtn.addEventListener("click", () => close());

  input.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") {
      ev.preventDefault();
      run(ev.shiftKey ? "prev" : "next", false);
    } else if (ev.key === "Escape") {
      ev.preventDefault();
      close();
    }
  });

  window.addEventListener(
    "keydown",
    (ev) => {
      const meta = ev.metaKey || ev.ctrlKey;
      const key = ev.key.toLowerCase();
      const code = ev.code;
      if (meta && !ev.altKey && (key === "f" || code === "KeyF") && !ev.shiftKey) {
        ev.preventDefault();
        open();
        return;
      }
      if (meta && !ev.altKey && (key === "g" || code === "KeyG")) {
        ev.preventDefault();
        if (bar.hidden) {
          open();
          return;
        }
        run(ev.shiftKey ? "prev" : "next", false);
        return;
      }
      if (ev.key === "Escape" && !bar.hidden) {
        ev.preventDefault();
        close();
      }
    },
    true,
  );

  return { bind, open };
}

async function spawnSession(session: Session): Promise<void> {
  const onData = new Channel<unknown>();
  onData.onmessage = (message) => {
    session.term.write(toBytes(message));
  };

  const onExit = new Channel<number>();
  onExit.onmessage = (code) => {
    if (session.ptyId !== null) {
      session.ptyId = null;
    }
    const status = code === 0 ? "0" : String(code);
    session.term.write(`\r\n[process exited with code ${status}]\r\n`);
    session.term.write("Press any key to restart.\r\n");
    const disposable = session.term.onData(() => {
      disposable.dispose();
      void restartSession(session);
    });
  };

  session.ptyId = await invoke<number>("pty_spawn", {
    cols: session.term.cols,
    rows: session.term.rows,
    cwd: null,
    onData,
    onExit,
  });
}

async function closePty(session: Session): Promise<void> {
  if (session.ptyId === null) {
    return;
  }
  const id = session.ptyId;
  session.ptyId = null;
  try {
    await invoke("pty_close", { id });
  } catch {
    // Session may already be gone.
  }
}

async function restartSession(session: Session): Promise<void> {
  await closePty(session);
  session.term.reset();
  try {
    await spawnSession(session);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    session.term.write(`\r\nFailed to start shell:\r\n${message}\r\n`);
  }
}

async function main(): Promise<void> {
  const tabsEl = document.querySelector<HTMLElement>("#tabs");
  const terminalsEl = document.querySelector<HTMLElement>("#terminals");
  const addBtn = document.querySelector<HTMLButtonElement>("#new-tab");
  const titlebar = document.querySelector<HTMLElement>("#titlebar");
  const tabbar = document.querySelector<HTMLElement>("#tabbar");
  const ctx = document.querySelector<HTMLElement>("#ctx-menu");
  const ctxClose = document.querySelector<HTMLButtonElement>("#ctx-close-tab");
  if (!tabsEl || !terminalsEl || !addBtn || !titlebar || !tabbar || !ctx || !ctxClose) {
    return;
  }

  const appWindow = getCurrentWindow();
  const sessions = new Map<number, Session>();
  let activeId: number | null = null;
  let nextId = 1;
  let disposing = false;
  let ctxTabId: number | null = null;
  let fontSize = loadFontSize();
  let lastZoomAt = 0;
  applyChromeScale(fontSize);

  const getActive = (): Session | null =>
    activeId === null ? null : (sessions.get(activeId) ?? null);

  const find = setupFindBar(getActive);

  const setWindowTitle = (title: string): void => {
    void appWindow.setTitle(title);
  };

  const fitSession = (session: Session): void => {
    session.fit.fit();
    if (session.ptyId !== null) {
      void invoke("pty_resize", {
        id: session.ptyId,
        cols: session.term.cols,
        rows: session.term.rows,
      });
    }
  };

  const showZoomHud = (size: number): void => {
    const hud = document.querySelector<HTMLElement>("#zoom-hud");
    if (!hud) {
      return;
    }
    hud.textContent = `${size}px`;
    hud.classList.add("show");
    window.setTimeout(() => hud.classList.remove("show"), 700);
  };

  const applyFontSize = (next: number): void => {
    const size = Math.min(FONT_MAX, Math.max(FONT_MIN, Math.round(next)));
    fontSize = size;
    saveFontSize(size);
    applyChromeScale(size);
    for (const session of sessions.values()) {
      session.term.options.fontSize = size;
      fitSession(session);
    }
    showZoomHud(size);
  };

  const bumpFont = (delta: number): void => {
    const now = Date.now();
    if (now - lastZoomAt < 40) {
      return;
    }
    lastZoomAt = now;
    applyFontSize(fontSize + delta);
  };

  const activate = (id: number): void => {
    const session = sessions.get(id);
    if (!session) {
      return;
    }
    activeId = id;
    for (const other of sessions.values()) {
      other.host.classList.toggle("active", other.id === id);
      other.tabBtn.classList.toggle("active", other.id === id);
    }
    fitSession(session);
    session.term.focus();
    setWindowTitle(session.title);
  };

  const addTab = async (): Promise<Session | null> => {
    const id = nextId++;
    const host = document.createElement("div");
    host.className = "term-host";
    terminalsEl.append(host);

    const tabBtn = document.createElement("button");
    tabBtn.type = "button";
    tabBtn.className = "tab";
    tabBtn.title = "Terminal";
    const titleEl = document.createElement("span");
    titleEl.className = "tab-title";
    titleEl.textContent = "Terminal";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "tab-close";
    closeBtn.title = "Close Tab";
    closeBtn.textContent = "×";
    tabBtn.append(titleEl, closeBtn);
    tabsEl.append(tabBtn);

    const { term, fit, search } = createTerminal(host, fontSize);
    const session: Session = {
      id,
      ptyId: null,
      title: "Terminal",
      host,
      tabBtn,
      titleEl,
      term,
      fit,
      search,
    };
    sessions.set(id, session);
    find.bind(session);

    tabBtn.addEventListener("click", (ev) => {
      if ((ev.target as HTMLElement).closest(".tab-close")) {
        return;
      }
      activate(id);
    });
    tabBtn.addEventListener("contextmenu", (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      showCtx(ev.clientX, ev.clientY, id);
    });
    closeBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      void closeTab(id);
    });

    term.onTitleChange((title) => {
      const next = title.trim() || "Terminal";
      session.title = next;
      titleEl.textContent = next;
      tabBtn.title = next;
      if (activeId === id) {
        setWindowTitle(next);
      }
    });

    term.attachCustomKeyEventHandler((ev) => {
      const meta = ev.metaKey || ev.ctrlKey;
      const key = ev.key.toLowerCase();
      if (meta && !ev.altKey && (key === "f" || ev.code === "KeyF") && !ev.shiftKey) {
        return false;
      }
      if (meta && !ev.altKey && (key === "g" || ev.code === "KeyG")) {
        return false;
      }
      if (meta && !ev.altKey && (key === "t" || ev.code === "KeyT") && !ev.shiftKey) {
        return false;
      }
      if (meta && !ev.altKey && (key === "n" || ev.code === "KeyN") && !ev.shiftKey) {
        return false;
      }
      if (meta && !ev.altKey && (key === "w" || ev.code === "KeyW") && !ev.shiftKey) {
        return false;
      }
      if (meta && !ev.altKey && (key === "-" || key === "_" || ev.code === "Minus")) {
        return false;
      }
      if (
        meta &&
        !ev.altKey &&
        (key === "=" || key === "+" || ev.code === "Equal" || ev.code === "NumpadAdd")
      ) {
        return false;
      }
      if (meta && !ev.altKey && (key === "0" || ev.code === "Digit0" || ev.code === "Numpad0")) {
        return false;
      }
      if (meta && ev.key.toLowerCase() === "c" && term.hasSelection()) {
        if (ev.type === "keydown") {
          void navigator.clipboard.writeText(term.getSelection());
        }
        return false;
      }
      if ((ev.metaKey || (ev.ctrlKey && ev.shiftKey)) && ev.key.toLowerCase() === "v") {
        if (ev.type === "keydown") {
          void navigator.clipboard.readText().then((text) => {
            if (session.ptyId !== null && text) {
              void invoke("pty_write", { id: session.ptyId, data: text });
            }
          });
        }
        return false;
      }
      return true;
    });

    term.onData((data) => {
      if (session.ptyId !== null) {
        void invoke("pty_write", { id: session.ptyId, data });
      }
    });

    activate(id);
    fit.fit();
    try {
      await spawnSession(session);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      term.write(`\r\nFailed to start shell:\r\n${message}\r\n`);
    }
    return session;
  };

  const closeTab = async (id: number): Promise<void> => {
    const session = sessions.get(id);
    if (!session) {
      return;
    }
    await closePty(session);
    session.term.dispose();
    session.host.remove();
    session.tabBtn.remove();
    sessions.delete(id);

    if (sessions.size === 0) {
      disposing = true;
      await appWindow.close();
      return;
    }
    if (activeId === id) {
      const ids = [...sessions.keys()];
      const last = ids[ids.length - 1];
      if (last !== undefined) {
        activate(last);
      }
    }
  };

  const hideCtx = (): void => {
    ctx.hidden = true;
    ctxTabId = null;
  };

  const showCtx = (x: number, y: number, tabId: number | null): void => {
    ctxTabId = tabId;
    ctxClose.hidden = tabId === null;
    ctx.hidden = false;
    const maxX = window.innerWidth - ctx.offsetWidth - 8;
    const maxY = window.innerHeight - ctx.offsetHeight - 8;
    ctx.style.left = `${Math.max(8, Math.min(x, maxX))}px`;
    ctx.style.top = `${Math.max(8, Math.min(y, maxY))}px`;
  };

  addBtn.addEventListener("click", () => {
    void addTab();
  });

  titlebar.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    showCtx(ev.clientX, ev.clientY, null);
  });

  tabbar.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    const tab = (ev.target as HTMLElement).closest(".tab");
    const tabId = tab
      ? ([...sessions.values()].find((s) => s.tabBtn === tab)?.id ?? null)
      : null;
    showCtx(ev.clientX, ev.clientY, tabId);
  });

  ctx.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const action = (ev.target as HTMLElement)
      .closest("button")
      ?.getAttribute("data-action");
    const tabId = ctxTabId;
    hideCtx();
    if (action === "new-tab") {
      void addTab();
    } else if (action === "new-window") {
      void invoke("new_window");
    } else if (action === "close-tab" && tabId !== null) {
      void closeTab(tabId);
    }
  });

  window.addEventListener("click", () => hideCtx());
  window.addEventListener("blur", () => hideCtx());

  window.addEventListener(
    "keydown",
    (ev) => {
      const meta = ev.metaKey || ev.ctrlKey;
      if (!meta || ev.altKey) {
        return;
      }
      const key = ev.key.toLowerCase();
      if ((key === "t" || ev.code === "KeyT") && !ev.shiftKey) {
        ev.preventDefault();
        void addTab();
      } else if ((key === "n" || ev.code === "KeyN") && !ev.shiftKey) {
        ev.preventDefault();
        void invoke("new_window");
      } else if ((key === "w" || ev.code === "KeyW") && !ev.shiftKey) {
        ev.preventDefault();
        if (activeId !== null) {
          void closeTab(activeId);
        }
      } else if (key === "-" || key === "_" || ev.code === "Minus") {
        ev.preventDefault();
        bumpFont(-1);
      } else if (key === "=" || key === "+" || ev.code === "Equal" || ev.code === "NumpadAdd") {
        ev.preventDefault();
        bumpFont(1);
      } else if (key === "0" || ev.code === "Digit0" || ev.code === "Numpad0") {
        ev.preventDefault();
        applyFontSize(FONT_DEFAULT);
      }
    },
    true,
  );

  await listen("terminal://new-tab", () => {
    void addTab();
  });
  await listen("terminal://close-tab", () => {
    if (activeId !== null) {
      void closeTab(activeId);
    }
  });
  await listen("terminal://zoom-in", () => bumpFont(1));
  await listen("terminal://zoom-out", () => bumpFont(-1));
  await listen("terminal://zoom-reset", () => applyFontSize(FONT_DEFAULT));

  titlebar.addEventListener("mousedown", (ev) => {
    if (ev.button !== 0) {
      return;
    }
    const target = ev.target as HTMLElement;
    if (target.closest("button, .tab, .tabs")) {
      return;
    }
    void appWindow.startDragging();
  });

  for (const handle of document.querySelectorAll<HTMLElement>("[data-resize]")) {
    handle.addEventListener("mousedown", (ev) => {
      if (ev.button !== 0) {
        return;
      }
      ev.preventDefault();
      const direction = handle.dataset.resize;
      if (direction) {
        void appWindow.startResizeDragging(
          direction as
            | "East"
            | "North"
            | "NorthEast"
            | "NorthWest"
            | "South"
            | "SouthEast"
            | "SouthWest"
            | "West",
        );
      }
    });
  }

  window.addEventListener(
    "wheel",
    (ev) => {
      if (!(ev.metaKey || ev.ctrlKey)) {
        return;
      }
      ev.preventDefault();
      bumpFont(ev.deltaY < 0 ? 1 : -1);
    },
    { passive: false },
  );

  let resizeTimer = 0;
  const observer = new ResizeObserver(() => {
    window.clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      const session = getActive();
      if (!session) {
        return;
      }
      fitSession(session);
    }, 16);
  });
  observer.observe(terminalsEl);

  window.addEventListener("beforeunload", () => {
    if (disposing) {
      return;
    }
    disposing = true;
    for (const session of sessions.values()) {
      void closePty(session);
    }
  });

  await addTab();
}

void main().catch((err: unknown) => {
  const host = document.querySelector("#terminals");
  if (host) {
    host.textContent = `Terminal failed to start: ${String(err)}`;
  }
});
