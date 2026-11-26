# Clock App Plan

A small native desktop app with four features: **Clock, Timer, Reminders, Calendar**. Windows first; eframe is cross-platform, so Linux and macOS are possible later (untested).

> **Workflow:** every change goes on its own branch, each edit is committed and pushed, and the assistant asks before merging into `main` and deleting the branch. Full rules: [`CLAUDE.md`](CLAUDE.md).

## Principles (no bloatware)

- A single native Rust `.exe`. No web view, no Electron. The installer (D7) is optional: the `.exe` also runs on its own, and it links the C runtime statically, so it needs no VC++ runtime.
- No background services, telemetry, accounts or network access. Data stays in a local file.
- Few dependencies: every new crate needs a reason, with default features off.
- No async runtime. `std` threads are enough for an app this size.
- Idle means idle: the UI repaints only when something on screen changes.

## Stack

| Part | Choice | Why |
| --- | --- | --- |
| GUI | `eframe`/`egui` 0.37, `glow` (OpenGL) backend, default features off | Pure Rust, simple immediate-mode code, cross-platform |
| Time and dates | `chrono` 0.4 with only `clock` + `std` | Local time now; date math for the calendar later |
| Storage | `serde` + `serde_json` | Settings, reminders and the pop-out layout as small, hand-editable JSON files (D1) |
| System tray | `tray-icon` 0.26, default features off | Close-to-tray with a left-click restore and a Restore/Quit menu (D4) |
| Font | Manrope variable font (`assets/fonts/`, SIL Open Font License), embedded with `include_bytes!` | One 161 KB file gives semi-bold text and bold numbers; egui's built-in fonts stay as fallbacks |
| Release build | `opt-level = "z"`, LTO, 1 codegen unit, `panic = "abort"`, stripped | Smallest binary |

**Lighter alternative considered:** a Windows-only native Win32 UI (`windows-rs`). It would use less disk and memory, but it is Windows-only and needs much more code per screen. Switching later means rewriting the `ui` functions; the logic modules would carry over.

### eframe default features we turned off

Re-enable one by adding it to the `eframe` features list in `Cargo.toml`.

| Feature | What you lose without it | Plan |
| --- | --- | --- |
| `wgpu` | Nothing for this app; `glow` renders the UI with far fewer dependencies | Keep off |
| `clipboard` | Copy/paste between this app and other apps in text fields | Turn on in Phase 3 if reminder text needs pasting |
| `accesskit` | Screen reader support | Turn on if needed |
| `system_fonts` | Fallback to system fonts for characters the built-in fonts lack (e.g. Chinese, Japanese) | Turn on if needed |
| `links` | Opening web links in the browser | Keep off |
| `persistence` | eframe's built-in state saving | Keep off: we save our own JSON files (D1) |

## Measured baseline

Windows 11, release build, window open on the Clock tab, three runs. "Phase 5" is after start with Windows, single instance, the static C runtime and the installer.

| Metric | Phase 0 | Phase 5 |
| --- | --- | --- |
| `.exe` size | 4.6 MB (4,632,064 bytes) | 5.4 MB (5,373,952 bytes; static C runtime included) |
| Installer size | — | 3.8 MB (`Clock-Setup-0.1.0.exe`) |
| of which built-in fonts | 1.4 MB, including 0.74 MB of emoji fonts | unchanged |
| Crates compiled for Windows | 91 dependencies plus the app (`cargo tree --target x86_64-pc-windows-msvc -e normal`) | 103 unique (same before and after adding `windows-sys`) |
| Memory, about 14 s after start | 72–75 MB (working set 72.4–74.6 MB, private 72.7–74.7 MB) | 78–79 MB (working set 78.4–79.2 MB, private 78.8–79.3 MB) |
| Repaints when idle | Exactly 1 per second, just after the second ticks (checked by logging every frame). Each input event (mouse, focus) adds a burst of 3 frames. | not re-measured; no new repaint sources |
| CPU, idle on the Clock tab | 0–141 ms per 10 s (at most 1.4% of one core) | not re-measured |
| Cold build time | debug 25 s, release 71 s | release 91 s |
| Needs at runtime | `VCRUNTIME140.dll` (Microsoft VC++ runtime) | nothing beyond Windows |

Memory is the weakest number: an OpenGL window costs more than a native Win32 one. Re-measure at the end of every phase and update this table. Budget: `.exe` under 6 MB; no repaints on views where nothing changes.

## Architecture

```text
src/
  main.rs        window setup, starts eframe
  app.rs         ClockApp: tab bar, routes to the selected feature
  clock.rs       ClockView      analog dial, digital time and date, 12/24-hour and seconds
  timer.rs       TimerView      countdown on hour/minute/second scroll wheels
  reminders/     RemindersView  create/edit/delete, repeat, due and missed
    mod.rs       Reminders      list, storage and firing
    schedule.rs                 time parsing and repeat rules (unit tested)
    view.rs                     the Reminders tab
  calendar.rs    CalendarView   week strip, hour-by-hour day view, quick add
  settings.rs    Settings       saved preferences
  storage.rs     Store          JSON files in %APPDATA%\clock-app\
  icon.rs        the window icon, drawn in code
  theme.rs       black/white style, painted widgets and icons, eased animations
  popout.rs      frameless floating clock, move/resize, lock and unlock (D6)
  tray.rs        system tray icon, close-to-tray, restore, Start with Windows and quit (D4)
  autostart.rs   Start with Windows: the HKCU Run registry value (D7)
  instance.rs    single instance: a second launch shows the running app (D7)
installer/
  clock.iss      Inno Setup script (D7)
  build.ps1      builds the release .exe, then target\installer\Clock-Setup-<version>.exe
```

- One module per feature. Each owns its state and has `ui(&mut self, ui: &mut egui::Ui)`.
- Time math, scheduling and storage stay out of `ui` code so they can be unit tested without a window. A module that grows becomes a folder (`reminders/mod.rs`, `reminders/store.rs`, ...).
- A view that shows changing time asks for its next repaint at the exact moment it changes (see `clock.rs`). Other views schedule nothing.
- Look: black background, white text, no gradients; red only for alerts and errors. Text is Manrope: semi-bold (600) by default, bold (780) for large numbers and headings, set through the font's weight axis. Widgets (tab bar, buttons, checkboxes, icons, the clock dial and timer wheels) are painted in `theme.rs` and the views, with no extra crates. Content sits in a centred column up to 560 px wide and scales with the window; long tabs scroll.
- Animations (tab fade, sliding selectors, hover, month slide, banner) only repaint while they move. Nothing loops. The one exception by choice: the clock's second hand bounces for about 0.2 s on each tick, so the Clock tab repaints for a short burst every second while seconds are shown. Every other tab is idle when nothing changes.

## Phases

Each phase (or part of one) is its own branch. Tick the boxes as work lands on `main`.

### Phase 0: Base ✅

- [x] Cargo project with minimal dependencies and a size-tuned release profile
- [x] Window with tabs: Clock, Timer, Reminders, Calendar
- [x] Live clock: local time and date, one repaint per second
- [x] `PLAN.md`, `CLAUDE.md` (AI git workflow), `README.md`

### Phase 1: Clock

- [x] 12/24-hour format and show/hide seconds
- [x] Remember these settings between runs (D1)
- [x] App icon

### Phase 2: Timer

- [x] Countdown: set hours, minutes, seconds; start, pause, resume, reset
- [x] Count with `std::time::Instant` (monotonic), so changing the system clock doesn't affect a running timer
- [x] Repaint only while a timer is running
- [x] Alert when it reaches zero (D2)
- [x] Unit tests for the timer state machine
- [ ] Maybe: stopwatch with laps (only if wanted)

### Phase 3: Reminders

- [x] Create, edit, delete: title, date, time, optional note
- [x] Optional repeat: daily or weekly
- [x] Fire on time, even with the window minimized (D3)
- [ ] Alert with snooze and dismiss (D2 — dismiss works; snooze is still missing)
- [x] Saved to disk (D1); reminders missed while the app was closed show on next start
- [x] Unit tests for "which reminder is due next" and the repeat rules

### Phase 4: Calendar

- [x] Week strip with today marked (replaced the month grid in the redesign)
- [x] Previous/next week and "today" buttons
- [x] Days with reminders are marked; clicking a day lists its reminders and can add one
- [x] Unit tests for the week and the day view's hour range (weeks across months and years, leap day)

### Phase 5: Polish (optional)

- [x] Keep running in the tray when the window closes (D4)
- [x] Pop-out desktop clock with move/resize edit mode, lock and right-click unlock (D6)
- [x] Optional start with Windows, in the Clock tab, the tray menu and the installer (D7)
- [x] Single instance: a second launch shows the running app (D7)
- [x] Installer (D7)
- [ ] Light/dark theme setting
- [ ] Size trim: drop the emoji fonts (0.74 MB) if the UI doesn't use emoji
- [x] Link the C runtime statically (`-C target-feature=+crt-static`, in `.cargo/config.toml`) so the `.exe` runs without the VC++ runtime installed
- [ ] Release: a zip with the `.exe` on GitHub Releases

## Open decisions

Settle each one at the start of the phase that needs it, then write the answer here. Check a crate's size and dependencies before adding it.

- **D1 Storage (Phase 1 and 3) — settled:** `serde` + `serde_json`, one small file per thing in `%APPDATA%\clock-app\`: `settings.json`, `reminders.json` and `popout.json`. Pretty-printed JSON so both are easy to read and edit by hand; unknown fields are ignored and missing fields fall back to their defaults. Writes go to a temp file that is renamed over the old one, so a crash can't leave a half-written file. eframe's `persistence` feature stays off: it would also save egui's own state, which we don't need.
- **D2 Alerts (Phase 2 and 3) — settled for now:** in-app only. A red "Time's up!" / "Reminder due" banner plus `ViewportCommand::RequestUserAttention(Critical)` (taskbar flash). No sound and no native Windows notification: both would add weight (a crate or Win32 plumbing) for an app this small. Can be revisited if the flash proves too easy to miss.
- **D3 Firing while minimized (Phase 3) — settled:** all due-time checks live in `App::logic`, which eframe 0.37 calls while the window is hidden when a repaint was requested. `logic` calls `request_repaint_after(time until next reminder)`, so the app sleeps until the next due time and wakes up without polling or a background thread. The code is in place; please confirm on Windows that the taskbar flash still happens with the window minimized, since that OS behaviour can't be checked in this environment.
- **D4 Running in the background (Phase 5) — settled:** closing the window hides it into a system tray icon instead of quitting; left-click the icon to restore, right-click for Restore and Quit. Uses `tray-icon` with default features off. eframe's own repaint requests wake the app while hidden, so reminders keep firing without polling.
- **D7 Start with Windows, single instance and installer (Phase 5) — settled:** "Start with Windows" is a `Clock` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` holding `"<exe>" --tray`; the registry is the only source of truth, so the Clock tab checkbox, the tray menu's check item and the installer always agree. With `--tray` the window is hidden right after eframe's forced first show, so a login start goes straight to the tray (only when the tray icon exists). A named mutex (`friends-clock-app`) keeps one Clock running; a second launch signals a named event and exits, and the running app shows its window. Both use `windows-sys` 0.61, which eframe and tray-icon already build, so no crate was added. The installer is Inno Setup 6 (`installer/clock.iss`): per-user, no admin rights, installs to `%LOCALAPPDATA%\Programs\Clock` with a Start menu shortcut, an optional desktop shortcut and a "Start with Windows" task (on by default). It refuses to install or uninstall while Clock runs (`AppMutex`), removes the Run value on uninstall, and keeps the user's data in `%APPDATA%\clock-app`. The `.exe` has no embedded icon resource yet, so shortcuts show the default program icon.
- **D5 Calendar scope (Phase 4) — settled:** local reminders only. No `.ics` import/export for now: the calendar reads the same `reminders.json` as the Reminders tab, and clicking a day can add one there.
- **D6 Pop-out clock (Phase 5) — settled:** the Clock tab opens a frameless, transparent child viewport that stays behind other apps and has no taskbar icon or Windows drop shadow/border. Only the time and date text is drawn: no panel background, no visible buttons, no focus outline and no selectable text. It starts locked; right-clicking the text opens a menu with Unlock (or Lock) and Close. Unlocking takes effect on the very next drag: press the text to move it and drag the invisible bottom-right corner to resize. Position, size, open and locked state are saved in `popout.json`; clock formatting still comes from `settings.json`. The parent keeps the initial geometry fixed; dragging and resizing use native window commands, and the final geometry is read back from `ViewportInfo`. Because Windows only applies AlwaysOnBottom on a level change, the child reasserts it whenever it is idle. The menu also has a Resize toggle: corner handles around the text scale the clock and date (0.5x–3x) and refit the window; the scale and mode are saved in `popout.json`.

## Out of scope for now

- Online calendar sync (Google, Outlook), accounts, cloud sync
- World clocks and time zones (they need a time zone database, which adds size)
- Mobile and web versions
- Auto-update and telemetry
