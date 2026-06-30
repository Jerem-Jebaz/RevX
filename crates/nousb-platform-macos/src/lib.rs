use nousb_core::{HostPlatform, NoUsbError, PlatformAdapter, SystemInventory};

pub struct MacosSafeAdapter;

impl PlatformAdapter for MacosSafeAdapter {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError> {
        HostPlatform.inspect()
    }

    fn name(&self) -> &'static str {
        "macos-safe-skeleton"
    }
}

pub fn real_boot_changes_enabled() -> bool {
    cfg!(feature = "real-macos-boot")
}
