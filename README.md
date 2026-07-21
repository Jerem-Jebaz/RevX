# NoUSB OS Switcher

NoUSB OS Switcher is a safe-first desktop and CLI scaffold for staging official OS installers without a USB stick. This version is intentionally non-destructive: it can inspect the host, list installer profiles, create dry-run operation plans, acquire pinned Ubuntu and Fedora images over HTTPS, verify exact size and SHA-256, extract declared boot assets into a non-executable staging bundle, write an operation journal, and generate a metadata recovery pack.

It does not modify boot configuration, partition tables, encryption settings, EFI files, BCD, GRUB, systemd-boot, or macOS boot policy in the default build.

## Quick Start

```sh
cargo run -p nousb -- doctor
cargo run -p nousb -- tui
cargo run -p nousb -- system inspect
cargo run -p nousb -- catalog list
cargo run -p nousb -- catalog validate
cargo run -p nousb -- workflow assess ubuntu-24.04-desktop
cargo run -p nousb -- workflow assess-fake windows-bitlocker ubuntu-24.04-desktop
cargo run -p nousb -- --json image manifest ubuntu-24.04-desktop
cargo run -p nousb -- image probe ubuntu-24.04-desktop
cargo run -p nousb -- vm doctor
cargo run -p nousb -- vm rehearse windows-bcd-bootsequence ubuntu-24.04-desktop
cargo run -p nousb -- vm qemu plan-smoke
cargo run -p nousb -- vm qemu plan-ubuntu-netboot
cargo run -p nousb -- vm qemu plan-ubuntu-full-ram
cargo run -p nousb -- vm qemu plan-ubuntu-uefi-full-ram
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

Run `cargo run -p nousb -- tui` for the cross-platform guided terminal application. It provides host summary, profile selection, no-USB feasibility assessment, official artifact manifests and probes, typed download/staging gates, boot-plan previews, and read-only display of disposable VM plans/evidence. It deliberately exposes no host install or boot executor.

Open `apps/desktop/preview.html` in a browser for a dependency-free dashboard preview. It mirrors the current safe CLI/TUI surface: catalog validation, fake scenarios, plan previews, persisted reports, and diagnostic bundles.

## Project Layout

- `crates/nousb-core`: operation model, planner, risk gates, catalog, journal, recovery pack.
- `crates/nousb-cli`: working CLI.
- `crates/nousb-platform-*`: read-only Windows/Linux inventory adapters plus the excluded macOS scaffold.
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
nousb tui
nousb system inspect
nousb system inspect-fake <scenario>
nousb catalog list
nousb catalog validate
nousb workflow assess <profile>
nousb workflow assess-fake <scenario> <profile>
nousb image manifest <profile>
nousb image probe <profile>
nousb image download <profile>
nousb image verify-profile <profile> <path>
nousb image verify <path> <sha256>
nousb staging inspect <profile> <path>
nousb staging create <profile> <path>
nousb vm doctor
nousb vm rehearse <backend> <profile>
nousb vm qemu manifest
nousb vm qemu acquire
nousb vm qemu plan-smoke
nousb vm qemu smoke "RUN DISPOSABLE QEMU UEFI SMOKE"
nousb vm qemu plan-ubuntu-netboot
nousb vm qemu rehearse-ubuntu-netboot "RUN DISPOSABLE UBUNTU NETBOOT RAM REHEARSAL"
nousb vm qemu plan-ubuntu-full-ram
nousb vm qemu rehearse-ubuntu-full-ram "RUN DISPOSABLE UBUNTU FULL ISO RAM REHEARSAL"
nousb vm qemu plan-ubuntu-uefi-full-ram
nousb vm qemu rehearse-ubuntu-uefi-full-ram "RUN DISPOSABLE UBUNTU UEFI FULL ISO RAM REHEARSAL"
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

`execute` remains intentionally blocked. Image acquisition is enabled only for profiles with a stable official HTTPS URL, pinned byte size, and pinned SHA-256. Ubuntu 24.04.4 and Fedora 44 meet that rule. Windows 11 remains vendor-mediated because Microsoft generates time-limited consumer ISO links; NoUSB refuses to invent or cache an unpinned URL.

The TUI adds a separate typed phrase before starting a multi-gigabyte download (`DOWNLOAD <profile>`) and before creating a verified bundle (`STAGE <profile>`). Neither phrase enables boot execution.

Press `v` in the TUI, or use `vm rehearse`, to run the one-shot boot/fallback state-machine matrix. It tests UEFI BootNext, Windows BCD bootsequence, systemd-boot one-shot, or GRUB next-entry semantics without starting a VM or touching host boot state. Press `r` to display the separate real disposable-VM plans and saved evidence.

On the current Windows host, typed QEMU commands now prove the combined UEFI and full-installer RAM path. OVMF selected a read-only FAT source built from SHA-256-verified Ubuntu 24.04.4 Server EFI/kernel/initrd assets; QMP then deleted that USB/vvfat source. Casper downloaded the exact 3,405,469,696-byte pinned ISO into guest RAM, the owned HTTP source was stopped, and Ubuntu, the Subiquity payload, and the installer tty continued afterward. Final QMP storage contained only immutable OVMF code and a disposable variables copy. The run used no physical or virtual install disk and made no host boot change. Secure Boot, guest-side HTTPS, real current-OS fallback, and interruption recovery remain unproven; see `docs/VM_ONE_SHOT.md`.

Downloads are stored under `.nousb/artifacts/<profile>/`. A mismatch never replaces the final artifact, and resumable `.part` data is retained for inspection. `staging create` first re-verifies the entire ISO, then extracts only profile-declared assets under `.nousb/staging/`; the resulting bundle records `execution_enabled=false` and cannot select a boot entry.

`workflow assess` compares the host and installer profile against host-staged RAM boot, GRUB loopback, Windows Setup/WinPE, UEFI HTTP Boot, and vendor-firmware recovery routes. It is a feasibility report, not an executor. See `docs/BOOT_DELIVERY.md` for the design boundary and primary references.

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
```

macOS code remains an excluded compatibility scaffold and is not part of the current Windows/Linux objective.
