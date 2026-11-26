# Clock

A lightweight desktop clock for Windows, written in Rust: clock, timer, reminders and calendar in one small native window. No web view, no background services, no telemetry, no network access. Your data stays in a few small JSON files on your PC.

## Features

- **Clock:** analog dial with digital time and date, 12/24-hour and seconds options.
- **Pop-out widget:** puts the time and date on your desktop as transparent text behind your windows. It starts locked: right-click it to unlock, then drag to move and drag the corner to resize. Three fonts and a choice of text colours.
- **Timer:** countdown set on hour, minute and second wheels.
- **Reminders:** title, date, time and an optional note; repeat daily or weekly. Reminders missed while Clock was closed show up on the next start.
- **Calendar:** a week strip with your reminders, an hour-by-hour day view and quick add.
- **Runs in the tray:** closing the window keeps Clock running in the system tray so reminders still fire. Click the tray icon to bring it back; right-click for Restore, Start with Windows and Quit.
- **Start with Windows:** a checkbox on the Clock tab, in the tray menu and in the installer. Clock then starts hidden in the tray when you sign in.
- **One at a time:** opening Clock again brings the running one to the front.

## Install

Download `Clock-Setup-<version>.exe` from [Releases](https://github.com/aIeics/clock-app/releases) and run it. It installs for your user only (no admin rights) to `%LOCALAPPDATA%\Programs\Clock` and adds a Start menu shortcut.

Prefer no installer? Download the `.zip`, unpack it anywhere and run `clock-app.exe`.

Uninstall from Settings → Apps. Your settings and reminders in `%APPDATA%\clock-app` are kept; delete that folder to remove them too.

Windows 10 or 11, 64-bit. The app is not code-signed yet, so Windows SmartScreen may warn on first run: choose "More info" → "Run anyway".

## Build from source

Needs Rust 1.95 or newer ([rustup.rs](https://rustup.rs)).

```sh
cargo run                 # debug build, with a console for logs
cargo build --release     # small optimized build: target/release/clock-app.exe
```

The installer needs [Inno Setup 6](https://jrsoftware.org/isinfo.php) (`winget install --id JRSoftware.InnoSetup -e --scope user`):

```powershell
.\installer\build.ps1     # → target\installer\Clock-Setup-<version>.exe and a .zip
```

## Contributing

Issues and pull requests are welcome. Work happens on a branch per task, merged into `main` when done; the rules (also used by AI assistants) are in [`CLAUDE.md`](CLAUDE.md), and the roadmap and design decisions are in [`PLAN.md`](PLAN.md).

## License

[MIT](LICENSE). The bundled fonts keep their own licenses: Manrope and E1234 under the SIL Open Font License, Digit Tech in the public domain (see `assets/fonts/`).
