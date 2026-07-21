# Boot Delivery Feasibility

This document defines what "no USB" can safely mean for NoUSB OS Switcher. The current objective covers Windows and Linux hosts on PCs, with macOS intentionally excluded.

## Engineering Conclusion

A normal application cannot keep an installer in RAM across a reboot because volatile memory is lost when firmware restarts the machine. A practical no-USB workflow therefore needs one of two roots of trust:

1. Firmware fetches a boot environment from the network using UEFI HTTP Boot or PXE.
2. The running OS stores a small, verified boot environment on persistent storage and selects it for one boot.

After that boot environment starts, an installer may copy its live media into RAM. Ubuntu's casper `toram` option is an example. Windows PE also runs its boot environment from a RAM disk, but the Windows installation payload must remain reachable on disk or over the network.

NoUSB must not promise a universal no-USB route. Firmware networking, preboot network drivers, Secure Boot policy, boot-manager capabilities, RAM, disk layout, and installer behavior differ across machines.

## Delivery Routes

| Route | Useful for | Persistent staging | Current status |
| --- | --- | --- | --- |
| Host-staged signed kernel/initrd plus RAM copy | Windows or Linux host to a compatible Linux live installer | Yes, for initial boot assets; the ISO may arrive over the network | Full pinned Ubuntu Server ISO RAM residency and UEFI boot-source detachment proven in disposable QEMU; host one-shot fallback remains unproven |
| Linux GRUB ISO loopback | Compatible Linux host and distribution | Yes | Design candidate only |
| Windows mounted ISO Setup | Supported Windows reinstall or upgrade | Yes | Design candidate only |
| WinPE RAM disk plus network install source | Windows deployment or recovery | No when firmware/network boot is available | Capability and artifact manifest required |
| UEFI HTTP Boot | Cross-vendor machines whose firmware implements it | No | Firmware capability required |
| Dell BIOSConnect | Supported Dell Windows recovery | Firmware-managed | Reference only; not a general route |

Run the safe assessment without making boot changes:

```sh
cargo run -p nousb -- tui
cargo run -p nousb -- workflow assess ubuntu-24.04-desktop
cargo run -p nousb -- workflow assess-fake windows-bitlocker ubuntu-24.04-desktop
cargo run -p nousb -- --json workflow assess-fake windows-bitlocker ubuntu-24.04-desktop
```

Inspect or acquire a pinned official artifact, then create a non-booting bundle:

```sh
cargo run -p nousb -- --json image manifest ubuntu-24.04-desktop
cargo run -p nousb -- image probe ubuntu-24.04-desktop
cargo run -p nousb -- image download ubuntu-24.04-desktop
cargo run -p nousb -- staging inspect ubuntu-24.04-desktop .nousb/artifacts/ubuntu-24.04-desktop/ubuntu-24.04.4-desktop-amd64.iso
cargo run -p nousb -- staging create ubuntu-24.04-desktop .nousb/artifacts/ubuntu-24.04-desktop/ubuntu-24.04.4-desktop-amd64.iso
```

These commands do not install an OS or modify boot configuration. The ISO download is roughly 6.2 GiB, so `image manifest` and `image probe` are the appropriate lightweight checks during development.

Readiness values mean:

- `design_candidate`: the route fits the supplied fake or detected inventory, but execution is still disabled.
- `requires_host_capability`: firmware, memory, storage, network, or artifact metadata is unknown or missing.
- `unsupported`: the route does not fit the host/profile combination.
- `out_of_scope`: excluded from the current objective.

## Dell Comparison

Dell BIOSConnect works because Dell ships a preboot network client in supported firmware. It can download Dell's SupportAssist OS Recovery environment when the installed OS or recovery partition cannot boot. An OS application cannot add an equivalent firmware service to every PC, and BIOSConnect does not accept arbitrary operating-system images.

The portable lesson is the architecture, not the Dell implementation: establish a trusted preboot client, fetch a signed recovery environment, run it independently of the installed OS, and keep a recovery path. For non-Dell systems that client must already exist in firmware or be staged through the current boot chain.

## Safe Implementation Order

1. Expand read-only Windows and Linux inspection to report real firmware mode, Secure Boot, memory, free staging space, encryption, boot manager, ESP, and network-boot hints.
2. Maintain versioned direct official artifact metadata and signatures. Ubuntu 24.04.4 and Fedora 44 are pinned; Windows remains vendor-mediated.
3. Extend the implemented and synthetic-ISO-tested staging bundle with distribution-specific extraction validation.
4. Test Windows and Linux one-shot boot designs entirely in VMs with boot metadata backup, interruption injection, and restore checks.
5. Add real adapters only behind `real-windows-boot` and `real-linux-boot`, with preview, recovery pack, typed confirmation, journal, and rollback gates.
6. Add optional UEFI HTTP Boot/iPXE integration as a separate server-assisted mode; never assume preboot Wi-Fi or HTTP Boot exists.
7. Connect the existing guided TUI workflow to the React/Tauri command bridge before exposing any executor control.

The first real milestone should boot an official Ubuntu live environment once in a disposable UEFI VM, copy the live media to RAM, and return to the original default boot entry after failure or reboot. It must not install or repartition automatically.

The transaction and interruption model for that milestone is implemented by `vm rehearse` and documented in [VM_ONE_SHOT.md](VM_ONE_SHOT.md). Real QEMU evidence now also proves the combined payload path: OVMF boots official Ubuntu EFI assets from a read-only FAT source, QMP removes that source, casper downloads and loop-mounts the entire pinned server ISO in guest RAM, the HTTP source stops, and Subiquity plus the installer tty continue. This closes the full-payload/UEFI/backing-detachment portion, but it does not prove Secure Boot, guest-side HTTPS, a real second-boot fallback, or interruption recovery on a persistent guest OS.

## Primary References

- [Dell BIOSConnect and SupportAssist OS Recovery](https://www.dell.com/support/kbdoc/en-us/000177771/using-biosconnect-to-recover-supportassist-os-recovery-partition)
- [UEFI specifications](https://uefi.org/specifications), including the HTTP Boot architecture in the UEFI specification
- [Ubuntu 24.04 casper live-boot options](https://manpages.ubuntu.com/manpages/noble/man7/casper.7.html), including `toram`
- [Ubuntu 24.04.4 official release files](https://releases.ubuntu.com/24.04/), including the ISO and signed checksum files
- [Ubuntu 24.04 netboot assets](https://releases.ubuntu.com/24.04/netboot/amd64/), including the EFI loader, kernel, and initrd used by the transport-only rehearsal
- [Canonical's amd64 netboot installer flow](https://documentation.ubuntu.com/server/how-to/installation/how-to-netboot-the-server-installer-on-amd64/), which documents firmware loading a bootloader, then kernel/initrd, followed by the initrd downloading and loop-mounting the server ISO
- [Fedora Workstation official download](https://fedoraproject.org/workstation/download/)
- [Microsoft Windows 11 software download](https://www.microsoft.com/en-us/software-download/windows11), whose consumer ISO links are generated per session
- [Linux kernel initial RAM disk documentation](https://www.kernel.org/doc/html/latest/admin-guide/initrd.html)
- [GNU GRUB loopback booting](https://www.gnu.org/software/grub/manual/grub/html_node/Loopback-booting.html)
- [Microsoft BCDEdit one-time boot sequence](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/bcdedit--bootsequence)
- [Microsoft WinPE customization and RAM-disk constraints](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/winpe-mount-and-customize)
- [iPXE wimboot](https://ipxe.org/wimboot) for network-loaded WinPE
