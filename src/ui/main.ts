import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { recordShortcut, shortcutKeys } from './shortcut';
import './style.css';

interface Status {
  microphone: { id: number; name: string; muted: boolean } | null;
  error: string | null;
  shortcut: string;
  shortcut_error: string | null;
  border_enabled: boolean;
  badge_position: string;
  launch_at_login: boolean;
  preview: boolean;
}
const root = document.querySelector<HTMLDivElement>('#app')!;
const overlay = new URLSearchParams(location.search).has('overlay');
if (overlay) {
  document.documentElement.classList.add('overlay-root');
  document.body.className = 'overlay';
  root.innerHTML =
    '<div class="screen-border" aria-hidden="true"></div><div class="mute-badge"><span id="badge-text">MIC • MUTED</span></div>';
} else {
  root.innerHTML = `
    <main>
      <header>
        <svg class="app-mark" viewBox="0 0 512 512" aria-hidden="true"><rect x="24" y="24" width="464" height="464" rx="112" fill="#282a2b"/><rect x="48" y="48" width="416" height="416" rx="92" fill="none" stroke="#ec3046" stroke-width="20"/><rect x="218" y="128" width="76" height="162" rx="38" fill="#fff"/><path d="M180 244v14a76 76 0 0 0 152 0v-14M256 334v48m-46 0h92" fill="none" stroke="#fff" stroke-width="18" stroke-linecap="round"/></svg>
        <div><h1>MicMute</h1><p>A shortcut. A clear signal.</p></div>
      </header>
      <section class="status-card" id="status-card" aria-live="polite">
        <div class="status-line"><span class="status-dot" id="status-dot"></span><strong id="mic-state">Checking microphone…</strong><span class="keys" id="status-keys" title="Shortcut"></span></div>
        <p id="mic-name">System input</p>
        <button id="toggle" class="primary" disabled>Checking…</button>
      </section>
      <form id="settings">
        <section class="panel panel-general">
          <h2>General</h2>
          <div class="shortcut-row"><input id="shortcut" aria-label="Keyboard shortcut" spellcheck="false" autocomplete="off" aria-describedby="shortcut-help" value="CommandOrControl+Shift+M"><button type="button" id="record">Record</button></div>
          <p class="help" id="shortcut-help">Works in any app. Select Record, then press the new keys.</p>
          <label class="switch-row divided"><span>Open at login<small>Starts quietly in the menu bar when you log in.</small></span><input id="login" class="switch" type="checkbox" role="switch"></label>
        </section>
        <section class="panel panel-indicator">
          <h2>Indicator</h2>
          <label class="switch-row"><span>Show a red border while muted</span><input id="border" class="switch" type="checkbox" role="switch" checked></label>
          <fieldset class="position-control"><legend>Muted label position</legend>
            <div class="position-layout">
              <div class="position-grid">
                ${[
                  ['nw', 'Top left', '↖'],
                  ['n', 'Top', '↑'],
                  ['ne', 'Top right', '↗'],
                  ['w', 'Left', '←'],
                  ['center', '', 'SCREEN'],
                  ['e', 'Right', '→'],
                  ['sw', 'Bottom left', '↙'],
                  ['s', 'Bottom', '↓'],
                  ['se', 'Bottom right', '↘'],
                ]
                  .map(([value, label, arrow]) =>
                    value === 'center'
                      ? '<span class="position-center" aria-hidden="true">SCREEN</span>'
                      : `<label class="position-choice" title="${label}"><input type="radio" name="position" value="${value}" aria-label="${label}" ${value === 'se' ? 'checked' : ''}><span aria-hidden="true">${arrow}</span></label>`,
                  )
                  .join('')}
              </div>
              <p class="help">Choose a position to save it and preview it for 3 seconds.</p>
            </div>
          </fieldset>
          <div class="actions"><button id="save" class="secondary" type="submit">Save settings</button><button type="button" id="preview" class="text-button">Preview border</button></div>
        </section>
      </form>
      <p id="message" role="status" class="message"></p>
      <p id="error" role="alert" class="error" hidden></p>
      <footer><p>Controls the current system input. Quitting keeps its mute state.</p><div><button id="hide" class="text-button">Hide to menu bar</button><button id="quit" class="text-button subtle">Quit</button></div></footer>
    </main>`;
}

let latest: Status | undefined;
let dirty = false;
let editRevision = 0;
let saving = false;
let recording = false;
let busy = false;
const element = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const message = (text: string, error = false) => {
  const node = element<HTMLParagraphElement>('message');
  node.textContent = text;
  node.classList.toggle('failed', error);
};
function render(status: Status) {
  latest = status;
  if (overlay) {
    element('badge-text').textContent = status.preview ? 'PREVIEW · MIC • MUTED' : 'MIC • MUTED';
    document.querySelector<HTMLElement>('.mute-badge')!.dataset.position = status.badge_position;
    return;
  }
  const mic = status.microphone;
  element('mic-state').textContent = mic
    ? mic.muted
      ? 'Microphone muted'
      : 'Microphone unmuted'
    : 'Microphone unavailable';
  element('mic-name').textContent = mic?.name ?? 'Choose a supported input in macOS Sound settings.';
  const state = mic ? (mic.muted ? 'muted' : 'live') : 'unknown';
  element('status-dot').className = `status-dot ${state}`;
  element('status-card').dataset.state = state;
  element('status-keys').replaceChildren(
    ...shortcutKeys(status.shortcut).map((key) => Object.assign(document.createElement('kbd'), { textContent: key })),
  );
  element('toggle').textContent = mic?.muted ? 'Unmute microphone' : 'Mute microphone';
  element<HTMLButtonElement>('toggle').disabled = !mic || busy;
  const error = [status.error, status.shortcut_error].filter(Boolean).join(' ');
  element('error').textContent = error;
  element('error').hidden = !error;
  document.querySelectorAll<HTMLInputElement>('input[name="position"]').forEach((input) => {
    input.checked = input.value === status.badge_position;
  });
  element<HTMLInputElement>('login').checked = status.launch_at_login;
  if (!dirty) {
    element<HTMLInputElement>('shortcut').value = status.shortcut;
    element<HTMLInputElement>('border').checked = status.border_enabled;
  }
}

async function act(command: string, args?: Record<string, unknown>) {
  try {
    const status = await invoke<Status>(command, args);
    render(status);
    return true;
  } catch (error) {
    if (!overlay) message(String(error), true);
    return false;
  }
}

// Install the listener before requesting the initial state.
await listen<Status>('status', (event) => render(event.payload));
await act('get_status');

if (!overlay) {
  const shortcut = element<HTMLInputElement>('shortcut');
  element('toggle').addEventListener('click', async () => {
    if (busy) return;
    busy = true;
    if (latest) render(latest);
    message('');
    await act('toggle_mute');
    busy = false;
    if (latest) render(latest);
  });
  shortcut.addEventListener('input', () => {
    dirty = true;
    editRevision += 1;
  });
  element('border').addEventListener('change', () => {
    dirty = true;
    editRevision += 1;
  });
  document.querySelectorAll<HTMLInputElement>('input[name="position"]').forEach((input) =>
    input.addEventListener('change', async () => {
      if (!input.checked) return;
      if (await act('set_badge_position', { position: input.value }))
        message('Label position saved. Previewing for 3 seconds.');
      else if (latest) render(latest);
    }),
  );
  element('login').addEventListener('change', async (event) => {
    const input = event.currentTarget as HTMLInputElement;
    input.disabled = true;
    if (await act('set_launch_at_login', { enabled: input.checked }))
      message(input.checked ? 'MicMute will open at login.' : 'MicMute will not open at login.');
    else if (latest) render(latest);
    input.disabled = false;
  });
  const endRecording = () => {
    recording = false;
    void invoke('set_recording', { recording: false });
    element('record').textContent = 'Record';
    element('record').classList.remove('recording');
  };
  element('record').addEventListener('click', async () => {
    recording = !recording;
    await invoke('set_recording', { recording });
    element('record').textContent = recording ? 'Press keys…' : 'Record';
    element('record').classList.toggle('recording', recording);
    if (recording) message('Press a shortcut with ⌘, ⌃, or ⌥. Escape cancels.');
  });
  window.addEventListener('blur', () => {
    if (recording) {
      endRecording();
      message('Recording cancelled.');
    }
  });
  document.addEventListener(
    'keydown',
    (event) => {
      if (!recording) return;
      event.preventDefault();
      event.stopPropagation();
      const recorded = recordShortcut(event);
      if (recorded.kind === 'modifier-only') return;
      if (recorded.kind === 'needs-modifier') {
        message('Include ⌘, ⌃, or ⌥.', true);
        return;
      }
      if (recorded.kind === 'cancel') {
        endRecording();
        message('Recording cancelled.');
        return;
      }
      shortcut.value = recorded.value;
      dirty = true;
      editRevision += 1;
      endRecording();
      message('Shortcut recorded. Select Save settings to apply it.');
    },
    true,
  );
  element('settings').addEventListener('submit', async (event) => {
    event.preventDefault();
    if (saving) return;
    saving = true;
    element<HTMLButtonElement>('save').disabled = true;
    const submittedRevision = editRevision;
    endRecording();
    try {
      const ok = await act('save_settings', {
        shortcut: shortcut.value,
        borderEnabled: element<HTMLInputElement>('border').checked,
      });
      if (ok) {
        if (editRevision === submittedRevision) {
          dirty = false;
          if (latest) render(latest);
          message('Settings saved.');
        } else {
          message('Settings saved. Your newer changes are not saved yet.');
        }
      }
    } finally {
      saving = false;
      element<HTMLButtonElement>('save').disabled = false;
    }
  });
  element('preview').addEventListener('click', async () => {
    if (await act('preview_border')) message('Border preview for 3 seconds. The microphone state stays unchanged.');
  });
  element('hide').addEventListener('click', () => invoke('hide_settings'));
  element('quit').addEventListener('click', () => invoke('quit'));
}
