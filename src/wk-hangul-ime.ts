import type { Terminal } from "@xterm/xterm";

/** Hangul jamo, compatibility jamo, syllables, extended blocks. */
function isHangul(text: string): boolean {
  const cp = text.codePointAt(0);
  if (cp === undefined) {
    return false;
  }
  return isHangulCp(cp);
}

function isHangulCp(cp: number): boolean {
  return (
    (cp >= 0x1100 && cp <= 0x11ff) ||
    (cp >= 0x3130 && cp <= 0x318f) ||
    (cp >= 0xac00 && cp <= 0xd7af) ||
    (cp >= 0xa960 && cp <= 0xa97f) ||
    (cp >= 0xd7b0 && cp <= 0xd7ff)
  );
}

function isJamoCp(cp: number): boolean {
  return (
    (cp >= 0x1100 && cp <= 0x11ff) ||
    (cp >= 0x3130 && cp <= 0x318f) ||
    (cp >= 0xa960 && cp <= 0xa97f) ||
    (cp >= 0xd7b0 && cp <= 0xd7ff)
  );
}

function isSyllableCp(cp: number): boolean {
  return cp >= 0xac00 && cp <= 0xd7af;
}

function stripJongseong(cp: number): number {
  if (!isSyllableCp(cp)) {
    return cp;
  }
  const s = cp - 0xac00;
  return 0xac00 + s - (s % 28);
}

/** 자모→음절, or 받침 도깨비불 (간→가). Replace preedit; do not insert a second glyph. */
function isSamePreedit(pending: string, next: string): boolean {
  if (!pending || pending === next) {
    return false;
  }
  const pendingCp = pending.codePointAt(0);
  const nextCp = next.codePointAt(0);
  if (pendingCp === undefined || nextCp === undefined) {
    return false;
  }
  if ([...pending].length !== 1 || [...next].length !== 1) {
    return false;
  }
  if (isJamoCp(pendingCp) && isHangulCp(nextCp)) {
    return true;
  }
  if (isSyllableCp(pendingCp) && isSyllableCp(nextCp)) {
    return stripJongseong(pendingCp) === nextCp || stripJongseong(pendingCp) === stripJongseong(nextCp);
  }
  return false;
}

function isWkWebView(): boolean {
  if ("__TAURI_INTERNALS__" in window || "__TAURI__" in window) {
    return true;
  }
  const ua = navigator.userAgent;
  return /AppleWebKit/i.test(ua) && !/Chromium|Chrome|Edg|OPR/i.test(ua);
}

function isPreeditInputType(inputType: string): boolean {
  return inputType === "insertReplacementText" || inputType === "insertCompositionText";
}

export type WkHangulIme = {
  isComposing: () => boolean;
  /** True when xterm onData is a duplicate of Hangul we already wrote. */
  ignorePtyData: (data: string) => boolean;
  flush: () => void;
  handleKeyEvent: (ev: KeyboardEvent) => boolean | undefined;
  dispose: () => void;
};

/**
 * WKWebView Korean IME sends preedit (insertCompositionText) and then a
 * committed insertText / compositionend for the same syllable. Writing both
 * doubled every other glyph (가나다 → 가가나다다…). Preedit is the only
 * writer; commit only freezes. Stale echoes with the same IME key sequence
 * are ignored. A later keyCode 229 starts a new sequence so 가가 still works.
 */
export function setupWkHangulIme(
  term: Terminal,
  onCommit: (text: string) => void,
): WkHangulIme {
  const textarea = term.textarea;
  const root = term.element;
  const wkEnv = isWkWebView();

  let composing = false;
  let pending = "";
  let echoed = "";
  let holdOnData = false;
  let lastConfirmed = "";
  let imeKeySeq = 0;
  let confirmedAtSeq = -1;

  const isStaleEcho = (text: string): boolean =>
    text === lastConfirmed && imeKeySeq === confirmedAtSeq;

  const swallowHangul = (ev: Event): void => {
    ev.preventDefault();
    ev.stopPropagation();
    ev.stopImmediatePropagation();
  };

  const syncPty = (next: string): void => {
    if (next === echoed) {
      return;
    }
    const deletes = [...echoed].length;
    const payload = `${"\x7f".repeat(deletes)}${next}`;
    echoed = next;
    if (payload) {
      onCommit(payload);
    }
  };

  const confirmPreedit = (): void => {
    if (echoed) {
      lastConfirmed = echoed;
      confirmedAtSeq = imeKeySeq;
    }
    composing = false;
    pending = "";
    echoed = "";
    holdOnData = true;
    window.setTimeout(() => {
      holdOnData = false;
    }, 0);
    if (textarea) {
      textarea.value = "";
    }
  };

  const setPreedit = (text: string): void => {
    if (isStaleEcho(text)) {
      return;
    }
    composing = true;
    pending = text;
    syncPty(text);
  };

  const flush = (): void => {
    if (!composing) {
      return;
    }
    confirmPreedit();
  };

  const applyCommittedHangul = (text: string): void => {
    if (isStaleEcho(text) || echoed === text || pending === text) {
      confirmPreedit();
      return;
    }
    if (composing && isSamePreedit(pending, text)) {
      setPreedit(text);
      confirmPreedit();
      return;
    }
    if (composing) {
      confirmPreedit();
    }
    if (isStaleEcho(text)) {
      return;
    }
    setPreedit(text);
    confirmPreedit();
  };

  const onInput = (ev: Event): void => {
    if (!(ev instanceof InputEvent)) {
      return;
    }

    if (ev.inputType === "insertFromPaste" || ev.inputType === "insertFromYank") {
      if (composing) {
        flush();
      }
      return;
    }

    if (ev.inputType === "deleteCompositionText" || ev.inputType === "deleteByComposition") {
      swallowHangul(ev);
      return;
    }

    if (ev.inputType === "deleteContentBackward" && composing) {
      swallowHangul(ev);
      setPreedit("");
      return;
    }

    if (isPreeditInputType(ev.inputType)) {
      const text = ev.data ?? "";
      if (text && !isHangul(text)) {
        flush();
        return;
      }
      swallowHangul(ev);
      setPreedit(text);
      return;
    }

    if (!ev.data) {
      return;
    }

    if (ev.inputType !== "insertText" || !isHangul(ev.data)) {
      if (composing) {
        flush();
      }
      return;
    }

    if (!(wkEnv || composing || echoed || isStaleEcho(ev.data))) {
      return;
    }

    swallowHangul(ev);
    if (ev.isComposing) {
      setPreedit(ev.data);
      return;
    }
    applyCommittedHangul(ev.data);
  };

  const onKeyDown = (ev: KeyboardEvent): void => {
    if (ev.keyCode === 229) {
      imeKeySeq += 1;
      if (!composing) {
        return;
      }
      ev.preventDefault();
      ev.stopPropagation();
      ev.stopImmediatePropagation();
      return;
    }
    if (!composing) {
      return;
    }
    flush();
  };

  const inputTarget = root ?? textarea;
  if (inputTarget) {
    inputTarget.addEventListener("beforeinput", onInput, true);
    inputTarget.addEventListener("input", onInput, true);
  }
  if (textarea) {
    textarea.addEventListener("keydown", onKeyDown, true);
  }

  return {
    isComposing: () => composing || holdOnData,
    ignorePtyData: (data: string) => {
      if (composing || holdOnData) {
        return true;
      }
      return [...data].length === 1 && isHangul(data) && isStaleEcho(data);
    },
    flush,
    handleKeyEvent: (ev: KeyboardEvent) => {
      if (ev.keyCode === 229) {
        return composing ? false : undefined;
      }
      if (!composing && !holdOnData) {
        return undefined;
      }
      if (ev.type === "keydown") {
        flush();
      }
      return undefined;
    },
    dispose: () => {
      flush();
      if (inputTarget) {
        inputTarget.removeEventListener("beforeinput", onInput, true);
        inputTarget.removeEventListener("input", onInput, true);
      }
      if (textarea) {
        textarea.removeEventListener("keydown", onKeyDown, true);
      }
    },
  };
}
