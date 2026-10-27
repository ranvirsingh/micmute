import { describe, expect, it } from 'vitest';
import { recordShortcut, type KeyPress } from '../src/ui/shortcut';

const press = (code: string, key: string, mods: Partial<KeyPress> = {}): KeyPress => ({
  code,
  key,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe('recordShortcut', () => {
  it('records the default Command + Shift + M shortcut', () => {
    expect(recordShortcut(press('KeyM', 'm', { metaKey: true, shiftKey: true }))).toEqual({
      kind: 'shortcut',
      value: 'Super+Shift+M',
    });
  });

  it('orders modifiers as Super, Control, Alt, Shift', () => {
    const all = { metaKey: true, ctrlKey: true, altKey: true, shiftKey: true };
    expect(recordShortcut(press('KeyA', 'a', all))).toEqual({ kind: 'shortcut', value: 'Super+Control+Alt+Shift+A' });
  });

  it('strips Digit and Arrow prefixes and keeps function keys', () => {
    expect(recordShortcut(press('Digit5', '5', { ctrlKey: true }))).toEqual({ kind: 'shortcut', value: 'Control+5' });
    expect(recordShortcut(press('ArrowUp', 'ArrowUp', { altKey: true }))).toEqual({
      kind: 'shortcut',
      value: 'Alt+Up',
    });
    expect(recordShortcut(press('F5', 'F5', { metaKey: true }))).toEqual({ kind: 'shortcut', value: 'Super+F5' });
  });

  it('requires Command, Control, or Option', () => {
    expect(recordShortcut(press('KeyM', 'M', { shiftKey: true }))).toEqual({ kind: 'needs-modifier' });
    expect(recordShortcut(press('KeyM', 'm'))).toEqual({ kind: 'needs-modifier' });
  });

  it('waits while only a modifier is held', () => {
    for (const key of ['Meta', 'Control', 'Alt', 'Shift']) {
      expect(recordShortcut(press(`${key}Left`, key, { metaKey: true }))).toEqual({ kind: 'modifier-only' });
    }
  });

  it('cancels on Escape', () => {
    expect(recordShortcut(press('Escape', 'Escape', { metaKey: true }))).toEqual({ kind: 'cancel' });
  });
});
