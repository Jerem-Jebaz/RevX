# Installer Profiles

Installer profiles are versioned TOML files in `crates/nousb-installer-profiles/profiles`.

```toml
schema_version = "1"
id = "ubuntu-24.04-desktop"
name = "Ubuntu Desktop"
vendor = "Canonical"
version = "24.04.4 LTS"
architecture = "x86_64"
source_url = "https://releases.ubuntu.com/24.04/ubuntu-24.04.4-desktop-amd64.iso"
checksum_url = "https://releases.ubuntu.com/24.04/SHA256SUMS"
sha256 = "<official sha256>"
artifact_filename = "ubuntu-24.04.4-desktop-amd64.iso"
artifact_size_bytes = "6655619072"
homepage = "https://ubuntu.com"
required_free_space_gb = 8
supported_staging_strategies = ["iso_loopback", "kernel_initrd"]
secure_boot_notes = "Use official signed boot assets."
known_limitations = ["Default build only creates dry-run plans."]
```

Downloadable profiles must pin an official HTTPS artifact URL, safe filename, exact nonzero byte size, and 64-character SHA-256. The downloader rejects HTTPS downgrade redirects, validates response length where available, hashes the completed file in-process on Windows and Linux, and atomically promotes a verified `.part` file. Profiles such as the Windows 11 consumer ISO that lack a stable vendor URL remain catalog entries but fail closed for manifest/download commands.

Validate the embedded catalog with:

```sh
cargo run -p nousb -- catalog validate
cargo run -p nousb -- --json image manifest ubuntu-24.04-desktop
cargo run -p nousb -- --json image manifest fedora-44-workstation
```
