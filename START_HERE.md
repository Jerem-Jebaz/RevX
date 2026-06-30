# NoUSB OS Switcher Start Here

Use this file when starting a new Codex chat or returning after a context reset.

## Resume Order

1. Read `START_HERE.md`.
2. Read `TODO.md` for the current usable version, next best steps, and backlog.
3. Read `docs/HANDOFF.md` for the latest working state and first commands to run.
4. Read `AGENTS.md` for safety and coding rules.
5. Skim `README.md` for the user-facing CLI surface.
6. Read `docs/MASTER_PROMPT.md` only when you need the original full product prompt.

## Current Rule

Keep default builds non-destructive. Do not add real boot, disk, partition, encryption, EFI, BCD, GRUB, systemd-boot, NVRAM, `bless`, or macOS installer changes unless they are feature-gated, planned first, confirmation-gated, journaled, and tested through fake adapters.

## First Commands

```sh
cargo test --workspace
cargo run -p nousb -- doctor
cargo run -p nousb -- catalog validate
cargo run -p nousb -- --json plan boot-once ubuntu-24.04-desktop
cargo run -p nousb -- dashboard export
```

## Continue From

Use the `Next Best Steps` section in `TODO.md` as the active project queue.

Current highest-value next tasks:

1. Replace hand-written JSON and CLI parsing with `serde` and `clap` when dependency access is available.
2. Replace the SQLite shell-out with a Rust SQLite crate such as `rusqlite`.
3. Move persisted report storage into the SQLite journal schema.
4. Add resume/recover commands that consume persisted reports.
5. Wire the React/Tauri app to load `dashboard export` data through a Tauri command.

## Fresh Chat Prompt

Paste this into a new chat:

```text
We are continuing the NoUSB OS Switcher project in /home/jebarson/Documents/RevX.
Read START_HERE.md, TODO.md, docs/HANDOFF.md, AGENTS.md, README.md, and docs/MASTER_PROMPT.md first.
Keep default builds non-destructive.
Run cargo test --workspace before finalizing changes.
Continue from the "Next Best Steps" section in TODO.md.
```
