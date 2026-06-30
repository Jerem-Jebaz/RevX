# Project Handoff

This file is for quickly resuming work in another chat.

## Where We Are

NoUSB OS Switcher is currently a safe planning scaffold. It does not modify boot configuration or disks. The CLI can inspect the host, list catalog entries, preview plans, emit JSON for scripting, verify SHA256 checksums, create a recovery pack, and export the journal location.

Operation plans now include typed `BootPlan` data. The default plan keeps the current OS as the default boot target and disallows permanent boot-order changes.

Installer profiles are now versioned TOML files in `crates/nousb-installer-profiles/profiles`, parsed and validated by `nousb-core`.

Fake platform scenarios are available through `FakePlatformAdapter` and the CLI:

- `windows-bitlocker`
- `linux-luks-low-space`
- `macos-apple-silicon`

Execution simulation is available through `simulate` and `simulate-fake`, with `--interrupt-after`, `--fail-step`, and `--rollback`.

Simulation reports are persisted under `.nousb/reports/`. Use `nousb reports list` and `nousb reports show <plan-id>` to resume inspection in another chat.

Diagnostic bundles are exported with `nousb diagnostics export` and saved under `.nousb/diagnostics/`. They include redacted local logs, saved reports, catalog metadata, and safe host inventory.

The original master prompt is saved at `docs/MASTER_PROMPT.md`.

The main resume entry point is `START_HERE.md`. The detailed continuation list is in `TODO.md`.

The no-dependency desktop dashboard preview is `apps/desktop/preview.html`. It can be opened directly in a browser and mirrors the current safe CLI workflows.

Portable preview data lives at `apps/desktop/preview-data.json`; live data can be generated with `cargo run -p nousb -- dashboard export`.

## First Commands To Run

```sh
cargo test --workspace
cargo run -p nousb -- doctor
cargo run -p nousb -- catalog list
cargo run -p nousb -- catalog validate
cargo run -p nousb -- system inspect-fake windows-bitlocker
cargo run -p nousb -- plan-fake linux-luks-low-space boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop
cargo run -p nousb -- reports list
cargo run -p nousb -- diagnostics export
cargo run -p nousb -- dashboard export
cargo run -p nousb -- --json plan boot-once ubuntu-24.04-desktop
```

## Important Files

- `TODO.md`: project state, next steps, backlog, fresh-chat prompt.
- `START_HERE.md`: shortest resume path for a fresh chat.
- `AGENTS.md`: coding and safety rules.
- `README.md`: user-facing quick start.
- `crates/nousb-core/src/lib.rs`: current core model and planner.
- `crates/nousb-cli/src/main.rs`: current CLI.
- `apps/desktop/preview.html`: dependency-free dashboard preview.

## Recommended Next Task

Move CLI parsing/JSON to `clap` and `serde`, or replace the SQLite shell-out with a Rust SQLite dependency if network/dependency access is available. After that, migrate persisted report JSON into the SQLite journal schema and add richer resume/recover commands. For frontend work, call `dashboard export` through Tauri instead of relying on the portable fixture.
