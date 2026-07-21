# Project Handoff

This file is for quickly resuming work in another chat.

## Where We Are

NoUSB OS Switcher is currently a safe planning and verified-staging scaffold. It does not modify boot configuration or disks. The CLI can inspect the host, list catalog entries, preview plans, emit JSON, acquire pinned Ubuntu/Fedora artifacts, verify size and SHA-256, extract a non-executable staging bundle, create a recovery pack, and export the journal location.

`nousb tui` is now the usable guided terminal application for Windows and Linux. It exposes the supported inspect/assess/manifest/probe/download/stage/plan flow and uses typed phrases before downloads and staging. It contains no install or boot-execution action.

`vm rehearse` now exercises one complete one-shot boot cycle and interruption after every transition for UEFI BootNext, Windows BCD bootsequence, systemd-boot one-shot, and GRUB next-entry models. The original default/order, selector cleanup, temporary asset cleanup, and fallback invariants pass. This is a state-machine proof, not a real VM boot; see `docs/VM_ONE_SHOT.md`.

The current Windows host also has a project-local QEMU 10.1.5/OVMF runtime assembled from Alpine v3.23 signed packages through the existing `docker-desktop` WSL2 distribution. Typed disposable runs now cover firmware-only startup, direct netboot kernel/initrd execution, a full Ubuntu Server ISO copied into initramfs, and the combined UEFI-to-full-RAM path. In the combined run, OVMF launched the pinned official EFI assets, Linux reported EDK II UEFI, casper downloaded and mounted the verified 3.2 GiB Ubuntu 24.04.4 Server ISO in RAM, QMP removed the temporary FAT boot device and backing node, the source server stopped, and fresh Ubuntu/Subiquity/TTY evidence appeared afterward. The final QMP block list contained only the two OVMF pflash devices. Neither the run nor its code touched host boot state or a physical disk. Secure Boot, guest-side HTTPS, a disposable current-OS fallback boot, and the power-loss matrix remain unproven gates.

Operation plans now include typed `BootPlan` data. The default plan keeps the current OS as the default boot target and disallows permanent boot-order changes.

Installer profiles are now versioned TOML files in `crates/nousb-installer-profiles/profiles`, parsed and validated by `nousb-core`.

Fake platform scenarios are available through `FakePlatformAdapter` and the CLI:

- `windows-bitlocker`
- `linux-luks-low-space`

The macOS fixture remains in core exclusion tests but is hidden from the current Windows/Linux user-facing objective.

Execution simulation is available through `simulate` and `simulate-fake`, with `--interrupt-after`, `--fail-step`, and `--rollback`.

Simulation reports are persisted under `.nousb/reports/`. Use `nousb reports list` and `nousb reports show <plan-id>` to resume inspection in another chat.

Diagnostic bundles are exported with `nousb diagnostics export` and saved under `.nousb/diagnostics/`. They include redacted local logs, saved reports, catalog metadata, and safe host inventory.

The original master prompt is saved at `docs/MASTER_PROMPT.md`.

The main resume entry point is `START_HERE.md`. The detailed continuation list is in `TODO.md`.

The no-dependency desktop dashboard preview is `apps/desktop/preview.html`. It can be opened directly in a browser and mirrors the current safe CLI workflows.

No-USB delivery feasibility is now modeled by `workflow assess` and documented in `docs/BOOT_DELIVERY.md`. The current recommended design candidate for a suitable Windows UEFI host and Ubuntu profile is persistent one-shot staging of signed kernel/initrd assets followed by Ubuntu's RAM-copy live boot. Execution remains disabled.

The CLI now selects platform-isolated read-only adapters on Windows and Linux. Windows inspection queries UEFI/Secure Boot, RAM, free system-drive space, and BitLocker status where available. Linux inspection reads memory, Secure Boot efivars, root free space, crypttab, and GRUB/systemd-boot presence. Unknowns remain unknown.

Portable preview data lives at `apps/desktop/preview-data.json`; live data can be generated with `cargo run -p nousb -- dashboard export`.

Ubuntu 24.04.4 and Fedora 44 now have direct official artifact manifests with exact size and SHA-256. `image manifest`, `image probe`, `image download`, and `image verify-profile` expose the acquisition workflow. Windows 11 remains fail-closed because its consumer ISO link is generated per session.

`staging inspect` verifies an ISO and previews its declared kernel/initrd/EFI assets. `staging create` extracts only those paths, hashes each output, writes `bundle.json`, and leaves `execution_enabled=false`. No boot adapter consumes the bundle.

## First Commands To Run

```sh
cargo test --workspace
cargo run -p nousb -- doctor
cargo run -p nousb -- tui
cargo run -p nousb -- catalog list
cargo run -p nousb -- catalog validate
cargo run -p nousb -- --json image manifest ubuntu-24.04-desktop
cargo run -p nousb -- --json image manifest fedora-44-workstation
cargo run -p nousb -- vm doctor
cargo run -p nousb -- vm rehearse windows-bcd-bootsequence ubuntu-24.04-desktop
cargo run -p nousb -- vm qemu plan-smoke
cargo run -p nousb -- vm qemu plan-ubuntu-netboot
cargo run -p nousb -- vm qemu plan-ubuntu-full-ram
cargo run -p nousb -- vm qemu plan-ubuntu-uefi-full-ram
cargo run -p nousb -- workflow assess-fake windows-bitlocker ubuntu-24.04-desktop
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

Add a disposable current-OS guest disk and prove one-shot selection, fallback to that OS, cleanup, and interruption/power-loss recovery around the already-proven combined UEFI full-RAM handoff. Then add Secure Boot validation and guest-side HTTPS or another authenticated transport. Do not implement host boot changes until that recovery matrix passes.
