# Platform Compatibility

| Platform | Current support | Future adapter scope |
| --- | --- | --- |
| Windows 10/11 UEFI | Safe inspection scaffold | BitLocker checks, BCD export, WinRE checks, mounted ISO flow |
| Linux UEFI | Safe inspection scaffold | GRUB loopback, systemd-boot entries, efibootmgr BootNext |
| Intel macOS | Safe inspection scaffold | Apple-supported `startosinstall`, `diskutil`, and `bless` workflows |
| Apple Silicon macOS | Guidance only | Supported workflows only; no boot policy bypass |
| Legacy BIOS | Not implemented | Lower-priority compatibility mode |

Default builds must not perform real boot changes.

