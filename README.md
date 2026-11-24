# Clock

A lightweight desktop clock app written in Rust: clock, timer, reminders and calendar in one small native window. No web view, no background services, no telemetry.

**Status:** all four features work: Clock with saved 12/24-hour and seconds options, Timer countdown, Reminders with repeats and disk storage, and a Calendar tied to the same reminders. The Clock tab can pop the time and date into a transparent, always-behind desktop widget with no background, border or buttons. It starts locked: right-click the text to unlock or close it. Unlocking works on the next drag; drag the text to move it and drag the bottom-right corner to resize. Closing the main window hides the app in the system tray; click the tray icon to restore it or right-click for Restore/Quit. See [`PLAN.md`](PLAN.md) for the roadmap and remaining optional polish.

## Run

Needs Rust 1.95 or newer ([rustup.rs](https://rustup.rs)).

```sh
cargo run                 # debug build
cargo build --release     # small optimized build: target/release/clock-app.exe
```

## Contributing

Work happens on a branch per task, merged into `main` when done. The rules (also used by AI assistants) are in [`CLAUDE.md`](CLAUDE.md).
