// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const bridge = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: bridge.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: bridge.listen }));

const initialStatus = {
  microphone: { id: 1, name: 'Test microphone', muted: false },
  error: null as string | null,
  shortcut: 'Super+Shift+M',
  shortcut_error: null as string | null,
  border_enabled: true,
  badge_position: 'se',
  launch_at_login: false,
  preview: false,
};
type Status = Omit<typeof initialStatus, 'microphone'> & { microphone: typeof initialStatus.microphone | null };
let status: Status;
let emit: (event: { payload: Status }) => void;
const cleanup: (() => void)[] = [];
const node = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const settle = async () => {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
};
async function mount(search = '') {
  history.replaceState({}, '', `/${search}`);
  await import('../src/ui/main');
}

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  status = structuredClone(initialStatus);
  document.documentElement.className = '';
  document.body.className = '';
  document.body.innerHTML = '<div id="app"></div>';
  for (const target of [document, window]) {
    const original = target.addEventListener.bind(target);
    vi.spyOn(target, 'addEventListener').mockImplementation((type, listener, options) => {
      original(type, listener, options);
      cleanup.push(() => target.removeEventListener(type, listener, options));
    });
  }
  bridge.listen.mockImplementation(async (_name, callback) => {
    emit = callback;
    return () => {};
  });
  bridge.invoke.mockImplementation(async () => status);
});
afterEach(() => {
  cleanup.splice(0).forEach((remove) => remove());
  vi.restoreAllMocks();
});

describe('settings window', () => {
  it('subscribes before reading state and reflects device and shortcut errors', async () => {
    await mount();
    expect(bridge.listen.mock.invocationCallOrder[0]).toBeLessThan(bridge.invoke.mock.invocationCallOrder[0]);
    expect(node('mic-name').textContent).toBe('Test microphone');
    expect(node<HTMLButtonElement>('toggle').disabled).toBe(false);
    emit({ payload: { ...status, microphone: null, error: 'Input disconnected', shortcut_error: 'Shortcut in use' } });
    expect(node<HTMLButtonElement>('toggle').disabled).toBe(true);
    expect(node('error').textContent).toBe('Input disconnected Shortcut in use');
    expect(node('error').hidden).toBe(false);
    emit({ payload: status });
    expect(node('error').hidden).toBe(true);
  });

  it('preserves unsaved edits across status events and submits them', async () => {
    await mount();
    node<HTMLInputElement>('shortcut').value = 'Control+Alt+M';
    node('shortcut').dispatchEvent(new Event('input'));
    node<HTMLInputElement>('border').checked = false;
    node('border').dispatchEvent(new Event('change'));
    emit({ payload: status });
    expect(node<HTMLInputElement>('shortcut').value).toBe('Control+Alt+M');
    expect(node<HTMLInputElement>('border').checked).toBe(false);
    status = { ...status, shortcut: 'Control+Alt+M', border_enabled: false };
    node('settings').dispatchEvent(new Event('submit', { cancelable: true }));
    await settle();
    expect(bridge.invoke).toHaveBeenCalledWith('save_settings', { shortcut: 'Control+Alt+M', borderEnabled: false });
    expect(node('message').textContent).toBe('Settings saved.');
  });

  it('preserves newer edits while saving', async () => {
    await mount();
    node<HTMLInputElement>('shortcut').value = 'Control+Alt+M';
    node('shortcut').dispatchEvent(new Event('input'));
    let finish!: (value: Status) => void;
    bridge.invoke.mockImplementation(async (command) => {
      if (command === 'save_settings')
        return new Promise<Status>((resolve) => {
          finish = resolve;
        });
      return status;
    });
    node('settings').dispatchEvent(new Event('submit', { cancelable: true }));
    node<HTMLInputElement>('shortcut').value = 'Super+K';
    node('shortcut').dispatchEvent(new Event('input'));
    node<HTMLInputElement>('border').checked = false;
    node('border').dispatchEvent(new Event('change'));
    finish({ ...status, shortcut: 'Control+Alt+M' });
    await settle();
    expect(node<HTMLInputElement>('shortcut').value).toBe('Super+K');
    expect(node<HTMLInputElement>('border').checked).toBe(false);
    emit({ payload: status });
    expect(node<HTMLInputElement>('shortcut').value).toBe('Super+K');
    expect(node('message').textContent).toContain('newer changes');
  });

  it('blocks duplicate saves and permits retry after a failed save', async () => {
    await mount();
    let fail!: (reason: string) => void;
    bridge.invoke.mockImplementation(async (command) => {
      if (command === 'save_settings')
        return new Promise<Status>((_resolve, reject) => {
          fail = reject;
        });
      return status;
    });
    node('settings').dispatchEvent(new Event('submit', { cancelable: true }));
    node('settings').dispatchEvent(new Event('submit', { cancelable: true }));
    expect(bridge.invoke.mock.calls.filter(([command]) => command === 'save_settings')).toHaveLength(1);
    fail('Cannot write settings');
    await settle();
    expect(node('message').textContent).toBe('Cannot write settings');
    bridge.invoke.mockImplementation(async () => status);
    node('settings').dispatchEvent(new Event('submit', { cancelable: true }));
    await settle();
    expect(node('message').textContent).toBe('Settings saved.');
  });

  it('prevents duplicate mute commands while a command is pending', async () => {
    await mount();
    let finish!: (value: Status) => void;
    bridge.invoke.mockReturnValueOnce(new Promise<Status>((resolve) => (finish = resolve)));
    node('toggle').click();
    node('toggle').click();
    expect(bridge.invoke.mock.calls.filter(([command]) => command === 'toggle_mute')).toHaveLength(1);
    expect(node<HTMLButtonElement>('toggle').disabled).toBe(true);
    finish({ ...status, microphone: { ...initialStatus.microphone, muted: true } });
    await settle();
    expect(node('mic-state').textContent).toBe('Microphone muted');
    expect(node<HTMLButtonElement>('toggle').disabled).toBe(false);
  });

  it('shows command errors and permits another attempt', async () => {
    await mount();
    bridge.invoke.mockRejectedValueOnce('Device rejected mute');
    node('toggle').click();
    await settle();
    expect(node('message').textContent).toBe('Device rejected mute');
    expect(node('message').classList.contains('failed')).toBe(true);
    expect(node<HTMLButtonElement>('toggle').disabled).toBe(false);
  });

  it('saves a label position and restores the selection when saving fails', async () => {
    await mount();
    const right = document.querySelector<HTMLInputElement>('input[value="e"]')!;
    status = { ...status, badge_position: 'e', preview: true };
    right.click();
    await settle();
    expect(bridge.invoke).toHaveBeenCalledWith('set_badge_position', { position: 'e' });
    bridge.invoke.mockRejectedValueOnce('Cannot save settings');
    document.querySelector<HTMLInputElement>('input[value="w"]')!.click();
    await settle();
    expect(right.checked).toBe(true);
    expect(node('message').textContent).toBe('Cannot save settings');
  });

  it('turns open at login on and restores the switch when the change fails', async () => {
    await mount();
    const login = node<HTMLInputElement>('login');
    expect(login.checked).toBe(false);
    expect(node('status-keys').textContent).toBe('⌘⇧M');
    status = { ...status, launch_at_login: true };
    login.click();
    await settle();
    expect(bridge.invoke).toHaveBeenCalledWith('set_launch_at_login', { enabled: true });
    expect(login.checked).toBe(true);
    expect(login.disabled).toBe(false);
    expect(node('message').textContent).toBe('MicMute will open at login.');
    bridge.invoke.mockRejectedValueOnce('Cannot change the login item');
    login.click();
    await settle();
    expect(login.checked).toBe(true);
    expect(node('message').textContent).toBe('Cannot change the login item');
  });

  it('records shortcuts and cancels recording when the window loses focus', async () => {
    await mount();
    node('record').click();
    await settle();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', code: 'KeyA', metaKey: true }));
    expect(node<HTMLInputElement>('shortcut').value).toBe('Super+A');
    expect(bridge.invoke).toHaveBeenCalledWith('set_recording', { recording: false });
    node('record').click();
    await settle();
    window.dispatchEvent(new Event('blur'));
    expect(node('record').textContent).toBe('Record');
    expect(node('message').textContent).toBe('Recording cancelled.');
  });
});

describe('overlay window', () => {
  it('shows a labelled preview at the selected edge and then the mute label', async () => {
    status = { ...status, preview: true, badge_position: 'w' };
    await mount('?overlay=1');
    expect(document.body.className).toBe('overlay');
    expect(node('badge-text').textContent).toBe('PREVIEW · MIC • MUTED');
    expect(document.querySelector<HTMLElement>('.mute-badge')!.dataset.position).toBe('w');
    expect(document.querySelector('form')).toBeNull();
    emit({ payload: { ...status, preview: false } });
    expect(node('badge-text').textContent).toBe('MIC • MUTED');
  });
});
