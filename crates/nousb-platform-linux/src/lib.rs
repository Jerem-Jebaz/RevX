use nousb_core::{HostPlatform, NoUsbError, PlatformAdapter, SystemInventory};

pub struct LinuxSafeAdapter;

impl PlatformAdapter for LinuxSafeAdapter {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError> {
        HostPlatform.inspect()
    }

    fn name(&self) -> &'static str {
        "linux-safe-skeleton"
    }
}

pub fn real_boot_changes_enabled() -> bool {
    cfg!(feature = "real-linux-boot")
}
