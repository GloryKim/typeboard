import type { Terminal } from "@xterm/xterm";

/** Hangul jamo, compatibility jamo, syllables, extended blocks. */
function isHangul(text: string): boolean {
  const cp = text.codePointAt(0);
  if (cp === undefined) {
    return false;
  }
  return (
    (cp >= 0x1100 && cp <= 0x11ff) ||
    (cp >= 0x3130 && cp <= 0x318f) ||
    (cp >= 0xac00 && cp <= 0xd7af) ||
    (cp >= 0xa960 && cp <= 0xa97f) ||
    (cp >= 0xd7b0 && cp <= 0xd7ff)
  );
}

function isWkWebView(): boolean {
  if ("__TAURI_INTERNALS__" in window || "__TAURI__" in window) {
    return true;
  }
  const ua = navigator.userAgent;
  return /AppleWebKit/i.test(ua) && !/Chromium|Chrome|Edg|OPR/i.test(ua);
}

export type WkHangulIme = {
  isComposing: () => boolean;
  flush: () => void;
  handleKeyEvent: (ev: KeyboardEvent) => boolean | undefined;
  dispose: () => void;
};

/**
 * WKWebView (Tauri macOS, Safari) fires insertReplacementText instead of
 * composition events for Korean IME. xterm.js 5.5 drops those updates.
 */
export function setupWkHangulIme(
  term: Terminal,
  onCommit: (text: string) => void,
): WkHangulIme {
  const textarea = term.textarea;
  const root = term.element;
  const compositionView = root?.querySelector<HTMLElement>(".composition-view") ?? null;
  const wkEnv = isWkWebView();

  let composing = false;
  let pending = "";
  let sawReplacement = false;

  const syncCompositionView = (): void => {
    if (!compositionView || !root || !pending) {
      return;
    }
    const cursor = root.querySelector<HTMLElement>(".xterm-cursor");
    if (!cursor) {
      return;
    }
    const rootRect = root.getBoundingClientRect();
    const cursorRect = cursor.getBoundingClientRect();
    compositionView.textContent = pending;
    compositionView.classList.add("active");
    compositionView.style.left = `${cursorRect.left - rootRect.left}px`;
    compositionView.style.top = `${cursorRect.top - rootRect.top}px`;
    compositionView.style.height = `${cursorRect.height}px`;
    compositionView.style.lineHeight = `${cursorRect.height}px`;
    compositionView.style.fontFamily = term.options.fontFamily ?? "";
    compositionView.style.fontSize = `${term.options.fontSize ?? 13}px`;
  };

  const hideCompositionView = (): void => {
    if (!compositionView) {
      return;
    }
    compositionView.textContent = "";
    compositionView.classList.remove("active");
  };

  const clearTextarea = (): void => {
    if (textarea) {
      textarea.value = "";
    }
  };

  const flush = (): void => {
    if (!composing) {
      return;
    }
    const text = pending;
    composing = false;
    pending = "";
    sawReplacement = false;
    hideCompositionView();
    clearTextarea();
    if (text) {
      onCommit(text);
    }
  };

  const beginComposition = (text: string, showPreview: boolean): void => {
    composing = true;
    pending = text;
    clearTextarea();
    if (showPreview) {
      syncCompositionView();
    }
  };

  const onInput = (ev: Event): void => {
    if (!(ev instanceof InputEvent) || !ev.data) {
      return;
    }

    if (ev.inputType === "insertReplacementText") {
      ev.preventDefault();
      ev.stopImmediatePropagation();
      sawReplacement = true;
      beginComposition(ev.data, true);
      return;
    }

    if (ev.inputType !== "insertText" || !isHangul(ev.data)) {
      if (composing) {
        flush();
      }
      return;
    }

    const useWkPath = sawReplacement || (wkEnv && !ev.isComposing);
    if (!useWkPath) {
      return;
    }

    ev.preventDefault();
    ev.stopImmediatePropagation();

    const hadPending = composing;
    flush();
    beginComposition(ev.data, !hadPending);
  };

  const onKeyDown = (ev: KeyboardEvent): void => {
    if (!composing) {
      return;
    }
    if (ev.keyCode === 229) {
      ev.preventDefault();
      ev.stopImmediatePropagation();
      clearTextarea();
      return;
    }
    flush();
  };

  const renderDisposable = term.onRender(() => {
    if (composing && pending) {
      syncCompositionView();
    }
  });

  if (textarea) {
    textarea.addEventListener("input", onInput, true);
    textarea.addEventListener("keydown", onKeyDown, true);
  }

  return {
    isComposing: () => composing,
    flush,
    handleKeyEvent: (ev: KeyboardEvent) => {
      if (!composing) {
        return undefined;
      }
      if (ev.keyCode === 229) {
        return false;
      }
      if (ev.type === "keydown") {
        flush();
      }
      return undefined;
    },
    dispose: () => {
      flush();
      renderDisposable.dispose();
      if (textarea) {
        textarea.removeEventListener("input", onInput, true);
        textarea.removeEventListener("keydown", onKeyDown, true);
      }
    },
  };
}
