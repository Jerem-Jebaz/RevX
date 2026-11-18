//! Host one-shot boot of a verified Linux live installer.
//!
//! This is the transaction proven by the disposable one-shot matrix (`docs/VM_ONE_SHOT.md`),
//! applied to a real host: back up boot state, copy the verified ISO to an unencrypted volume,
//! place the ISO's own signed shim/GRUB on the EFI System Partition under `\EFI\NoUSB`, create a
//! temporary firmware entry outside the persistent boot order, and select it for exactly one
//! boot. Firmware consumes the one-shot selector, so every later boot returns to the unchanged
//! default. `cleanup` removes everything the transaction created.
//!
//! Planning is pure and works in every build. Execution needs a [`HostBootBackend`]; real
//! backends exist only in the platform crates behind `real-windows-boot` / `real-linux-boot`,
//! and every run requires the exact typed phrase from [`arm_confirmation_phrase`].

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    artifact_cache_path, artifact_manifest, find_profile, iso_has_file, now_stamp,
    read_iso_text_file, sha256_file, write_journal_entry, FirmwareMode, NoUsbError,
    SecureBootState,
};

/// Directory under the ESP's `\EFI` that holds the temporary loader chain.
pub const HOST_BOOT_ESP_DIR: &str = "NoUSB";
/// Firmware entry description. Cleanup only ever deletes the entry id recorded in the state.
pub const HOST_BOOT_ENTRY_LABEL: &str = "NoUSB One-Shot Installer";
/// Directory at the staging volume root that holds the verified ISO copy.
pub const HOST_BOOT_STAGING_DIR: &str = "NoUSB";
pub const CLEANUP_CONFIRMATION_PHRASE: &str = "CLEAN UP NOUSB BOOT";

/// Files the official ISO carries in `/EFI/BOOT`. shim (`BOOTX64.EFI`) loads `grubx64.efi` from
/// its own directory and `mmx64.efi` when a MOK prompt is needed.
const ISO_LOADERS: [(&str, &str, bool); 3] = [
    ("/EFI/BOOT/BOOTX64.EFI", "BOOTX64.EFI", true),
    ("/EFI/BOOT/grubx64.efi", "grubx64.efi", true),
    ("/EFI/BOOT/mmx64.efi", "mmx64.efi", false),
];

/// Official GRUB configurations to derive the live-boot recipe from, in order.
const ISO_GRUB_CONFIGS: [&str; 3] = [
    "/boot/grub/grub.cfg",
    "/EFI/BOOT/grub.cfg",
    "/boot/grub2/grub.cfg",
];

/// Headroom beyond the ISO size before `toram`/`rd.live.ram` is requested.
const RAM_COPY_HEADROOM_MB: u64 = 4096;

/// Pseudo-profile for a Linux live ISO the user supplies. Nothing about it is pinned: the recipe
/// comes from the ISO's own GRUB configuration and the digest from the user or from the file.
pub const CUSTOM_PROFILE_ID: &str = "custom";
/// The catalog profile whose media Microsoft only issues through per-session links.
pub const WINDOWS_TARGET_PROFILE_ID: &str = "windows-11";

/// What the planned one-shot entry would start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootTargetKind {
    /// A Linux live installer loop-booted from its ISO. The only armable kind.
    LinuxLive,
    /// Windows Setup. Plan-only: `arm` refuses it in every build.
    WindowsSetup,
}

/// How far the ISO's identity is established before arming.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsoVerification {
    /// Size and SHA-256 are pinned in the catalog (and vendor-signed).
    Pinned,
    /// The user supplied the SHA-256 to expect; arming enforces it.
    UserSha256,
    /// No expected digest: arming only proves the staged copy equals the source file.
    Unverified,
}

impl IsoVerification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pinned => "pinned in the catalog",
            Self::UserSha256 => "SHA-256 supplied by you",
            Self::Unverified => "unverified, no expected SHA-256",
        }
    }
}

pub fn arm_confirmation_phrase(profile_id: &str) -> String {
    format!("BOOT ONCE INTO {profile_id}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveBootFamily {
    /// Ubuntu and derivatives: `iso-scan/filename=` and `toram`.
    Casper,
    /// Fedora and other dracut live images: `iso-scan/filename=` and `rd.live.ram=1`.
    Dracut,
    /// Windows Setup (WinPE `boot.wim`). Never rendered into a GRUB configuration.
    WindowsSetup,
}

impl LiveBootFamily {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Casper => "casper",
            Self::Dracut => "dracut",
            Self::WindowsSetup => "windows_setup",
        }
    }
}

/// The kernel, initrds, and arguments of the official ISO's first live menu entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveBootRecipe {
    pub family: LiveBootFamily,
    pub kernel: String,
    pub initrds: Vec<String>,
    pub kernel_args: Vec<String>,
    pub source_config: String,
}

/// Derives the live-boot recipe from an official GRUB configuration. Only plain absolute paths
/// are accepted; anything that needs GRUB variable expansion fails closed.
pub fn derive_live_boot_recipe(
    grub_cfg: &str,
    source_config: &str,
) -> Result<LiveBootRecipe, NoUsbError> {
    let mut in_entry = false;
    let mut kernel_line: Option<Vec<String>> = None;
    let mut initrd_line: Option<Vec<String>> = None;
    for raw in grub_cfg.lines() {
        let line = raw.trim();
        if line.starts_with("menuentry ") {
            if kernel_line.is_some() {
                break;
            }
            in_entry = true;
            continue;
        }
        if !in_entry {
            continue;
        }
        if line == "}" {
            if kernel_line.is_some() {
                break;
            }
            in_entry = false;
            initrd_line = None;
            continue;
        }
        let mut tokens = line.split_whitespace().map(str::to_string);
        match tokens.next().as_deref() {
            Some("linux" | "linuxefi") => kernel_line = Some(tokens.collect()),
            Some("initrd" | "initrdefi") => initrd_line = Some(tokens.collect()),
            _ => {}
        }
    }
    let kernel_tokens = kernel_line.ok_or_else(|| {
        NoUsbError::Verification(format!("{source_config} has no linux line in a menu entry"))
    })?;
    let initrd_tokens = initrd_line.ok_or_else(|| {
        NoUsbError::Verification(format!(
            "{source_config} has no initrd line in a menu entry"
        ))
    })?;
    let mut kernel_tokens = kernel_tokens.into_iter();
    let kernel = iso_boot_path(kernel_tokens.next().as_deref(), source_config)?;
    let initrds = initrd_tokens
        .iter()
        .map(|token| iso_boot_path(Some(token), source_config))
        .collect::<Result<Vec<_>, _>>()?;
    if initrds.is_empty() {
        return Err(NoUsbError::Verification(format!(
            "{source_config} initrd line names no file"
        )));
    }
    let kernel_args: Vec<String> = kernel_tokens
        // Media checks re-read the whole ISO and are meaningless for a loopback image.
        .filter(|arg| arg != "rd.live.check" && arg != "integrity-check")
        .collect();
    if let Some(arg) = kernel_args.iter().find(|arg| arg.contains('$')) {
        return Err(NoUsbError::Verification(format!(
            "{source_config} kernel argument {arg} needs GRUB variable expansion; refusing to guess"
        )));
    }
    let family = if kernel.starts_with("/casper/")
        || kernel_args.iter().any(|arg| arg == "boot=casper")
    {
        LiveBootFamily::Casper
    } else if kernel_args
        .iter()
        .any(|arg| arg == "rd.live.image" || arg.starts_with("root=live:"))
    {
        LiveBootFamily::Dracut
    } else {
        return Err(NoUsbError::Verification(format!(
            "{source_config} is neither a casper nor a dracut live image; no loopback recipe exists for it"
        )));
    };
    Ok(LiveBootRecipe {
        family,
        kernel,
        initrds,
        kernel_args,
        source_config: source_config.into(),
    })
}

fn iso_boot_path(token: Option<&str>, source_config: &str) -> Result<String, NoUsbError> {
    let token = token.unwrap_or_default();
    let path = token
        .strip_prefix("($root)")
        .or_else(|| token.strip_prefix("${root}"))
        .unwrap_or(token);
    let valid = path.starts_with('/')
        && path.len() > 1
        && !path.contains("..")
        && path
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || "/._-".contains(value));
    if valid {
        Ok(path.into())
    } else {
        Err(NoUsbError::Verification(format!(
            "{source_config} names unsupported boot path {token:?}"
        )))
    }
}

/// What the platform backend reports about the host before planning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostBootFacts {
    pub os_name: String,
    pub elevated: bool,
    pub firmware: FirmwareMode,
    pub secure_boot: SecureBootState,
    pub memory_total_mb: Option<u64>,
    /// Volume root that will hold `NoUSB/<token>/<iso>` (for example `C:\` or `/`).
    pub staging_volume_root: PathBuf,
    /// GRUB-visible path of `staging_volume_root` inside its filesystem (`/` unless the volume
    /// is a btrfs subvolume, where it is the subvolume path, for example `/root`).
    pub staging_grub_prefix: String,
    pub staging_filesystem: Option<String>,
    /// `Some(true)` blocks: GRUB and the live initramfs cannot read BitLocker or LUKS volumes.
    pub staging_volume_encrypted: Option<bool>,
    pub staging_free_bytes: Option<u64>,
    /// Host tools the real backend needs but could not run (for example `efibootmgr`).
    pub missing_tools: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedFile {
    /// `esp` or `staging`.
    pub location: String,
    /// Path relative to the ESP root or staging volume root, with `/` separators.
    pub relative_path: String,
    pub source: String,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostBootPlan {
    pub id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub host_os: String,
    pub backend: String,
    pub iso_path: PathBuf,
    pub iso_filename: String,
    /// Lowercase hex digest arming enforces; empty when [`IsoVerification::Unverified`].
    pub iso_sha256: String,
    pub iso_size_bytes: u64,
    pub iso_verification: IsoVerification,
    pub target_kind: BootTargetKind,
    pub recipe: LiveBootRecipe,
    pub ram_copy: bool,
    pub staging_volume_root: PathBuf,
    pub staging_relative_dir: String,
    pub grub_iso_path: String,
    pub grub_config: String,
    pub write_disk_info: bool,
    pub files: Vec<PlannedFile>,
    pub entry_label: String,
    pub loader_path: String,
    pub confirmation_phrase: String,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub steps: Vec<String>,
    pub recovery: Vec<String>,
    pub execution_enabled: bool,
}

impl HostBootPlan {
    pub fn ready(&self) -> bool {
        self.blockers.is_empty()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|error| {
            format!(
                "{{\"error\":{}}}",
                serde_json::Value::String(error.to_string())
            )
        })
    }

    pub fn preview(&self) -> String {
        let mut out = String::new();
        out.push_str("NoUSB host one-shot boot plan\n");
        out.push_str(&format!(
            "Target: {} ({})\nHost: {} via {}\n",
            self.profile_name, self.profile_id, self.host_os, self.backend
        ));
        out.push_str(&format!(
            "ISO: {} ({} bytes, {})\n",
            self.iso_path.display(),
            self.iso_size_bytes,
            self.iso_verification.as_str()
        ));
        out.push_str(&format!(
            "Live family: {} (from {}), RAM copy: {}\n",
            self.recipe.family.as_str(),
            self.recipe.source_config,
            self.ram_copy
        ));
        out.push_str(&format!(
            "Real execution compiled in: {}\n",
            self.execution_enabled
        ));
        out.push_str(&format!(
            "Ready: {}\n",
            if self.ready() { "yes" } else { "no" }
        ));
        for blocker in &self.blockers {
            out.push_str(&format!("BLOCKER: {blocker}\n"));
        }
        for warning in &self.warnings {
            out.push_str(&format!("warning: {warning}\n"));
        }
        out.push_str("Steps:\n");
        for (index, step) in self.steps.iter().enumerate() {
            out.push_str(&format!("  {}. {step}\n", index + 1));
        }
        if self.target_kind == BootTargetKind::WindowsSetup {
            out.push_str("Files created: none (plan-only target)\n");
            out.push_str("Arming is refused for this target in every build.\n");
            return out;
        }
        out.push_str("Files created (all removed by cleanup):\n");
        for file in &self.files {
            out.push_str(&format!(
                "  [{}] {} <- {}\n",
                file.location, file.relative_path, file.source
            ));
        }
        out.push_str("Generated GRUB configuration:\n");
        for line in self.grub_config.lines() {
            out.push_str(&format!("  | {line}\n"));
        }
        out.push_str("If anything goes wrong:\n");
        for line in &self.recovery {
            out.push_str(&format!("  - {line}\n"));
        }
        // Always name the ISO, so arming uses the same file this plan was made for.
        let iso_hint = format!(" --iso \"{}\"", self.iso_path.display());
        let sha_hint = if self.iso_verification == IsoVerification::UserSha256 {
            format!(" --sha256 {}", self.iso_sha256)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "To arm (needs an elevated shell and a real-boot build): nousb boot arm {} \"{}\"{iso_hint}{sha_hint}\n",
            self.profile_id, self.confirmation_phrase
        ));
        out
    }
}

/// Where the ISO comes from and how far its identity is established.
struct BootSource {
    profile_id: String,
    profile_name: String,
    /// Display name used in the GRUB menu title.
    title: String,
    /// File name of the staged copy; always GRUB-safe.
    filename: String,
    /// Expected size for pinned media; `None` takes the file's own size.
    expected_size: Option<u64>,
    /// Lowercase hex digest to enforce; empty when unverified.
    sha256: String,
    verification: IsoVerification,
    /// Kernel/initrd paths shown before the ISO is available.
    placeholder_paths: Vec<String>,
    missing_hint: String,
}

/// Plans a one-shot boot of a catalog profile from facts and an ISO path. Pure apart from
/// reading the ISO's GRUB configuration; blockers are reported, never silently waived.
pub fn plan_host_one_shot(
    profile_id: &str,
    iso_path: &Path,
    facts: &HostBootFacts,
    backend: &str,
    execution_enabled: bool,
) -> Result<HostBootPlan, NoUsbError> {
    let profile = find_profile(profile_id)?;
    let manifest = artifact_manifest(profile_id)?;
    let source = BootSource {
        profile_id: profile.id.clone(),
        profile_name: format!("{} {}", profile.name, profile.version),
        title: profile.name.clone(),
        filename: manifest.filename,
        expected_size: Some(manifest.size_bytes),
        sha256: manifest.sha256,
        verification: IsoVerification::Pinned,
        placeholder_paths: profile.kernel_initrd_paths.clone(),
        missing_hint: format!(
            "run `nousb image download {profile_id}` first, or pass a copy you already have with --iso"
        ),
    };
    plan_from_source(source, iso_path, facts, backend, execution_enabled)
}

/// Plans a one-shot boot of a Linux live ISO the user supplies (profile id `custom`). The
/// recipe is derived from the ISO's own GRUB configuration and fails closed for anything that
/// is not a casper or dracut live image. `expected_sha256` should come from the vendor's
/// checksum file; without it the plan warns that the image's origin is unverified.
pub fn plan_host_one_shot_custom(
    iso_path: &Path,
    expected_sha256: Option<&str>,
    facts: &HostBootFacts,
    backend: &str,
    execution_enabled: bool,
) -> Result<HostBootPlan, NoUsbError> {
    let sha256 = match expected_sha256 {
        Some(value) => {
            let value = value.trim().to_ascii_lowercase();
            if value.len() != 64 || !value.chars().all(|digit| digit.is_ascii_hexdigit()) {
                return Err(NoUsbError::InvalidInput(
                    "--sha256 must be the 64 hex digits of the vendor's SHA-256".into(),
                ));
            }
            value
        }
        None => String::new(),
    };
    let original = iso_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let source = BootSource {
        profile_id: CUSTOM_PROFILE_ID.into(),
        profile_name: format!("Custom Linux live ISO ({original})"),
        title: "Custom Linux live ISO".into(),
        filename: grub_safe_iso_filename(&original),
        expected_size: None,
        verification: if sha256.is_empty() {
            IsoVerification::Unverified
        } else {
            IsoVerification::UserSha256
        },
        sha256,
        placeholder_paths: vec![],
        missing_hint: "pass an existing Linux live ISO with --iso".into(),
    };
    plan_from_source(source, iso_path, facts, backend, execution_enabled)
}

/// Routes a plan request by profile id: `custom` needs `iso`, `windows-11` is plan-only, and
/// catalog profiles use `iso` or the verified download under `.nousb/artifacts`.
pub fn plan_host_boot(
    base_dir: &Path,
    profile_id: &str,
    iso: Option<&Path>,
    expected_sha256: Option<&str>,
    facts: &HostBootFacts,
    backend: &str,
    execution_enabled: bool,
) -> Result<HostBootPlan, NoUsbError> {
    match profile_id {
        CUSTOM_PROFILE_ID => {
            let iso = iso.ok_or_else(|| {
                NoUsbError::InvalidInput(
                    "the custom profile needs a Linux live ISO: --iso <path> [--sha256 <hex>]"
                        .into(),
                )
            })?;
            plan_host_one_shot_custom(iso, expected_sha256, facts, backend, execution_enabled)
        }
        WINDOWS_TARGET_PROFILE_ID => Ok(plan_windows_setup_target(iso, facts, backend)),
        _ => {
            if expected_sha256.is_some() {
                return Err(NoUsbError::InvalidInput(format!(
                    "{profile_id} is pinned in the catalog; --sha256 applies to the custom profile only"
                )));
            }
            match iso {
                Some(path) => {
                    plan_host_one_shot(profile_id, path, facts, backend, execution_enabled)
                }
                None => {
                    let cached = artifact_cache_path(base_dir, profile_id)?;
                    plan_host_one_shot(profile_id, &cached, facts, backend, execution_enabled)
                }
            }
        }
    }
}

/// Keeps the user's file name when GRUB can take it unquoted; otherwise a fixed name.
fn grub_safe_iso_filename(original: &str) -> String {
    let safe = !original.is_empty()
        && original.len() <= 96
        && original.to_ascii_lowercase().ends_with(".iso")
        && !original.starts_with('.')
        && original
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || "._-".contains(value));
    if safe {
        original.into()
    } else {
        "custom-live.iso".into()
    }
}

/// What a Windows ISO reveals without a UDF reader: its size, ISO9660 volume label, and
/// whether a UDF file system (where Microsoft puts the real content) is present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowsIsoFacts {
    pub size_bytes: u64,
    pub volume_id: String,
    pub udf: bool,
}

/// Reads the volume recognition sequence (sectors 16 onwards) of an optical disc image.
pub fn inspect_windows_iso(path: &Path) -> Result<WindowsIsoFacts, NoUsbError> {
    const SECTOR: usize = 2048;
    let mut file = fs::File::open(path)?;
    let size_bytes = file.metadata()?.len();
    let mut volume_id = None;
    let mut udf = false;
    let mut sector = [0_u8; SECTOR];
    for index in 16_u64..48 {
        if file.seek(SeekFrom::Start(index * SECTOR as u64)).is_err()
            || file.read_exact(&mut sector).is_err()
        {
            break;
        }
        match &sector[1..6] {
            b"CD001" if sector[0] == 1 => {
                volume_id = Some(
                    String::from_utf8_lossy(&sector[40..72])
                        .trim_matches(|value: char| value == ' ' || value == '\0')
                        .to_string(),
                );
            }
            b"NSR02" | b"NSR03" => udf = true,
            b"CD001" | b"BEA01" | b"TEA01" | b"BOOT2" => {}
            _ => break,
        }
    }
    match volume_id {
        Some(volume_id) => Ok(WindowsIsoFacts {
            size_bytes,
            volume_id,
            udf,
        }),
        None if udf => Ok(WindowsIsoFacts {
            size_bytes,
            volume_id: String::new(),
            udf,
        }),
        None => Err(NoUsbError::Verification(format!(
            "{} is not an optical disc image",
            path.display()
        ))),
    }
}

/// Plan-only assessment for Windows Setup as the target. It lists what a no-USB route would
/// need and every blocker on this host, creates nothing, and can never be armed: the route is
/// not implemented and has no disposable-VM evidence.
pub fn plan_windows_setup_target(
    iso: Option<&Path>,
    facts: &HostBootFacts,
    backend: &str,
) -> HostBootPlan {
    let mut blockers = vec![
        "Windows Setup is a plan-only target: staging WinPE and booting it once is not implemented and has no disposable-VM evidence, so `nousb boot arm` refuses it in every build".to_string(),
    ];
    let mut warnings = vec![
        "Microsoft publishes no stable ISO link, so NoUSB can neither download nor pin this image. Download it yourself and compare its SHA-256 with the values on Microsoft's download page".to_string(),
    ];
    let mut iso_facts = None;
    match iso {
        None => blockers.push(
            "no Windows ISO given: download it from https://www.microsoft.com/software-download/windows11 and pass --iso <path>".into(),
        ),
        Some(path) => match inspect_windows_iso(path) {
            Ok(found) if found.udf => iso_facts = Some(found),
            Ok(_) => blockers.push(format!(
                "{} has no UDF file system, so it is not Microsoft Windows Setup media",
                path.display()
            )),
            Err(error) => blockers.push(format!("{} cannot be used: {error}", path.display())),
        },
    }
    let iso_size_bytes = iso_facts.as_ref().map_or(0, |found| found.size_bytes);

    if facts.firmware != FirmwareMode::Uefi {
        blockers.push(
            "UEFI firmware was not detected; legacy BIOS one-shot boot is not supported".into(),
        );
    }
    if facts.staging_volume_encrypted == Some(true) {
        blockers.push(format!(
            "staging volume {} is encrypted; Windows Setup cannot read its sources from a BitLocker/LUKS volume before it starts",
            facts.staging_volume_root.display()
        ));
    }
    match facts.staging_filesystem.as_deref() {
        Some(name) => {
            let lower = name.to_ascii_lowercase();
            if !matches!(lower.as_str(), "ntfs" | "exfat" | "vfat" | "fat32") {
                blockers.push(format!(
                    "staging filesystem {name} cannot be read by Windows Setup; it needs an existing NTFS, exFAT, or FAT32 volume (choose one with --staging-root). NoUSB never creates or resizes partitions"
                ));
            }
            if matches!(lower.as_str(), "vfat" | "fat32") {
                warnings.push(
                    "FAT32 holds files up to 4 GiB; Windows 11's install.wim is usually larger and would have to be split".into(),
                );
            }
        }
        None => warnings.push("staging filesystem is unknown".into()),
    }
    if let (Some(free), true) = (facts.staging_free_bytes, iso_size_bytes > 0) {
        let needed = iso_size_bytes.saturating_add(1024 * 1024 * 1024);
        if free < needed {
            blockers.push(format!(
                "staging volume has {} MiB free; the extracted media needs {} MiB",
                free / (1024 * 1024),
                needed / (1024 * 1024)
            ));
        }
    }
    warnings.push(match facts.os_name.as_str() {
        "windows" => "from Windows, the route that works today needs no boot change: mount the ISO in Explorer and run setup.exe yourself (Microsoft's supported reinstall or upgrade)".to_string(),
        _ => "from Linux there is no no-USB route to Windows Setup yet; use Microsoft's own media on a USB stick, or wait for the WinPE route to pass its VM rehearsal".to_string(),
    });

    let token = format!("{WINDOWS_TARGET_PROFILE_ID}-{}", now_stamp());
    let label = iso_facts
        .as_ref()
        .filter(|found| !found.volume_id.is_empty())
        .map(|found| format!(" (volume {})", found.volume_id))
        .unwrap_or_default();
    HostBootPlan {
        id: format!("host-boot-{token}"),
        profile_id: WINDOWS_TARGET_PROFILE_ID.into(),
        profile_name: format!("Windows Setup{label}"),
        host_os: facts.os_name.clone(),
        backend: backend.into(),
        iso_path: iso.map(Path::to_path_buf).unwrap_or_default(),
        iso_filename: String::new(),
        iso_sha256: String::new(),
        iso_size_bytes,
        iso_verification: IsoVerification::Unverified,
        target_kind: BootTargetKind::WindowsSetup,
        recipe: LiveBootRecipe {
            family: LiveBootFamily::WindowsSetup,
            kernel: "/sources/boot.wim".into(),
            initrds: vec![],
            kernel_args: vec![],
            source_config: "Microsoft Windows Setup media (UDF, not inspected)".into(),
        },
        ram_copy: false,
        staging_volume_root: facts.staging_volume_root.clone(),
        staging_relative_dir: String::new(),
        grub_iso_path: String::new(),
        grub_config: String::new(),
        write_disk_info: false,
        files: vec![],
        entry_label: HOST_BOOT_ENTRY_LABEL.into(),
        loader_path: String::new(),
        confirmation_phrase: arm_confirmation_phrase(WINDOWS_TARGET_PROFILE_ID),
        blockers,
        warnings,
        steps: vec![
            "Design only, nothing below is implemented or executed".into(),
            "Hash the ISO you supply and record its SHA-256".into(),
            "Extract the media (UDF) to NoUSB/<token>/ on an NTFS, exFAT, or FAT32 volume that Windows Setup can read".into(),
            "Place Microsoft's signed boot manager, a generated BCD, and boot.sdi on the ESP with a RAM-disk entry for sources/boot.wim (roughly 1 GiB of FAT space)".into(),
            "Create a firmware entry outside the persistent boot order and select it for one boot".into(),
            "Windows Setup starts from RAM and reads install.wim from the staging volume; installing stays your choice inside Setup".into(),
            "Before any host run: prove all of this in a disposable VM, including fallback and power-loss cleanup".into(),
        ],
        recovery: vec![
            "This plan changes nothing: no files, no firmware entry, no boot selector.".into(),
        ],
        execution_enabled: false,
    }
}

fn plan_from_source(
    source: BootSource,
    iso_path: &Path,
    facts: &HostBootFacts,
    backend: &str,
    execution_enabled: bool,
) -> Result<HostBootPlan, NoUsbError> {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let pinned = source.verification == IsoVerification::Pinned;

    let actual_size = fs::metadata(iso_path)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len());
    let iso_size_bytes = source.expected_size.or(actual_size).unwrap_or(0);
    let usable = match (actual_size, source.expected_size) {
        (None, _) => {
            blockers.push(format!(
                "{} ISO not found at {}; {}",
                if pinned { "verified" } else { "custom" },
                iso_path.display(),
                source.missing_hint
            ));
            false
        }
        (Some(size), Some(expected)) if size != expected => {
            blockers.push(format!(
                "{} is {size} bytes, but the pinned artifact is {expected} bytes",
                iso_path.display()
            ));
            false
        }
        (Some(0), _) => {
            blockers.push(format!("{} is empty", iso_path.display()));
            false
        }
        (Some(_), _) => true,
    };

    let recipe = match (usable, pinned) {
        (false, _) => placeholder_recipe(&source.placeholder_paths),
        // Pinned media is known to carry a usable recipe; anything else is a hard error.
        (true, true) => read_recipe_from_iso(iso_path)?,
        (true, false) => match read_recipe_from_iso(iso_path) {
            Ok(recipe) => recipe,
            Err(error) => {
                blockers.push(format!(
                    "{error}. Only casper (Ubuntu family) and dracut (Fedora family) live ISOs can be loop-booted"
                ));
                placeholder_recipe(&[])
            }
        },
    };
    if usable && !pinned {
        for (iso_file, _, required) in ISO_LOADERS {
            if required && !iso_has_file(iso_path, iso_file).unwrap_or(false) {
                blockers.push(format!(
                    "the ISO has no {iso_file}; it cannot be started through its own signed shim and GRUB"
                ));
            }
        }
    }
    if !pinned {
        warnings.push(match source.verification {
            IsoVerification::UserSha256 => format!(
                "custom ISO: arming enforces the SHA-256 you supplied ({}); NoUSB cannot check a vendor signature for it",
                source.sha256
            ),
            _ => "custom ISO without --sha256: its origin is unverified. Compare it with the vendor's checksum file and pass --sha256 <hex>".into(),
        });
        warnings.push(
            "custom ISOs are outside the disposable-VM matrix; if the live system cannot find its medium, reboot and the firmware returns to this OS".into(),
        );
    }

    let (host_blockers, host_warnings) =
        host_preflight(facts, recipe.family, iso_size_bytes, execution_enabled);
    blockers.extend(host_blockers);
    warnings.extend(host_warnings);

    let ram_copy = facts
        .memory_total_mb
        .is_some_and(|memory| memory >= iso_size_bytes / (1024 * 1024) + RAM_COPY_HEADROOM_MB);
    if !ram_copy {
        warnings.push(
            "not enough RAM (or unknown) to copy the live medium to RAM; the live session keeps reading the ISO from disk".into(),
        );
    }

    let token = format!("{}-{}", source.profile_id, now_stamp());
    let staging_relative_dir = format!("{HOST_BOOT_STAGING_DIR}/{token}");
    let prefix = facts.staging_grub_prefix.trim_end_matches('/');
    let grub_iso_path = format!("{prefix}/{staging_relative_dir}/{}", source.filename);
    let marker_name = format!("nousb-{token}.marker");
    let grub_marker_path = format!("{prefix}/{staging_relative_dir}/{marker_name}");
    let grub_config = render_grub_config(
        &source.title,
        &recipe,
        &grub_iso_path,
        &grub_marker_path,
        ram_copy,
    );
    let write_disk_info = recipe.family == LiveBootFamily::Casper;

    let mut files = vec![
        PlannedFile {
            location: "staging".into(),
            relative_path: format!("{staging_relative_dir}/{}", source.filename),
            source: format!("verified copy of {}", iso_path.display()),
            size_bytes: Some(iso_size_bytes),
        },
        PlannedFile {
            location: "staging".into(),
            relative_path: format!("{staging_relative_dir}/{marker_name}"),
            source: "generated marker that GRUB searches for".into(),
            size_bytes: None,
        },
    ];
    for (iso_file, name, _) in ISO_LOADERS {
        files.push(PlannedFile {
            location: "esp".into(),
            relative_path: format!("EFI/{HOST_BOOT_ESP_DIR}/{name}"),
            source: format!(
                "ISO {iso_file} (the ISO's own signed loader; optional files only if present)"
            ),
            size_bytes: None,
        });
    }
    files.push(PlannedFile {
        location: "esp".into(),
        relative_path: format!("EFI/{HOST_BOOT_ESP_DIR}/grub.cfg"),
        source: "generated GRUB configuration".into(),
        size_bytes: Some(grub_config.len() as u64),
    });
    if write_disk_info {
        // Ubuntu's image GRUB finds its configuration by searching for /.disk/info and then
        // reading $root/boot/grub/grub.cfg; both are created only if absent.
        for (path, source) in [
            (
                ".disk/info",
                "generated ISO-style volume marker for Ubuntu's image GRUB",
            ),
            ("boot/grub/grub.cfg", "generated GRUB configuration (copy)"),
        ] {
            files.push(PlannedFile {
                location: "esp".into(),
                relative_path: path.into(),
                source: source.into(),
                size_bytes: None,
            });
        }
    }

    let loader_path = format!("\\EFI\\{HOST_BOOT_ESP_DIR}\\BOOTX64.EFI");
    let steps = vec![
        "Re-check elevation, UEFI, staging-volume encryption, and free space".into(),
        match source.verification {
            IsoVerification::Pinned => {
                "Re-verify the full ISO against the pinned size and SHA-256".to_string()
            }
            IsoVerification::UserSha256 => {
                "Hash the full ISO and require the SHA-256 you supplied".to_string()
            }
            IsoVerification::Unverified => {
                "Hash the full ISO and record its SHA-256 (no expected value to compare)"
                    .to_string()
            }
        },
        "Back up firmware boot entries (and the BCD store on Windows) to .nousb/host-boot/<plan>/backup".into(),
        format!(
            "Copy the ISO to {} and verify the copy's SHA-256",
            facts
                .staging_volume_root
                .join(relative_native(&staging_relative_dir))
                .display()
        ),
        format!("Mount the ESP (Windows: a free drive letter via mountvol) and create EFI/{HOST_BOOT_ESP_DIR}; refuse if it exists"),
        "Copy the ISO's signed shim/GRUB/MokManager to the ESP and write the generated grub.cfg".into(),
        format!("Create firmware entry \"{HOST_BOOT_ENTRY_LABEL}\" -> {loader_path}, outside the persistent boot order"),
        "Select that entry for the next boot only (UEFI BootNext / bcdedit {fwbootmgr} bootsequence)".into(),
        "Verify the persistent boot order is unchanged; on any failure, undo every completed step".into(),
        "You reboot. Firmware consumes the one-shot selector; later boots return to the current OS".into(),
        "Back in the current OS, run `nousb boot cleanup` to remove the entry and every file".into(),
    ];
    let recovery = vec![
        "If the installer does not start, simply reboot: the one-shot selector is already consumed and firmware returns to your current OS.".into(),
        "If a machine ever boots the installer repeatedly, choose your OS in the firmware boot menu (often F12/F11/Esc/F8) and run `nousb boot cleanup`.".into(),
        "Backups of the firmware entries and BCD are in .nousb/host-boot/<plan>/backup; `bcdedit /import` restores the BCD on Windows.".into(),
        "On BitLocker systems keep the recovery key at hand before any boot change.".into(),
    ];

    Ok(HostBootPlan {
        id: format!("host-boot-{token}"),
        confirmation_phrase: arm_confirmation_phrase(&source.profile_id),
        profile_id: source.profile_id,
        profile_name: source.profile_name,
        host_os: facts.os_name.clone(),
        backend: backend.into(),
        iso_path: iso_path.to_path_buf(),
        iso_filename: source.filename,
        iso_sha256: source.sha256,
        iso_size_bytes,
        iso_verification: source.verification,
        target_kind: BootTargetKind::LinuxLive,
        recipe,
        ram_copy,
        staging_volume_root: facts.staging_volume_root.clone(),
        staging_relative_dir,
        grub_iso_path,
        grub_config,
        write_disk_info,
        files,
        entry_label: HOST_BOOT_ENTRY_LABEL.into(),
        loader_path,
        blockers,
        warnings,
        steps,
        recovery,
        execution_enabled,
    })
}

/// Host-level preflight shared by planning and arming. Never waives a blocker.
fn host_preflight(
    facts: &HostBootFacts,
    family: LiveBootFamily,
    iso_size_bytes: u64,
    execution_enabled: bool,
) -> (Vec<String>, Vec<String>) {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    if !execution_enabled {
        blockers.push(
            "this build has no real boot backend; rebuild with --features real-windows-boot or real-linux-boot".into(),
        );
    }
    if !matches!(facts.os_name.as_str(), "windows" | "linux") {
        blockers.push(format!(
            "{} hosts are out of scope for real boot changes",
            facts.os_name
        ));
    }
    if facts.firmware != FirmwareMode::Uefi {
        blockers.push(
            "UEFI firmware was not detected; legacy BIOS one-shot boot is not supported".into(),
        );
    }
    if !facts.elevated {
        blockers.push(
            "not elevated: arming needs Administrator (Windows) or root (Linux); planning works without it".into(),
        );
    }
    for tool in &facts.missing_tools {
        blockers.push(format!(
            "required tool {tool} is not installed; install it with your package manager (for example `apt install {tool}` or `dnf install {tool}`)"
        ));
    }
    match facts.staging_volume_encrypted {
        Some(true) => blockers.push(format!(
            "staging volume {} is encrypted; GRUB and the live initramfs cannot read BitLocker/LUKS volumes. Choose an unencrypted volume with --staging-root",
            facts.staging_volume_root.display()
        )),
        None => blockers.push(format!(
            "encryption state of staging volume {} is unknown; arming re-checks it with elevation and refuses if it stays unknown",
            facts.staging_volume_root.display()
        )),
        Some(false) => {}
    }
    let needed = iso_size_bytes.saturating_add(1024 * 1024 * 1024);
    match facts.staging_free_bytes {
        Some(free) if free < needed => blockers.push(format!(
            "staging volume has {} MiB free; the ISO copy needs {} MiB",
            free / (1024 * 1024),
            needed / (1024 * 1024)
        )),
        None => warnings.push("staging volume free space is unknown".into()),
        _ => {}
    }
    if let Some(fs_name) = facts.staging_filesystem.as_deref() {
        let fs_lower = fs_name.to_ascii_lowercase();
        if !matches!(
            fs_lower.as_str(),
            "ntfs" | "ext4" | "ext3" | "ext2" | "btrfs" | "xfs" | "vfat" | "fat32" | "exfat"
        ) {
            blockers.push(format!(
                "staging filesystem {fs_name} is not known to be readable by the official GRUB"
            ));
        }
        const FAT32_MAX_FILE_BYTES: u64 = 0xFFFF_FFFF;
        if matches!(fs_lower.as_str(), "vfat" | "fat32") && iso_size_bytes > FAT32_MAX_FILE_BYTES {
            let iso_gib = iso_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            blockers.push(format!(
                "staging filesystem {fs_name} has a 4 GiB single-file limit; this ISO is {iso_gib:.1} GiB (choose another staging volume with --staging-root)"
            ));
        }
        if fs_lower == "ntfs" && family == LiveBootFamily::Dracut {
            blockers.push(
                "dracut live images cannot loop-mount an ISO from NTFS; stage it on an ext4/FAT/exFAT volume with --staging-root".into(),
            );
        }
        if fs_lower == "ntfs" && family == LiveBootFamily::Casper {
            warnings.push(
                "casper must mount NTFS from its initramfs to find the ISO; this is a physical-test item (see docs/PHYSICAL_TESTING.md)".into(),
            );
        }
    }
    if facts.secure_boot == SecureBootState::Enabled {
        warnings.push(
            "Secure Boot is enabled: only the ISO's own signed shim/GRUB/kernel are used. If firmware rejects them, it falls back to the normal OS".into(),
        );
    }

    (blockers, warnings)
}

fn placeholder_recipe(kernel_initrd_paths: &[String]) -> LiveBootRecipe {
    let kernel = kernel_initrd_paths
        .first()
        .cloned()
        .unwrap_or_else(|| "(read from the ISO once it is downloaded)".into());
    LiveBootRecipe {
        // With nothing known yet, casper adds no family-specific blocker of its own.
        family: if kernel.starts_with("/casper/") || kernel_initrd_paths.is_empty() {
            LiveBootFamily::Casper
        } else {
            LiveBootFamily::Dracut
        },
        kernel,
        initrds: kernel_initrd_paths.iter().skip(1).cloned().collect(),
        kernel_args: vec![],
        source_config: "profile metadata (ISO not yet available)".into(),
    }
}

fn read_recipe_from_iso(iso_path: &Path) -> Result<LiveBootRecipe, NoUsbError> {
    let mut errors = Vec::new();
    for config in ISO_GRUB_CONFIGS {
        match read_iso_text_file(iso_path, config, 256 * 1024) {
            Ok(Some(text)) => match derive_live_boot_recipe(&text, config) {
                Ok(recipe) => return Ok(recipe),
                Err(error) => errors.push(error.to_string()),
            },
            Ok(None) => {}
            Err(error) => errors.push(error.to_string()),
        }
    }
    Err(NoUsbError::Verification(format!(
        "no usable live-boot recipe in the ISO's GRUB configuration: {}",
        if errors.is_empty() {
            "no GRUB configuration found".into()
        } else {
            errors.join("; ")
        }
    )))
}

pub fn render_grub_config(
    title: &str,
    recipe: &LiveBootRecipe,
    grub_iso_path: &str,
    grub_marker_path: &str,
    ram_copy: bool,
) -> String {
    let mut ours = vec![format!("iso-scan/filename={grub_iso_path}")];
    if ram_copy {
        ours.push(
            match recipe.family {
                LiveBootFamily::Casper => "toram",
                LiveBootFamily::Dracut | LiveBootFamily::WindowsSetup => "rd.live.ram=1",
            }
            .into(),
        );
    }
    // Arguments before `---` belong to the live system; insert ours there.
    let mut args = Vec::new();
    let mut inserted = false;
    for arg in &recipe.kernel_args {
        if arg == "---" && !inserted {
            args.extend(ours.iter().cloned());
            inserted = true;
        }
        args.push(arg.clone());
    }
    if !inserted {
        args.extend(ours);
    }
    let initrds = recipe
        .initrds
        .iter()
        .map(|path| format!("(loop){path}"))
        .collect::<Vec<_>>()
        .join(" ");
    let title = title.replace('"', "'");
    format!(
        "# Generated by NoUSB OS Switcher for exactly one boot. `nousb boot cleanup` removes it.\n\
set timeout=10\n\
set default=0\n\
\n\
menuentry \"{title} - live (NoUSB one-shot)\" {{\n\
\x20   search --no-floppy --set=nousb_root --file {grub_marker_path}\n\
\x20   loopback loop ($nousb_root){grub_iso_path}\n\
\x20   linux (loop){kernel} {args}\n\
\x20   initrd {initrds}\n\
}}\n\
\n\
menuentry \"Cancel and reboot to the installed OS\" {{\n\
\x20   reboot\n\
}}\n",
        kernel = recipe.kernel,
        args = args.join(" "),
    )
}

/// The ESP as mounted for this transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EspMount {
    pub path: PathBuf,
    /// True when the backend mounted it (Windows `mountvol`) and must unmount it afterwards.
    pub mounted_by_us: bool,
}

/// Platform operations the transaction needs. Real implementations live in the platform
/// crates behind `real-*-boot` features; [`FakeHostBootBackend`] backs the tests.
pub trait HostBootBackend {
    fn name(&self) -> &'static str;
    fn facts(&mut self, staging_root: &Path) -> Result<HostBootFacts, NoUsbError>;
    fn backup_boot_state(&mut self, dir: &Path) -> Result<Vec<PathBuf>, NoUsbError>;
    fn mount_esp(&mut self) -> Result<EspMount, NoUsbError>;
    fn unmount_esp(&mut self, mount: &EspMount) -> Result<(), NoUsbError>;
    fn esp_free_bytes(&mut self, mount: &EspMount) -> Result<Option<u64>, NoUsbError>;
    fn boot_order(&mut self) -> Result<Vec<String>, NoUsbError>;
    /// Creates a firmware entry outside the persistent boot order; returns its id.
    fn create_entry(
        &mut self,
        label: &str,
        loader: &str,
        mount: &EspMount,
    ) -> Result<String, NoUsbError>;
    fn arm_one_shot(&mut self, entry: &str) -> Result<(), NoUsbError>;
    fn pending_one_shot(&mut self) -> Result<Option<String>, NoUsbError>;
    fn clear_one_shot(&mut self) -> Result<(), NoUsbError>;
    fn delete_entry(&mut self, entry: &str) -> Result<(), NoUsbError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostBootPhase {
    Preparing,
    Armed,
    RolledBack,
    CleanedUp,
    Failed,
}

/// Persisted after every step so cleanup is idempotent and survives power loss.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostBootState {
    pub plan_id: String,
    pub profile_id: String,
    pub backend: String,
    pub phase: HostBootPhase,
    pub boot_order_before: Vec<String>,
    pub entry_id: Option<String>,
    pub armed: bool,
    pub esp_mount: Option<EspMount>,
    pub created_files: Vec<PathBuf>,
    /// Created directories, parents first; removed in reverse and only when empty.
    pub created_dirs: Vec<PathBuf>,
    /// ESP-relative paths, recorded so cleanup can find them after a later remount.
    pub esp_files: Vec<String>,
    pub esp_dirs: Vec<String>,
    pub backup_files: Vec<PathBuf>,
    pub events: Vec<String>,
    pub updated_at: u64,
}

impl HostBootState {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|error| {
            format!(
                "{{\"error\":{}}}",
                serde_json::Value::String(error.to_string())
            )
        })
    }

    pub fn needs_cleanup(&self) -> bool {
        matches!(
            self.phase,
            HostBootPhase::Preparing | HostBootPhase::Armed | HostBootPhase::Failed
        )
    }
}

pub fn host_boot_dir(base_dir: &Path) -> PathBuf {
    base_dir.join(".nousb").join("host-boot")
}

pub fn host_boot_state_path(base_dir: &Path) -> PathBuf {
    host_boot_dir(base_dir).join("state.json")
}

pub fn load_host_boot_state(base_dir: &Path) -> Result<Option<HostBootState>, NoUsbError> {
    let path = host_boot_state_path(base_dir);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
            NoUsbError::InvalidInput(format!("{} is unreadable: {error}", path.display()))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn save_state(base_dir: &Path, state: &mut HostBootState, event: &str) -> Result<(), NoUsbError> {
    state.updated_at = now_stamp();
    state.events.push(format!("{} {event}", state.updated_at));
    let dir = host_boot_dir(base_dir);
    fs::create_dir_all(&dir)?;
    let partial = dir.join("state.json.partial");
    {
        let mut file = fs::File::create(&partial)?;
        file.write_all(state.to_json().as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&partial, host_boot_state_path(base_dir))?;
    write_journal_entry(base_dir, &format!("host-boot {}: {event}", state.plan_id))?;
    Ok(())
}

/// Arms the one-shot boot. On any failure every completed step is undone before returning.
pub fn arm_host_one_shot(
    base_dir: &Path,
    plan: &HostBootPlan,
    backend: &mut dyn HostBootBackend,
    confirmation: &str,
) -> Result<HostBootState, NoUsbError> {
    arm_with_extractor(
        base_dir,
        plan,
        backend,
        confirmation,
        &|iso_file, target| super::extract_iso_file(&plan.iso_path, iso_file, target),
    )
}

/// Extracts one ISO file to a new target; `Ok(false)` when the ISO does not contain it.
type LoaderExtractor<'a> = &'a dyn Fn(&str, &Path) -> Result<bool, NoUsbError>;

fn arm_with_extractor(
    base_dir: &Path,
    plan: &HostBootPlan,
    backend: &mut dyn HostBootBackend,
    confirmation: &str,
    extract: LoaderExtractor<'_>,
) -> Result<HostBootState, NoUsbError> {
    if confirmation != plan.confirmation_phrase {
        return Err(NoUsbError::UnsafeExecution(format!(
            "typed confirmation must be exactly \"{}\"",
            plan.confirmation_phrase
        )));
    }
    if let Some(existing) = load_host_boot_state(base_dir)? {
        if existing.needs_cleanup() {
            return Err(NoUsbError::UnsafeExecution(format!(
                "{} is still {:?}; run `nousb boot cleanup` first",
                existing.plan_id, existing.phase
            )));
        }
    }
    if plan.target_kind != BootTargetKind::LinuxLive {
        return Err(NoUsbError::UnsafeExecution(
            "this target is plan-only; no build can arm it".into(),
        ));
    }
    if !plan.ready() {
        return Err(NoUsbError::UnsafeExecution(format!(
            "preflight blocked: {}",
            plan.blockers.join("; ")
        )));
    }
    // Re-check live facts with whatever elevation the caller now has.
    let facts = backend.facts(&plan.staging_volume_root)?;
    let (blockers, _) = host_preflight(
        &facts,
        plan.recipe.family,
        plan.iso_size_bytes,
        plan.execution_enabled,
    );
    if !blockers.is_empty() {
        return Err(NoUsbError::UnsafeExecution(format!(
            "preflight blocked: {}",
            blockers.join("; ")
        )));
    }

    let mut state = HostBootState {
        plan_id: plan.id.clone(),
        profile_id: plan.profile_id.clone(),
        backend: backend.name().into(),
        phase: HostBootPhase::Preparing,
        boot_order_before: vec![],
        entry_id: None,
        armed: false,
        esp_mount: None,
        created_files: vec![],
        created_dirs: vec![],
        esp_files: vec![],
        esp_dirs: vec![],
        backup_files: vec![],
        events: vec![],
        updated_at: now_stamp(),
    };
    save_state(base_dir, &mut state, "transaction started")?;

    match run_arm_steps(base_dir, plan, backend, &mut state, extract) {
        Ok(()) => Ok(state),
        Err(error) => {
            let undo = undo_transaction(base_dir, backend, &mut state);
            state.phase = match undo {
                Ok(()) => HostBootPhase::RolledBack,
                Err(_) => HostBootPhase::Failed,
            };
            let note = match &undo {
                Ok(()) => format!("failed and rolled back: {error}"),
                Err(undo_error) => {
                    format!("failed: {error}; rollback incomplete: {undo_error}")
                }
            };
            save_state(base_dir, &mut state, &note)?;
            Err(NoUsbError::UnsafeExecution(note))
        }
    }
}

fn run_arm_steps(
    base_dir: &Path,
    plan: &HostBootPlan,
    backend: &mut dyn HostBootBackend,
    state: &mut HostBootState,
    extract: LoaderExtractor<'_>,
) -> Result<(), NoUsbError> {
    let backup_dir = host_boot_dir(base_dir).join(&plan.id).join("backup");
    fs::create_dir_all(&backup_dir)?;
    state.backup_files = backend.backup_boot_state(&backup_dir)?;
    state.boot_order_before = backend.boot_order()?;
    save_state(base_dir, state, "boot state backed up")?;

    let mount = backend.mount_esp()?;
    state.esp_mount = Some(mount.clone());
    save_state(base_dir, state, &format!("ESP at {}", mount.path.display()))?;
    let needed: u64 = 64 * 1024 * 1024;
    if let Some(free) = backend.esp_free_bytes(&mount)? {
        if free < needed {
            return Err(NoUsbError::UnsafeExecution(format!(
                "ESP has {} MiB free; at least {} MiB is required",
                free / (1024 * 1024),
                needed / (1024 * 1024)
            )));
        }
    }
    let efi_dir = mount.path.join("EFI");
    if !efi_dir.is_dir() {
        return Err(NoUsbError::UnsafeExecution(format!(
            "{} has no EFI directory; this does not look like an EFI System Partition",
            mount.path.display()
        )));
    }
    let loader_dir_rel = format!("EFI/{HOST_BOOT_ESP_DIR}");
    let loader_dir = mount.path.join(relative_native(&loader_dir_rel));
    if loader_dir.exists() {
        return Err(NoUsbError::UnsafeExecution(format!(
            "{} already exists; refusing to overwrite (run `nousb boot cleanup` or inspect it)",
            loader_dir.display()
        )));
    }
    if plan.write_disk_info {
        for relative in [".disk/info", "boot/grub/grub.cfg"] {
            let target = mount.path.join(relative_native(relative));
            if target.exists() {
                return Err(NoUsbError::UnsafeExecution(format!(
                    "{} already exists on the ESP; refusing to overwrite it",
                    target.display()
                )));
            }
        }
    }

    // Unmount ESP right after checks so it does not stay mounted during the multi-GB copy.
    if mount.mounted_by_us {
        backend.unmount_esp(&mount)?;
    }
    state.esp_mount = None;
    save_state(base_dir, state, "ESP pre-checks passed")?;

    // Hash the multi-GiB source only after the cheap ESP checks have passed.
    let source_sha256 = source_digest(plan)?;
    save_state(
        base_dir,
        state,
        &format!(
            "source ISO hashed ({}): {source_sha256}",
            plan.iso_verification.as_str()
        ),
    )?;

    // Staging volume: NoUSB/<token>/<iso> plus the marker GRUB searches for.
    let staging_parent = plan.staging_volume_root.join(HOST_BOOT_STAGING_DIR);
    if !staging_parent.exists() {
        fs::create_dir(&staging_parent)?;
        state.created_dirs.push(staging_parent.clone());
    }
    let staging_dir = plan
        .staging_volume_root
        .join(relative_native(&plan.staging_relative_dir));
    fs::create_dir(&staging_dir)?;
    state.created_dirs.push(staging_dir.clone());
    save_state(base_dir, state, "staging directory created")?;

    let iso_copy = staging_dir.join(&plan.iso_filename);
    state.created_files.push(iso_copy.clone());
    save_state(base_dir, state, "copying ISO")?;
    copy_create_new(&plan.iso_path, &iso_copy)?;
    let copied = sha256_file(&iso_copy)?;
    if copied != source_sha256 {
        return Err(NoUsbError::Verification(format!(
            "ISO copy hash {copied} does not match the source's {source_sha256}"
        )));
    }
    let marker_name = plan
        .grub_config
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("search --no-floppy --set=nousb_root --file ")
        })
        .and_then(|path| path.rsplit('/').next())
        .ok_or_else(|| NoUsbError::InvalidInput("plan has no GRUB marker".into()))?
        .to_string();
    let marker = staging_dir.join(marker_name);
    write_create_new(&marker, plan.id.as_bytes())?;
    state.created_files.push(marker);
    save_state(base_dir, state, "ISO copied and verified")?;

    let mount = backend.mount_esp()?;
    state.esp_mount = Some(mount.clone());
    save_state(
        base_dir,
        state,
        &format!("ESP mounted at {}", mount.path.display()),
    )?;

    let loader_dir = mount.path.join(relative_native(&loader_dir_rel));
    fs::create_dir(&loader_dir)?;
    state.esp_dirs.push(loader_dir_rel.clone());
    save_state(base_dir, state, "ESP loader directory created")?;

    for (iso_file, name, required) in ISO_LOADERS {
        let relative = format!("{loader_dir_rel}/{name}");
        let target = mount.path.join(relative_native(&relative));
        state.esp_files.push(relative.clone());
        save_state(base_dir, state, &format!("writing {relative}"))?;
        let written = extract(iso_file, &target)?;
        if !written {
            state.esp_files.retain(|path| path != &relative);
            if required {
                return Err(NoUsbError::Verification(format!(
                    "ISO has no {iso_file}; it cannot be booted through shim"
                )));
            }
        }
    }
    let mut generated = vec![(
        format!("{loader_dir_rel}/grub.cfg"),
        plan.grub_config.clone(),
    )];
    if plan.write_disk_info {
        for dir in [".disk", "boot", "boot/grub"] {
            let path = mount.path.join(relative_native(dir));
            if !path.exists() {
                fs::create_dir(&path)?;
                state.esp_dirs.push(dir.into());
            }
        }
        generated.push((
            ".disk/info".into(),
            format!("NoUSB one-shot {} {}\n", plan.profile_name, plan.id),
        ));
        generated.push(("boot/grub/grub.cfg".into(), plan.grub_config.clone()));
    }
    for (relative, contents) in generated {
        let target = mount.path.join(relative_native(&relative));
        if target.exists() {
            return Err(NoUsbError::UnsafeExecution(format!(
                "{} already exists on the ESP; refusing to overwrite it",
                target.display()
            )));
        }
        state.esp_files.push(relative.clone());
        save_state(base_dir, state, &format!("writing {relative}"))?;
        write_create_new(&target, contents.as_bytes())?;
    }
    save_state(base_dir, state, "loader chain staged on ESP")?;

    let entry = backend.create_entry(&plan.entry_label, &plan.loader_path, &mount)?;
    state.entry_id = Some(entry.clone());
    save_state(base_dir, state, &format!("firmware entry {entry} created"))?;

    backend.arm_one_shot(&entry)?;
    state.armed = true;
    save_state(
        base_dir,
        state,
        &format!("one-shot selector set to {entry}"),
    )?;

    let order_after = backend.boot_order()?;
    if order_after != state.boot_order_before {
        return Err(NoUsbError::UnsafeExecution(format!(
            "persistent boot order changed from {:?} to {:?}",
            state.boot_order_before, order_after
        )));
    }
    match backend.pending_one_shot()? {
        Some(pending) if pending == entry => {}
        other => {
            return Err(NoUsbError::UnsafeExecution(format!(
                "one-shot selector reads back as {other:?}, expected {entry}"
            )))
        }
    }

    // The ESP stays mounted only as long as needed; files persist on disk.
    if mount.mounted_by_us {
        backend.unmount_esp(&mount)?;
        state.esp_mount = Some(EspMount {
            path: mount.path,
            mounted_by_us: false,
        });
    }
    state.phase = HostBootPhase::Armed;
    save_state(base_dir, state, "armed: reboot to start the installer once")?;
    Ok(())
}

/// Removes everything the transaction created. Idempotent: missing items are skipped.
pub fn cleanup_host_one_shot(
    base_dir: &Path,
    backend: &mut dyn HostBootBackend,
    confirmation: &str,
) -> Result<HostBootState, NoUsbError> {
    if confirmation != CLEANUP_CONFIRMATION_PHRASE {
        return Err(NoUsbError::UnsafeExecution(format!(
            "typed confirmation must be exactly \"{CLEANUP_CONFIRMATION_PHRASE}\""
        )));
    }
    let mut state = load_host_boot_state(base_dir)?.ok_or_else(|| {
        NoUsbError::InvalidInput("no NoUSB host boot transaction is recorded".into())
    })?;
    if !state.needs_cleanup() {
        return Ok(state);
    }
    undo_transaction(base_dir, backend, &mut state)?;
    state.phase = HostBootPhase::CleanedUp;
    save_state(base_dir, &mut state, "cleaned up")?;
    Ok(state)
}

fn undo_transaction(
    base_dir: &Path,
    backend: &mut dyn HostBootBackend,
    state: &mut HostBootState,
) -> Result<(), NoUsbError> {
    let mut errors = Vec::new();
    if let Some(entry) = state.entry_id.clone() {
        match backend.pending_one_shot() {
            Ok(Some(pending)) if pending == entry => {
                if let Err(error) = backend.clear_one_shot() {
                    errors.push(format!("clear one-shot: {error}"));
                }
            }
            Ok(_) => {}
            Err(error) => errors.push(format!("read one-shot: {error}")),
        }
        state.armed = false;
        match backend.delete_entry(&entry) {
            Ok(()) => state.entry_id = None,
            Err(error) => errors.push(format!("delete entry {entry}: {error}")),
        }
        save_state(base_dir, state, "firmware entry removed")?;
    }

    let saved_esp_mount = state.esp_mount.clone();
    let has_esp_items = !state.esp_files.is_empty() || !state.esp_dirs.is_empty();

    if has_esp_items {
        // After a crash, the mount the dead process made may still be live. Reuse it rather than
        // mounting the ESP a second time (Windows `mountvol /S` may refuse an already-lettered ESP).
        let reusable = saved_esp_mount
            .as_ref()
            .filter(|saved| saved.mounted_by_us && saved.path.join("EFI").is_dir())
            .cloned();
        let cleanup_mount = match reusable {
            Some(saved) => Ok(saved),
            None => backend.mount_esp(),
        };
        match cleanup_mount {
            Ok(cleanup_mount) => {
                for relative in state.esp_files.iter().rev() {
                    let path = cleanup_mount.path.join(relative_native(relative));
                    if let Err(error) = remove_file_if_present(&path) {
                        errors.push(format!("remove {}: {error}", path.display()));
                    }
                }
                for relative in state.esp_dirs.iter().rev() {
                    let path = cleanup_mount.path.join(relative_native(relative));
                    if let Err(error) = remove_dir_if_empty(&path) {
                        errors.push(format!("remove {}: {error}", path.display()));
                    }
                }
                let mut unmount_ok = true;
                if cleanup_mount.mounted_by_us {
                    if let Err(error) = backend.unmount_esp(&cleanup_mount) {
                        errors.push(format!("unmount ESP: {error}"));
                        unmount_ok = false;
                    }
                }
                if let Some(ref saved) = saved_esp_mount {
                    if saved.mounted_by_us && saved.path != cleanup_mount.path {
                        if let Err(error) = backend.unmount_esp(saved) {
                            errors.push(format!(
                                "unmount saved ESP {}: {error}",
                                saved.path.display()
                            ));
                            unmount_ok = false;
                        }
                    }
                }
                if unmount_ok {
                    state.esp_mount = None;
                }
                if errors.is_empty() {
                    state.esp_files.clear();
                    state.esp_dirs.clear();
                }
            }
            Err(error) => {
                errors.push(format!("mount ESP for cleanup: {error}"));
                if let Some(ref saved) = saved_esp_mount {
                    if saved.mounted_by_us {
                        if let Err(unmount_error) = backend.unmount_esp(saved) {
                            errors.push(format!(
                                "unmount saved ESP {}: {unmount_error}",
                                saved.path.display()
                            ));
                        } else {
                            state.esp_mount = None;
                        }
                    }
                }
            }
        }
        save_state(base_dir, state, "ESP files removed")?;
    } else if let Some(ref saved) = saved_esp_mount {
        if saved.mounted_by_us {
            if let Err(error) = backend.unmount_esp(saved) {
                errors.push(format!("unmount ESP {}: {error}", saved.path.display()));
            } else {
                state.esp_mount = None;
            }
            save_state(base_dir, state, "ESP unmounted")?;
        }
    }

    for path in state.created_files.iter().rev() {
        if let Err(error) = remove_file_if_present(path) {
            errors.push(format!("remove {}: {error}", path.display()));
        }
    }
    for path in state.created_dirs.iter().rev() {
        if let Err(error) = remove_dir_if_empty(path) {
            errors.push(format!("remove {}: {error}", path.display()));
        }
    }
    if errors.is_empty() {
        state.created_files.clear();
        state.created_dirs.clear();
        Ok(())
    } else {
        Err(NoUsbError::Io(errors.join("; ")))
    }
}

/// Hashes the source ISO. With an expected digest (pinned or user-supplied) a mismatch fails;
/// without one the digest only anchors the staged copy to this exact file.
fn source_digest(plan: &HostBootPlan) -> Result<String, NoUsbError> {
    if plan.iso_sha256.is_empty() {
        if plan.iso_verification != IsoVerification::Unverified {
            return Err(NoUsbError::Verification(
                "plan carries no SHA-256 for a verified ISO".into(),
            ));
        }
        let actual_size = fs::metadata(&plan.iso_path)?.len();
        if actual_size != plan.iso_size_bytes {
            return Err(NoUsbError::Verification(format!(
                "{} changed size since planning ({actual_size} bytes, planned {})",
                plan.iso_path.display(),
                plan.iso_size_bytes
            )));
        }
        return sha256_file(&plan.iso_path);
    }
    verify_file(&plan.iso_path, plan.iso_size_bytes, &plan.iso_sha256)?;
    Ok(plan.iso_sha256.to_ascii_lowercase())
}

fn verify_file(path: &Path, size_bytes: u64, sha256: &str) -> Result<(), NoUsbError> {
    let actual_size = fs::metadata(path)?.len();
    if actual_size != size_bytes {
        return Err(NoUsbError::Verification(format!(
            "{} is {actual_size} bytes; expected {size_bytes}",
            path.display()
        )));
    }
    let actual = sha256_file(path)?;
    if !actual.eq_ignore_ascii_case(sha256) {
        return Err(NoUsbError::Verification(format!(
            "{} has SHA-256 {actual}; expected {sha256}",
            path.display()
        )));
    }
    Ok(())
}

fn relative_native(relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn remove_dir_if_empty(path: &Path) -> std::io::Result<()> {
    match fs::read_dir(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(mut entries) => {
            if entries.next().is_none() {
                fs::remove_dir(path)
            } else {
                Ok(())
            }
        }
    }
}

fn copy_create_new(source: &Path, destination: &Path) -> Result<(), NoUsbError> {
    let mut input = fs::File::open(source)?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}

fn write_create_new(path: &Path, contents: &[u8]) -> Result<(), NoUsbError> {
    let mut output = OpenOptions::new().create_new(true).write(true).open(path)?;
    output.write_all(contents)?;
    output.sync_all()?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs the platform boot tools (`bcdedit`, `mountvol`, `efibootmgr`, ...). Real backends use
/// [`SystemCommandRunner`]; tests use [`ScriptedCommandRunner`] so no tool ever runs.
pub trait CommandRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandOutput, NoUsbError>;

    /// Runs and fails unless the tool reports success.
    fn run_checked(&mut self, program: &str, args: &[&str]) -> Result<String, NoUsbError> {
        let output = self.run(program, args)?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(NoUsbError::Io(format!(
                "{program} {} failed: {}{}",
                args.join(" "),
                output.stderr.trim(),
                output.stdout.trim()
            )))
        }
    }
}

pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandOutput, NoUsbError> {
        let output = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| NoUsbError::Io(format!("cannot run {program}: {error}")))?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

type CommandHandler = Box<dyn FnMut(&str) -> CommandOutput>;

/// Answers each command line (`program arg arg`) from a handler and records every call.
pub struct ScriptedCommandRunner {
    pub calls: Vec<String>,
    handler: CommandHandler,
}

impl ScriptedCommandRunner {
    pub fn new(handler: impl FnMut(&str) -> CommandOutput + 'static) -> Self {
        Self {
            calls: Vec::new(),
            handler: Box::new(handler),
        }
    }

    pub fn ok(stdout: &str) -> CommandOutput {
        CommandOutput {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }
}

impl CommandRunner for ScriptedCommandRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandOutput, NoUsbError> {
        let line = std::iter::once(program)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        self.calls.push(line.clone());
        Ok((self.handler)(&line))
    }
}

/// In-memory backend for tests and rehearsals: the "ESP" is a directory and firmware
/// variables are plain fields. Failures can be injected per operation.
#[derive(Clone, Debug)]
pub struct FakeHostBootBackend {
    pub facts: HostBootFacts,
    pub esp_dir: PathBuf,
    pub boot_order: Vec<String>,
    pub entries: Vec<(String, String)>,
    pub boot_next: Option<String>,
    pub fail_on: Option<String>,
    pub mounted: bool,
    pub mounted_paths: Vec<PathBuf>,
}

impl FakeHostBootBackend {
    pub fn new(facts: HostBootFacts, esp_dir: PathBuf) -> Self {
        Self {
            facts,
            esp_dir,
            boot_order: vec!["0000".into(), "0001".into()],
            entries: vec![
                ("0000".into(), "Current OS".into()),
                ("0001".into(), "Network".into()),
            ],
            boot_next: None,
            fail_on: None,
            mounted: false,
            mounted_paths: Vec::new(),
        }
    }

    fn check(&self, operation: &str) -> Result<(), NoUsbError> {
        if self.fail_on.as_deref() == Some(operation) {
            Err(NoUsbError::Io(format!("injected failure in {operation}")))
        } else {
            Ok(())
        }
    }
}

impl HostBootBackend for FakeHostBootBackend {
    fn name(&self) -> &'static str {
        "fake-host-boot"
    }

    fn facts(&mut self, _staging_root: &Path) -> Result<HostBootFacts, NoUsbError> {
        self.check("facts")?;
        Ok(self.facts.clone())
    }

    fn backup_boot_state(&mut self, dir: &Path) -> Result<Vec<PathBuf>, NoUsbError> {
        self.check("backup")?;
        let path = dir.join("firmware-entries.txt");
        fs::write(
            &path,
            format!("{:?}\n{:?}\n", self.boot_order, self.entries),
        )?;
        Ok(vec![path])
    }

    fn mount_esp(&mut self) -> Result<EspMount, NoUsbError> {
        self.check("mount")?;
        if !self.mounted_paths.contains(&self.esp_dir) {
            self.mounted_paths.push(self.esp_dir.clone());
        }
        self.mounted = !self.mounted_paths.is_empty();
        Ok(EspMount {
            path: self.esp_dir.clone(),
            mounted_by_us: true,
        })
    }

    fn unmount_esp(&mut self, mount: &EspMount) -> Result<(), NoUsbError> {
        self.check("unmount")?;
        if !mount.mounted_by_us {
            return Ok(());
        }
        if let Some(pos) = self.mounted_paths.iter().position(|p| p == &mount.path) {
            self.mounted_paths.remove(pos);
        }
        self.mounted = !self.mounted_paths.is_empty();
        Ok(())
    }

    fn esp_free_bytes(&mut self, _mount: &EspMount) -> Result<Option<u64>, NoUsbError> {
        Ok(Some(512 * 1024 * 1024))
    }

    fn boot_order(&mut self) -> Result<Vec<String>, NoUsbError> {
        Ok(self.boot_order.clone())
    }

    fn create_entry(
        &mut self,
        label: &str,
        _loader: &str,
        _mount: &EspMount,
    ) -> Result<String, NoUsbError> {
        self.check("create")?;
        let id = format!("{:04X}", self.entries.len());
        self.entries.push((id.clone(), label.into()));
        Ok(id)
    }

    fn arm_one_shot(&mut self, entry: &str) -> Result<(), NoUsbError> {
        self.check("arm")?;
        self.boot_next = Some(entry.into());
        Ok(())
    }

    fn pending_one_shot(&mut self) -> Result<Option<String>, NoUsbError> {
        Ok(self.boot_next.clone())
    }

    fn clear_one_shot(&mut self) -> Result<(), NoUsbError> {
        self.boot_next = None;
        Ok(())
    }

    fn delete_entry(&mut self, entry: &str) -> Result<(), NoUsbError> {
        self.check("delete")?;
        self.entries.retain(|(id, _)| id != entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UBUNTU_CFG: &str = "set timeout=30\nloadfont unicode\nmenuentry \"Try or Install Ubuntu\" {\n\tset gfxpayload=keep\n\tlinux\t/casper/vmlinuz  --- quiet splash\n\tinitrd\t/casper/initrd\n}\nmenuentry \"Ubuntu (safe graphics)\" {\n\tlinux /casper/vmlinuz nomodeset ---\n\tinitrd /casper/initrd\n}\n";
    const FEDORA_CFG: &str = "set default=\"1\"\nsearch --no-floppy --set=root -l 'Fedora-WS-Live-44-1-7'\nmenuentry 'Start Fedora-Workstation-Live 44' --class fedora {\n\tlinux ($root)/images/pxeboot/vmlinuz root=live:CDLABEL=Fedora-WS-Live-44-1-7 rd.live.image quiet rhgb\n\tinitrd ($root)/images/pxeboot/initrd.img\n}\nmenuentry 'Test this media & start' {\n\tlinux /images/pxeboot/vmlinuz root=live:CDLABEL=Fedora-WS-Live-44-1-7 rd.live.image rd.live.check quiet\n\tinitrd /images/pxeboot/initrd.img\n}\n";

    /// Writes a minimal ISO9660 image. Every directory and every file takes one sector.
    fn write_test_iso(path: &Path, files: &[(&str, &[u8])]) {
        const SECTOR: usize = 2048;
        fn both_u32(target: &mut [u8], value: u32) {
            target[..4].copy_from_slice(&value.to_le_bytes());
            target[4..8].copy_from_slice(&value.to_be_bytes());
        }
        fn both_u16(target: &mut [u8], value: u16) {
            target[..2].copy_from_slice(&value.to_le_bytes());
            target[2..4].copy_from_slice(&value.to_be_bytes());
        }
        fn record(lba: usize, size: usize, directory: bool, name: &[u8]) -> Vec<u8> {
            let length = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
            let mut record = vec![0_u8; length];
            record[0] = length as u8;
            both_u32(&mut record[2..10], lba as u32);
            both_u32(&mut record[10..18], size as u32);
            record[18..25].copy_from_slice(&[126, 1, 1, 0, 0, 0, 0]);
            record[25] = if directory { 2 } else { 0 };
            both_u16(&mut record[28..32], 1);
            record[32] = name.len() as u8;
            record[33..33 + name.len()].copy_from_slice(name);
            record
        }

        // Directory paths ("" is the root), parents before children.
        let mut dirs = vec![String::new()];
        for (file, _) in files {
            let parts: Vec<&str> = file.trim_start_matches('/').split('/').collect();
            for depth in 1..parts.len() {
                let dir = parts[..depth].join("/");
                if !dirs.contains(&dir) {
                    dirs.push(dir);
                }
            }
        }
        let first = 20;
        let dir_lba = |dir: &str| first + dirs.iter().position(|known| known == dir).unwrap();
        let file_lba = |index: usize| first + dirs.len() + index;
        let parent_of = |path: &str| match path.rsplit_once('/') {
            Some((parent, _)) => parent.to_string(),
            None => String::new(),
        };
        let leaf_of = |path: &str| path.rsplit('/').next().unwrap().to_ascii_uppercase();
        let mut image = vec![0_u8; SECTOR * (first + dirs.len() + files.len())];

        let pvd = 16 * SECTOR;
        image[pvd] = 1;
        image[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        image[pvd + 6] = 1;
        image[pvd + 40..pvd + 50].copy_from_slice(b"NOUSB_TEST");
        let sectors = (image.len() / SECTOR) as u32;
        both_u32(&mut image[pvd + 80..pvd + 88], sectors);
        both_u16(&mut image[pvd + 120..pvd + 124], 1);
        both_u16(&mut image[pvd + 124..pvd + 128], 1);
        both_u16(&mut image[pvd + 128..pvd + 132], SECTOR as u16);
        let root = record(first, SECTOR, true, &[0]);
        image[pvd + 156..pvd + 190].copy_from_slice(&root);
        image[pvd + 881] = 1;
        let terminator = 17 * SECTOR;
        image[terminator] = 255;
        image[terminator + 1..terminator + 6].copy_from_slice(b"CD001");
        image[terminator + 6] = 1;

        for dir in &dirs {
            let mut records = vec![
                record(dir_lba(dir), SECTOR, true, &[0]),
                record(dir_lba(&parent_of(dir)), SECTOR, true, &[1]),
            ];
            for child in dirs.iter().filter(|child| !child.is_empty()) {
                if &parent_of(child) == dir {
                    records.push(record(
                        dir_lba(child),
                        SECTOR,
                        true,
                        leaf_of(child).as_bytes(),
                    ));
                }
            }
            for (index, (file, contents)) in files.iter().enumerate() {
                let file = file.trim_start_matches('/');
                if &parent_of(file) == dir {
                    let name = format!("{};1", leaf_of(file));
                    records.push(record(
                        file_lba(index),
                        contents.len(),
                        false,
                        name.as_bytes(),
                    ));
                }
            }
            let mut cursor = dir_lba(dir) * SECTOR;
            for entry in records {
                image[cursor..cursor + entry.len()].copy_from_slice(&entry);
                cursor += entry.len();
            }
        }
        for (index, (_, contents)) in files.iter().enumerate() {
            let offset = file_lba(index) * SECTOR;
            image[offset..offset + contents.len()].copy_from_slice(contents);
        }
        fs::write(path, image).unwrap();
    }

    fn write_live_iso(path: &Path, grub_cfg: &str) {
        write_test_iso(
            path,
            &[
                ("/boot/grub/grub.cfg", grub_cfg.as_bytes()),
                ("/EFI/BOOT/BOOTX64.EFI", b"signed shim"),
                ("/EFI/BOOT/grubx64.efi", b"signed grub"),
                ("/casper/vmlinuz", b"kernel"),
                ("/casper/initrd", b"initrd"),
            ],
        );
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nousb-hb-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn custom_iso_is_planned_from_its_own_grub_config_and_armed() {
        let dir = scratch("custom");
        let iso = dir.join("My Distro 1.0.iso");
        write_live_iso(&iso, UBUNTU_CFG);
        let plan = plan_host_one_shot_custom(&iso, None, &facts(&dir), "fake", true).unwrap();
        assert!(plan.ready(), "{:?}", plan.blockers);
        assert_eq!(plan.profile_id, CUSTOM_PROFILE_ID);
        assert_eq!(plan.confirmation_phrase, "BOOT ONCE INTO custom");
        assert_eq!(plan.iso_verification, IsoVerification::Unverified);
        assert_eq!(plan.target_kind, BootTargetKind::LinuxLive);
        assert_eq!(plan.recipe.family, LiveBootFamily::Casper);
        // A name with spaces cannot go into a GRUB command line unquoted.
        assert_eq!(plan.iso_filename, "custom-live.iso");
        assert!(plan.grub_config.contains("/custom-live.iso toram") || !plan.ram_copy);
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("origin is unverified")));
        assert!(plan.preview().contains("--iso"));

        let esp = dir.join("esp");
        fs::create_dir_all(esp.join("EFI")).unwrap();
        let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());
        let state =
            arm_host_one_shot(&dir, &plan, &mut backend, &plan.confirmation_phrase).unwrap();
        assert_eq!(state.phase, HostBootPhase::Armed);
        assert!(state
            .events
            .iter()
            .any(|event| event.contains(&sha256_file(&iso).unwrap())));
        // The loaders on the ESP are the ISO's own files, byte for byte.
        let loader = esp.join("EFI").join("NoUSB");
        assert_eq!(
            fs::read(loader.join("BOOTX64.EFI")).unwrap(),
            b"signed shim"
        );
        assert_eq!(
            fs::read(loader.join("grubx64.efi")).unwrap(),
            b"signed grub"
        );
        let staged = dir
            .join(relative_native(&plan.staging_relative_dir))
            .join("custom-live.iso");
        assert_eq!(fs::read(&staged).unwrap(), fs::read(&iso).unwrap());
        assert_eq!(backend.boot_order, vec!["0000", "0001"]);

        let cleaned =
            cleanup_host_one_shot(&dir, &mut backend, CLEANUP_CONFIRMATION_PHRASE).unwrap();
        assert_eq!(cleaned.phase, HostBootPhase::CleanedUp);
        assert!(!staged.exists() && !loader.exists());
        assert!(backend.boot_next.is_none() && backend.entries.len() == 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_iso_with_a_supplied_sha256_must_match_it() {
        let dir = scratch("custom-sha");
        let iso = dir.join("distro-live.iso");
        write_live_iso(&iso, UBUNTU_CFG);
        assert!(plan_host_one_shot_custom(&iso, Some("abc"), &facts(&dir), "fake", true).is_err());

        let right = sha256_file(&iso).unwrap().to_ascii_uppercase();
        let plan =
            plan_host_one_shot_custom(&iso, Some(&right), &facts(&dir), "fake", true).unwrap();
        assert_eq!(plan.iso_verification, IsoVerification::UserSha256);
        assert_eq!(plan.iso_filename, "distro-live.iso");
        assert_eq!(source_digest(&plan).unwrap(), right.to_ascii_lowercase());

        let wrong = "0".repeat(64);
        let plan =
            plan_host_one_shot_custom(&iso, Some(&wrong), &facts(&dir), "fake", true).unwrap();
        let esp = dir.join("esp");
        fs::create_dir_all(esp.join("EFI")).unwrap();
        let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());
        let error =
            arm_host_one_shot(&dir, &plan, &mut backend, &plan.confirmation_phrase).unwrap_err();
        assert!(error.to_string().contains("rolled back"), "{error}");
        assert!(error.to_string().contains("expected"), "{error}");
        assert!(!dir.join("NoUSB").exists());
        assert!(!esp.join("EFI").join("NoUSB").exists());
        assert!(backend.boot_next.is_none() && backend.entries.len() == 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_iso_that_cannot_be_loop_booted_is_blocked_not_guessed() {
        let dir = scratch("custom-bad");
        let other = dir.join("other.iso");
        write_live_iso(
            &other,
            "menuentry x {\n linux /live/vmlinuz boot=live findiso=${iso_path}\n initrd /live/initrd\n}\n",
        );
        let plan = plan_host_one_shot_custom(&other, None, &facts(&dir), "fake", true).unwrap();
        assert!(!plan.ready());
        assert!(plan
            .blockers
            .iter()
            .any(|blocker| blocker.contains("Only casper")));

        let bare = dir.join("bare.iso");
        write_test_iso(&bare, &[("/boot/grub/grub.cfg", UBUNTU_CFG.as_bytes())]);
        let plan = plan_host_one_shot_custom(&bare, None, &facts(&dir), "fake", true).unwrap();
        let text = plan.blockers.join("\n");
        assert!(text.contains("no /EFI/BOOT/BOOTX64.EFI"), "{text}");
        assert!(text.contains("no /EFI/BOOT/grubx64.efi"), "{text}");

        let missing =
            plan_host_one_shot_custom(&dir.join("nope.iso"), None, &facts(&dir), "fake", true)
                .unwrap();
        assert!(missing.blockers[0].contains("custom ISO not found"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_requests_are_routed_by_profile() {
        let dir = scratch("route");
        let host = facts(&dir);
        let custom = plan_host_boot(&dir, "custom", None, None, &host, "fake", true).unwrap_err();
        assert!(custom.to_string().contains("--iso"));
        let pinned = plan_host_boot(
            &dir,
            "ubuntu-24.04-server",
            None,
            Some(&"0".repeat(64)),
            &host,
            "fake",
            true,
        )
        .unwrap_err();
        assert!(pinned.to_string().contains("custom profile only"));
        let cached =
            plan_host_boot(&dir, "ubuntu-24.04-server", None, None, &host, "fake", true).unwrap();
        assert!(cached.blockers[0].contains(".nousb"));
        assert!(cached.blockers[0].contains("--iso"));
        assert!(plan_host_boot(&dir, "no-such", None, None, &host, "fake", true).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn windows_setup_is_plan_only_and_can_never_be_armed() {
        let dir = scratch("windows");
        let iso = dir.join("Win11.iso");
        let mut image = vec![0_u8; 2048 * 24];
        for (sector, kind, id) in [
            (16, 1_u8, b"CD001"),
            (17, 255, b"CD001"),
            (18, 0, b"BEA01"),
            (19, 0, b"NSR03"),
            (20, 0, b"TEA01"),
        ] {
            image[sector * 2048] = kind;
            image[sector * 2048 + 1..sector * 2048 + 6].copy_from_slice(id);
        }
        image[16 * 2048 + 40..16 * 2048 + 51].copy_from_slice(b"CCCOMA_X64F");
        fs::write(&iso, &image).unwrap();
        let found = inspect_windows_iso(&iso).unwrap();
        assert!(found.udf);
        assert_eq!(found.volume_id, "CCCOMA_X64F");

        // ext4 host: Windows Setup could not read the staging volume.
        let plan = plan_host_boot(
            &dir,
            "windows-11",
            Some(iso.as_path()),
            None,
            &facts(&dir),
            "fake",
            true,
        )
        .unwrap();
        assert_eq!(plan.target_kind, BootTargetKind::WindowsSetup);
        assert!(!plan.ready() && !plan.execution_enabled && plan.files.is_empty());
        let text = plan.blockers.join("\n");
        assert!(text.contains("plan-only"), "{text}");
        assert!(
            text.contains("ext4 cannot be read by Windows Setup"),
            "{text}"
        );
        assert!(plan.preview().contains("Arming is refused"));
        serde_json::from_str::<serde_json::Value>(&plan.to_json()).unwrap();

        // Even with every blocker stripped, arm refuses the target kind.
        let mut forced = plan.clone();
        forced.blockers.clear();
        let mut backend = FakeHostBootBackend::new(facts(&dir), dir.join("esp"));
        let error = arm_host_one_shot(&dir, &forced, &mut backend, &forced.confirmation_phrase)
            .unwrap_err();
        assert!(error.to_string().contains("plan-only"));
        assert!(backend.boot_next.is_none() && backend.entries.len() == 2);

        let none = plan_windows_setup_target(None, &facts(&dir), "fake");
        assert!(none
            .blockers
            .iter()
            .any(|b| b.contains("no Windows ISO given")));
        let linux_iso = dir.join("linux.iso");
        write_live_iso(&linux_iso, UBUNTU_CFG);
        let not_windows =
            plan_windows_setup_target(Some(linux_iso.as_path()), &facts(&dir), "fake");
        assert!(not_windows
            .blockers
            .iter()
            .any(|b| b.contains("no UDF file system")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn derives_casper_recipe_from_the_first_entry() {
        let recipe = derive_live_boot_recipe(UBUNTU_CFG, "/boot/grub/grub.cfg").unwrap();
        assert_eq!(recipe.family, LiveBootFamily::Casper);
        assert_eq!(recipe.kernel, "/casper/vmlinuz");
        assert_eq!(recipe.initrds, vec!["/casper/initrd"]);
        assert_eq!(recipe.kernel_args, vec!["---", "quiet", "splash"]);
    }

    #[test]
    fn derives_dracut_recipe_and_strips_root_prefix() {
        let recipe = derive_live_boot_recipe(FEDORA_CFG, "/EFI/BOOT/grub.cfg").unwrap();
        assert_eq!(recipe.family, LiveBootFamily::Dracut);
        assert_eq!(recipe.kernel, "/images/pxeboot/vmlinuz");
        assert_eq!(recipe.initrds, vec!["/images/pxeboot/initrd.img"]);
        assert!(recipe
            .kernel_args
            .contains(&"root=live:CDLABEL=Fedora-WS-Live-44-1-7".to_string()));
    }

    #[test]
    fn refuses_variables_unknown_families_and_traversal() {
        let variable = "menuentry x {\n linux /vmlinuz root=$foo\n initrd /initrd\n}\n";
        assert!(derive_live_boot_recipe(variable, "x").is_err());
        let unknown = "menuentry x {\n linux /vmlinuz quiet\n initrd /initrd\n}\n";
        assert!(derive_live_boot_recipe(unknown, "x").is_err());
        let traversal =
            "menuentry x {\n linux /casper/../../vmlinuz boot=casper\n initrd /initrd\n}\n";
        assert!(derive_live_boot_recipe(traversal, "x").is_err());
        assert!(derive_live_boot_recipe("set timeout=1\n", "x").is_err());
    }

    #[test]
    fn grub_config_inserts_iso_scan_before_the_separator() {
        let recipe = derive_live_boot_recipe(UBUNTU_CFG, "/boot/grub/grub.cfg").unwrap();
        let config = render_grub_config(
            "Ubuntu Desktop 24.04.4",
            &recipe,
            "/NoUSB/t/u.iso",
            "/NoUSB/t/m.marker",
            true,
        );
        assert!(config.contains(
            "linux (loop)/casper/vmlinuz iso-scan/filename=/NoUSB/t/u.iso toram --- quiet splash"
        ));
        assert!(config.contains("search --no-floppy --set=nousb_root --file /NoUSB/t/m.marker"));
        assert!(config.contains("loopback loop ($nousb_root)/NoUSB/t/u.iso"));
        assert!(config.contains("initrd (loop)/casper/initrd"));

        let fedora = derive_live_boot_recipe(FEDORA_CFG, "/EFI/BOOT/grub.cfg").unwrap();
        let config = render_grub_config("Fedora", &fedora, "/a.iso", "/m", false);
        assert!(config.contains("rd.live.image quiet rhgb iso-scan/filename=/a.iso\n"));
        assert!(!config.contains("rd.live.ram"));
    }

    fn facts(root: &Path) -> HostBootFacts {
        HostBootFacts {
            os_name: "linux".into(),
            elevated: true,
            firmware: FirmwareMode::Uefi,
            secure_boot: SecureBootState::Enabled,
            memory_total_mb: Some(4096),
            staging_volume_root: root.to_path_buf(),
            staging_grub_prefix: "/".into(),
            staging_filesystem: Some("ext4".into()),
            staging_volume_encrypted: Some(false),
            staging_free_bytes: Some(u64::MAX / 2),
            missing_tools: vec![],
        }
    }

    #[test]
    fn copy_step_joins_staging_root_without_trailing_separator() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-step-{}", std::process::id()));
        let mut host = facts(&dir);
        host.staging_volume_root = PathBuf::from("/boot");
        let plan = plan_host_one_shot(
            "ubuntu-24.04-desktop",
            &dir.join("missing.iso"),
            &host,
            "fake",
            true,
        )
        .unwrap();
        let step = plan
            .steps
            .iter()
            .find(|step| step.starts_with("Copy the ISO to"))
            .unwrap();
        assert!(
            step.starts_with(&format!(
                "Copy the ISO to {}",
                PathBuf::from("/boot").join("NoUSB").display()
            )),
            "{step}"
        );
    }

    #[test]
    fn missing_efibootmgr_blocks_the_plan() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-tools-{}", std::process::id()));
        let mut host = facts(&dir);
        host.missing_tools = vec!["efibootmgr".into()];
        let (blockers, _) = host_preflight(&host, LiveBootFamily::Casper, 1024, true);
        assert!(blockers
            .iter()
            .any(|blocker| blocker.contains("required tool efibootmgr is not installed")));
    }

    #[test]
    fn plan_reports_blockers_instead_of_waiving_them() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-plan-{}", std::process::id()));
        let mut host = facts(&dir);
        host.elevated = false;
        host.firmware = FirmwareMode::Bios;
        host.staging_volume_encrypted = Some(true);
        let plan = plan_host_one_shot(
            "ubuntu-24.04-desktop",
            &dir.join("missing.iso"),
            &host,
            "fake",
            false,
        )
        .unwrap();
        assert!(!plan.ready());
        let text = plan.blockers.join("\n");
        for needle in [
            "verified ISO not found",
            "no real boot backend",
            "UEFI firmware was not detected",
            "not elevated",
            "is encrypted",
        ] {
            assert!(text.contains(needle), "missing blocker {needle}: {text}");
        }
        assert_eq!(
            plan.confirmation_phrase,
            "BOOT ONCE INTO ubuntu-24.04-desktop"
        );
        assert!(plan.write_disk_info);
        assert!(!plan.ram_copy);
        serde_json::from_str::<serde_json::Value>(&plan.to_json()).unwrap();
    }

    #[test]
    fn fat32_staging_blocks_large_iso() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-fat32-{}", std::process::id()));
        let mut host = facts(&dir);
        host.staging_filesystem = Some("vfat".into());
        let plan = plan_host_one_shot(
            "ubuntu-24.04-desktop",
            &dir.join("missing.iso"),
            &host,
            "fake",
            true,
        )
        .unwrap();
        assert!(!plan.ready());
        assert!(plan
            .blockers
            .iter()
            .any(|blocker| blocker.contains("4 GiB single-file limit")));
        assert!(plan
            .blockers
            .iter()
            .any(|blocker| blocker.contains("this ISO is 6.2 GiB")));

        let (blockers, _) = host_preflight(
            &host,
            LiveBootFamily::Casper,
            (4.5 * 1024.0 * 1024.0 * 1024.0) as u64,
            true,
        );
        assert!(blockers.iter().any(|b| b.contains("this ISO is 4.5 GiB")));
    }

    #[test]
    fn arm_refuses_wrong_phrase_and_preflight_blockers() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-arm-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let plan = plan_host_one_shot(
            "ubuntu-24.04-desktop",
            &dir.join("missing.iso"),
            &facts(&dir),
            "fake",
            true,
        )
        .unwrap();
        let mut backend = FakeHostBootBackend::new(facts(&dir), dir.join("esp"));
        let wrong = arm_host_one_shot(&dir, &plan, &mut backend, "yes").unwrap_err();
        assert!(wrong.to_string().contains("typed confirmation"));
        let blocked =
            arm_host_one_shot(&dir, &plan, &mut backend, &plan.confirmation_phrase).unwrap_err();
        assert!(blocked.to_string().contains("preflight blocked"));
        assert!(backend.entries.len() == 2 && backend.boot_next.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A plan for a small stand-in ISO, so the full transaction runs without a 6 GiB download.
    fn fixture_plan(dir: &Path) -> HostBootPlan {
        let iso = dir.join("source.iso");
        fs::write(&iso, b"stand-in iso bytes").unwrap();
        let mut plan = plan_host_one_shot(
            "ubuntu-24.04-desktop",
            &dir.join("missing.iso"),
            &facts(dir),
            "fake",
            true,
        )
        .unwrap();
        plan.iso_path = iso.clone();
        plan.iso_filename = "ubuntu.iso".into();
        plan.iso_size_bytes = fs::metadata(&iso).unwrap().len();
        plan.iso_sha256 = sha256_file(&iso).unwrap();
        plan.recipe = derive_live_boot_recipe(UBUNTU_CFG, "/boot/grub/grub.cfg").unwrap();
        plan.blockers.clear();
        plan
    }

    fn fake_extract(iso_file: &str, target: &Path) -> Result<bool, NoUsbError> {
        if iso_file.ends_with("mmx64.efi") {
            return Ok(false);
        }
        write_create_new(target, iso_file.as_bytes())?;
        Ok(true)
    }

    #[test]
    fn arm_then_cleanup_runs_the_whole_transaction() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-e2e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let esp = dir.join("esp");
        fs::create_dir_all(esp.join("EFI").join("Microsoft")).unwrap();
        let plan = fixture_plan(&dir);
        let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());

        let state = arm_with_extractor(
            &dir,
            &plan,
            &mut backend,
            &plan.confirmation_phrase,
            &fake_extract,
        )
        .unwrap();
        assert_eq!(state.phase, HostBootPhase::Armed);
        let entry = state.entry_id.clone().unwrap();
        assert_eq!(backend.boot_next.as_deref(), Some(entry.as_str()));
        assert_eq!(backend.boot_order, vec!["0000", "0001"]);
        assert!(!backend.mounted);
        let loader = esp.join("EFI").join("NoUSB");
        assert!(loader.join("BOOTX64.EFI").exists());
        assert!(loader.join("grubx64.efi").exists());
        assert!(!loader.join("mmx64.efi").exists());
        let cfg = fs::read_to_string(loader.join("grub.cfg")).unwrap();
        assert!(cfg.contains("iso-scan/filename="));
        assert!(esp.join(".disk").join("info").exists());
        assert!(esp.join("boot").join("grub").join("grub.cfg").exists());
        let staged = dir.join(relative_native(&plan.staging_relative_dir));
        assert!(staged.join("ubuntu.iso").exists());
        assert_eq!(
            load_host_boot_state(&dir).unwrap().unwrap().phase,
            HostBootPhase::Armed
        );

        // A second arm is refused until cleanup.
        let again = arm_with_extractor(
            &dir,
            &plan,
            &mut backend,
            &plan.confirmation_phrase,
            &fake_extract,
        )
        .unwrap_err();
        assert!(again.to_string().contains("boot cleanup"));

        let cleaned =
            cleanup_host_one_shot(&dir, &mut backend, CLEANUP_CONFIRMATION_PHRASE).unwrap();
        assert_eq!(cleaned.phase, HostBootPhase::CleanedUp);
        assert!(backend.boot_next.is_none());
        assert_eq!(backend.entries.len(), 2);
        assert!(!loader.exists());
        assert!(!esp.join(".disk").exists());
        assert!(!esp.join("boot").exists());
        assert!(esp.join("EFI").join("Microsoft").exists());
        assert!(!dir.join("NoUSB").exists());
        assert!(dir.join("source.iso").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn failure_at_every_step_rolls_back_completely() {
        for failure in ["backup", "mount", "create", "arm"] {
            let dir = std::env::temp_dir()
                .join(format!("nousb-hb-fail-{failure}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            let esp = dir.join("esp");
            fs::create_dir_all(esp.join("EFI")).unwrap();
            let plan = fixture_plan(&dir);
            let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());
            backend.fail_on = Some(failure.into());
            let error = arm_with_extractor(
                &dir,
                &plan,
                &mut backend,
                &plan.confirmation_phrase,
                &fake_extract,
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("rolled back"),
                "{failure}: {error}"
            );
            let state = load_host_boot_state(&dir).unwrap().unwrap();
            assert_eq!(state.phase, HostBootPhase::RolledBack, "{failure}");
            assert!(backend.boot_next.is_none(), "{failure}");
            assert_eq!(backend.entries.len(), 2, "{failure}");
            assert!(
                !backend.mounted,
                "{failure}: ESP was not unmounted on rollback"
            );
            assert!(!esp.join("EFI").join("NoUSB").exists(), "{failure}");
            assert!(!dir.join("NoUSB").exists(), "{failure}");
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn existing_esp_directory_is_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-exists-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let esp = dir.join("esp");
        fs::create_dir_all(esp.join("EFI").join("NoUSB")).unwrap();
        fs::write(esp.join("EFI").join("NoUSB").join("keep.txt"), "user file").unwrap();
        let plan = fixture_plan(&dir);
        let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());
        let error = arm_with_extractor(
            &dir,
            &plan,
            &mut backend,
            &plan.confirmation_phrase,
            &fake_extract,
        )
        .unwrap_err();
        assert!(error.to_string().contains("refusing to overwrite"));
        assert_eq!(
            fs::read_to_string(esp.join("EFI").join("NoUSB").join("keep.txt")).unwrap(),
            "user file"
        );
        assert!(backend.boot_next.is_none());
        assert!(
            !backend.mounted,
            "ESP was not unmounted when loader dir already existed"
        );
        assert!(
            !dir.join("NoUSB").exists(),
            "staging directory should not be created if ESP check fails fast"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_removes_entry_selector_and_files_idempotently() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-undo-{}", std::process::id()));
        let esp = dir.join("esp");
        fs::create_dir_all(esp.join("EFI").join("NoUSB")).unwrap();
        fs::write(esp.join("EFI").join("NoUSB").join("grub.cfg"), "x").unwrap();
        let staged = dir.join("NoUSB");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("u.iso"), "iso").unwrap();
        let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());
        let entry = backend
            .create_entry(
                HOST_BOOT_ENTRY_LABEL,
                "\\EFI\\NoUSB\\BOOTX64.EFI",
                &EspMount {
                    path: esp.clone(),
                    mounted_by_us: true,
                },
            )
            .unwrap();
        backend.arm_one_shot(&entry).unwrap();
        let mut state = HostBootState {
            plan_id: "p".into(),
            profile_id: "ubuntu-24.04-desktop".into(),
            backend: "fake".into(),
            phase: HostBootPhase::Armed,
            boot_order_before: backend.boot_order.clone(),
            entry_id: Some(entry),
            armed: true,
            esp_mount: None,
            created_files: vec![staged.join("u.iso")],
            created_dirs: vec![staged.clone()],
            esp_files: vec!["EFI/NoUSB/grub.cfg".into()],
            esp_dirs: vec!["EFI/NoUSB".into()],
            backup_files: vec![],
            events: vec![],
            updated_at: 0,
        };
        save_state(&dir, &mut state, "test armed").unwrap();
        assert!(cleanup_host_one_shot(&dir, &mut backend, "cleanup").is_err());
        let cleaned =
            cleanup_host_one_shot(&dir, &mut backend, CLEANUP_CONFIRMATION_PHRASE).unwrap();
        assert_eq!(cleaned.phase, HostBootPhase::CleanedUp);
        assert!(backend.boot_next.is_none());
        assert_eq!(backend.entries.len(), 2);
        assert!(!esp.join("EFI").join("NoUSB").exists());
        assert!(esp.join("EFI").exists());
        assert!(!staged.exists());
        assert_eq!(backend.boot_order, vec!["0000", "0001"]);
        // Second run is a no-op.
        cleanup_host_one_shot(&dir, &mut backend, CLEANUP_CONFIRMATION_PHRASE).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_disk_info_and_grub_cfg_fail_fast_before_staging() {
        for (rel_dir, file_name) in [(".disk", "info"), ("boot/grub", "grub.cfg")] {
            let dir = std::env::temp_dir().join(format!(
                "nousb-hb-collision-{}-{}-{}",
                rel_dir.replace('/', "-"),
                file_name,
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            let esp = dir.join("esp");
            let target_dir = esp.join(relative_native(rel_dir));
            fs::create_dir_all(&target_dir).unwrap();
            fs::create_dir_all(esp.join("EFI")).unwrap();
            let target_file = target_dir.join(file_name);
            fs::write(&target_file, "existing file content").unwrap();

            let plan = fixture_plan(&dir);
            assert!(plan.write_disk_info);
            let mut backend = FakeHostBootBackend::new(facts(&dir), esp.clone());
            let error = arm_with_extractor(
                &dir,
                &plan,
                &mut backend,
                &plan.confirmation_phrase,
                &fake_extract,
            )
            .unwrap_err();

            assert!(
                error.to_string().contains("already exists on the ESP"),
                "expected collision error, got: {error}"
            );
            assert_eq!(
                fs::read_to_string(&target_file).unwrap(),
                "existing file content",
                "existing file was overwritten"
            );
            assert!(
                !backend.mounted,
                "ESP was not unmounted when collision occurred"
            );
            assert!(
                !dir.join("NoUSB").exists(),
                "staging directory should not be created if ESP check fails fast"
            );
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn cleanup_after_crash_unmounts_saved_mount_path_in_fresh_backend() {
        let dir = std::env::temp_dir().join(format!("nousb-hb-crash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let esp_x = dir.join("esp-x");
        let esp_y = dir.join("esp-y");
        fs::create_dir_all(esp_x.join("EFI").join("NoUSB")).unwrap();
        fs::write(esp_x.join("EFI").join("NoUSB").join("grub.cfg"), "grub").unwrap();
        fs::create_dir_all(esp_y.join("EFI").join("NoUSB")).unwrap();
        fs::write(esp_y.join("EFI").join("NoUSB").join("grub.cfg"), "grub").unwrap();

        // Process 1 crashed while ESP was mounted at esp_x
        let mut state = HostBootState {
            plan_id: "crash-test".into(),
            profile_id: "ubuntu-24.04-desktop".into(),
            backend: "fake".into(),
            phase: HostBootPhase::Preparing,
            boot_order_before: vec!["0000".into(), "0001".into()],
            entry_id: None,
            armed: false,
            esp_mount: Some(EspMount {
                path: esp_x.clone(),
                mounted_by_us: true,
            }),
            created_files: vec![],
            created_dirs: vec![],
            esp_files: vec!["EFI/NoUSB/grub.cfg".into()],
            esp_dirs: vec!["EFI/NoUSB".into()],
            backup_files: vec![],
            events: vec![],
            updated_at: 0,
        };
        save_state(&dir, &mut state, "process 1 crashed").unwrap();

        // Process 2 restarts with a fresh backend; esp_x was left mounted by the crashed process
        let mut backend2 = FakeHostBootBackend::new(facts(&dir), esp_y.clone());
        backend2.mounted_paths = vec![esp_x.clone()];
        backend2.mounted = true;

        let cleaned =
            cleanup_host_one_shot(&dir, &mut backend2, CLEANUP_CONFIRMATION_PHRASE).unwrap();
        assert_eq!(cleaned.phase, HostBootPhase::CleanedUp);
        assert!(
            backend2.mounted_paths.is_empty(),
            "both the new mount and the saved mount from before the crash should be unmounted: {:?}",
            backend2.mounted_paths
        );
        assert!(!backend2.mounted);
        assert!(
            !esp_x.join("EFI").join("NoUSB").exists(),
            "cleanup should reuse the still-live saved mount instead of mounting again"
        );
        assert_eq!(load_host_boot_state(&dir).unwrap().unwrap().esp_mount, None);

        // Also test crash when esp_files is empty (e.g. process crashed before extracting loaders)
        let mut state_empty = HostBootState {
            plan_id: "crash-empty".into(),
            profile_id: "ubuntu-24.04-desktop".into(),
            backend: "fake".into(),
            phase: HostBootPhase::Preparing,
            boot_order_before: vec!["0000".into(), "0001".into()],
            entry_id: None,
            armed: false,
            esp_mount: Some(EspMount {
                path: esp_x.clone(),
                mounted_by_us: true,
            }),
            created_files: vec![],
            created_dirs: vec![],
            esp_files: vec![],
            esp_dirs: vec![],
            backup_files: vec![],
            events: vec![],
            updated_at: 0,
        };
        save_state(&dir, &mut state_empty, "crashed before files staged").unwrap();

        let mut backend3 = FakeHostBootBackend::new(facts(&dir), esp_y.clone());
        backend3.mounted_paths = vec![esp_x.clone()];
        backend3.mounted = true;

        let cleaned =
            cleanup_host_one_shot(&dir, &mut backend3, CLEANUP_CONFIRMATION_PHRASE).unwrap();
        assert_eq!(cleaned.phase, HostBootPhase::CleanedUp);
        assert!(
            backend3.mounted_paths.is_empty(),
            "saved mount should be unmounted directly without calling mount_esp: {:?}",
            backend3.mounted_paths
        );
        assert!(!backend3.mounted);
        assert_eq!(load_host_boot_state(&dir).unwrap().unwrap().esp_mount, None);

        let _ = fs::remove_dir_all(&dir);
    }
}
