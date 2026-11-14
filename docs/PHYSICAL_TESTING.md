# Physical Testing

This guide covers testing NoUSB's host one-shot boot on real hardware. Read all of it before arming anything.

## What it does

`nousb boot arm` makes the computer boot an official Linux live installer **once**, without a USB stick:

1. It re-verifies the pinned ISO (exact size and SHA-256) and backs up the firmware boot entries (and the BCD store on Windows) to `.nousb/host-boot/<plan>/backup/`.
2. It copies the ISO to `<staging volume>/NoUSB/<token>/` and verifies the copy.
3. It copies the ISO's own signed `BOOTX64.EFI` (shim), `grubx64.efi`, and `mmx64.efi` to `\EFI\NoUSB\` on the EFI System Partition, and writes a generated `grub.cfg`. That configuration loop-mounts the ISO and boots its kernel with `iso-scan/filename=` (plus `toram` or `rd.live.ram=1` when RAM allows). Ubuntu images also get `\.disk\info` and `\boot\grub\grub.cfg` on the ESP, because Ubuntu's image GRUB looks for its configuration there. Existing files are never overwritten.
4. It creates a firmware entry named `NoUSB One-Shot Installer` **outside** the persistent boot order and selects it for the next boot only. Windows uses `bcdedit /set {fwbootmgr} bootsequence`. Linux uses `efibootmgr --create-only` followed by `--bootnext`.
5. It checks that the persistent boot order is unchanged and that the one-shot selector reads back correctly. If any step fails, it undoes every completed step.

Firmware consumes the one-shot selector at the next boot, so every boot after that returns to the current OS. `nousb boot cleanup` deletes the entry, the ESP files, and the ISO copy. Each transaction step is recorded in `.nousb/host-boot/state.json` and in the operation journal.

What it does **not** do: it never partitions, formats, resizes, or decrypts anything; never changes the default boot entry or boot order; never installs an OS by itself; and never touches Secure Boot keys. Installing (or not) is your choice inside the official installer.

## Support matrix

| Host | Target | Status |
| --- | --- | --- |
| Windows 10/11, UEFI, unencrypted staging volume | Ubuntu 24.04.4 Desktop/Server (casper) | Implemented. **Physical test needed:** casper must mount NTFS from its initramfs to find the ISO |
| Windows 10/11, UEFI | Fedora 44 (dracut) | Blocked on NTFS (dracut cannot loop-mount from NTFS); use an exFAT/FAT32/ext4 staging volume with `--staging-root` |
| Linux, UEFI, ext4/btrfs/xfs root without LUKS | Ubuntu 24.04.4, Fedora 44 | Implemented. The full `nousb boot arm` → one boot → return → `cleanup` cycle passes in disposable VMs from Ubuntu, Fedora, and Debian (see below). Needs `efibootmgr` installed; the plan blocks without it |
| Windows or Linux, UEFI | Your own Linux live ISO (`custom`) | Implemented for casper (Ubuntu family) and dracut (Fedora family) images; anything else is blocked. Not part of the VM matrix |
| BitLocker- or LUKS-encrypted staging volume | any | Blocked: GRUB cannot read it. Use another unencrypted volume with `--staging-root` |
| Legacy BIOS / CSM | any | Blocked |
| any | Windows 11 | Plan-only. `nousb boot plan windows-11 --iso <path>` checks the ISO and the host and lists the blockers; `boot arm` refuses it in every build. Microsoft issues only per-session download links that cannot be pinned, Windows Setup cannot read ext4/btrfs/xfs, and the WinPE staging route is not implemented or VM-proven. From Windows, mount the ISO and run `setup.exe` yourself |
| macOS | any | Out of scope |

So "any OS to any OS" is **not** true today. The tested direction is Windows or Linux to an official Linux live installer.

## Where the ISO comes from

| Source | Command | What arming enforces |
| --- | --- | --- |
| Download by NoUSB | `nousb image download <profile>` (desktop: "Download and verify" in the Boot once view) | Pinned size and SHA-256 |
| A copy of a pinned image you already have | `nousb boot plan <profile> --iso <path>` | The same pinned size and SHA-256; any other file is refused |
| Your own Linux live ISO | `nousb boot plan custom --iso <path> --sha256 <hex from the vendor>` | The SHA-256 you supplied. Without `--sha256` the plan warns that the origin is unverified, and arming only proves that the staged copy equals your file |

`nousb image status <profile>` shows whether the pinned image is present, partly downloaded, or absent. A custom ISO must carry `/EFI/BOOT/BOOTX64.EFI` and `grubx64.efi`, and its GRUB menu must be a casper or dracut live entry; the plan lists a blocker otherwise. With Secure Boot on, firmware accepts only loaders signed for it and falls back to the installed OS if it rejects one.

## Read-only check on a Windows host

On 2026-11-14 the default and `real-windows-boot` builds were run read-only on a physical Windows 11 PC (build 26300, UEFI, NTFS system volume, 200 MiB ESP); nothing was armed. Findings:

- `HKLM\SYSTEM\CurrentControlSet\Control\PEFirmwareType` does not exist on that build, so the plan reported "UEFI firmware was not detected" on a UEFI machine. Firmware type now falls back to `%firmware_type%` and `GetFirmwareType`.
- Without elevation, BitLocker state cannot be read (`Get-BitLockerVolume` and `manage-bde` both need Administrator), so an unelevated plan lists "encryption state unknown" as a blocker. Run the plan in an elevated terminal for the real answer.

## Disposable-VM evidence for Linux hosts

`scripts/vm/linux_host_boot_rehearsal.py` runs the real `real-linux-boot` binary inside a disposable KVM + OVMF guest. The guest is an official cloud image on a throwaway qcow2 overlay with a private copy of the OVMF variable store. No host disk, ESP, or firmware variable is attached. Each run:

1. Reboots once so the baseline `BootCurrent`/`BootOrder` is the steady-state default.
2. Checks that `boot arm` refuses a wrong phrase, then arms with the typed phrase and requires an unchanged `BootOrder` and a set `BootNext`.
3. Reboots. The host requires proof that the live installer is running: the live system's `/proc/cmdline` with `iso-scan/filename=` and a tmpfs `/cdrom`, or an OCR'd installer screen. It records the bytes read from the guest disk, which shows the full ISO was copied to RAM.
4. Resets the VM, the equivalent of rebooting from the live system without installing.
5. In the guest, requires the original `BootCurrent`, an unchanged `BootOrder`, no pending `BootNext`, then a successful `boot cleanup` that leaves no NoUSB firmware entry, ESP file, or staging directory.

```sh
cargo build --release -p nousb --features real-linux-boot --target x86_64-unknown-linux-musl
python3 scripts/vm/linux_host_boot_rehearsal.py --profile ubuntu-24.04-server \
  --nousb target/x86_64-unknown-linux-musl/release/nousb \
  --iso .nousb/artifacts/ubuntu-24.04-server/ubuntu-24.04.4-live-server-amd64.iso \
  --cloud-image ubuntu-24.04-server-cloudimg-amd64.img [--memory 12G] \
  [--secure-boot-vars OVMF_VARS-with-Microsoft-keys.fd]
```

Results on 2026-11-07 (QEMU 11.1.2, KVM, edk2 OVMF 4M, `nousb` built from `main`):

| Current OS (staging filesystem) | Ubuntu 24.04.4 Server | Ubuntu 24.04.4 Desktop | Fedora 44 Workstation |
| --- | --- | --- | --- |
| Ubuntu 24.04 cloud (ext4) | pass | pass | pass |
| Fedora 44 Cloud (btrfs subvolume `/root`) | pass | pass | pass |
| Debian 13 genericcloud (ext4, no `efibootmgr` preinstalled) | pass | pass | pass |

Installer evidence per target: Server showed Subiquity's language screen with the ISO in RAM (`/cdrom` on tmpfs, 3.3 GiB read). Desktop showed "Welcome to Ubuntu" with 6.3 GiB read. Fedora showed "Welcome to Fedora Linux" through dracut `iso-scan` plus `rd.live.ram=1`, with 2.6 GiB read.

Secure Boot: with `OVMF_CODE.secboot.4m.fd` and a variable store holding the Microsoft KEKs, Windows Production PCA 2011, and UEFI CA 2011/2023 (built with `virt-fw-vars --enroll-microsoft --secure-boot`), Ubuntu, Fedora, and Debian → Ubuntu Server passed, and Ubuntu → Fedora 44 passed. The guest kernel reported `Secure boot enabled` and lockdown, and firmware accepted each ISO's own signed shim from `\EFI\NoUSB\` without a MokManager prompt.

Before the `efibootmgr` blocker existed, Debian (no `efibootmgr` preinstalled) planned `Ready: yes`, then `boot arm` failed at its first step and rolled back cleanly. The rehearsal now requires the plan to name the missing tool before installing it.

What this does not prove: firmware other than OVMF, NTFS staging, BitLocker or LUKS hosts, Windows hosts, or installing an OS. Physical testing on real hardware is still required.

## Before you start

- Use a machine whose data is backed up and that you can recover with a USB stick if the worst happens. Physical testing is how unknown firmware quirks are found.
- Know your firmware boot-menu key (often F12, F11, F8, or Esc). It always lets you pick your normal OS.
- On BitLocker systems, have the recovery key ready. NoUSB does not modify Windows Boot Manager, but firmware changes can still trigger a recovery prompt on some machines.
- Keep the project, or at least its `.nousb/` folder, outside OneDrive, Dropbox, or similar sync folders.
- Have at least ISO size + 1 GiB free on the staging volume, and about 64 MiB free on the ESP.

## Build a real-boot binary

Release and default builds cannot change boot settings. Physical testing needs an explicit opt-in build:

```sh
# Windows (in an elevated terminal later)
cargo build --release -p nousb --features real-windows-boot
# Linux
cargo build --release -p nousb --features real-linux-boot
```

`nousb doctor` reports `real boot adapters: compiled in` for these builds.

Desktop app: run `pnpm tauri build --features real-boot` in `apps/desktop`. Start the app elevated to arm. Its "Boot once" view downloads and verifies the pinned image (typed phrase `DOWNLOAD <profile>`), accepts an ISO you already have or your own Linux live ISO, and shows the same plan, blockers, and typed phrases as the CLI.

## Test procedure

```sh
nousb image download ubuntu-24.04-desktop            # ~6.2 GiB, pinned SHA-256
nousb image verify-signature ubuntu-24.04-desktop    # vendor OpenPGP signature chain
nousb boot plan ubuntu-24.04-desktop                 # read-only; lists every blocker
nousb boot plan ubuntu-24.04-desktop --staging-root D:\    # pick another unencrypted volume
```

Do not continue until `Ready: yes`. Then, in an **elevated** shell (Administrator on Windows, `sudo` on Linux):

```sh
nousb boot arm ubuntu-24.04-desktop "BOOT ONCE INTO ubuntu-24.04-desktop"
nousb boot status
```

Reboot. On Windows, use **Restart** rather than Shut down, because Fast Startup hibernates the NTFS volume on shutdown. Expected sequence:

1. Firmware starts `NoUSB One-Shot Installer` (shim, then GRUB). With Secure Boot on, a MokManager prompt is possible but should not be needed for official media.
2. GRUB shows `<profile> - live (NoUSB one-shot)` with a 10-second timeout, plus a "Cancel and reboot" entry.
3. The live system starts. With `toram`, the medium is copied to RAM first, which takes several minutes.
4. Reboot from the live system **without installing**. The machine must return to the original OS.
5. Run `nousb boot cleanup "CLEAN UP NOUSB BOOT"` in an elevated shell, then `nousb boot status`.

## What to record

File a "Physical test report" issue with:

- Machine make, model, and firmware version; Secure Boot on or off; BitLocker or LUKS state.
- Output of `nousb --json boot plan <profile>` and `nousb boot status`.
- `.nousb/host-boot/<plan>/backup/` listings (remove anything you consider private).
- Where the boot stopped, if it stopped: firmware, shim, GRUB prompt, kernel, or "unable to find a medium containing a live file system".
- Whether the next boot returned to the original OS, and whether cleanup left the boot order identical to the backup.

## If something goes wrong

| Symptom | Recovery |
| --- | --- |
| Installer did not start, PC went straight back to the OS | The one-shot selector was consumed or ignored. Run cleanup and report the firmware model |
| GRUB prompt (`grub>`) or a GRUB error | Type `reboot`. Firmware returns to the default OS. Report the error text |
| "Unable to find a medium containing a live file system" | casper could not read the staging volume (likely NTFS). Reboot, run cleanup, and retry with an exFAT/FAT32/ext4 `--staging-root` |
| Secure Boot violation | Firmware rejected the loader. Reboot to the default OS and report it. Never disable Secure Boot just for NoUSB |
| PC keeps booting the installer | Pick your OS in the firmware boot menu, then run cleanup. On Windows, `bcdedit /enum firmware` shows the entry, and `bcdedit /delete {id}` removes it |
| Windows will not boot | Boot any recovery media and run `bcdedit /import <backup>\bcd-store.bak`, or use Startup Repair |

## Known physical-test questions

These are the facts the disposable-VM evidence could not settle:

- Whether casper in Ubuntu 24.04.4 can mount an NTFS staging volume from its initramfs (`iso-scan`).
- Whether every firmware honours `bootsequence` / `BootNext` for an entry that is not in `BootOrder`. The VM matrix proved this on OVMF only.
- Whether Windows syncs a `bcdedit /copy {bootmgr}` firmware entry into NVRAM without adding it to `displayorder`. If it does add it, NoUSB removes it immediately and arm verifies the order is unchanged.
- How shim handles the `WINDOWS` load-option blob that Windows writes for BCD-managed firmware entries.
