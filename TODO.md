# NoUSB OS Switcher To-Do

Use `START_HERE.md` first, then this file as the detailed project queue when starting a new Codex chat.

## Current Usable Version

The repository now contains a safe, non-destructive scaffold for NoUSB OS Switcher.

Working pieces:

- Rust workspace with core, CLI, platform skeleton, and profile crates.
- CLI commands for doctor, inspect, catalog, plan preview, checksum verify, recovery-pack creation, and log export.
- CLI global options for `--json`, `--dry-run`, and guarded `--yes`.
- Risk model and typed confirmation requirements.
- Typed `BootPlan` in every operation plan, preferring one-time boot/fallback behavior and disallowing permanent boot-order changes by default.
- Explicit safe executor gate that refuses default-build execution and blocks `--yes` for high/critical plans.
- Built-in installer profiles for Ubuntu, Fedora, and Windows 11.
- Versioned TOML installer profiles under `crates/nousb-installer-profiles/profiles`.
- Profile parser and validation for schema version, supported platforms, staging strategies, and free-space requirements.
- Safe host inspection adapter.
- Fake platform adapters for Windows UEFI + BitLocker, Linux GRUB/LUKS/Btrfs low space, and macOS Apple Silicon + FileVault.
- Inventory-aware plan enrichment for encryption, Secure Boot, and low disk space preflight warnings.
- Simulated operation executor with complete, interrupted, failed, and rolled-back reports.
- Persisted execution reports under `.nousb/reports` with `reports list` and `reports show`.
- Diagnostic bundle export under `.nousb/diagnostics` with secret-like log redaction.
- SQLite-backed operation journal with plain-log fallback.
- Recovery pack generator.
- Tauri/React desktop source scaffold.
- Standalone dashboard preview at `apps/desktop/preview.html` covering catalog, planner, fake scenarios, reports, diagnostics, and copyable CLI commands.
- Dashboard data export via `nousb dashboard export` plus portable preview fixture at `apps/desktop/preview-data.json`.
- Docs and original prompt stored in `docs/MASTER_PROMPT.md`.
- GitHub Actions CI workflow for Rust format and tests.

Verified commands:

```sh
cargo fmt --all
cargo test --workspace
cargo run -p nousb -- doctor
cargo run -p nousb -- catalog validate
cargo run -p nousb -- system inspect-fake windows-bitlocker
cargo run -p nousb -- plan-fake linux-luks-low-space boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop
cargo run -p nousb -- reports list
cargo run -p nousb -- diagnostics export
cargo run -p nousb -- dashboard export
cargo run -p nousb -- plan boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --json plan boot-once ubuntu-24.04-desktop
cargo run -p nousb -- --dry-run plan replace-os ubuntu-24.04-desktop
```

## Safety Baseline

Default builds must stay non-destructive.

Do not add real boot, partition, encryption, BCD, GRUB, systemd-boot, EFI, NVRAM, `bless`, or `startosinstall` changes unless they are:

- Isolated behind the relevant feature flag.
- Planned before executed.
- Covered by confirmation gates.
- Logged to the operation journal.
- Tested with fake adapters first.

Current feature flags to preserve:

- `real-windows-boot`
- `real-linux-boot`
- `real-macos-boot`
- `destructive-tests`
- `technician-mode`

## Next Best Steps

1. Replace hand-written JSON and CLI option parsing with `serde` and `clap` when dependency access is available.
2. Replace the SQLite shell-out with a Rust SQLite crate such as `rusqlite`.
3. Move persisted report storage into the SQLite journal schema with operation/step tables.
4. Add resume/recover commands that consume persisted reports.
5. Expand the safe executor into a real feature-gated executor family with default-build refusal behavior.
6. Teach the React/Tauri app to load `dashboard export` data through a Tauri command.
7. Add package metadata and release signing plan.
8. Add VM test plan fixtures and no-real-boot CI checks.
9. Add profile catalog signing/provenance metadata and external catalog loading.
10. Add richer real host detection parsers behind safe platform adapters.

## Backlog

Core:

- Expand `BootPlan` with real adapter capability mapping once platform adapters mature.
- Add `ImageArtifact` and richer `VerificationResult`.
- Expand `PlatformCapability` coverage as real adapters mature.
- Move current JSON report persistence into operation, step, artifact, backup, warning, confirmation, and log tables.
- Add file locking to prevent concurrent operations.
- Add richer resume/recover flow on top of persisted reports.
- Expand diagnostic bundles with SBOM/license/audit outputs once those commands exist.

CLI:

- Use `clap` for command parsing.
- Replace hand-written JSON helpers with `serde`.
- Add machine-readable exit codes.
- Keep stdout/stderr separation.
- Add tests around CLI JSON and `--yes` blocking behavior.

Profiles:

- Keep catalog profiles in `crates/nousb-installer-profiles/profiles/*.toml`.
- Replace the small dependency-free profile parser with `toml`/`serde` when dependency access is available.
- Add signature metadata fields.
- Add official-source provenance notes.

Platform adapters:

- Windows: detect BitLocker, WinRE, Secure Boot, firmware mode, admin status, disk layout.
- Linux: detect GRUB, systemd-boot, Secure Boot, ESP, LUKS, Btrfs, free space.
- macOS: detect architecture, FileVault, APFS, supported installer tooling, Apple Silicon restrictions.

Desktop:

- Install and verify Vite/Tauri dependencies when network access is available.
- Replace the current static dashboard preview data with runtime calls once Tauri command wiring exists.
- Add recovery pack and logs screens to the React app.
- Use the same safety wording as the CLI.

Docs:

- Expand `docs/SAFETY_MODEL.md` with confirmation matrix.
- Expand `docs/THREAT_MODEL.md` with trust boundaries.
- Add VM test plan details.
- Add user guide screenshots after desktop UI is wired.

## Fresh Chat Start Prompt

Paste this into a new chat:

```text
We are continuing the NoUSB OS Switcher project in /home/jebarson/Documents/RevX.
Read START_HERE.md, TODO.md, docs/HANDOFF.md, AGENTS.md, README.md, and docs/MASTER_PROMPT.md first.
Keep default builds non-destructive.
Run cargo test --workspace before finalizing changes.
Continue from the "Next Best Steps" section in TODO.md.
```
