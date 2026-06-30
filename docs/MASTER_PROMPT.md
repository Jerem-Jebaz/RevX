Yes. Use this as a **master Codex prompt**. I’d put this in an empty GitHub repo, then run it in Codex. OpenAI’s Codex docs say Codex can read, edit, and run code, and that repo-level `AGENTS.md` files are useful for project conventions, test commands, and coding standards. ([OpenAI Developers][1])

Below are two parts:

1. **Main Codex build prompt**
2. **Recommended `AGENTS.md` for the repo**

---

## 1. Main Codex prompt

Copy-paste this into Codex:

```text
You are building a production-grade cross-platform app called NoUSB OS Switcher.

Mission:
Build a desktop + CLI software system for Windows, Linux, and macOS that eliminates the need for a USB stick when switching, reinstalling, upgrading, dual-booting, or booting into another operating system. The app must safely stage official OS installers on the internal disk, verify them, prepare boot entries, set a one-time boot target, reboot into the installer/live environment, and provide robust rollback/fallback behavior.

This is not a toy MVP. Build it as an industry-grade system with fail-safe boot behavior, backup-first workflows, platform-specific safety checks, logs, tests, documentation, and a clear architecture that can grow to support many OS installers.

Core principle:
Never silently destroy, wipe, resize, repartition, encrypt, decrypt, overwrite, or replace anything. Every potentially destructive action must require preflight validation, a backup/recovery plan, a dry-run preview, and explicit user confirmation.

Important product definition:
The app does NOT magically change the OS while the current OS is running. It stages a verified installer/live environment, configures a safe temporary boot path, and reboots into that environment. The user may then install, upgrade, replace, or dual boot using the official installer flow.

Target users:
- People who want to switch from Windows to Linux without a USB.
- People who want to boot a Linux live installer from Windows/Linux/macOS without flashing an ISO.
- People who want to reinstall Windows from Windows without USB.
- People who want to stage macOS installers from macOS where Apple supports it.
- Advanced users who want internal-disk ISO boot, one-time boot entries, rollback, and boot metadata backup.
- Technicians/labs that want a repeatable, auditable, safer alternative to USB sticks.

High-level platform goals:
1. Windows 10/11 UEFI systems.
2. Major Linux distributions using GRUB, systemd-boot, or direct UEFI boot.
3. Intel macOS where supported by Apple tooling.
4. Apple Silicon macOS only where safe and supported, using platform-approved mechanisms. Do not bypass Apple boot security.
5. Legacy BIOS support may be added as a lower-priority compatibility mode, but UEFI should be the primary target.

Non-goals:
- Do not bypass Secure Boot, BitLocker, FileVault, SIP, TPM, activation, licensing, OS EULAs, or vendor security policies.
- Do not ship pirated OS images.
- Do not download unofficial ISOs by default.
- Do not implement stealth boot persistence, rootkit-like behavior, credential bypass, firmware modification, or any hidden boot manipulation.
- Do not wipe disks automatically.
- Do not hide risk from the user.
- Do not store recovery keys unencrypted.
- Do not claim universal support where firmware, Secure Boot, encryption, or vendor restrictions make it unsafe.

Product modes:
A. Safe Explore Mode
- Boot a live/rescue/install environment once.
- Current OS remains the default boot target.
- Use one-time boot where available.
- If the staged environment fails, the machine should naturally return to the original OS on next reboot.

B. Dual-Boot Setup Mode
- Help the user prepare for dual boot.
- Perform preflight checks.
- Recommend backup.
- Optionally shrink partitions only after strict validation and explicit confirmation.
- Prefer letting the official OS installer perform final installation steps.
- Never delete an existing OS automatically.

C. Replace Current OS Mode
- Highest-risk mode.
- Must require full backup verification.
- Must require typed confirmation.
- Must display exactly what data may be lost.
- Must create a recovery pack before proceeding.
- Must prefer handing control to the official installer rather than doing raw installation itself.

D. Same-OS Reinstall/Upgrade Mode
- Windows-to-Windows using official mounted ISO/setup flow or Microsoft-supported install flow.
- macOS-to-macOS using Apple-supported installers/startosinstall where available.
- Linux-to-Linux using distro-supported methods where possible.

E. Technician / Lab Mode
- Optional advanced mode.
- Support catalogs, local mirror, network boot/iPXE mode, and repeated workflows.
- Must still preserve safety gates.

Recommended tech stack:
- Rust workspace for core logic and platform adapters.
- Tauri desktop app with TypeScript/React frontend.
- CLI binary using Rust clap.
- SQLite for local operation journal, state tracking, and audit logs.
- Strongly typed operation model.
- No unsafe Rust unless justified, isolated, reviewed, and documented.
- Use platform APIs where possible instead of shelling out. Shell commands are allowed only behind typed wrappers with strict argument construction, output parsing, timeout, and audit logging.
- Separate “planner” from “executor” so every dangerous operation can be previewed before execution.

Repository structure:
- /crates/nousb-core
  - operation graph
  - risk model
  - platform-neutral types
  - installer profile engine
  - catalog and verification logic
  - backup metadata model
  - journal model
  - dry-run planner
- /crates/nousb-platform-windows
  - Windows detection
  - BitLocker checks
  - BCD/UEFI boot entry management
  - WinRE checks
  - VSS/System Restore integration where applicable
  - mounted ISO / setup.exe flow for Windows installers
- /crates/nousb-platform-linux
  - UEFI detection
  - GRUB/systemd-boot detection
  - efibootmgr integration
  - grub-reboot integration
  - partition and filesystem detection
  - LUKS/Btrfs/snapshot awareness
- /crates/nousb-platform-macos
  - macOS version and architecture detection
  - FileVault checks
  - APFS snapshot awareness
  - diskutil/bless/startosinstall wrappers where supported
  - Apple Silicon restrictions and safe messages
- /crates/nousb-installer-profiles
  - declarative OS installer profiles
  - Ubuntu, Fedora, Debian, Linux Mint, Arch, Windows 10/11, macOS supported-profile placeholders
  - checksum/signature metadata schema
- /crates/nousb-cli
  - command-line interface
- /apps/desktop
  - Tauri app
  - React/TypeScript UI
- /docs
  - architecture
  - threat model
  - boot safety design
  - backup and recovery guide
  - platform compatibility matrix
  - developer guide
  - user guide
- /tests
  - fixtures
  - fake platform adapters
  - VM integration test plans
- /scripts
  - dev helpers only
- /packaging
  - Windows installer packaging plan
  - Linux deb/rpm/AppImage plan
  - macOS dmg/pkg plan

Architecture requirements:
1. Use a command planner/executor model.
   - Planner creates an OperationPlan.
   - OperationPlan contains steps, risks, required privileges, rollback actions, and expected state changes.
   - Executor executes only approved plans.
   - Every step writes to an append-only operation journal.

2. Every operation must be idempotent where possible.
   - Retrying after crash should not corrupt the system.
   - Use transaction-like step states: Pending, Running, Completed, Failed, RolledBack, ManualInterventionRequired.

3. Every high-risk operation must have:
   - Preflight check.
   - Dry-run preview.
   - Backup or recovery record.
   - Rollback action where technically possible.
   - Clear user-facing warning.
   - Explicit confirmation.

4. Default boot safety:
   - Existing OS must remain the default boot target whenever possible.
   - Prefer one-time boot mechanisms.
   - Never permanently change boot order unless user explicitly chooses advanced mode.
   - Create a “return to current OS” boot path when technically possible.
   - Backup UEFI NVRAM boot entries before editing.
   - Backup Windows BCD store before editing.
   - Backup Linux bootloader config before editing.
   - Backup EFI System Partition files touched by the app before modifying them.

5. Recovery pack:
   Generate a local encrypted recovery pack before high-risk changes. It should include:
   - Disk inventory.
   - Partition table metadata.
   - EFI System Partition file manifest and backup of files modified by this app.
   - Boot entry dump.
   - BCD export on Windows.
   - GRUB/systemd-boot config backup on Linux where applicable.
   - BitLocker/FileVault/LUKS status, but not raw keys unless the user explicitly exports them into an encrypted local vault.
   - Clear manual recovery instructions.
   - Operation journal.

6. Backup expectations:
   - Detect whether user data backup exists when possible.
   - Support backup reminders and backup checklist.
   - Support VSS/System Restore on Windows when available.
   - Support APFS local snapshots on macOS where available.
   - Support Btrfs snapshots on Linux where available.
   - For LUKS systems, warn user to back up LUKS headers manually or provide a guided command with explicit warning.
   - Do not pretend metadata backups equal full user-data backups.
   - Replace Current OS Mode must require explicit acknowledgement that a full user-data backup is needed.

7. Installer image handling:
   - Only download from official sources by default.
   - Support a signed OS catalog format.
   - Verify SHA256 at minimum.
   - Support stronger signature verification where distros/vendors provide it.
   - Store downloads in a controlled cache directory.
   - Never execute unverified downloaded content.
   - Show source, version, size, checksum, and verification result.
   - Allow custom ISO only in advanced mode with strong warnings.

8. Installer staging:
   Implement a staging abstraction with these strategies:
   - Extracted EFI installer partition.
   - ISO loopback boot where supported.
   - Kernel/initrd boot profile for Linux installers where supported.
   - Windows mounted ISO/setup flow for Windows reinstall/upgrade.
   - macOS startosinstall flow where supported.
   - iPXE/network boot profile as an advanced/lab feature.
   - VHD/native boot for Windows advanced scenarios where appropriate.

9. Platform-specific behavior:

Windows adapter:
- Detect Windows version, edition, architecture, firmware mode, Secure Boot state, TPM, BitLocker status, WinRE status, disk layout, EFI System Partition, free space, admin privileges.
- Use Windows APIs or well-wrapped system tools.
- Support BCD backup/export before any BCD change.
- Support one-time boot target where feasible.
- For Windows ISO reinstall/upgrade, support official mounted ISO setup flow.
- For Linux installer boot, design a safe UEFI staging mechanism.
- Secure Boot:
  - Prefer official distro signed bootloaders from official ISOs.
  - If Secure Boot prevents booting, stop and explain; do not bypass.
- BitLocker:
  - Detect BitLocker.
  - Require recovery key backup confirmation before boot changes.
  - Offer to suspend BitLocker only using documented OS-supported mechanisms and only with explicit user consent.
  - Never disable BitLocker permanently without explicit advanced confirmation.

Linux adapter:
- Detect distro, kernel, architecture, firmware mode, Secure Boot, bootloader, EFI System Partition, root filesystem, encryption, free space, root privileges.
- Support GRUB loopback ISO boot where supported.
- Support grml-like ISO staging where appropriate.
- Support efibootmgr BootNext where safe.
- Support systemd-boot entries where applicable.
- Backup relevant bootloader configs before modification.
- Detect LUKS and warn about recovery.
- Detect Btrfs and offer snapshot integration where possible.
- Never overwrite /boot or ESP files without manifest/backup.

macOS adapter:
- Detect macOS version, architecture, SIP status if needed, FileVault status, APFS layout, free space.
- For macOS installers, use Apple-supported startosinstall where available.
- For Intel Mac Windows flows, do not reimplement Boot Camp unsafely; provide supported integration/guidance where possible.
- For Apple Silicon Linux flows, do not bypass Apple boot policy. Provide profile-based integration only for trusted, supported projects and require explicit compatibility checks.
- Use diskutil/bless/startosinstall wrappers only with strict validation.
- Do not attempt unsupported bootloader modifications on Apple Silicon.

Desktop UI requirements:
- Home screen:
  - “Boot another OS once”
  - “Install alongside current OS”
  - “Replace current OS”
  - “Reinstall or upgrade current OS”
  - “Technician mode”
- Risk-first UX:
  - Show current system summary.
  - Show boot mode, encryption, Secure Boot, disk layout, available space.
  - Show warnings before choices.
- OS catalog:
  - Show official OS options.
  - Show verified/unverified state.
  - Show supported modes per OS.
- Plan preview:
  - Display every step before execution.
  - Display what will be modified.
  - Display rollback capability per step.
  - Display risk level.
- Confirmation:
  - For high-risk actions, require typed confirmation.
  - For destructive flows, require user to type the target disk/partition label.
- Recovery:
  - Show recovery pack location.
  - Offer printable recovery instructions.
- Logs:
  - Provide exportable diagnostic bundle with secrets redacted.

CLI requirements:
Commands should include:
- nousb system inspect
- nousb catalog list
- nousb image download <profile>
- nousb image verify <path-or-id>
- nousb plan boot-once <profile-or-image>
- nousb plan dual-boot <profile-or-image>
- nousb plan replace-os <profile-or-image>
- nousb plan reinstall-current-os <profile-or-image>
- nousb execute <plan-id>
- nousb rollback <operation-id>
- nousb recovery-pack create
- nousb logs export
- nousb doctor

CLI must support:
- --dry-run
- --json
- --yes only for low-risk actions
- No --yes bypass for destructive actions
- Machine-readable exit codes
- Clear stderr/stdout separation

Security requirements:
- Threat model document required.
- No hidden telemetry.
- Telemetry disabled by default.
- Crash reports opt-in only.
- Secret redaction in logs.
- No recovery key upload.
- No privileged helper running permanently unless explicitly justified.
- Least privilege.
- Privilege escalation only for specific operations.
- Signed releases plan.
- SBOM generation.
- Dependency license audit.
- Supply-chain checklist.
- Reproducible build notes where feasible.
- All downloads verified.
- Custom ISO mode clearly marked as advanced/untrusted.

Reliability requirements:
- Operation journal must survive crashes.
- Simulate power failure between steps in tests.
- Support resume/recover after failed execution.
- Use file locks to prevent concurrent operations.
- Check battery/AC power on laptops where possible before high-risk changes.
- Check disk health indicators where possible.
- Check free space with safety margin.
- Avoid modifying boot configuration if system is already in an unhealthy boot state.
- Always provide manual recovery instructions when automatic rollback is impossible.

Testing requirements:
1. Unit tests for:
   - Operation planner.
   - Risk scoring.
   - Confirmation gates.
   - Installer profile parsing.
   - Checksum verification.
   - Journal state machine.
   - Rollback decision logic.
   - Platform detection parsers.
   - Command wrappers.

2. Integration tests using fake adapters:
   - Windows UEFI + BitLocker on/off.
   - Windows Secure Boot on/off.
   - Linux GRUB/systemd-boot.
   - Linux LUKS/Btrfs/ext4.
   - macOS Intel/Apple Silicon simulated capabilities.
   - Low disk space.
   - Corrupt ISO.
   - Checksum mismatch.
   - Interrupted operation.
   - Failed boot entry creation.
   - Rollback after partial failure.

3. VM test plan:
   - QEMU/OVMF for UEFI Linux.
   - Windows VM manual/automated test checklist.
   - Fedora/Ubuntu/Debian test matrix.
   - Secure Boot test notes.
   - Snapshot-based destructive testing.
   - macOS tests documented separately due licensing/hardware constraints.

4. CI:
   - Rust fmt/clippy/test.
   - TypeScript lint/test/build.
   - Dependency audit.
   - License check.
   - SBOM generation.
   - Package build smoke tests.
   - No real boot modification in CI.

Coding standards:
- Prefer explicit error types using thiserror/anyhow appropriately.
- Library crates should expose typed errors.
- User-facing errors must be understandable and actionable.
- Do not use unwrap/expect in production paths.
- Use structured logging.
- Use serde for typed config.
- Use semver for profile schema.
- Use rustfmt/clippy.
- Use TypeScript strict mode.
- Use accessibility-friendly UI.
- Keep platform-specific code isolated.

Deliverables:
1. Working Rust workspace scaffold.
2. Tauri desktop app scaffold.
3. CLI scaffold with real commands.
4. Core operation planner implemented.
5. Installer profile schema implemented.
6. Image download/verify module implemented.
7. Fake platform adapters implemented.
8. Windows/Linux/macOS platform adapter skeletons with safe detection functions implemented where possible.
9. Operation journal implemented using SQLite.
10. Recovery pack generator implemented for available metadata.
11. Dry-run plan preview implemented in CLI and desktop UI.
12. Risk model implemented.
13. Confirmation gate implemented.
14. Test suite implemented.
15. CI workflow implemented.
16. Docs written:
    - README.md
    - docs/ARCHITECTURE.md
    - docs/SAFETY_MODEL.md
    - docs/THREAT_MODEL.md
    - docs/PLATFORM_COMPATIBILITY.md
    - docs/RECOVERY.md
    - docs/TESTING.md
    - docs/INSTALLER_PROFILES.md
    - docs/CONTRIBUTING.md

Implementation strategy:
Start by creating the full architecture and safe non-destructive core. Implement real destructive or boot-changing behavior only behind platform adapter traits and feature flags. The default build must not modify the developer machine bootloader. Provide mocked/fake adapters and dry-run behavior first. Then implement guarded real adapters with compile-time features and runtime confirmation gates.

Use feature flags:
- real-windows-boot
- real-linux-boot
- real-macos-boot
- destructive-tests
- technician-mode

Default features must be safe and non-destructive.

Data model requirements:
Define types such as:
- SystemInventory
- FirmwareMode
- SecureBootState
- EncryptionState
- DiskInventory
- PartitionInfo
- BootEntry
- InstallerProfile
- ImageArtifact
- VerificationResult
- StagingStrategy
- OperationPlan
- OperationStep
- RiskLevel
- ConfirmationRequirement
- RollbackAction
- JournalEntry
- RecoveryPack
- PlatformCapability

Risk levels:
- Info
- Low
- Medium
- High
- Critical

Critical risk examples:
- Replace current OS.
- Delete partition.
- Resize encrypted partition.
- Modify default boot order permanently.
- Suspend disk encryption.
- Write to EFI System Partition.
- Modify BCD store.
- Modify GRUB/systemd-boot config.
- Boot unverified image.

Critical risk actions must require typed confirmation and cannot be executed with a simple --yes flag.

Installer profile schema:
Create a versioned TOML or YAML schema with:
- id
- name
- vendor
- version
- architecture
- source_url
- checksum_url
- sha256
- signature_url optional
- homepage
- license/eula notes
- supported_platforms
- supported_staging_strategies
- secure_boot_notes
- required_free_space
- extraction_rules
- kernel_initrd_paths optional
- bootloader_paths optional
- known_limitations
- post_boot_instructions

Boot safety design:
Implement a BootPlan that prefers:
1. One-time boot entry / BootNext.
2. Temporary boot entry with fallback to current OS.
3. Existing boot manager one-shot mechanism.
4. Manual reboot instructions only if safe automation is unavailable.

Never make the staged installer the permanent default unless the user explicitly requests advanced behavior.

Operation journal:
Use SQLite with tables:
- operations
- steps
- artifacts
- backups
- warnings
- confirmations
- logs
- recovery_packs

The journal must store:
- created_at
- updated_at
- platform
- app_version
- operation_type
- plan_hash
- state
- step state
- rollback status
- artifact paths
- redacted command logs

Desktop app:
Build the UI around the same planner/executor API used by CLI. Do not duplicate logic in frontend. Frontend calls backend commands exposed by Tauri.

Frontend screens:
- System Check
- Choose OS
- Download & Verify
- Choose Mode
- Safety & Backup
- Plan Preview
- Execute
- Reboot Ready
- Recovery
- Logs

Accessibility:
- Keyboard navigable.
- Clear contrast.
- No warning hidden only by color.
- Plain-language risk explanations.

Docs tone:
Be clear and honest. Say that no software can guarantee recovery from every boot/disk failure. Explain that backups are still required. Explain firmware limitations. Explain Secure Boot and disk encryption limitations.

Acceptance criteria:
- `cargo test` passes.
- `cargo clippy --all-targets --all-features` passes or documented exceptions exist.
- TypeScript lint/build passes.
- CLI can inspect fake systems, create plans, create recovery packs, and execute fake operations.
- Desktop app can display fake system inventory, catalog, plan preview, and recovery pack state.
- Real platform adapters never perform destructive operations unless the relevant feature flag is enabled and confirmation gates pass.
- All high-risk operations are blocked in default safe mode.
- Documentation is complete enough for another engineer to continue the project.
- The app must never call real partition/boot modification commands during normal unit tests.
- Include a clear compatibility matrix.

Development process:
1. Inspect the empty/current repository.
2. Propose a concise implementation plan.
3. Create the workspace.
4. Implement core models and planner.
5. Implement fake adapters and tests.
6. Implement CLI.
7. Implement desktop scaffold.
8. Implement platform adapter skeletons.
9. Implement docs.
10. Run format, lint, and tests.
11. Summarize what was built, what is safe, what remains platform-specific, and how to test in VMs.

Do not fake completed support for dangerous platform operations. If a platform operation cannot be safely implemented in this environment, create the typed interface, safe preflight checks, mocked tests, docs, and TODOs that specify exact future implementation steps. Be honest in README and compatibility matrix.

Final output:
- Provide a summary of files created.
- Provide exact commands to build, test, and run.
- Provide next engineering milestones.
- Provide a safety warning that the real boot-changing features must only be tested inside disposable VMs/snapshots first.
```

---

## 2. Recommended `AGENTS.md`

Create a file named `AGENTS.md` in the repo root and paste this:

````markdown
# AGENTS.md

## Project

This repository builds **NoUSB OS Switcher**, a cross-platform Windows/Linux/macOS app that stages verified OS installers on the internal disk and configures safe one-time boot flows to reduce or eliminate the need for USB installer media.

This is low-level system software. Safety is more important than feature speed.

## Hard safety rules

- Do not implement silent destructive operations.
- Do not wipe, format, resize, repartition, overwrite, or modify boot configuration without:
  - typed operation plan,
  - dry-run preview,
  - explicit confirmation gate,
  - backup/recovery record,
  - journal entry,
  - rollback strategy where possible.
- Default builds must be non-destructive.
- Unit tests and CI must never modify the real machine bootloader, partitions, BCD store, EFI System Partition, GRUB config, systemd-boot config, APFS layout, or firmware NVRAM.
- Use fake platform adapters for tests.
- Real boot-changing behavior must be behind feature flags.
- Never bypass Secure Boot, BitLocker, FileVault, SIP, TPM, activation, licensing, or OS vendor security controls.
- Never download unofficial OS images by default.
- Never execute unverified downloaded content.
- Never store raw recovery keys unencrypted.
- Never hide risk from the user.

## Architecture expectations

Use a Rust workspace.

Core logic must live in platform-neutral crates. Platform-specific boot/disk code must be isolated in platform adapter crates.

Preferred structure:

```text
crates/
  nousb-core/
  nousb-platform-windows/
  nousb-platform-linux/
  nousb-platform-macos/
  nousb-installer-profiles/
  nousb-cli/
apps/
  desktop/
docs/
tests/
scripts/
packaging/
````

The desktop app should use Tauri + React + TypeScript. The CLI should use the same Rust core as the desktop app.

Do not duplicate safety logic in the UI. The UI should call backend planner/executor APIs.

## Design pattern

Use planner/executor separation.

The planner produces an `OperationPlan`.

The executor executes only approved plans.

Every plan step must include:

* ID
* description
* risk level
* required privilege
* affected resources
* preflight checks
* execution action
* rollback action where possible
* confirmation requirement
* journal behavior

## Required core types

Implement and maintain types similar to:

* `SystemInventory`
* `FirmwareMode`
* `SecureBootState`
* `EncryptionState`
* `DiskInventory`
* `PartitionInfo`
* `BootEntry`
* `InstallerProfile`
* `ImageArtifact`
* `VerificationResult`
* `StagingStrategy`
* `OperationPlan`
* `OperationStep`
* `RiskLevel`
* `ConfirmationRequirement`
* `RollbackAction`
* `JournalEntry`
* `RecoveryPack`
* `PlatformCapability`

## Risk model

Risk levels:

* `Info`
* `Low`
* `Medium`
* `High`
* `Critical`

Critical actions include:

* replacing the current OS,
* deleting partitions,
* resizing encrypted partitions,
* modifying permanent boot order,
* suspending disk encryption,
* writing to EFI System Partition,
* modifying Windows BCD,
* modifying GRUB/systemd-boot config,
* booting unverified images.

Critical actions must require typed confirmation. A generic `--yes` flag must not bypass them.

## Coding standards

Rust:

* Use stable Rust.
* Use `rustfmt`.
* Use `clippy`.
* No `unwrap()` or `expect()` in production paths unless justified in a comment.
* Prefer typed errors in libraries.
* Use `thiserror` for library errors where appropriate.
* Use `anyhow` only at application boundaries where appropriate.
* Use structured logging.
* Keep unsafe code out unless absolutely required and documented.

TypeScript:

* Use strict mode.
* Keep UI state typed.
* Keep backend API bindings typed.
* Do not put platform safety logic only in frontend code.

Shell/system commands:

* Prefer native APIs where possible.
* When shelling out, use typed command wrappers.
* Never concatenate untrusted strings into shell commands.
* Set timeouts.
* Capture and redact logs.
* Parse outputs strictly.
* Handle missing commands gracefully.

## Test requirements

Required checks before claiming completion:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features
```

For frontend:

```bash
npm install
npm run lint
npm run build
npm test
```

Adjust package manager commands if the repo uses pnpm/yarn/npm, but document the final commands in README.

## CI requirements

CI should include:

* Rust formatting
* Rust tests
* Rust clippy
* TypeScript lint
* TypeScript build
* dependency audit
* license check if available
* SBOM generation if available

CI must not run real boot/disk modifications.

## Documentation requirements

Maintain:

* `README.md`
* `docs/ARCHITECTURE.md`
* `docs/SAFETY_MODEL.md`
* `docs/THREAT_MODEL.md`
* `docs/PLATFORM_COMPATIBILITY.md`
* `docs/RECOVERY.md`
* `docs/TESTING.md`
* `docs/INSTALLER_PROFILES.md`
* `docs/CONTRIBUTING.md`

Docs must be honest. Do not claim full platform support until tested.

## Product truth

This app stages installers and configures boot. It does not magically transform a running OS into another OS.

Always explain that full user-data backup is still required before replacing an OS.

## Development behavior for Codex

When working on this repo:

1. Inspect existing files first.
2. Make a concise plan.
3. Prefer small safe commits/changes.
4. Add tests with every meaningful change.
5. Do not fake low-level platform support.
6. Use mocks/fakes for dangerous adapters.
7. Run available tests before final response.
8. In final summaries, clearly separate:

   * implemented,
   * tested,
   * scaffolded,
   * not yet safe for real machines.

```

---

## Best way to run this in Codex

Use Codex on a repo, not as a one-off loose prompt. Codex supports working in IDE, CLI, and cloud environments, and OpenAI’s docs specifically recommend project instructions such as `AGENTS.md` for repo norms and test commands. :contentReference[oaicite:1]{index=1}

The important thing is that the first build should be **safe by default**: fake adapters, dry-runs, recovery packs, plan previews, and VM test docs. Real boot modification should come later behind feature flags after testing on disposable VMs.
```

[1]: https://developers.openai.com/codex?utm_source=chatgpt.com "Codex | OpenAI Developers"
