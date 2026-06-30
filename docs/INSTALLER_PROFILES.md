# Installer Profiles

Installer profiles are versioned TOML files in `crates/nousb-installer-profiles/profiles`.

```toml
schema_version = "1"
id = "ubuntu-24.04-desktop"
name = "Ubuntu Desktop"
vendor = "Canonical"
version = "24.04 LTS"
architecture = "x86_64"
source_url = "https://ubuntu.com/download/desktop"
sha256 = "<official sha256>"
homepage = "https://ubuntu.com"
required_free_space_gb = 8
supported_staging_strategies = ["iso_loopback", "kernel_initrd"]
secure_boot_notes = "Use official signed boot assets."
known_limitations = ["Default build only creates dry-run plans."]
```

Downloads must come from official sources by default and must be verified before execution.

Validate the embedded catalog with:

```sh
cargo run -p nousb -- catalog validate
```
