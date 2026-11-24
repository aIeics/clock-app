# Clock

A lightweight desktop clock app written in Rust: clock, timer, reminders and calendar in one small native window. No web view, no background services, no telemetry.

**Status:** the Clock tab shows the live time with 12/24-hour and seconds options, saved between runs. Timer, Reminders and Calendar are placeholders. See [`PLAN.md`](PLAN.md) for the roadmap.

## Run

Needs Rust 1.95 or newer ([rustup.rs](https://rustup.rs)).

```sh
cargo run                 # debug build
cargo build --release     # small optimized build: target/release/clock-app.exe
```

## Contributing

Work happens on a branch per task, merged into `main` when done. The rules (also used by AI assistants) are in [`CLAUDE.md`](CLAUDE.md).
