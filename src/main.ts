import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  readText as readOsClipboard,
  writeText as writeOsClipboard,
} from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon, type ISearchOptions } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { setupWkHangulIme, type WkHangulIme } from "./wk-hangul-ime";

const HOMEBREW = {
  background: "#000000",
  foreground: "#00ff00",
  cursor: "#00ff00",
  cursorAccent: "#000000",
  selectionBackground: "#00ff0066",
  selectionForeground: "#00ff00",
  black: "#000000",
  red: "#c41e3a",
  green: "#00ff00",
  yellow: "#999900",
  blue: "#0000ee",
  magenta: "#b200b2",
  cyan: "#00a6b2",
  white: "#bbbbbb",
  brightBlack: "#555555",
  brightRed: "#ff0000",
  brightGreen: "#00ff00",
  brightYellow: "#ffff00",
  brightBlue: "#5c5cff",
  brightMagenta: "#ff00ff",
  brightCyan: "#00ffff",
  brightWhite: "#ffffff",
};

const SEARCH_DECORATIONS: NonNullable<ISearchOptions["decorations"]> = {
  matchBackground: "#003300",
  matchBorder: "#1a8f1a",
  matchOverviewRuler: "#1a8f1a",
  activeMatchBackground: "#00ff00",
  activeMatchBorder: "#00ff00",
  activeMatchColorOverviewRuler: "#00ff00",
};

const SHELL_NAMES = new Set([
  "zsh",
  "bash",
  "fish",
  "sh",
  "dash",
  "ksh",
  "csh",
  "tcsh",
  "pwsh",
  "powershell",
  "login",
]);

const MAX_TAB_LABEL = 22;

type Session = {
  id: number;
  ptyId: number | null;
  title: string;
  oscTitle: string | null;
  fgName: string | null;
  titlePoll: number | null;
  host: HTMLElement;
  tabBtn: HTMLButtonElement;
  titleEl: HTMLElement;
  term: Terminal;
  fit: FitAddon;
  search: SearchAddon;
  wkIme: WkHangulIme;
};

function isShellName(name: string): boolean {
  return SHELL_NAMES.has(name.toLowerCase());
}

/** Find bar / other chrome — not the hidden xterm textarea. */
function isChromeEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.classList.contains("xterm-helper-textarea")) {
    return false;
  }
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target.isContentEditable
  );
}

function isPasteChord(ev: KeyboardEvent): boolean {
  if (ev.altKey) {
    return false;
  }
  const key = ev.key.toLowerCase();
  const pasteKey = key === "v" || ev.code === "KeyV";
  if (!pasteKey) {
    return false;
  }
  return ev.metaKey || (ev.ctrlKey && ev.shiftKey);
}

/**
 * WKWebView's Clipboard API often only sees text this document wrote, not
 * the system pasteboard from other apps. Read NSPasteboard via Tauri first.
 */
async function readSystemClipboard(eventText = ""): Promise<string> {
  try {
    const text = await readOsClipboard();
    if (text) {
      return text;
    }
  } catch {
    // fall through
  }
  if (eventText) {
    return eventText;
  }
  try {
    return await navigator.clipboard.readText();
  } catch {
    return "";
  }
}

async function writeSystemClipboard(text: string): Promise<void> {
  try {
    await writeOsClipboard(text);
    return;
  } catch {
    // fall through
  }
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // ignore
  }
}

let lastPasteAt = 0;

async function pasteFromSystem(session: Session, eventText = ""): Promise<void> {
  const now = performance.now();
  if (now - lastPasteAt < 100) {
    return;
  }
  lastPasteAt = now;
  const text = await readSystemClipboard(eventText);
  if (!text) {
    return;
  }
  session.wkIme.flush();
  session.term.paste(text);
}

function shortenOsc(raw: string | null): string | null {
  if (!raw) {
    return null;
  }
  let text = raw.trim();
  if (!text || text === "Terminal" || text === "typeboard") {
    return null;
  }
  const colon = text.lastIndexOf(": ");
  if (colon >= 0) {
    text = text.slice(colon + 2).trim();
  }
  const slash = Math.max(text.lastIndexOf("/"), text.lastIndexOf("\\"));
  if (slash >= 0 && slash < text.length - 1) {
    text = text.slice(slash + 1);
  }
  text = text.replace(/^~\/?/, "");
  return text || null;
}

function tabLabel(session: Session): string {
  const fg = session.fgName?.trim() || null;
  if (fg && !isShellName(fg)) {
    return truncateLabel(fg);
  }
  const osc = shortenOsc(session.oscTitle);
  if (osc) {
    return truncateLabel(osc);
  }
  if (fg) {
    return truncateLabel(fg);
  }
  return "zsh";
}

function processTitleName(session: Session): string {
  const fg = session.fgName?.trim() || null;
  if (fg && !isShellName(fg)) {
    return fg;
  }
  const osc = shortenOsc(session.oscTitle);
  if (osc && !isShellName(osc)) {
    return osc;
  }
  const shell = (fg && isShellName(fg) ? fg : "zsh").toLowerCase();
  return `-${shell}`;
}

function sessionWindowTitle(session: Session, user: string): string {
  return `${user} — ${processTitleName(session)} — ${session.term.cols}×${session.term.rows}`;
}

function truncateLabel(text: string): string {
  if (text.length <= MAX_TAB_LABEL) {
    return text;
  }
  return `${text.slice(0, MAX_TAB_LABEL - 1)}…`;
}

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
    theme: HOMEBREW,
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
  term.options.theme = HOMEBREW;

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

let onTabChromeChange: () => void = () => {};

function stopTitlePoll(session: Session): void {
  if (session.titlePoll !== null) {
    window.clearInterval(session.titlePoll);
    session.titlePoll = null;
  }
}

function applyTabTitle(
  session: Session,
  setWindowTitle: (title: string) => void,
  isActive: boolean,
): void {
  const next = tabLabel(session);
  session.title = next;
  session.titleEl.textContent = next;
  session.tabBtn.title = next;
  if (isActive) {
    setWindowTitle(next);
  }
  onTabChromeChange();
}

async function refreshFgName(session: Session): Promise<void> {
  if (session.ptyId === null) {
    return;
  }
  try {
    const name = await invoke<string | null>("pty_tab_title", { id: session.ptyId });
    session.fgName = name;
  } catch {
    // PTY may have closed between polls.
  }
}

function startTitlePoll(
  session: Session,
  setWindowTitle: (title: string) => void,
  isActive: () => boolean,
): void {
  stopTitlePoll(session);
  const tick = (): void => {
    void refreshFgName(session).then(() => {
      applyTabTitle(session, setWindowTitle, isActive());
    });
  };
  tick();
  session.titlePoll = window.setInterval(tick, 400);
}

async function spawnSession(
  session: Session,
  setWindowTitle: (title: string) => void,
  isActive: () => boolean,
): Promise<void> {
  const onData = new Channel<unknown>();
  onData.onmessage = (message) => {
    session.term.write(toBytes(message));
  };

  const onExit = new Channel<number>();
  onExit.onmessage = (code) => {
    stopTitlePoll(session);
    if (session.ptyId !== null) {
      session.ptyId = null;
    }
    const status = code === 0 ? "0" : String(code);
    session.term.write(`\r\n[process exited with code ${status}]\r\n`);
    session.term.write("Press any key to restart.\r\n");
    const disposable = session.term.onData(() => {
      disposable.dispose();
      void restartSession(session, setWindowTitle, isActive);
    });
  };

  session.ptyId = await invoke<number>("pty_spawn", {
    cols: session.term.cols,
    rows: session.term.rows,
    cwd: null,
    onData,
    onExit,
  });
  startTitlePoll(session, setWindowTitle, isActive);
}

async function closePty(session: Session): Promise<void> {
  stopTitlePoll(session);
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

async function restartSession(
  session: Session,
  setWindowTitle: (title: string) => void,
  isActive: () => boolean,
): Promise<void> {
  await closePty(session);
  session.term.reset();
  try {
    await spawnSession(session, setWindowTitle, isActive);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    session.term.write(`\r\nFailed to start shell:\r\n${message}\r\n`);
  }
}

async function main(): Promise<void> {
  const tabsEl = document.querySelector<HTMLElement>("#tabs");
  const terminalsEl = document.querySelector<HTMLElement>("#terminals");
  const addBtn = document.querySelector<HTMLButtonElement>("#new-tab");
  const tabbar = document.querySelector<HTMLElement>("#tabbar");
  const overflowBtn = document.querySelector<HTMLButtonElement>("#tab-overflow");
  const picker = document.querySelector<HTMLElement>("#tab-picker");
  const ctx = document.querySelector<HTMLElement>("#ctx-menu");
  const ctxClose = document.querySelector<HTMLButtonElement>("#ctx-close-tab");
  if (
    !tabsEl ||
    !terminalsEl ||
    !addBtn ||
    !tabbar ||
    !overflowBtn ||
    !picker ||
    !ctx ||
    !ctxClose
  ) {
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
  let hostUser = "user";
  applyChromeScale(fontSize);

  try {
    hostUser = await invoke<string>("host_user");
  } catch {
    // Fall back to a generic label.
  }

  const getActive = (): Session | null =>
    activeId === null ? null : (sessions.get(activeId) ?? null);

  const find = setupFindBar(getActive);

  const refreshChromeTitle = (): void => {
    const session = getActive();
    const text = session
      ? sessionWindowTitle(session, hostUser)
      : "typeboard";
    void appWindow.setTitle(text);
  };

  const setWindowTitle = (_title: string): void => {
    refreshChromeTitle();
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
    if (session.id === activeId) {
      refreshChromeTitle();
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
    syncOverflow();
  };

  const bumpFont = (delta: number): void => {
    const now = Date.now();
    if (now - lastZoomAt < 40) {
      return;
    }
    lastZoomAt = now;
    applyFontSize(fontSize + delta);
  };

  const hidePicker = (): void => {
    picker.hidden = true;
    overflowBtn.classList.remove("open");
  };

  const placePicker = (): void => {
    const rect = overflowBtn.getBoundingClientRect();
    picker.style.top = `${rect.bottom + 4}px`;
    picker.style.left = "auto";
    picker.style.right = `${Math.max(8, window.innerWidth - rect.right)}px`;
  };

  const pickerIds = (): number[] =>
    [...picker.querySelectorAll<HTMLElement>(".tab-picker-item")].map((row) =>
      Number(row.dataset.id),
    );

  const selectPickerTab = (id: number): void => {
    hidePicker();
    activate(id);
  };

  const fillPicker = (): void => {
    picker.replaceChildren();
    let index = 0;
    for (const session of sessions.values()) {
      index += 1;
      const row = document.createElement("div");
      row.className = "tab-picker-item";
      if (session.id === activeId) {
        row.classList.add("active");
      }
      row.dataset.id = String(session.id);

      const num = document.createElement("span");
      num.className = "tab-picker-index";
      num.textContent = String(index);

      const pick = document.createElement("button");
      pick.type = "button";
      pick.className = "tab-picker-title";
      pick.textContent = session.title;
      pick.title = session.title;

      const close = document.createElement("button");
      close.type = "button";
      close.className = "tab-picker-close";
      close.title = "Close Tab";
      close.textContent = "×";

      pick.addEventListener("pointerdown", (ev) => {
        if (ev.button !== 0) {
          return;
        }
        ev.preventDefault();
        ev.stopPropagation();
        selectPickerTab(session.id);
      });
      close.addEventListener("pointerdown", (ev) => {
        if (ev.button !== 0) {
          return;
        }
        ev.preventDefault();
        ev.stopPropagation();
        void closeTab(session.id);
      });

      row.append(num, pick, close);
      picker.append(row);
    }
  };

  const refreshPicker = (): void => {
    if (picker.hidden) {
      return;
    }
    const ids = [...sessions.keys()];
    const rows = pickerIds();
    const same =
      ids.length === rows.length && ids.every((id, i) => id === rows[i]);
    if (!same) {
      fillPicker();
    } else {
      let index = 0;
      for (const session of sessions.values()) {
        const row = picker.children[index] as HTMLElement | undefined;
        index += 1;
        if (!row) {
          continue;
        }
        row.classList.toggle("active", session.id === activeId);
        const title = row.querySelector(".tab-picker-title");
        const num = row.querySelector(".tab-picker-index");
        if (title) {
          title.textContent = session.title;
          title.setAttribute("title", session.title);
        }
        if (num) {
          num.textContent = String(index);
        }
      }
    }
    placePicker();
  };

  const showPicker = (): void => {
    hideCtx();
    fillPicker();
    picker.hidden = false;
    overflowBtn.classList.add("open");
    placePicker();
  };

  const syncOverflow = (): void => {
    requestAnimationFrame(() => {
      const overflowing = tabsEl.scrollWidth > tabsEl.clientWidth + 1;
      overflowBtn.hidden = !overflowing;
      if (!overflowing) {
        hidePicker();
        return;
      }
      refreshPicker();
    });
  };

  onTabChromeChange = () => {
    refreshPicker();
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
    session.tabBtn.scrollIntoView({ inline: "nearest", block: "nearest" });
    syncOverflow();
  };

  const addTab = async (): Promise<Session | null> => {
    const id = nextId++;
    const host = document.createElement("div");
    host.className = "term-host";
    terminalsEl.append(host);

    const tabBtn = document.createElement("button");
    tabBtn.type = "button";
    tabBtn.className = "tab";
    tabBtn.title = "zsh";
    const titleEl = document.createElement("span");
    titleEl.className = "tab-title";
    titleEl.textContent = "zsh";
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
      title: "zsh",
      oscTitle: null,
      fgName: null,
      titlePoll: null,
      host,
      tabBtn,
      titleEl,
      term,
      fit,
      search,
      wkIme: null!,
    };
    session.wkIme = setupWkHangulIme(term, (data) => {
      if (session.ptyId !== null) {
        void invoke("pty_write", { id: session.ptyId, data });
      }
    });
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
      session.oscTitle = title.trim() || null;
      applyTabTitle(session, setWindowTitle, activeId === id);
    });

    term.attachCustomKeyEventHandler((ev) => {
      const wkKey = session.wkIme.handleKeyEvent(ev);
      if (wkKey === false) {
        return false;
      }
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
          void writeSystemClipboard(term.getSelection());
        }
        return false;
      }
      if (isPasteChord(ev)) {
        if (ev.type === "keydown") {
          ev.preventDefault();
          void pasteFromSystem(session);
        }
        return false;
      }
      return true;
    });

    term.onData((data) => {
      if (session.wkIme.isComposing() || session.wkIme.ignorePtyData(data)) {
        return;
      }
      if (session.ptyId !== null) {
        void invoke("pty_write", { id: session.ptyId, data });
      }
    });

    activate(id);
    fit.fit();
    try {
      await spawnSession(session, setWindowTitle, () => activeId === id);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      term.write(`\r\nFailed to start shell:\r\n${message}\r\n`);
    }
    syncOverflow();
    return session;
  };

  const closeTab = async (id: number): Promise<void> => {
    const session = sessions.get(id);
    if (!session) {
      return;
    }
    await closePty(session);
    session.wkIme.dispose();
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
    syncOverflow();
  };

  const hideCtx = (): void => {
    ctx.hidden = true;
    ctxTabId = null;
  };

  const showCtx = (x: number, y: number, tabId: number | null): void => {
    hidePicker();
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

  overflowBtn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    if (picker.hidden) {
      showPicker();
    } else {
      hidePicker();
    }
  });

  picker.addEventListener("click", (ev) => {
    ev.stopPropagation();
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

  window.addEventListener("click", () => {
    hideCtx();
    hidePicker();
  });
  window.addEventListener("blur", () => {
    hideCtx();
    hidePicker();
  });

  window.addEventListener(
    "keydown",
    (ev) => {
      if (ev.key === "Escape" && !picker.hidden) {
        ev.preventDefault();
        hidePicker();
        return;
      }
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
      } else if (isPasteChord(ev)) {
        if (isChromeEditable(ev.target)) {
          return;
        }
        ev.preventDefault();
        const session = getActive();
        if (session) {
          void pasteFromSystem(session);
        }
      }
    },
    true,
  );

  window.addEventListener(
    "paste",
    (ev) => {
      if (isChromeEditable(ev.target)) {
        return;
      }
      const fromEvent = ev.clipboardData?.getData("text/plain") ?? "";
      ev.preventDefault();
      ev.stopPropagation();
      const session = getActive();
      if (session) {
        void pasteFromSystem(session, fromEvent);
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

  tabbar.addEventListener("mousedown", (ev) => {
    if (ev.button !== 0) {
      return;
    }
    const target = ev.target as HTMLElement;
    if (target.closest("button, .tab")) {
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
      if (session) {
        fitSession(session);
      }
      syncOverflow();
    }, 16);
  });
  observer.observe(terminalsEl);
  observer.observe(tabsEl);

  const syncFullscreen = (): void => {
    void appWindow.isFullscreen().then((fullscreen) => {
      document.body.classList.toggle("is-fullscreen", fullscreen);
      const session = getActive();
      if (session) {
        fitSession(session);
      }
      syncOverflow();
    });
  };

  syncFullscreen();
  void appWindow.onResized(() => {
    syncFullscreen();
    window.setTimeout(syncFullscreen, 80);
  });

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
