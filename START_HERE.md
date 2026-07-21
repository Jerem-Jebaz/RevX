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
cargo run -p nousb -- tui
cargo run -p nousb -- catalog validate
cargo run -p nousb -- --json image manifest ubuntu-24.04-desktop
cargo run -p nousb -- vm doctor
cargo run -p nousb -- vm rehearse uefi-bootnext ubuntu-24.04-desktop
cargo run -p nousb -- vm qemu plan-smoke
cargo run -p nousb -- vm qemu plan-ubuntu-netboot
cargo run -p nousb -- vm qemu plan-ubuntu-full-ram
cargo run -p nousb -- vm qemu plan-ubuntu-uefi-full-ram
cargo run -p nousb -- workflow assess-fake windows-bitlocker ubuntu-24.04-desktop
cargo run -p nousb -- --json plan boot-once ubuntu-24.04-desktop
cargo run -p nousb -- dashboard export
```

## Continue From

Use the `Next Best Steps` section in `TODO.md` as the active project queue.

Current highest-value next tasks:

1. Add a disposable current-OS guest disk and prove a real one-shot return/fallback after the now-proven UEFI full-ISO RAM boot.
2. Inject interruptions through that disposable-guest path and prove the complete recovery matrix.
3. Add Secure Boot coverage and replace the isolated guest HTTP handoff with a pinned TLS-capable recovery transport.
4. Extend read-only inspection with ESP, boot-manager, and network-boot hints.
5. Add signature-chain verification in addition to pinned SHA-256.
6. Wire the guided TUI workflow and live evidence into the React/Tauri command bridge.

## Fresh Chat Prompt

Paste this into a new chat:

```text
We are continuing the NoUSB OS Switcher project in /home/jebarson/Documents/RevX.
Read START_HERE.md, TODO.md, docs/HANDOFF.md, AGENTS.md, README.md, and docs/MASTER_PROMPT.md first.
Keep default builds non-destructive.
Run cargo test --workspace before finalizing changes.
Continue from the "Next Best Steps" section in TODO.md.
```
