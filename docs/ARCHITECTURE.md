# Architecture

NoUSB is split into a platform-neutral planner and platform-specific adapters.

The planner creates an `OperationPlan` containing steps, risk, confirmation requirements, warnings, expected modifications, and rollback notes. The default build only previews plans. Executors for real boot changes must live behind platform feature flags and must consume approved plans rather than constructing ad hoc actions.

Each `OperationPlan` includes a typed `BootPlan`. The safe default prefers one-time boot, keeps the current OS as the default target, disallows permanent boot-order changes, and records fallback/manual recovery expectations.

`DeliveryAssessment` is a separate pre-planning layer. It compares a host inventory and installer profile against host-staged RAM boot, GRUB loopback, Windows Setup/WinPE, UEFI HTTP Boot, and vendor-firmware recovery. It records payload residency, persistent-staging needs, capability blockers, limitations, and primary references. A design candidate never implies that default-build execution is enabled.

Artifact acquisition is a separate verified boundary. `ArtifactManifest` is constructed only from pinned profile metadata. The blocking HTTPS client allows limited redirects but rejects a final non-HTTPS URL. Downloads use a profile-local lock, retain resumable partial data, require the exact byte count and SHA-256, and never overwrite a conflicting final file.

`StagingPreview` and `StagingBundle` form the next boundary. The source ISO is verified again before ISO9660 inspection. Only profile-declared paths may be extracted; traversal, Windows separators, drive prefixes, and overwrite are rejected. Each extracted asset is hashed, and every bundle is explicitly non-executable. No platform boot adapter consumes these bundles yet.

The current CLI exposes both human-readable previews and dependency-free JSON output. The default safe executor returns an explicit refusal for execution and rejects `--yes` for plans that require typed confirmation.

`nousb tui` is the current cross-platform application surface for Windows and Linux. It reuses the same core inventory, assessment, manifest, downloader, staging, and planner functions rather than duplicating execution logic. Large downloads require `DOWNLOAD <profile>` and bundle creation requires `STAGE <profile>`. Its `r` view reads disposable QEMU plans and evidence, but the TUI has no action that invokes a host installer or boot adapter.

`VmRehearsalReport` models the transaction boundary between a verified staging bundle and a future platform adapter. It runs a complete one-shot cycle plus interruption after each transition for UEFI BootNext, Windows BCD bootsequence, systemd-boot LoaderEntryOneShot, and GRUB next-entry semantics. Reports prove state-machine invariants only and always record that no real VM or host boot change occurred.

The Windows platform crate also contains a narrowly scoped WSL2 QEMU harness. It launches only project-local QEMU/OVMF, requires exact typed phrases, builds a preview before each run, and never attaches a physical or virtual install disk. Earlier reports separately prove OVMF startup and direct kernel/initrd execution. New reports prove a pinned full Ubuntu Server ISO can be downloaded into guest RAM, continue after the HTTP source stops, and—under OVMF—continue after a read-only FAT boot source is removed with QMP `device_del` plus `blockdev-del`. The final combined block list contains only OVMF code and a disposable variables copy. Networking is enabled only for this isolated recovery transfer; Secure Boot, guest HTTPS, fallback, and host boot execution remain false.

`FakePlatformAdapter` provides deterministic inventories for preflight planning tests. The current user-facing objective and dashboard expose Windows and Linux scenarios; macOS remains explicitly out of scope. `plan_operation_for_inventory` enriches plan previews with encryption, Secure Boot, low-space, and platform-policy warnings without performing any host changes.

`SimulatedExecutor` implements the executor contract without host mutation. It can complete a plan, interrupt after N completed steps, fail a named step, and produce a simulated rollback report. This gives the journal/resume design something concrete to test before real adapters exist.

Execution reports are currently persisted as JSON files in `.nousb/reports/` and indexed through CLI report commands. This is an intentional stepping stone toward the prompt's full SQLite operation/step/artifact/backup/warning/confirmation/log schema.

Diagnostic bundles are currently directory-based exports under `.nousb/diagnostics/`. They collect project-local reports, journal logs, catalog metadata, and safe host inventory, then apply conservative line-based redaction to secret-like fields before writing bundle files.

Core crates:

- `nousb-core`: typed model, catalog, risk scoring, SQLite-backed journal, recovery pack.
- `nousb-cli`: command-line interface.
- `nousb-platform-windows`: Windows detection, isolated WSL2/QEMU evidence harness, and future BCD/UEFI wrappers.
- `nousb-platform-linux`: Linux detection and future GRUB/systemd-boot/efibootmgr wrappers.
- `nousb-platform-macos`: macOS detection and future Apple-supported wrappers.
- `nousb-installer-profiles`: versioned TOML profile files and profile catalog facade.

Desktop source lives in `apps/desktop` and is designed to display the same plan previews as the CLI. `apps/desktop/preview.html` is a dependency-free dashboard preview that mirrors the current safe CLI workflows while the full Tauri command bridge is still pending.

`nousb dashboard export` emits a consolidated JSON view for the desktop surface, including any project-local disposable VM reports. `apps/desktop/preview-data.json` is a portable checked-in fixture for the same dashboard shape and intentionally contains no host-specific reports.
