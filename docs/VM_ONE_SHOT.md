# VM One-Shot Boot and Recovery Rehearsal

This document defines the proof required before NoUSB may modify a real Windows or Linux boot path. macOS is outside the current objective.

## Current Status

`nousb vm rehearse <backend> <profile>` runs a deterministic boot-transaction state machine. It injects interruption after every transition and verifies all of these invariants:

- The original default boot entry never changes.
- The persistent boot order never changes.
- A pending one-shot selector is consumed or cleared.
- Temporary installer entries and staged assets are removed.
- If control reached the installer, recovery returns to the original default.
- The recovery backup remains available for audit.

The matrix contains one complete cycle and eight interruption cases:

1. Capture boot-state backup.
2. Stage verified assets on the virtual disk.
3. Register a temporary installer entry.
4. Select it for one boot.
5. Transfer control to the installer and consume the one-shot selector.
6. Observe the live payload in RAM.
7. Reboot to the original default.
8. Remove temporary state.

Run all supported backend models:

```sh
cargo run -p nousb -- vm doctor
cargo run -p nousb -- vm rehearse uefi-bootnext ubuntu-24.04-desktop
cargo run -p nousb -- vm rehearse windows-bcd-bootsequence ubuntu-24.04-desktop
cargo run -p nousb -- vm rehearse linux-systemd-boot-oneshot ubuntu-24.04-desktop
cargo run -p nousb -- vm rehearse linux-grub-reboot ubuntu-24.04-desktop
```

Reports are stored under `.nousb/vm-rehearsals/`. Every report states `real_vm_executed=false` and `host_boot_changes=false`. Passing the state-machine matrix is necessary but is not evidence that firmware, Secure Boot, an installer, or a particular computer will boot.

## Real disposable-VM evidence on the current Windows host

Hyper-V is installed, but the current non-elevated identity cannot query or create Hyper-V VMs. NoUSB therefore assembled QEMU 10.1.5 and OVMF from Alpine v3.23's signed repositories under `.nousb/tools/qemu-alpine-root` and runs it through the already-enabled `docker-desktop` WSL2 distribution. Nothing is installed into Windows system directories.

Preview and run the typed, disposable checks:

```sh
cargo run -p nousb -- vm qemu plan-smoke
cargo run -p nousb -- vm qemu smoke "RUN DISPOSABLE QEMU UEFI SMOKE"
cargo run -p nousb -- vm qemu plan-ubuntu-netboot
cargo run -p nousb -- vm qemu rehearse-ubuntu-netboot "RUN DISPOSABLE UBUNTU NETBOOT RAM REHEARSAL"
cargo run -p nousb -- vm qemu plan-ubuntu-full-ram
cargo run -p nousb -- vm qemu rehearse-ubuntu-full-ram "RUN DISPOSABLE UBUNTU FULL ISO RAM REHEARSAL"
cargo run -p nousb -- vm qemu plan-ubuntu-uefi-full-ram
cargo run -p nousb -- vm qemu rehearse-ubuntu-uefi-full-ram "RUN DISPOSABLE UBUNTU UEFI FULL ISO RAM REHEARSAL"
```

The UEFI smoke has real QMP evidence that QEMU reached `running` under OVMF. `query-block` showed only the read-only OVMF code image and a fresh project-local variables copy. It attached no OS installer, network, virtual disk, physical disk, or host boot state.

The Ubuntu rehearsal booted the official 24.04.4 netboot kernel and initrd far enough to observe the Linux banner and initramfs `init-premount` scripts. QMP returned an empty block-device list. This proves that the kernel and initrd executed from guest RAM, but it uses QEMU's direct-kernel path, bypasses UEFI, disables networking, and does **not** prove that the 3.2 GB server ISO or 6.2 GB desktop live payload reached RAM.

The direct full-RAM rehearsal closes the payload half of that gap. It boots the verified Server kernel/initrd with casper's `url=` flow, downloads the pinned 3.2 GiB Server ISO into initramfs, stops the isolated source server, and requires fresh systemd, Subiquity mount, and serial-console evidence after the stop. It is still a direct-kernel boot and therefore is not UEFI evidence.

The combined UEFI full-RAM rehearsal closes the firmware-to-payload gap in the disposable harness. A run-local read-only FAT source contains only the hash-verified official `BOOTX64.EFI`, GRUB, MokManager, kernel, initrd, and generated GRUB configuration. OVMF selects that source; Linux reports EDK II UEFI; QMP removes both the temporary boot device and its block node; casper saves and mounts the full verified ISO in RAM; the isolated HTTP source stops; and fresh Ubuntu, Subiquity, and `ttyS0` evidence follows. The final QMP block list contains only OVMF code and variable pflash devices. This proves full installer payload continuity after source removal, not an OS installation.

Real reports are stored under `.nousb/vm/qemu/*/report.json`; serial and QMP logs are stored beside each report. The successful combined evidence is in `.nousb/vm/qemu/ubuntu-uefi-full-ram-1784649708-5480/report.json`. The netboot tarball is deliberately labeled `official-https-observed-digest-transport-only-not-installable` because Canonical's signed 24.04.4 `SHA256SUMS` lists the desktop/server ISOs but not the netboot tarball. It is valid only for the earlier diskless rehearsal, never as an installation payload. The full-RAM runners instead verify the official Server ISO's exact size and SHA-256.

## Backend Semantics

### UEFI BootNext

UEFI `BootNext` names one `Boot####` option to try first on the next boot. Firmware deletes `BootNext` before transferring control, then returns to `BootOrder` on later boots. This is the strongest cross-vendor primitive when firmware variables are usable, but creating the temporary `Boot####` option and staging a trusted EFI loader remain platform-specific operations.

### Windows BCD bootsequence

Microsoft documents `bcdedit /bootsequence` as a one-time boot-manager sequence. NoUSB may eventually use it only after exporting the BCD store and proving that a temporary installer entry can chain into signed EFI assets. BCDEdit requires administrative rights, and Microsoft notes that BitLocker or Secure Boot handling may be required. The default build does not call BCDEdit.

### systemd-boot LoaderEntryOneShot

`bootctl set-oneshot ID` selects a Boot Loader Specification entry only for the next boot. A future adapter must place the kernel, initrd, entry file, and ISO on the virtual ESP/XBOOTLDR, preserve the original entry, and remove the temporary files after fallback. The default build does not call `bootctl set-oneshot`.

### GRUB next_entry

GRUB supports saved and next-boot entries through `grub-reboot`, but the official manual warns that one-shot configurations need a known-good fallback and can stop at GRUB when the new entry itself is invalid. This backend therefore needs stricter VM failure tests than UEFI BootNext or systemd-boot. The default build does not call `grub-reboot`.

## Remaining real-VM proof before host boot work

The isolated QEMU engine, OVMF startup, official EFI asset chain, and full Server installer continuity in RAM after temporary-source removal are now proven. A host boot adapter still must not be implemented until the remaining recovery test satisfies all of these conditions:

- No physical disk, host ESP, host firmware variable store, or host boot configuration is attached writable.
- The VM disk and writable UEFI variable store are disposable copies.
- The original guest OS boots successfully before testing.
- The pinned and verified assets continue to pass exactly as they do in the combined rehearsal.
- A real one-shot entry is selected without changing the disposable guest's persistent default/order.
- The temporary boot source can be removed while Ubuntu's full installer payload remains available, as already proven in the combined rehearsal.
- Power loss is injected after every transaction step and the next boot returns to the original guest OS.
- The test captures serial console, firmware variable, boot-entry, artifact, and cleanup evidence.
- Secure Boot is enabled and the complete boot chain is accepted, or the product blocks that host with a precise explanation.
- Guest acquisition uses authenticated transport, or its threat model and verified-digest handoff are explicitly accepted.

The remaining critical path is: disposable current OS -> one-shot UEFI selection -> already-proven full installer in RAM -> reboot/failure -> original OS fallback -> interruption cleanup. Until that passes, `real-windows-boot` and `real-linux-boot` remain disabled. Full-RAM combined reports may truthfully set `full_installer_payload_in_ram=true`, while `secure_boot_enabled`, `guest_transport_https`, and `fallback_boot_observed` remain false.

## Primary References

- [UEFI 2.11 Boot Manager and BootNext](https://uefi.org/specs/UEFI/2.11/03_Boot_Manager.html)
- [Microsoft BCDEdit /bootsequence](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/bcdedit--bootsequence)
- [systemd bootctl set-oneshot](https://www.freedesktop.org/software/systemd/man/latest/bootctl.html)
- [systemd-boot and automatic fallback](https://www.freedesktop.org/software/systemd/man/latest/systemd-boot.html)
- [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html)
- [QEMU system invocation and firmware boot](https://www.qemu.org/docs/master/system/invocation.html)
- [QEMU direct Linux boot](https://www.qemu.org/docs/master/system/linuxboot.html)
- [Ubuntu amd64 netboot installer flow](https://documentation.ubuntu.com/server/how-to/installation/how-to-netboot-the-server-installer-on-amd64/)
- [Ubuntu 24.04.4 release images and netboot bundle](https://releases.ubuntu.com/24.04/)
- [Ubuntu casper `toram`](https://manpages.ubuntu.com/manpages/noble/man7/casper.7.html)
