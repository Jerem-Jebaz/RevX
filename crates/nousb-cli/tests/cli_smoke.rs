use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn nousb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nousb"))
}

#[test]
fn json_plan_contains_machine_readable_fields() {
    let output = nousb()
        .args(["--json", "plan", "boot-once", "ubuntu-24.04-desktop"])
        .output()
        .expect("run nousb");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"kind\":\"boot-once\""));
    assert!(stdout.contains("\"risk\":\"medium\""));
    assert!(stdout.contains("\"steps\""));
}

#[test]
fn yes_does_not_bypass_critical_confirmation() {
    let output = nousb()
        .args(["--yes", "execute", "replace-os", "ubuntu-24.04-desktop"])
        .output()
        .expect("run nousb");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("--yes cannot approve critical risk plan"));
}

#[test]
fn fake_linux_low_space_plan_surfaces_manual_intervention() {
    let output = nousb()
        .args([
            "plan-fake",
            "linux-luks-low-space",
            "boot-once",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("resolve-low-disk-space"));
    assert!(stdout.contains("LUKS"));
}

#[test]
fn fake_windows_inspect_json_is_available() {
    let output = nousb()
        .args(["--json", "system", "inspect-fake", "windows-bitlocker"])
        .output()
        .expect("run nousb");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"adapter\":\"fake-windows-uefi-bitlocker\""));
    assert!(stdout.contains("\"memory_total_mb\":32768"));
    assert!(stdout.contains("BitLocker"));
}

#[test]
fn delivery_assessment_exposes_ram_and_firmware_routes() {
    let output = nousb()
        .args([
            "--json",
            "workflow",
            "assess-fake",
            "windows-bitlocker",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb workflow assess-fake");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"recommended_route\":\"host-staged-ram-boot\""));
    assert!(stdout.contains("\"id\":\"uefi-http-boot\""));
    assert!(stdout.contains("\"safe_default_execution_enabled\":false"));
    assert!(stdout.contains("\"universal_no_usb_guarantee\":false"));
}

#[test]
fn artifact_manifest_is_machine_readable_and_pinned() {
    let output = nousb()
        .args(["--json", "image", "manifest", "ubuntu-24.04-desktop"])
        .output()
        .expect("run nousb image manifest");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"filename\":\"ubuntu-24.04.4-desktop-amd64.iso\""));
    assert!(stdout.contains("\"size_bytes\":6655619072"));
    assert!(stdout.contains("3a4c9877b483ab46d7c3fbe165a0db275e1ae3cfe56a5657e5a47c2f99a99d1e"));
}

#[test]
fn ubuntu_server_manifest_is_machine_readable_and_pinned() {
    let output = nousb()
        .args(["--json", "image", "manifest", "ubuntu-24.04-server"])
        .output()
        .expect("run nousb image manifest");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"filename\":\"ubuntu-24.04.4-live-server-amd64.iso\""));
    assert!(stdout.contains("\"size_bytes\":3405469696"));
    assert!(stdout.contains("e907d92eeec9df64163a7e454cbc8d7755e8ddc7ed42f99dbc80c40f1a138433"));
}

#[test]
fn vendor_mediated_image_download_fails_closed() {
    let output = nousb()
        .args(["image", "manifest", "windows-11"])
        .output()
        .expect("run nousb image manifest");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stderr.contains("no stable vendor artifact URL"));
}

#[test]
fn guided_tui_runs_as_a_safe_terminal_application() {
    let mut child = nousb()
        .arg("tui")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start nousb tui");
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"3\n4\n8\n9\n").expect("script tui input");
    }
    let output = child.wait_with_output().expect("wait for nousb tui");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("NoUSB guided terminal application"));
    assert!(stdout.contains("Universal no-USB guarantee: false"));
    assert!(stdout.contains("ubuntu-24.04.4-desktop-amd64.iso"));
    assert!(stdout.contains("Preview only: no executor was invoked."));
}

#[test]
fn vm_doctor_reports_disposable_runtime_separately_from_host_boot_execution() {
    let output = nousb()
        .args(["--json", "vm", "doctor"])
        .output()
        .expect("run nousb vm doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"runtime_access_verified\":"));
    assert!(stdout.contains("\"disposable_vm_execution_enabled\":"));
    assert!(stdout.contains("\"real_boot_execution_enabled\":false"));
    assert!(stdout.contains("\"host_boot_changes\":false"));
}

#[test]
fn qemu_runtime_manifest_is_pinned_and_does_not_execute_installer() {
    let output = nousb()
        .args(["--json", "vm", "qemu", "manifest"])
        .output()
        .expect("run nousb vm qemu manifest");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"version\":\"11.0.0\""));
    assert!(stdout.contains("\"size_bytes\":185604552"));
    assert!(stdout.contains("64a43c0d39acddc9d30d290935a312a2"));
    assert!(stdout.contains("\"project_local_only\":true"));
}

#[test]
fn qemu_smoke_plan_never_attaches_host_storage_or_network() {
    let output = nousb()
        .args(["--json", "vm", "qemu", "plan-smoke"])
        .output()
        .expect("run nousb vm qemu plan-smoke");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("\"accelerator\":\"tcg\""));
        assert!(stdout.contains("\"network_enabled\":false"));
        assert!(stdout.contains("\"physical_disks_attached\":false"));
        assert!(stdout.contains("\"host_boot_changes\":false"));
        assert!(stdout.contains("\"ram_boot_verified\":false"));
    } else {
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert!(stderr.contains("project-local QEMU runtime is incomplete"));
    }
}

#[test]
fn ubuntu_netboot_plan_labels_partial_ram_evidence_and_direct_boot() {
    let output = nousb()
        .args(["--json", "vm", "qemu", "plan-ubuntu-netboot"])
        .output()
        .expect("run nousb vm qemu plan-ubuntu-netboot");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("\"direct_kernel_boot\":true"));
        assert!(stdout.contains("\"uefi_firmware_initialized\":false"));
        assert!(stdout.contains("\"full_installer_payload_in_ram\":false"));
        assert!(stdout.contains("\"network_enabled\":false"));
        assert!(stdout.contains("\"physical_disks_attached\":false"));
        assert!(stdout.contains("transport-only-not-installable"));
    } else {
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert!(stderr.contains("Ubuntu rehearsal") || stderr.contains("QEMU runtime"));
    }
}

#[test]
fn ubuntu_full_ram_plan_is_explicit_about_network_http_and_uefi_limits() {
    let output = nousb()
        .args(["--json", "vm", "qemu", "plan-ubuntu-full-ram"])
        .output()
        .expect("run nousb vm qemu plan-ubuntu-full-ram");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("\"artifact_size_bytes\":3405469696"));
        assert!(stdout.contains("\"direct_kernel_boot\":true"));
        assert!(stdout.contains("\"uefi_firmware_initialized\":false"));
        assert!(stdout.contains("\"full_installer_payload_in_ram\":false"));
        assert!(stdout.contains("\"guest_transport_https\":false"));
        assert!(stdout.contains("\"network_enabled\":true"));
        assert!(stdout.contains("\"physical_disks_attached\":false"));
        assert!(stdout.contains("\"virtual_install_disks_attached\":false"));
        assert!(stdout.contains("\"host_boot_changes\":false"));
    } else {
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert!(
            stderr.contains("pinned Ubuntu Server ISO is missing")
                || stderr.contains("QEMU runtime")
                || stderr.contains("staging")
        );
    }
}

#[test]
fn ubuntu_uefi_full_ram_plan_requires_verified_official_boot_assets() {
    let output = nousb()
        .args(["--json", "vm", "qemu", "plan-ubuntu-uefi-full-ram"])
        .output()
        .expect("run nousb vm qemu plan-ubuntu-uefi-full-ram");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("\"direct_kernel_boot\":false"));
        assert!(stdout.contains("\"uefi_firmware_initialized\":false"));
        assert!(stdout.contains("\"full_installer_payload_in_ram\":false"));
        assert!(stdout.contains("\"secure_boot_enabled\":false"));
        assert!(stdout.contains("\"detachable_read_only_boot_source\":true"));
        assert!(stdout.contains("6fe6e1bcbe6cf6baec8e056d40361ca1"));
        assert!(stdout.contains("52b8b3503d3efabfac8793f07e1db5d"));
        assert!(stdout.contains("d2fa8e52fddc99dad94a0009fee23cb"));
        assert!(stdout.contains("\"physical_disks_attached\":false"));
        assert!(stdout.contains("\"host_boot_changes\":false"));
    } else {
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert!(
            stderr.contains("pinned Ubuntu Server ISO is missing")
                || stderr.contains("QEMU runtime")
                || stderr.contains("staging")
                || stderr.contains("EFI asset")
        );
    }
}

#[test]
fn current_os_baseline_plan_is_disposable_and_does_not_overclaim_fallback() {
    let output = nousb()
        .args(["--json", "vm", "qemu", "plan-current-os-baseline"])
        .output()
        .expect("run nousb vm qemu plan-current-os-baseline");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        assert!(stdout.contains("\"root_disk_size_bytes\":536870912"));
        assert!(stdout.contains("\"uses_disposable_guest_disk\":true"));
        assert!(stdout.contains("\"network_enabled\":false"));
        assert!(stdout.contains("\"physical_disks_attached\":false"));
        assert!(stdout.contains("\"host_boot_changes\":false"));
        assert!(stdout.contains("\"secure_boot_enabled\":false"));
        assert!(stdout.contains("\"installer_fallback_proven\":false"));
        assert!(stdout.contains("\"uefi_bootnext_proven\":false"));
        assert!(stdout.contains("RUN DISPOSABLE CURRENT OS UEFI BASELINE"));
    } else {
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert!(
            stderr.contains("pinned Ubuntu Server ISO is missing")
                || stderr.contains("QEMU runtime")
                || stderr.contains("staging")
                || stderr.contains("EFI asset")
                || stderr.contains("disposable current OS")
        );
    }
}

#[test]
fn vm_rehearsal_persists_full_recovery_matrix() {
    let temp_dir = std::env::temp_dir().join(format!(
        "nousb-cli-vm-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");
    let output = nousb()
        .current_dir(&temp_dir)
        .args([
            "--json",
            "vm",
            "rehearse",
            "windows-bcd-bootsequence",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb vm rehearse");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"all_invariants_hold\":true"));
    assert!(stdout.contains("\"real_vm_executed\":false"));
    assert!(stdout.contains("\"host_boot_changes\":false"));
    assert_eq!(stdout.matches("\"invariants_hold\":true").count(), 9);
    let report_dir = temp_dir.join(".nousb/vm-rehearsals");
    assert_eq!(
        std::fs::read_dir(&report_dir)
            .expect("read vm reports")
            .count(),
        1
    );

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn simulate_can_interrupt_plan() {
    let output = nousb()
        .args([
            "--interrupt-after",
            "2",
            "simulate",
            "boot-once",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("Status: interrupted"));
    assert!(stdout.contains("verify-installer: pending"));
}

#[test]
fn simulate_json_rollback_reports_rolled_back_state() {
    let output = nousb()
        .args([
            "--json",
            "--rollback",
            "--interrupt-after",
            "2",
            "simulate",
            "boot-once",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"status\":\"rolled_back\""));
    assert!(stdout.contains("\"state\":\"rolled_back\""));
}

#[test]
fn simulate_persists_report_for_listing_and_showing() {
    let temp_dir = std::env::temp_dir().join(format!(
        "nousb-cli-smoke-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let simulate = nousb()
        .current_dir(&temp_dir)
        .args([
            "--interrupt-after",
            "1",
            "simulate",
            "boot-once",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb simulate");
    assert!(simulate.status.success());

    let list = nousb()
        .current_dir(&temp_dir)
        .args(["--json", "reports", "list"])
        .output()
        .expect("run nousb reports list");
    assert!(list.status.success());
    let list_stdout = String::from_utf8(list.stdout).expect("utf8 list stdout");
    assert!(list_stdout.contains("\"status\":\"interrupted\""));
    assert!(list_stdout.contains("\"recovery_required\":true"));

    let plan_id = extract_json_string(&list_stdout, "plan_id").expect("plan id");
    let show = nousb()
        .current_dir(&temp_dir)
        .args(["reports", "show", &plan_id])
        .output()
        .expect("run nousb reports show");
    assert!(show.status.success());
    let show_stdout = String::from_utf8(show.stdout).expect("utf8 show stdout");
    assert!(show_stdout.contains("\"status\":\"interrupted\""));

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn diagnostics_export_creates_redacted_bundle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "nousb-cli-diag-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(temp_dir.join(".nousb/journal")).expect("create journal dir");
    std::fs::write(
        temp_dir.join(".nousb/journal/operations.log"),
        "token=super-secret\nnormal=value\n",
    )
    .expect("write log");

    let simulate = nousb()
        .current_dir(&temp_dir)
        .args([
            "--interrupt-after",
            "1",
            "simulate",
            "boot-once",
            "ubuntu-24.04-desktop",
        ])
        .output()
        .expect("run nousb simulate");
    assert!(simulate.status.success());

    let export = nousb()
        .current_dir(&temp_dir)
        .args(["--json", "diagnostics", "export"])
        .output()
        .expect("run diagnostics export");
    assert!(export.status.success());
    let stdout = String::from_utf8(export.stdout).expect("utf8 stdout");
    let bundle_path = extract_json_string(&stdout, "path").expect("bundle path");
    let log = std::fs::read_to_string(std::path::Path::new(&bundle_path).join("operations.log"))
        .expect("read bundle log");
    assert!(log.contains("token=[REDACTED]"));
    assert!(!log.contains("super-secret"));
    assert!(std::path::Path::new(&bundle_path)
        .join("execution-report-index.json")
        .exists());

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn dashboard_export_contains_preview_data() {
    let output = nousb()
        .args(["dashboard", "export"])
        .output()
        .expect("run dashboard export");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("\"catalog\""));
    assert!(stdout.contains("\"artifact_manifests\""));
    assert!(stdout.contains("\"real_vm_reports\""));
    assert!(stdout.contains("\"fake_scenarios\""));
    assert!(stdout.contains("\"sample_plan\""));
    assert!(stdout.contains("\"sample_vm_rehearsal\""));
    assert!(stdout.contains("\"all_invariants_hold\":true"));
    assert!(stdout.contains("\"boot_plan\""));
    assert!(stdout.contains("ubuntu-24.04-desktop"));
    assert!(stdout.contains("linux-luks-low-space"));
}

fn extract_json_string(source: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = source.find(&marker)? + marker.len();
    let end = source[start..].find('"')?;
    Some(source[start..start + end].to_string())
}
