# micmute

MicMute is a small macOS menu bar app built with Rust and Tauri.
It controls the current system input microphone through Core Audio.

## Use the app

1. Open `MicMute.app`.
2. Use **Command + Shift + M** to mute or unmute the system input microphone.
3. Open **Settings…** from the microphone icon in the menu bar.
4. Select **Record** to enter another shortcut.
5. Select **Save settings** to apply the shortcut and border preference.

The thin red border and glowing **MIC • MUTED** label appear on each connected display while the microphone reports muted.
The border is three pixels wide. The label appears in the bottom-right corner by default.
Use the eight-position picker in settings to move the label to another corner or the middle of an edge.
Each selection saves immediately and displays a three-second preview without changing the microphone.
The left and right positions stack upright characters vertically. The other six positions display a horizontal label.
The bullet separates MIC and MUTED in all eight positions.
The menu bar microphone icon turns red when muted. Its normal color follows the system appearance when unmuted.
An amber icon indicates that the microphone state is unavailable.
The indicator permits clicks through to the app underneath it.
**Preview border** displays a labelled preview for three seconds without changing the microphone.
Closing the settings window keeps the app running in the menu bar.
Quitting removes the border and keeps the microphone's current mute state.
The app does not start automatically at login.
Close Mic Drop before you use MicMute. Both apps can change the same microphone and overwrite each other's state.

## Microphone scope

Use the same input in your call app and macOS Sound settings.
MicMute follows the system default input. It does not control other input devices.
When the default input changes, the indicator follows the new input's reported mute state.
The previous input keeps its current state.

This version requires a writable device-wide input mute property.
Unsupported devices display an error. The app does not substitute a zero-volume setting.
The indicator follows the device state reported by Core Audio, with a normal update interval of 300 milliseconds.
Call apps can retain their own mute state after you unmute the device.

The app does not record audio. It does not send audio or settings to a server.
The app stores settings in its local configuration directory as `settings.json`.

## Project layout

```text
src/
  ui/                 Settings window and border overlay (TypeScript, Vite)
  core/               Native application (Rust, Tauri)
    src/main.rs       Windows, tray, shortcut, settings
    src/audio.rs      Core Audio microphone backend
    src/overlay.rs    Display window coordination and recovery
    src/tests.rs      Rust unit tests
tests/                TypeScript unit and UI integration tests (Vitest)
config/               Vite, Vitest, and ESLint configuration
assets/               Source artwork
.github/              CI and bug report template
```

Both application layers share one source root.
The `core` folder contains the Rust application and its operating system integration.
The interface starts at `src/ui/index.html`.
`config/` contains web tooling configuration. npm scripts select these files explicitly.
TypeScript configuration stays at the root so editors find it automatically.
Prettier settings use the `prettier` field in `package.json`.

## Build from source

Requirements: macOS, Xcode command line tools, Node.js 22.22.2 or later in the 22.x series, and a current stable Rust toolchain.

1. Open a terminal in this source folder.
2. Run `npm ci`.
3. Run `npm run tauri -- dev` for development.
4. Run `npm run tauri -- build --bundles app` to create the app bundle.

The default build location is `src/core/target/release/bundle/macos/MicMute.app`.
If you set `CARGO_TARGET_DIR`, the bundle appears under that target directory instead.

## Checks

Run all checks before you commit changes:

```sh
npm run check
```

This command checks TypeScript, ESLint, Prettier, browser tests, and the Vite build.
It also runs `cargo fmt`, Clippy with warnings treated as errors, and Rust tests.
CI runs the same checks on pushes to `main` and manual workflow runs.
Pull requests and bug reports do not trigger CI.
Run `npm run format` and `cargo fmt --manifest-path src/core/Cargo.toml` to fix formatting.

## Diagnostics

Run these commands against the executable inside the app bundle:

```sh
"MicMute.app/Contents/MacOS/micmute" --probe
"MicMute.app/Contents/MacOS/micmute" --test-mic
```

`--probe` reports the current input name and mute state without changing them.
`--test-mic` changes the mute state for two seconds, checks that the state remains stable, and restores the original state.
Do not run the mute test during a call.

## Platform boundary

`src/core/src/audio.rs` contains the microphone backend.
The macOS backend uses Core Audio directly from Rust.
A future Windows backend can use `IAudioEndpointVolume` behind the same interface.
The Windows backend is not implemented or tested in this version.

The default build uses an ad hoc local signature.
Builds from this repository have no Developer ID signature or Apple notarization.
macOS Gatekeeper can block an unsigned app that you download. Build the app from source for local use.

## Bug reports and contributions

This project does not currently accept external contributions or pull requests.
Please report bugs through GitHub Issues with steps to reproduce the problem.
Forks are welcome under the MIT license. Retain the original copyright and license notice.

## License

MIT. See [LICENSE](LICENSE).
