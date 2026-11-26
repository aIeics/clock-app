# CLAUDE.md

Rules for AI assistants working in this repo. `PLAN.md` has the roadmap, the phases and the settled decisions; its installer parts describe scripts that are no longer in the repo.

The repo is public: never commit installer scripts, secrets, personal data or files you haven't looked at. Commits use the GitHub noreply email set in the repo's git config.

## Git workflow (required for every task)

1. **Never work on `main` directly.** Start each task on a new branch from an up-to-date `main`:

   ```sh
   git switch main
   git pull --ff-only
   git switch -c <type>/<short-name>   # e.g. feat/timer, fix/clock-format, docs/plan
   ```

   Types: `feat`, `fix`, `docs`, `refactor`, `chore`. One task = one branch.

2. **Commit every edit.** After each logical change (one file, or a few files that belong together), commit it with a clear message. Never end a turn with uncommitted changes. Commits that touch Rust code must build (`cargo build`) and be formatted (`cargo fmt`).

3. **Push after every commit:** `git push -u origin <branch>` the first time, then `git push`.

4. **When the task is done:** run the checks below, make sure everything is committed and pushed, then **ask the user: "Merge `<branch>` into main and delete the branch?"** Do not merge or delete anything without a clear yes.

5. **On yes, merge and delete the leftovers:**

   ```sh
   git switch main
   git pull --ff-only
   git merge --ff-only <branch>
   git push origin main
   git branch -d <branch>
   git push origin --delete <branch>
   git fetch --prune
   ```

   - "Leftovers" means the merged branch (local and on GitHub) and stale remote-tracking refs. Nothing else gets deleted (no other branches, no `cargo clean`) unless the user asks.
   - Use fast-forward merges only. No squash merges: `git branch -d` refuses to delete a squashed branch.
   - If `--ff-only` fails because `main` moved on, stop and ask the user whether to rebase the branch onto `main`.
   - If pushing to `main` is rejected (branch protection), open a pull request with `gh pr create` instead and ask the user to merge it on GitHub.

## Commands

| What | Command |
| --- | --- |
| Run (debug, console visible) | `cargo run` |
| Release build | `cargo build --release` → `target/release/clock-app.exe` |
| Format | `cargo fmt` |
| Checks before asking to merge | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` |
| Registry test (touches the real Run key, restores it) | `cargo test -- --ignored` |

On Windows, close the running app before rebuilding, or the `.exe` stays locked.

## Releases

Publishing a GitHub Release is public: ask the user first. Bump `version` in `Cargo.toml` and add a `CHANGELOG.md` entry. The Inno Setup installer script was removed from the repo on purpose, so ask the user how to build the installer for a new version. The installer and the app share the Run value name `Clock` and the mutex name `friends-clock-app`; don't change them.

## Project rules

- **Lightweight first.** Ask the user before adding a dependency. Use `default-features = false` and enable only the features needed. No async runtimes, web views, telemetry or network access.
- **Only build what was asked.** Work on the phase or task the user names; don't start later phases.
- **One module per feature** (`clock`, `timer`, `reminders`, `calendar`). Each owns its state and has a `ui(&mut self, ui: &mut egui::Ui)` method. Keep time math, scheduling and storage out of the `ui` code so they can be unit tested. When a module grows, turn it into a folder (`reminders/mod.rs`, `reminders/store.rs`, ...).
- **Idle means idle.** Never repaint continuously. Request a repaint only for the moment something visible changes (see `clock.rs`).
- Release builds have no console window; use `cargo run` to see panics and logs.
