use std::process::Command;
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
    assert!(stdout.contains("BitLocker"));
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
    assert!(stdout.contains("\"fake_scenarios\""));
    assert!(stdout.contains("\"sample_plan\""));
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
