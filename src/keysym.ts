// Map browser KeyboardEvent to X11 keysyms for the RFB KeyEvent message.

const SPECIAL: Record<string, number> = {
  Backspace: 0xff08,
  Tab: 0xff09,
  Enter: 0xff0d,
  Escape: 0xff1b,
  Insert: 0xff63,
  Delete: 0xffff,
  Home: 0xff50,
  End: 0xff57,
  PageUp: 0xff55,
  PageDown: 0xff56,
  ArrowLeft: 0xff51,
  ArrowUp: 0xff52,
  ArrowRight: 0xff53,
  ArrowDown: 0xff54,
  Shift: 0xffe1,
  Control: 0xffe3,
  Alt: 0xffe9,
  AltGraph: 0xffea,
  Meta: 0xffeb, // Super_L
  CapsLock: 0xffe5,
  NumLock: 0xff7f,
  ContextMenu: 0xff67,
  Pause: 0xff13,
  PrintScreen: 0xff61,
  ScrollLock: 0xff14,
};

export function eventToKeysym(e: KeyboardEvent): number | null {
  if (e.key.length === 1) {
    const cp = e.key.codePointAt(0)!;
    return cp <= 0xff ? cp : 0x01000000 + cp;
  }
  if (e.key.startsWith("F") && e.key.length <= 3) {
    const n = parseInt(e.key.slice(1), 10);
    if (n >= 1 && n <= 12) return 0xffbe + (n - 1);
  }
  return SPECIAL[e.key] ?? null;
}
