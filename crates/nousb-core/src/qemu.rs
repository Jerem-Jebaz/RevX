use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use reqwest::header::RANGE;
use reqwest::StatusCode;
use sha2::{Digest, Sha512};

use super::{http_client, json_pair, validate_https_response, DownloadLock, NoUsbError};

const QEMU_WINDOWS_VERSION: &str = "11.0.0";
const QEMU_WINDOWS_FILENAME: &str = "qemu-w64-setup-20260422.exe";
const QEMU_WINDOWS_URL: &str = "https://qemu.weilnetz.de/w64/qemu-w64-setup-20260422.exe";
const QEMU_WINDOWS_CHECKSUM_URL: &str =
    "https://qemu.weilnetz.de/w64/qemu-w64-setup-20260422.sha512";
const QEMU_WINDOWS_SHA512: &str = "64a43c0d39acddc9d30d290935a312a2b5c4fa62cffe6c27090f2a45ca6c8de0f0e8673e1e5117fb116a8742f86df92163531afc23f34758aadfc6d82c1f41a5";
const QEMU_WINDOWS_SIZE_BYTES: u64 = 185_604_552;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QemuRuntimeManifest {
    pub version: String,
    pub platform: String,
    pub filename: String,
    pub source_url: String,
    pub checksum_url: String,
    pub sha512: String,
    pub size_bytes: u64,
    pub project_local_only: bool,
    pub signature_warning: String,
}

impl QemuRuntimeManifest {
    pub fn to_json(&self) -> String {
        format!(
            "{{{},{},{},{},{},{},\"size_bytes\":{},\"project_local_only\":{},{}}}",
            json_pair("version", &self.version),
            json_pair("platform", &self.platform),
            json_pair("filename", &self.filename),
            json_pair("source_url", &self.source_url),
            json_pair("checksum_url", &self.checksum_url),
            json_pair("sha512", &self.sha512),
            self.size_bytes,
            self.project_local_only,
            json_pair("signature_warning", &self.signature_warning),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QemuRuntimeDownload {
    pub manifest: QemuRuntimeManifest,
    pub path: PathBuf,
    pub resumed_from_bytes: u64,
    pub downloaded_bytes: u64,
    pub verified: bool,
    pub already_present: bool,
}

impl QemuRuntimeDownload {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"manifest\":{},{},\"resumed_from_bytes\":{},\"downloaded_bytes\":{},\"verified\":{},\"already_present\":{}}}",
            self.manifest.to_json(),
            json_pair("path", &self.path.display().to_string()),
            self.resumed_from_bytes,
            self.downloaded_bytes,
            self.verified,
            self.already_present,
        )
    }
}

pub fn qemu_windows_runtime_manifest() -> QemuRuntimeManifest {
    QemuRuntimeManifest {
        version: QEMU_WINDOWS_VERSION.into(),
        platform: "windows-x86_64".into(),
        filename: QEMU_WINDOWS_FILENAME.into(),
        source_url: QEMU_WINDOWS_URL.into(),
        checksum_url: QEMU_WINDOWS_CHECKSUM_URL.into(),
        sha512: QEMU_WINDOWS_SHA512.into(),
        size_bytes: QEMU_WINDOWS_SIZE_BYTES,
        project_local_only: true,
        signature_warning: "The distributor states that newer installers use an expired code-signing certificate; NoUSB requires the published SHA-512 before use.".into(),
    }
}

pub fn qemu_runtime_cache_path(base_dir: &Path) -> Result<PathBuf, NoUsbError> {
    let base = fs::canonicalize(base_dir).map_err(|error| {
        NoUsbError::Io(format!(
            "cannot resolve project directory {}: {error}",
            base_dir.display()
        ))
    })?;
    Ok(base
        .join(".nousb")
        .join("tools")
        .join("downloads")
        .join(QEMU_WINDOWS_FILENAME))
}

fn sha512_file(path: &Path) -> Result<String, NoUsbError> {
    let mut file = File::open(path)?;
    let mut digest = Sha512::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_runtime(path: &Path, manifest: &QemuRuntimeManifest) -> Result<(), NoUsbError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() != manifest.size_bytes {
        return Err(NoUsbError::Verification(format!(
            "QEMU runtime artifact {} is {} bytes; expected {} bytes",
            path.display(),
            metadata.len(),
            manifest.size_bytes
        )));
    }
    let actual = sha512_file(path)?;
    if !actual.eq_ignore_ascii_case(&manifest.sha512) {
        return Err(NoUsbError::Verification(format!(
            "QEMU runtime artifact {} has SHA-512 {actual}; expected {}",
            path.display(),
            manifest.sha512
        )));
    }
    Ok(())
}

fn finalize_runtime_download(
    manifest: QemuRuntimeManifest,
    partial_path: PathBuf,
    final_path: PathBuf,
    resumed_from_bytes: u64,
    downloaded_bytes: u64,
) -> Result<QemuRuntimeDownload, NoUsbError> {
    verify_runtime(&partial_path, &manifest)?;
    if final_path.exists() {
        return Err(NoUsbError::Verification(format!(
            "refused to overwrite existing QEMU artifact {}",
            final_path.display()
        )));
    }
    fs::rename(&partial_path, &final_path)?;
    Ok(QemuRuntimeDownload {
        manifest,
        path: final_path,
        resumed_from_bytes,
        downloaded_bytes,
        verified: true,
        already_present: false,
    })
}

pub fn acquire_qemu_windows_runtime(base_dir: &Path) -> Result<QemuRuntimeDownload, NoUsbError> {
    let manifest = qemu_windows_runtime_manifest();
    let final_path = qemu_runtime_cache_path(base_dir)?;
    let artifact_dir = final_path.parent().ok_or_else(|| {
        NoUsbError::Verification("QEMU runtime cache path has no parent directory".into())
    })?;
    fs::create_dir_all(artifact_dir)?;

    if final_path.exists() {
        verify_runtime(&final_path, &manifest)?;
        return Ok(QemuRuntimeDownload {
            downloaded_bytes: manifest.size_bytes,
            manifest,
            path: final_path,
            resumed_from_bytes: 0,
            verified: true,
            already_present: true,
        });
    }

    let _lock = DownloadLock::acquire(artifact_dir.join("qemu-download.lock"))?;
    let partial_path = artifact_dir.join(format!("{QEMU_WINDOWS_FILENAME}.part"));
    let resumed_from_bytes = fs::metadata(&partial_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if resumed_from_bytes > manifest.size_bytes {
        return Err(NoUsbError::Verification(format!(
            "partial QEMU download is {resumed_from_bytes} bytes, larger than expected {} bytes; move it aside and retry",
            manifest.size_bytes
        )));
    }
    if resumed_from_bytes == manifest.size_bytes {
        return finalize_runtime_download(
            manifest,
            partial_path,
            final_path,
            resumed_from_bytes,
            0,
        );
    }

    let client = http_client()?;
    let mut request = client.get(&manifest.source_url);
    if resumed_from_bytes > 0 {
        request = request.header(RANGE, format!("bytes={resumed_from_bytes}-"));
    }
    let mut response = request
        .send()
        .map_err(|error| NoUsbError::Network(error.to_string()))?;
    validate_https_response(&response)?;
    let expected_status = if resumed_from_bytes > 0 {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    if response.status() != expected_status {
        return Err(NoUsbError::Network(format!(
            "{} returned HTTP {}; expected {}. Existing partial data was kept unchanged",
            manifest.source_url,
            response.status(),
            expected_status
        )));
    }
    let expected_remaining = manifest.size_bytes - resumed_from_bytes;
    if let Some(content_length) = response.content_length() {
        if content_length != expected_remaining {
            return Err(NoUsbError::Verification(format!(
                "QEMU server offered {content_length} bytes, expected {expected_remaining} remaining bytes"
            )));
        }
    }

    let mut output = if resumed_from_bytes > 0 {
        OpenOptions::new().append(true).open(&partial_path)?
    } else {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&partial_path)?
    };
    let downloaded_bytes = io::copy(&mut response, &mut output)?;
    output.sync_all()?;
    finalize_runtime_download(
        manifest,
        partial_path,
        final_path,
        resumed_from_bytes,
        downloaded_bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_manifest_is_exact_and_https_pinned() {
        let manifest = qemu_windows_runtime_manifest();
        assert_eq!(manifest.version, "11.0.0");
        assert_eq!(manifest.size_bytes, 185_604_552);
        assert!(manifest.source_url.starts_with("https://"));
        assert_eq!(manifest.sha512.len(), 128);
        assert!(manifest
            .sha512
            .chars()
            .all(|value| value.is_ascii_hexdigit()));
        assert!(manifest.project_local_only);
    }

    #[test]
    fn cache_path_is_beneath_resolved_project_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let base =
            std::env::temp_dir().join(format!("nousb-qemu-cache-{}-{unique}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let resolved = fs::canonicalize(&base).unwrap();
        let cache = qemu_runtime_cache_path(&base).unwrap();
        assert!(cache.starts_with(&resolved));
        assert!(cache.ends_with(QEMU_WINDOWS_FILENAME));
        let _ = fs::remove_dir_all(base);
    }
}
