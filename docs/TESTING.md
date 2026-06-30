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

Future integration tests should use fake adapters for Windows UEFI, BitLocker, Secure Boot, Linux GRUB/systemd-boot, LUKS, Btrfs, macOS Intel, Apple Silicon, low-space systems, corrupt ISOs, checksum mismatch, interrupted operation, and rollback after partial failure.

No CI test may modify real boot configuration.
