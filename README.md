# NoUSB OS Switcher

NoUSB OS Switcher is a safe-first desktop and CLI scaffold for staging official OS installers without a USB stick. This version is intentionally non-destructive: it can inspect the host, list installer profiles, create dry-run operation plans, verify local images by SHA256, write a SQLite-backed operation journal, and generate a metadata recovery pack.

It does not modify boot configuration, partition tables, encryption settings, EFI files, BCD, GRUB, systemd-boot, or macOS boot policy in the default build.

## Quick Start

```sh
cargo run -p nousb -- doctor
cargo run -p nousb -- system inspect
cargo run -p nousb -- catalog list
cargo run -p nousb -- catalog validate
cargo run -p nousb -- plan boot-once ubuntu-24.04-desktop
cargo run -p nousb -- system inspect-fake windows-bitlocker
cargo run -p nousb -- plan-fake linux-luks-low-space boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --rollback --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop
cargo run -p nousb -- reports list
cargo run -p nousb -- diagnostics export
cargo run -p nousb -- dashboard export
cargo run -p nousb -- --json plan boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --dry-run plan replace-os ubuntu-24.04-desktop
cargo run -p nousb -- recovery-pack create
```

## Resuming Work

Start with `START_HERE.md`, then `TODO.md` and `docs/HANDOFF.md`. They capture the current project state, verified commands, next steps, and a fresh-chat prompt.

## Desktop Preview

Open `apps/desktop/preview.html` in a browser for a dependency-free dashboard preview. It mirrors the current safe CLI surface: catalog validation, fake scenarios, plan previews, persisted reports, and diagnostic bundles.

## Project Layout

- `crates/nousb-core`: operation model, planner, risk gates, catalog, journal, recovery pack.
- `crates/nousb-cli`: working CLI.
- `crates/nousb-platform-*`: safe adapter skeletons for Windows, Linux, and macOS.
- `crates/nousb-installer-profiles`: versioned TOML installer profiles and catalog facade.
- `apps/desktop`: Tauri/React source scaffold plus dependency-free dashboard preview.
- `docs`: architecture, safety, testing, recovery, and the original master prompt.

## Safety Status

The current build is a usable planning and audit tool. Real boot changes are reserved for future feature-gated adapters:

- `real-windows-boot`
- `real-linux-boot`
- `real-macos-boot`
- `destructive-tests`
- `technician-mode`

## Current CLI

```text
nousb system inspect
nousb system inspect-fake <scenario>
nousb catalog list
nousb catalog validate
nousb image download <profile>
nousb image verify <path> <sha256>
nousb plan boot-once <profile>
nousb plan dual-boot <profile>
nousb plan replace-os <profile>
nousb plan reinstall-current-os <profile>
nousb plan-fake <scenario> <plan-kind> <profile>
nousb simulate <plan-kind> <profile>
nousb simulate-fake <scenario> <plan-kind> <profile>
nousb reports list
nousb reports show <plan-id>
nousb diagnostics export
nousb dashboard export
nousb execute <plan-kind> <profile>
nousb rollback <operation-id>
nousb recovery-pack create
nousb logs export
nousb doctor
```

`execute` and real image downloads are intentionally blocked in this offline-safe scaffold.

Global options:

```text
--json       Print machine-readable JSON where supported
--dry-run    Mark plan output as a dry run
--yes        Only allowed for low-risk actions; never bypasses typed confirmation
--interrupt-after <count>
--fail-step <step-id>
--rollback   Roll back the simulated report
```

In the default build, `nousb execute <plan-kind> <profile>` creates the requested plan and then refuses execution through the safe executor gate. Critical plans reject `--yes` before any execution path is considered.

Simulation reports are saved under `.nousb/reports/` and can be listed or shown later with `nousb reports list` and `nousb reports show <plan-id>`.

Diagnostic bundles are saved under `.nousb/diagnostics/` and include redacted project-local logs, saved reports, catalog metadata, and safe host inventory.

Dashboard data can be exported with `nousb dashboard export`; a portable preview fixture is checked in at `apps/desktop/preview-data.json`.

Fake scenarios:

```text
windows-bitlocker
linux-luks-low-space
macos-apple-silicon
```
