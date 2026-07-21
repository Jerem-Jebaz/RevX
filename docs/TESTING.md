# Testing

Run:

```sh
cargo test --workspace
```

Open `apps/desktop/preview.html` directly in a browser to inspect the static dashboard preview. It does not require a dev server.

Current unit coverage validates:

- Critical risk classification for replace-OS plans.
- BootPlan invariants for one-time boot and permanent boot-order safety.
- Typed confirmation gates.
- Unknown profile rejection.
- Built-in catalog sanity.
- Embedded TOML profile validation.
- Fake Windows BitLocker/Secure Boot preflight warnings.
- Fake Linux LUKS/Btrfs low-space manual intervention.
- Fake macOS Apple Silicon/FileVault policy warnings.
- CLI smoke tests for fake scenario planning and JSON output.
- Simulated executor completion, interruption, failure, and rollback behavior.
- CLI smoke tests for interrupted simulation and rollback JSON.
- Persistence tests for saving, listing, and showing execution reports.
- Diagnostic bundle export and secret-like log redaction tests.
- Delivery assessment for Windows-to-Ubuntu host-staged RAM boot.
- Explicit macOS delivery exclusion and disabled universal no-USB claims.
- Pinned Ubuntu 24.04.4 and Fedora 44 artifact manifests.
- Fail-closed handling of the vendor-mediated Windows 11 ISO.
- Cross-platform in-process SHA-256 verification and atomic partial-file promotion.
- Staging path traversal and platform-separator rejection.
- CLI manifest JSON and unavailable-manifest failure behavior.
- A deterministic synthetic ISO9660 image covering verification, multi-sector directory lookup, version/trailing-dot normalization, kernel/initrd extraction, per-asset hashes, and final non-executable bundle metadata.
- Guided TUI assessment/manifest/plan behavior and rejection of an incorrect typed download confirmation.
- A process-level scripted `nousb tui` smoke test using the real host inspection adapter.
- Nine-scenario VM rehearsal matrices for each of four one-shot backends, including interruption after every transaction transition.
- Backend-specific selector evidence for UEFI BootNext, BCD bootsequence, LoaderEntryOneShot, and GRUB next-entry.
- Project-local VM rehearsal report persistence and CLI/runtime-doctor smoke tests.
- Exact QEMU 11.0.0 Windows runtime manifest size/SHA-512 and project-local cache-path validation.
- Disposable QEMU/OVMF plan invariants: no network, physical disk, host boot change, or implicit installer attachment.
- Ubuntu netboot, direct full-RAM, and combined UEFI full-RAM rehearsal plans with truthful transport, firmware, disk, and host-mutation constraints.

Current-host manual integration checks, each protected by an exact typed phrase:

```sh
cargo run -p nousb -- vm qemu smoke "RUN DISPOSABLE QEMU UEFI SMOKE"
cargo run -p nousb -- vm qemu rehearse-ubuntu-netboot "RUN DISPOSABLE UBUNTU NETBOOT RAM REHEARSAL"
cargo run -p nousb -- vm qemu rehearse-ubuntu-full-ram "RUN DISPOSABLE UBUNTU FULL ISO RAM REHEARSAL"
cargo run -p nousb -- vm qemu rehearse-ubuntu-uefi-full-ram "RUN DISPOSABLE UBUNTU UEFI FULL ISO RAM REHEARSAL"
```

The first check records OVMF/QMP startup and a firmware-only block list. The second records the Ubuntu kernel banner, initramfs scripts, and an empty QMP block list. The third copies the pinned full Server ISO into initramfs, stops its source, and requires fresh installer evidence afterward. The fourth combines that full-RAM proof with OVMF, verified official EFI assets, QMP removal of the temporary boot source, and a final firmware-only block list. All are disposable and attach no physical disk or host boot state. The combined report deliberately keeps `secure_boot_enabled`, `guest_transport_https`, and `fallback_boot_observed` false.

Future integration tests should use fake adapters for Windows UEFI, BitLocker, Secure Boot, Linux GRUB/systemd-boot, LUKS, Btrfs, low-space systems, corrupt ISOs, checksum mismatch, interrupted downloads, real distribution ISO extraction checks, and rollback after partial failure. macOS remains out of scope for the current objective.

No CI test may modify real boot configuration.
