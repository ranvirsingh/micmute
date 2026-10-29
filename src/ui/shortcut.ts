export interface KeyPress {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

export type RecordedKey =
  { kind: 'cancel' } | { kind: 'modifier-only' } | { kind: 'needs-modifier' } | { kind: 'shortcut'; value: string };

// Converts a key press into the accelerator format the Rust side parses.
export function recordShortcut(event: KeyPress): RecordedKey {
  if (event.key === 'Escape') return { kind: 'cancel' };
  if (['Meta', 'Control', 'Alt', 'Shift'].includes(event.key)) return { kind: 'modifier-only' };
  if (!(event.metaKey || event.ctrlKey || event.altKey)) return { kind: 'needs-modifier' };
  const modifiers = [
    event.metaKey && 'Super',
    event.ctrlKey && 'Control',
    event.altKey && 'Alt',
    event.shiftKey && 'Shift',
  ].filter(Boolean);
  let key = event.code;
  if (/^Key[A-Z]$/.test(key)) key = key.slice(3);
  if (/^Digit[0-9]$/.test(key)) key = key.slice(5);
  if (key.startsWith('Arrow')) key = key.slice(5);
  return { kind: 'shortcut', value: [...modifiers, key].join('+') };
}

const symbols: Record<string, string> = {
  CommandOrControl: '⌘',
  CmdOrCtrl: '⌘',
  Command: '⌘',
  Cmd: '⌘',
  Super: '⌘',
  Meta: '⌘',
  Control: '⌃',
  Ctrl: '⌃',
  Alt: '⌥',
  Option: '⌥',
  Shift: '⇧',
  Up: '↑',
  Down: '↓',
  Left: '←',
  Right: '→',
  Space: 'Space',
};

// Splits an accelerator into the key caps shown in the settings window.
export function shortcutKeys(value: string): string[] {
  return value
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => symbols[part] ?? (part.length === 1 ? part.toUpperCase() : part));
}
