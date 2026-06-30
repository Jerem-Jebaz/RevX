use nousb_core::{HostPlatform, NoUsbError, PlatformAdapter, SystemInventory};

pub struct WindowsSafeAdapter;

impl PlatformAdapter for WindowsSafeAdapter {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError> {
        HostPlatform.inspect()
    }

    fn name(&self) -> &'static str {
        "windows-safe-skeleton"
    }
}

pub fn real_boot_changes_enabled() -> bool {
    cfg!(feature = "real-windows-boot")
}
