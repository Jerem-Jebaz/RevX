# Architecture

NoUSB is split into a platform-neutral planner and platform-specific adapters.

The planner creates an `OperationPlan` containing steps, risk, confirmation requirements, warnings, expected modifications, and rollback notes. The default build only previews plans. Executors for real boot changes must live behind platform feature flags and must consume approved plans rather than constructing ad hoc actions.

Each `OperationPlan` includes a typed `BootPlan`. The safe default prefers one-time boot, keeps the current OS as the default target, disallows permanent boot-order changes, and records fallback/manual recovery expectations.

The current CLI exposes both human-readable previews and dependency-free JSON output. The default safe executor returns an explicit refusal for execution and rejects `--yes` for plans that require typed confirmation.

`FakePlatformAdapter` provides deterministic Windows, Linux, and macOS inventories for preflight planning tests. `plan_operation_for_inventory` enriches plan previews with encryption, Secure Boot, low-space, and platform-policy warnings without performing any host changes.

`SimulatedExecutor` implements the executor contract without host mutation. It can complete a plan, interrupt after N completed steps, fail a named step, and produce a simulated rollback report. This gives the journal/resume design something concrete to test before real adapters exist.

Execution reports are currently persisted as JSON files in `.nousb/reports/` and indexed through CLI report commands. This is an intentional stepping stone toward the prompt's full SQLite operation/step/artifact/backup/warning/confirmation/log schema.

Diagnostic bundles are currently directory-based exports under `.nousb/diagnostics/`. They collect project-local reports, journal logs, catalog metadata, and safe host inventory, then apply conservative line-based redaction to secret-like fields before writing bundle files.

Core crates:

- `nousb-core`: typed model, catalog, risk scoring, SQLite-backed journal, recovery pack.
- `nousb-cli`: command-line interface.
- `nousb-platform-windows`: Windows detection and future BCD/UEFI wrappers.
- `nousb-platform-linux`: Linux detection and future GRUB/systemd-boot/efibootmgr wrappers.
- `nousb-platform-macos`: macOS detection and future Apple-supported wrappers.
- `nousb-installer-profiles`: versioned TOML profile files and profile catalog facade.

Desktop source lives in `apps/desktop` and is designed to display the same plan previews as the CLI. `apps/desktop/preview.html` is a dependency-free dashboard preview that mirrors the current safe CLI workflows while the full Tauri command bridge is still pending.

`nousb dashboard export` emits a consolidated JSON view for the desktop surface. `apps/desktop/preview-data.json` is a portable checked-in fixture for the same dashboard shape.
