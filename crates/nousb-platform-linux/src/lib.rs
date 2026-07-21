use nousb_core::{HostPlatform, NoUsbError, PlatformAdapter, SystemInventory};

#[cfg(target_os = "linux")]
use nousb_core::{BootEntry, EncryptionState, SecureBootState};

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

pub struct LinuxSafeAdapter;

impl PlatformAdapter for LinuxSafeAdapter {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError> {
        let inventory = HostPlatform.inspect()?;
        #[cfg(target_os = "linux")]
        {
            let mut inventory = inventory;
            enrich_linux_inventory(&mut inventory);
            Ok(inventory)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(inventory)
        }
    }

    fn name(&self) -> &'static str {
        "linux-safe-read-only"
    }
}

#[cfg(target_os = "linux")]
fn enrich_linux_inventory(inventory: &mut SystemInventory) {
    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        inventory.memory_total_mb = meminfo.lines().find_map(|line| {
            let value = line.strip_prefix("MemTotal:")?.split_whitespace().next()?;
            value.parse::<u64>().ok().map(|kilobytes| kilobytes / 1024)
        });
    }

    if let Ok(entries) = fs::read_dir("/sys/firmware/efi/efivars") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with("SecureBoot-") {
                continue;
            }
            if let Ok(bytes) = fs::read(entry.path()) {
                inventory.secure_boot = match bytes.get(4) {
                    Some(1) => SecureBootState::Enabled,
                    Some(0) => SecureBootState::Disabled,
                    _ => SecureBootState::Unknown,
                };
            }
            break;
        }
    }

    if let Ok(output) = Command::new("df").args(["-Pk", "--", "/"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            inventory.disk_inventory.available_space_gb = stdout
                .lines()
                .last()
                .and_then(|line| line.split_whitespace().nth(3))
                .and_then(|value| value.parse::<u64>().ok())
                .map(|kilobytes| (kilobytes / 1024 / 1024) as u32);
        }
    }

    if let Ok(crypttab) = fs::read_to_string("/etc/crypttab") {
        if crypttab
            .lines()
            .any(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        {
            inventory.encryption = EncryptionState::Enabled("LUKS/dm-crypt configured".into());
        }
    }

    if Path::new("/boot/grub/grub.cfg").exists() || Path::new("/boot/grub2/grub.cfg").exists() {
        inventory.boot_entries.push(BootEntry {
            id: "grub-detected".into(),
            label: "GRUB configuration detected".into(),
            is_default: inventory.boot_entries.is_empty(),
        });
    }
    if Path::new("/boot/loader/loader.conf").exists() {
        inventory.boot_entries.push(BootEntry {
            id: "systemd-boot-detected".into(),
            label: "systemd-boot configuration detected".into(),
            is_default: inventory.boot_entries.is_empty(),
        });
    }
    inventory
        .warnings
        .push("Linux platform inventory was collected read-only; no GRUB, systemd-boot, EFI, disk, or LUKS settings were changed.".into());
}

pub fn real_boot_changes_enabled() -> bool {
    cfg!(feature = "real-linux-boot")
}
