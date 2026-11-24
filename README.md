# Clock

A lightweight desktop clock app written in Rust: clock, timer, reminders and calendar in one small native window. No web view, no background services, no telemetry.

**Status:** Clock (saved 12/24-hour and seconds options), Timer (countdown) and Reminders (create/edit/delete, daily/weekly repeat, saved to disk) work. Calendar is a placeholder. See [`PLAN.md`](PLAN.md) for the roadmap.

## Run

Needs Rust 1.95 or newer ([rustup.rs](https://rustup.rs)).

```sh
cargo run                 # debug build
cargo build --release     # small optimized build: target/release/clock-app.exe
```

## Contributing

Work happens on a branch per task, merged into `main` when done. The rules (also used by AI assistants) are in [`CLAUDE.md`](CLAUDE.md).
